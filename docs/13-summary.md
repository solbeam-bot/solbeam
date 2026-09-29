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
key, relay headers, sign payouts, and challenge theft. **Open membership, 1,000 BSV bond.**

---

## What is trustless, and what is not

**This is the honest division and it should not be blurred.**

| | |
|---|---|
| **Minting** | **Trustless.** A Solana program verifies BSV proof of work and Merkle inclusion directly. No signature, no committee and no oracle can mint anything |
| **Reversal** | **Trustless.** The program compares its own stored header hash against the one a deposit was proven with. A reorg is a fact about headers, not a report from anyone |
| **The reserve** | **Trusted, and bounded.** The BSV is held under a threshold key by the federation. No single member can move it. What protects you is a **bond that anyone can seize by proving misbehaviour on-chain**, not the absence of trust |

**The one trust assumption: a threshold of federation members do not collude.** Everything else is
verified. That assumption is not eliminated — it is **bounded**, by bonds that **at `k = 1` equal** what a colluding threshold could take, so theft is not profitable and holders are made whole from the bond. **The bond is what answers theft; the exit window is what answers governance.**

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

**Open membership.** Anyone with a **1,000 BSV bond** may join. The bond is posted as `solBSV`
because it must be seizable on Solana. The federation's **aggregate bond must always be at least
`k ×` the outstanding `solBSV` supply**, with `k ≥ 1`, so bonded capital exceeds what a colluding
threshold could take. A member cannot leave while its share of that cover is needed.

**Members run software, not judgement.** There is no manual approval of any transaction. Each node
watches both chains, verifies independently with its own light client, signs, and challenges —
automatically. It is **running a staked node**: pledge a bond, run the software, earn a yield, lose
the bond for misbehaving.

**The bond size is the scale limit, and that is stated rather than implied. Capital inefficiency is accepted deliberately** — a bridge that caps its size at what its members will bond cannot outrun its own collateral, and growth then requires new members rather than larger ones. With `k = 1`, total
value locked is capped by total bonds pledged. Ten members at 1,000 BSV is roughly **$300k** of
capacity. That is a proof of concept.

### Governance

| | Default |
|---|---|
| Who may propose | Any member |
| To pass | **85% of pledged coins** — the total bonded `solBSV`, weighted by bond size |
| Delay | **30 days** |
| Signal | **Live from the moment it is raised** |
| Includes | **The upgrade authority** |
| Cannot pause | **Redemptions. Ever.** Governance *could* rewrite this, since it holds the upgrade authority — which is exactly why the 30-day exit is the real guarantee, not the rule |

All of those are **parameters**, not constants.

**There is no immutable floor, deliberately.** A hostile change needs 85% *and* 30 days, and
redemptions run throughout — so a proposal that would harm holders **empties the bridge before it
lands.**

> **The floor is the exit window, not a constitution.** The protection was never that the rules are
> frozen. It is that you can always leave before they change.

**The residual, stated plainly:** a holder who does not watch and does not act within 30 days is
exposed. That is a disclosure obligation, not a mechanism.

### The bond is the float

**Nothing can be minted that the bond cannot cover.** Before a mint is accepted, the program requires

```
aggregate_bond  ≥  k × (non_bonded_supply + amount)      with k ≥ 1
```

**Not `k × total supply`** — bonded `solBSV` *is* part of the supply, so requiring
`B ≥ k × S` gives `B ≥ B + H`, which forces non-member holdings to zero. **A gate no real bridge
can satisfy.** The quantity the bond must cover is the **non-bonded** supply: what honest holders
could lose.

where `supply` is the outstanding `solBSV` and `aggregate_bond` is a running total maintained on
chain, since members cannot be enumerated. **The same check runs on `withdraw_bond`**, so a member
cannot leave while its share is needed.

**This is what makes "total value locked is capped by bonds pledged" true rather than aspirational.**
The bond is not collateral in the abstract — it is **the float for minting and redeeming**, and the
mint gate is where that is enforced. It also resolves an earlier contradiction: minting is *not*
"gated by nothing at all." It is gated by the bond.

The consequence is deliberate: **the bridge can only grow as fast as members will bond.** That is the
property that stops it outrunning its own collateral, and the price is that growth needs new members
rather than larger ones.

### Where the money comes from

**The fee is gross, and it is the only revenue.** 30 bp on mint and 30 bp on redeem cover the real
transaction costs — Solana fees, BSV relay — and **the remainder is the bonded members' income**,
shared pro rata to stake. At the 1 BSV minimum the non-refundable cost of a mint is about **$0.001**
against a **$0.09** fee, so the margin is the members', and the one cost that exceeds the fee
(first-time ATA rent, $0.115) is **refundable**.

**Members are paid to carry the risk, and the bond is the risk.** Their return is the fee share; their
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
| A member signs an intent matching **no authorised redemption** | **Yes** — intents are recorded on Solana, so it is checked against the redemption set |


## Collusion is a stated risk, not a mitigated one

**A threshold of members can collude, and nothing here prevents it.** There is no on-chain predicate
that proves which members signed an off-chain threshold signature, so there is nothing to slash on.
An earlier draft claimed otherwise; that row is deleted.

**The maximum loss, measured:**

```
colluders deposit B  →  mint B solBSV  →  bond it
honest holders hold H, and the reserve R = B + H
colluders take R     →  they recover their own B and take H
```

**Net gain to the colluders = H, the entire non-member supply — and the size of the bond does not
change it.** A bond denominated in the asset it protects is a round trip: funded by a deposit into the
very reserve it is meant to cover.

**Why the bond is not moved to a separate asset.** A `SOL` bond would be genuinely separate and
seizable — but sizing it against a `solBSV` liability needs a **SOL/BSV price**, and that is an oracle.
This design decides nothing on external data. So the denomination stays, and the weakness is recorded
rather than papered over.

**What the bond does do:** it deters, it prices entry, and it makes **provable** misbehaviour —
equivocation, a member signing two conflicting intents — expensive. It does **not** protect against a
colluding threshold.

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
It is **not** oracle-driven: no external metric gates anything. **Minting is gated by nothing at all** — a verified proof is sufficient. Peg-**outs** are gated by the threshold key, which is an internal quorum, not an oracle. Reorg depth and block time come from BSV
headers; deadlines come from Solana slots.

---

## Where it stands

| | |
|---|---|
| **Light client** | **Built.** cw-144 implemented and verified against 324/324 real mainnet headers |
| **Token and mint** | **Built.** 20 on-chain tests, with negative controls |
| **BSV-side peg-in** | **Built.** 51/51 synthetic, 21/21 against a live SV Node |
| **Vault** | **Designed, not built.** Rewritten against this model after two audits of the **pre-federation** vault (V1–V10, W1–W11, T1–T12): **28 of 33 findings dissolved on the model change**, 6 remain, two blocking |
| **Federation** | **Designed, not built.** Threshold custody, governance, slashing |
| **Peg-out** | **Designed, not built** |
| **Order book** | **Removed.** A governed 30 bp fee replaces it |

**A ceiling worth naming:** the built program caps the used-deposit list at `MAX_USED = 200` per
window, which bounds peg-ins to 200 per 32 hours *with no attacker at all*. **Decision P5 replaces
that list with a nullifier per minted deposit, removing the ceiling** — decided, not built. Until it
is, a volume spike cannot be served, which bounds the "redemptions never pause" story from the
peg-in side.

**Open, and not dressed up:** the vault's design has failed two audits; the genesis bootstrap has no
path (members bond `solBSV`, which does not exist until a mint happens); sharding is undecided; and
the DAA rule is hard-coded, so a BSV change would halt the bridge until governance acts.
