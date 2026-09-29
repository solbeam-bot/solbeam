# 13. SOLBEAM — the model

**A wrapped-BSV token on Solana, pegged one-for-one.** One `solBSV` is always backed by one BSV in
the reserve. Peg only: no exchange mechanism, no order book, no leverage. Branding **SOLBEAM**,
ticker `solBSV`, 8 decimals, MIT.

This is the canonical account. Every other document refers to it, and where one disagrees, this one
is right.

---

## The three parts

**1. A light client on Solana.** It holds a checkpoint and a rolling window of 192 BSV headers, and
verifies proof of work against BSV's real difficulty rule (**cw-144**, recomputed every block,
verified against 324 of 324 real mainnet headers) and Merkle inclusion of a transaction in a block.

**2. A vault.** Every mint lands in a program-owned token account rather than the depositor's. It
leaves when the program is satisfied, and it can be burned if it isn't.

**3. A federation.** A set of bonded members who run nodes, hold the BSV reserve under a threshold
**ECDSA** key, relay headers, sign payouts, and challenge theft. **Open membership.** The reserve
address is an ordinary **P2PKH** address; the key is simply **never assembled in one place**.
Members post **two-sided bonds** (*The two bonds*), not one.

---

## What is trustless, and what is not

**This is the honest division and it should not be blurred.**

| | |
|---|---|
| **Minting** | **Trustless, given the deployed program.** A Solana program verifies BSV proof of work and Merkle inclusion directly: no member's signature, no committee vote and no oracle mints anything. **The program's upgrade authority is the one exception** — it can re-anchor the checkpoint, so in production it must be threshold-held and timelocked (F4, and *Governance* below), and the exit window (30 days by default, floored at 7) is the real guarantee, not the absence of a key |
| **Reversal** | **Trustless.** The program compares its own stored header hash against the one a deposit was proven with. A reorg is a fact about headers, not a report from anyone |
| **The reserve** | **Trusted, and bounded.** The BSV is held under a **threshold ECDSA** key by the federation — an ordinary P2PKH address whose key is never assembled in one place. No single member can move it. What protects you is a **bond that anyone can seize by proving misbehaviour on-chain**, not the absence of trust |

**The one trust assumption: a threshold of federation members do not collude.** Everything else is
verified. That assumption is not eliminated — it is **bounded by two bonds, one per direction**:
the mint side posts **BSV outside the reserve**, sized against the BSV held, and the redeem side
posts **`solBSV`**, seizable on Solana, sized against the `solBSV` held (*The two bonds*). Neither
makes a colluding threshold unprofitable and neither makes holders whole: a colluding threshold can
take the reserve, and the maximum loss is the entire **non-member supply** — see *Collusion is a
stated risk* below. **The bonds size the system and price provable misbehaviour; the exit window is
what answers governance.**

---

## The account of a peg-in

```
1  CHOOSE    terms; 30 bp to mint and 30 bp to redeem, both governed
2  SEND      BSV to the federation's deposit script
             OP_RETURN = version ‖ cluster_id ‖ program_hash ‖ flags ‖ recipient
             (design. What is BUILT checks only that the recipient's 32 bytes appear in an
              OP_RETURN — see lib.rs. The longer form is the cross-deployment fix, P11, not built)
3  DEPTH     12 confirmations (FLOOR)
4  STAGE     solBSV is minted INTO THE VAULT, not to the depositor, and a record stores
             the block hash the deposit was proven against
5  MATURE    144 blocks
6  RELEASE   permissionless, and requires:
               the tip has ADVANCED past the deposit, and
               the stored hash at that height STILL MATCHES
             → vault to recipient
             if the hash DIFFERS, the deposit was reorged: the staged tokens BURN,
               and the depositor keeps the BSV the reorg returned
```

**Step 6 is decided by the program from its own headers.** No oracle, no reporter, no discretion.
`release_mint` and `burn_staged` are **permissionless** — anyone may resolve a pending item and
reclaim its rent, so **no party's cooperation is ever required.**

**Step 4 is what makes a fraudulent mint unsellable.** A staged token is not in anyone's wallet, so
there is nothing to dump and no innocent buyer to inherit the loss.

## The account of a peg-out

```
1  ESCROW    solBSV into the vault; a BSV destination and a deadline are set
2  ACCEPT    federation members sign payout INTENTS individually, on Solana
3  PAY       once enough attributed intents exist, the threshold key signs the BSV payment
4  SETTLE    the payout is proved against the light client; the escrow burns
   or
4' CANCEL    permissionless after the deadline: the escrow returns to the holder
```

**Failure returns; it never mints.** Supply is unchanged and the holder is whole without asking
anyone.

**Step 2 is the attribution mechanism.** Because each member signs separately, a member who signs
two conflicting intents has produced **their own proof of guilt** — see *Slashing* below.

---

## The federation

**Open membership.** Members post **two bonds**, one per direction, each denominated in the asset
its side holds and **neither inside the reserve** (*The two bonds*, below). A member cannot leave
while its share of either cover is needed.

**Members run software, not judgement.** There is no manual approval of any transaction. Each node
watches both chains, verifies independently with its own light client, signs, and challenges —
automatically. It is **running a staked node**: pledge a bond, run the software, earn a yield, lose
the bond for misbehaving.

**The bond sizes are the scale limit, and that is stated rather than implied. Capital inefficiency is accepted deliberately** — a bridge that caps its size at what its members will bond cannot outrun its own collateral, and growth then requires new members rather than larger ones. With `k = 1`, total
value locked is capped by total bonds pledged. Ten members at 1,000 BSV is roughly **~$180k** of
capacity. That is a proof of concept.

### Governance

| | Default |
|---|---|
| Who may propose | Any member |
| To pass | **85% of pledged coins** — the total bonded `solBSV`, weighted by bond size |
| Delay | **30 days** |
| Delay floor | **`open`** — 7 days proposed (`gov.delay_min`). With a floor, a majority cannot take the warning away; without one, the exit window is whatever the current majority allows. Both are defensible — the choice is a judgement about how much the majority is trusted, not a correctness question |
| Signal | **Live from the moment it is raised** |
| Includes | **The upgrade authority** |
| Cannot pause | **Redemptions. Ever.** Governance *could* rewrite this, since it holds the upgrade authority — which is exactly why the exit window is the real guarantee, not the rule |

All of those are **parameters**, not constants.

**There is no immutable floor, deliberately.** A hostile change needs 85% *and* the delay, and
redemptions run throughout — so a proposal that would harm holders **empties the bridge before it
lands.**

**The delay floor is `open`, not settled.** If `gov.delay_min` is set (7 days proposed), a first
proposal cannot set the delay to zero — it can only shorten it to the floor — so the two-step attack
(shorten, then land anything) still has to be exited during that window. If it is not set, the exit
window is whatever the current majority allows. **Both readings are defensible; the choice is a
judgement about how much the majority is trusted, not a correctness question.** This is the F7
decision: reducible with a floor, not a ratchet — the floor itself is the open part.

> **The floor is the exit window, not a constitution.** The protection was never that the rules are
> frozen. It is that you can always leave before they change.

**The residual, stated plainly:** a holder who does not watch and does not act within the delay —
30 days by default, **7 days at worst if `gov.delay_min` is set, otherwise whatever the current
majority allows** — is exposed. That is a disclosure obligation, not a mechanism.

### The two bonds

**Two bonds, one per direction, each denominated in the asset that side holds, and neither inside
the reserve:**

```
mint side     bsv_bond    ≥ k × (BSV held in the reserve)     BSV, OUTSIDE the reserve
redeem side   solbsv_bond ≥ k × (solBSV held)                 solBSV, seizable on Solana
```

where "held" on each side means held outside the bond set — what honest holders could lose. **The
same checks run on `withdraw_bond`**, so a member cannot leave while its share is needed.

**This replaces the earlier single-bond formula** `aggregate_bond ≥ k × non_bonded_supply`. That
formula was itself a fix — for the unsatisfiable `B ≥ k × total supply`, which demands `B ≥ B + H`
and forces non-member holdings to zero. **Its remaining problem is solved here by the BSV-side bond
not being `solBSV` at all:** the mint side's cover is a different asset held outside the reserve, so
it is not counted back into the supply it covers. The old formula is **superseded** wherever it
appears.

**How the BSV-side bond is enforced — by the members, collectively, not by the Solana program.**
The BSV-side bond sits under the **collective (threshold ECDSA) key** — the same primitive as the
reserve, pointed at the bond — and **not under the member's own key**. That is the design requirement
the whole mechanism rests on: if a member controls their own bond, they move it the moment they are
caught, or before they act, and there is nothing to slash. Under the collective key a member
**cannot** move their own bond, and the federation **can**, by signing a threshold transaction that
moves the bond. The members run BSV nodes, Solana nodes and key shares, so they have **detection,
capability and motive** — and **slashing pays the slashers out of the seized bond**, which is what
makes the collective action actually happen.

| Bond | Where | Who seizes | How |
|---|---|---|---|
| `solBSV` (redeem side) | Solana | **The program** | An instruction, on proof — automatic |
| **BSV** (mint side) | A BSV script under the collective key | **The members collectively** | A threshold-signed transaction moving the bond |

**Two residuals, stated rather than glossed.** (1) A **majority could seize an honest member's
bond** — the symmetric risk of collective custody, resting on the same majority already trusted with
the reserve. (2) The **obligation to slash is social, not on-chain**: nothing on BSV compels the
members to sign, so it rests on the majority being honest, on visibility, and on the bounty. The
BSV-side bond is therefore a mechanism enforced by a **collective action by the majority**, not an
automatic rule — but it is a mechanism, not a promise.

**Nothing can be minted that the bonds cannot cover.** The mint gate requires the BSV-side bond to
cover the BSV held; the redeem path requires the `solBSV`-side bond to cover the `solBSV` held. With
`k = 1` each side covers what its holders could lose, so **"total value locked is capped by bonds
pledged" is true rather than aspirational**, and minting is gated by the bonds rather than by
nothing.

**`fed.threshold` sizes nothing on-chain.** It is a parameter of the **signing protocol**, not of a
script: the reserve address is an ordinary P2PKH address, and the key is a **threshold ECDSA** key
whose shares are never assembled in one place. **Provisional `3-of-5`, marked `open`** — it is the
number every "no single member can move funds" claim depends on, and it sizes no account or script.
Changing `t` or `n` later is a **re-sharing**, not a migration — the reserve never moves and the
address never changes. Larger `n` costs only coordination, not space.

The consequence is deliberate: **the bridge can only grow as fast as members will bond.** That is the
property that stops it outrunning its own collateral, and the price is that growth needs new members
rather than larger ones.

### Where the money comes from

**The fee is gross, and it is the only revenue.** 30 bp on mint and 30 bp on redeem cover the real
transaction costs — Solana fees, BSV relay — and **the remainder is the bonded members' income**,
shared pro rata to stake. At the 1 BSV minimum the non-refundable cost of a mint is about **$0.001**
against a **$0.09** fee, so the margin is the members', and the one cost that exceeds the fee
(first-time ATA rent, $0.115) is **refundable**.

**Members are paid to carry the risk, and the bonds are the risk.** Their return is the fee share; their
exposure is 1,000 BSV that exists to be lost if they misbehave. Whether that trade is attractive is
a market question, not a design one — which is why the fee is a governed parameter rather than a
constant.

### Pause

**Mints can be paused. Redemptions cannot.** Pausing inbound is a safety valve; pausing outbound is
taking hostages. Because the power is bounded, a pause carries a lower threshold than a governance
change.

---

## Slashing — self-proving misbehaviour

You cannot deduce who was at fault from an opaque threshold signature. **So the design does not try
to.** Members sign individually, so misbehaviour produces **its own evidence**:

| Misbehaviour | Provable? |
|---|---|
| A member signs **two conflicting payout intents** | **Yes — self-proving.** Two signatures, one member, conflicting statements. Anyone submits it; anyone can be paid the bounty |

**One row was deleted, not corrected: "an intent matching no authorised redemption."** The predicate
is undecidable — a *closed* `PegOut` is indistinguishable from one that never existed — so the
program cannot check it, and attempting to would false-positive against an honest member who
attested before a cancel. An earlier draft claimed this row was provable; it is not, and the audit's
F8 records why. There is no enforceable predicate for an off-chain threshold signature over BSV.

## Three security layers

| Layer | Catches |
|---|---|
| **Threshold signature** over the BSV payout | A **minority** moving funds |
| **Individual attestations** on Solana | A **minority's equivocation** — a member signing two conflicting intents produces its own proof of guilt |
| **Covenant** (research, later) | A **colluding majority** — enforced by miners, not by members |

**A threshold signature does not reveal who signed**, which is exactly why the individual
attestation layer exists: it creates **attribution**. And **none of the three catches a consistent
majority**, who simply sign the same fraudulent thing and never equivocate. The phrase **"double
threshold"** must not be read as a stronger threshold — it means **a key plus a paper trail.**


## Collusion is a stated risk, not a mitigated one

**A threshold of members can collude, and nothing here prevents it.** There is no on-chain predicate
that proves which members signed an off-chain threshold signature, so there is nothing to slash on.
An earlier draft claimed otherwise; that row is deleted.

**The maximum loss is the entire non-member supply.** A colluding threshold can take the reserve and
nothing prevents it. The bonds are **outside the reserve** — the BSV-side bond is native BSV the
Solana program cannot seize, and the `solBSV`-side bond is a Solana token account — so they do not
enlarge the prize. The earlier `B_h + H` figure and its 1.15× / 1.49× multiples were computed for
the **superseded single-bond model**, where the bonds sat inside the reserve; **the two-sided bonds
supersede that arithmetic.**

**Why the bond's denomination is now split rather than moved.** A `SOL` bond would be genuinely
separate and seizable, but sizing it against a `solBSV` liability needs a **SOL/BSV price**, and
that is an oracle. The two-sided design gets separation without one: the mint side is denominated in
**BSV**, the same asset as the liability but held **outside** the reserve, and the redeem side stays
`solBSV` so it is seizable on Solana. No external data is consulted.

**The mitigation is transparency, and it is promoted to an early deliverable.** Publishing the
reserve and the supply continuously, so the backing ratio is public, converts a hidden theft into a
visible one. For the two cases nothing can enforce — collusion, and an unspent-outpoint spend nobody
challenges — **visibility is the only remaining defence.** See [`07-roadmap.md`](07-roadmap.md).

**What the bonds do:** they deter, they price entry, and they make **provable** misbehaviour —
equivocation, a member signing two conflicting intents — expensive. The `solBSV`-side bond is seized
by the program automatically; the BSV-side bond is seized by the **members collectively**, under the
collective key, with the bounty paid from the seized bond. Neither protects against a colluding
threshold.

**Accepted.** Measured, stated, and revisited if a mechanism is ever needed.

**This is copied from what RenVM actually shipped**, where `slashDuplicatePropose` and its siblings
take a node's own two conflicting signatures as the entire proof. **Only that cryptographic half was
ever built** — RenVM's own documentation says the slashing contract *"will become a voting system
for darknodes to deregister other misbehaving darknodes. Right now, it is a placeholder."*

**We copy the half that shipped and say plainly that the rest has no precedent.**

---

## What SOLBEAM is not

It is **not** a general-purpose bridge, and it does not try to wrap anything but BSV.
It is **not** a custodian in the single-party sense — no one entity holds the reserve.
It is **not** an exchange: there is no book, no matching, and no market-making.
It is **not** oracle-driven: no external metric gates anything. **Minting is gated by the bonds, not by an oracle** — a verified proof and two bonds that cover what their sides hold are sufficient. Peg-**outs** are gated by the threshold **ECDSA** key, which is an internal quorum, not an oracle. Reorg depth and block time come from BSV
headers; deadlines come from Solana slots.

---

## Where it stands

| | |
|---|---|
| **Light client** | **Built.** cw-144 implemented and verified against 324/324 real mainnet headers, and the **instruction path** — not just the pure function — is exercised by **160 real mainnet headers through `push_header`** (27 tests, 0 failing). Audit F1/F2/F3 are fixed and verified |
| **Token and mint** | **Built.** 27 on-chain tests, with negative controls |
| **BSV-side peg-in** | **Built.** 51/51 synthetic, 21/21 against a live SV Node |
| **Vault** | **Designed, not built.** Rewritten against this model after two audits of the **pre-federation** vault (V1–V10, W1–W11, T1–T12): **28 of 33 findings dissolved on the model change**, 6 remain, two blocking |
| **Federation** | **Designed, not built.** Threshold **ECDSA** custody, two-sided bonds, governance, slashing |
| **Peg-out** | **Designed, not built** |
| **Order book** | **Removed.** A governed 30 bp fee replaces it |

**A ceiling worth naming:** the built program caps the used-deposit list at `MAX_USED = 200` per
window, which bounds peg-ins to 200 per 32 hours *with no attacker at all*. **Decision P5 replaces
that list with a nullifier per minted deposit, removing the ceiling** — decided, not built. Until it
is, a volume spike cannot be served, which bounds the "redemptions never pause" story from the
peg-in side.

**Open, and not dressed up:** the vault's design has failed two audits; sharding is undecided; and
the DAA rule is hard-coded, so a BSV change would halt the bridge until governance acts.

**Genesis: decided.** Members post a **BSV-side bond at genesis**, so no `solBSV` needs to exist
first. The alternative — a **capped, explicitly-unbonded first mint** — is recorded as a documented
later option, not chosen, because it leaves the first mint backed by nothing but the members' word.
