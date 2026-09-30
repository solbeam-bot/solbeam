# 03. The federation

The system is run by a **bonded federation** whose members run software, hold the reserve, and are
governed by a slow supermajority. This chapter covers membership, the reserve script, governance,
slashing — and then puts the whole thing beside **RenVM**, the reference architecture, mechanism by
mechanism.

**Status: designed, not built.** No block, bond, member, threshold signature or governance vote
described here exists in code. The built set is the light client (cw-144), the `solBSV` token, the
mint, fork staging, the nullifier, the timelocked authority and the vault — **17 instructions, 37
passing / 0 failing**. The one piece of the federation that *is* in code is the **deposit-script
check**: the program accepts a 2-of-2 `OP_CHECKMULTISIG` reserve script (`is_reserve_multisig`,
`MAX_SCRIPT_LEN = 71`, `DepositScript::SPACE = 84`, committed). That is the *shape*; **the gateway
threshold key and the Greycore that would fill it are not built, and no deposit has ever paid one.**

---

## 1. The two ideas this rests on

Everything below follows from these, and both were arrived at by correcting earlier drafts.

**1. The floor is the exit, not immutability.** Governance holds the upgrade authority and can in
principle change anything. That is safe — not because the rules are frozen, but because **a change
takes 30 days and redemptions cannot be paused during it.** A hostile proposal is visible while it is
only a proposal, and anyone who dislikes it leaves. By the time it takes effect there is nothing left
to take. **The protection is the exit window, not a constitution.**

**2. Slashing works by making misbehaviour self-proving.** You cannot deduce who was at fault from an
opaque threshold signature. So you don't: **members sign payout intents individually**, and a member
who signs two conflicting things has produced **their own proof of guilt**, which anyone can submit.

---

## 2. Membership

**Admission is the Greycore's job.** The **Greycore** — the trusted, non-operational body — **finds
and admits replacement members**. Entry has a capital gate, so the set is permissioned but not
anonymous. That is the same shape as the reference, where the second set is chosen by governance
rather than by chance.

- **Two bonds, one per direction:** a **BSV-side bond** held **outside the reserve**, and a
  **`solBSV`-side bond**, because that leg must be **seizable on Solana**. **Neither bond sits inside
  the reserve.**
- **The bond is the float** — working capital that lets the federation serve redemptions — **not a
  capital requirement sized against the reserve, and not a capacity ceiling.** The reading that bond
  size caps total value locked is **withdrawn**, along with the `~$180k` capacity figure computed from
  it. **What constrains the reserve is the Greycore's co-signature on every reserve spend.**
- Fees are earned **pro rata to stake**.
- Leaving: announce, wait the unbonding period, withdraw if still covered on **both** sides.
- **A leaver's shares are an open finalisation item** (§8).

**Members run software, not judgement.** There is no manual approval of any transaction: each node
watches both chains, verifies independently with its own light client, signs, and challenges —
automatically. It is **running a staked node**: pledge a bond, run the software, earn a yield, lose
the bond for misbehaving.

**The BSV-side bond is enforced by the members, not by the Solana program.** It sits under the
**collective (threshold ECDSA) key** — the same primitive as the reserve, pointed at the bond — and
**not under the member's own key.** That is the design requirement: a member cannot move their own
bond, and the federation can, by signing a threshold transaction that moves it. **Slashing pays the
slashers from the seized bond**, which is the motive. It is a **collective action by the majority**,
not an automatic rule — nothing on BSV compels the members to sign. Two residuals are stated rather
than glossed: **a majority could seize an honest member's bond**, and the **duty to slash is social,
not on-chain.**

### What a member does — all automatic

| Job | How |
|---|---|
| Watch both chains and verify **independently** | Each node runs its own light client; it does not take the others' word |
| Relay BSV headers to Solana | Permissionless and unpaid in itself — but nothing releases, including the member's own deposits, if the tip does not advance |
| Hold the reserve under a **2-of-2 `OP_CHECKMULTISIG`** | The gateway's threshold ECDSA key plus the Greycore's. **Both must sign** |
| Hold the **mint-side bonds** under the same collective key | A caught member cannot move their own bond; the members seize it by signing a threshold transaction, slashers paid from it |
| **Sign payout intents individually** | Recorded on Solana, so every approval is attributed |
| Produce the threshold signature for the BSV payout | Only once enough attributed intents exist, and the **Greycore co-signs** it |
| **Report spent deposit outpoints** | Solana cannot read the BSV UTXO set; the software reports which deposit outpoints are spent and the program checks mints against that record |
| **Challenge theft** | The node software *is* the challenger |

**Making the challenger part of the node is the important change.** It was previously an unpaid chore
nobody owned; it is now a funded job done by the parties with the most to lose. Advancing headers,
verifying independently, signing intents and challenging theft are all work, and the **30 bp fee** is
what pays for it.

### Rewards

- **Revenue:** the governed fee — **30 bp** — paid pro rata to pledged stake. There is no bid to post
  and no price to set: a member's income is a function of volume and its share of the bond.
- **Costs:** BSV transaction fees (tiny), Solana transaction fees, and the opportunity cost of the
  bonds — the dominant cost.
- **Risk:** the bonds. They price **provable misbehaviour**; they do not restore a loss.
- **The coverage floor is not a member's choice:** `bsv_bond ≥ k × (BSV held)` and `solbsv_bond ≥
  k × (solBSV held)`, `k = 1`, checked on mint and on exit. It is a **solvency floor, not a capacity
  ceiling.**

---

## 3. The reserve script: threshold ECDSA **plus** a Greycore co-signature

**The reserve script is a 2-of-2 `OP_CHECKMULTISIG`:**

```
OP_2  <gateway threshold key>  <greycore key>  OP_2  OP_CHECKMULTISIG
```

**One leg is the gateway's threshold ECDSA key.** Shares are held by the members, a payout needs `t`
of `n` of them to cooperate, and the key is **never assembled in one place**. The threshold group
emits **one** signature however many members signed.

**The other leg is the Greycore's key.** **Both must sign**, so:

- the gateway majority **cannot move funds alone** — this fixes the collective-key hostage problem;
- the Greycore cannot move funds alone;
- **the Greycore polices every reserve spend**, which is what the reference does.

**The Greycore is trusted third parties, not node operators.** RenVM's own words are *"Darknodes that
have developed reputations with the community"*, chosen by governance and with a stake in the system's
safety: **people with reputations to lose, who do not run the reserve.** We mirror the reference
deliberately, because the precedent is good. Its **size** and **threshold** are separately `open`.

**The gateway threshold is `4-of-N`, with `N` a variable.** The number is arbitrary and deferred. It
is `t` of `n` for the gateway **threshold ECDSA** key. **Changing the gateway's `t` or `n` is a
re-sharing**, not a migration — the gateway key's address does not change. **Changing the Greycore's
key does change the deposit script**, so that one membership change requires moving the reserve.

**What is built, and what is not.** The code accepts the 2-of-2 reserve script shape: `is_p2pkh` and
`is_reserve_multisig` (71 bytes: `52 21 <33> 21 <33> 52 ae`) are both accepted by
`is_acceptable_deposit_script`, `MAX_SCRIPT_LEN = 71`, and `DepositScript::SPACE = 84` (8 discriminator
+ 4 length + 71 + 1). **This reverses audit F10**, which had concluded the P2PKH check was correct:
with the Greycore adopted as a co-signer the deposit script genuinely **is** a multisig, and the code
was right only for a P2PKH reserve. What is **not** built is anything behind the shape — no key
generation, no sharing, no Greycore, no signing.

**What the program cannot see:** the BSV itself. The published invariant — `custodied BSV ≥
outstanding solBSV` — is **monitored, not enforced**, because the reserve is off-chain. The one
**reported input is the spent-outpoint record**: Solana cannot read the BSV UTXO set, so the
federation's software reports spent deposit outpoints and the program checks mints against that
record. That is **an accepted oracle, and it is not a new trust**: the federation is already trusted
with the reserve, and a party that can take the whole reserve is not meaningfully constrained by an
honesty request. The honest sentence is two sentences — *"The program verifies deposits. The
federation reports backing."*

---

## 4. Governance

| | |
|---|---|
| **Who may propose** | Any member |
| **To pass** | **85% of pledged coins** |
| **Delay** | **30 days** from passing to taking effect |
| **Delay floor** | `gov.delay_min`, **`open`** — 7 days proposed |
| **Signal** | **Live from the moment it is raised**, not only when it passes |
| **Exit** | **Redemptions stay open throughout, and can never be paused** |
| **Scope** | Includes the **upgrade authority** |

All of those are **parameters**, not constants.

**Because governance holds the upgrade key, there is no immutable floor** — a deliberate choice, not
an oversight. The floor is the exit window: 30 days of live signal during which redemptions work, so
a change that would harm holders empties the bridge before it lands.

**The delay floor is open, not settled.** If `gov.delay_min` is set, a first proposal cannot set the
delay to zero — it can only shorten it to the floor — so the two-step attack still has to be exited
during that window. If it is not set, the exit window is whatever the current majority allows. Both
readings are defensible; the choice is a judgement about how much the majority is trusted, not a
correctness question. This is the F7 decision: reducible with a floor, not a ratchet.

**The residual, stated plainly:** a holder who does not watch and does not act within the delay is
exposed. That is a disclosure obligation, not a mechanism.

**Pause** stops **new mints only**, carries a lower threshold (a simple majority of pledged coins),
and lifts automatically after N days unless renewed. **Pausing inbound is a safety valve; pausing
outbound is taking hostages.** The two are deliberately not bundled.

---

## 5. Slashing — self-proving misbehaviour

**This is copied from what RenVM actually deployed, not from what it planned.**

RenVM's `DarknodeSlasher` exposes functions such as
([docs](https://renproject.github.io/ren-client-docs/contracts/darknode-sol/DarknodeSlasher)):

```
slashDuplicatePropose(height, round, blockhash1, signature1, blockhash2, signature2)
slashDuplicatePrevote(...)
slashDuplicatePrecommit(...)
```

**The node's own two conflicting signatures are the entire proof.** No judgement, no vote, no
inference — a member who signs two conflicting statements has signed their own evidence, and
**anyone** can submit it for a bounty.

Our equivalent, given that members sign payout intents individually:

| Misbehaviour | Provable? | How |
|---|---|---|
| A member signs **two conflicting payout intents** | **Yes — self-proving** | Two signatures, one member, conflicting statements. Anyone slashes, anyone is paid |
| A **threshold** of members signs something invalid | Attributable, since every signature is on record | But this is a governance matter, not a cryptographic one |

**One row was deleted, not corrected: "an intent matching no authorised redemption."** A *closed*
`PegOut` is indistinguishable from one that never existed, so the program cannot decide the
predicate, and checking it would false-positive against an honest member who attested before a cancel.

**And the warning worth carrying:** RenVM's own documentation says the slasher *"will become a voting
system for darknodes to deregister other misbehaving darknodes. **Right now, it is a placeholder.**"*
Only the cryptographic half was ever built. **We copy the half that shipped and are explicit that the
rest has no precedent.**

### Three security layers

| Layer | Catches |
|---|---|
| **Threshold signature** over the BSV payout | A **minority** moving funds |
| **Individual attestations** on Solana | A **minority's equivocation** |
| **Covenant** (research, later) | A **colluding majority** — enforced by miners, not members |

**A threshold signature does not reveal who signed**, which is exactly why the individual attestation
layer exists: it creates **attribution**. And **none of the three catches a consistent majority**, who
sign the same fraudulent thing and never equivocate. **"Double threshold"** must not be read as a
stronger threshold — it means **a key plus a paper trail.**

---

## 6. The reference: RenVM in one page

Read from the source rather than recalled:
[RenVM Sharding](https://raw.githubusercontent.com/wiki/renproject/ren/Sharding.md) and
[Safety and Liveliness](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md).

**Three kinds of shard:**

| | |
|---|---|
| **Gateway shards** | Randomly selected, 100 Darknodes each. Generate, hold and rotate a **threshold ECDSA key** used to custody assets. A shard is *corrupted* at **1/3+** adversarial |
| **Coordination shard** | 100 nodes. Decides which nodes are in which shard, and orders transactions. **Holds no funds** |
| **Greycore** | Selected **by community governance**, not randomisation — *"Darknodes that have developed reputations with the community."* **Acts as a secondary signature for every gateway shard** |

**The double signature is the Greycore.** In the source's own words: *"gateway shards **cannot mint or
release assets without this second signature**."* Two independent quorums must both agree, and the
second is chosen by governance rather than by chance.

**Epochs.** A discrete interval (24 hours) at which everything rebases: pending registrations activate,
pending deregistrations deactivate, nodes are shuffled into new shards, and **all gateway keys are
regenerated** — *"the gateway shards in E sign transactions that forwards all assets to the newly
generated"* keys.

**Membership.** Join: bond 100,000 REN, then wait until the next epoch. Leave: deregister, then wait
**until the next epoch *and* one full epoch** before withdrawing, because *"Darknode bonds that have
not been withdrawn can be slashed."*

**Slashing — two mechanisms, and only one is about attribution.**

1. **Equivocation.** Proposing, prevoting or precommitting two different blocks at the same height and
   round. Both conflicting signatures go to the slasher contract. Self-proving.
2. **Challenge-and-prove, shard-wide.** A shard that signs a mint **without witnessing a lock**, or a
   release **without witnessing a burn**, can be challenged. **The challenger posts a bond.** The shard
   must produce an **SPV proof** before the end of the next epoch — or *"every Darknode in the
   challenged shard"* has its bond slashed. If the proof succeeds, the challenger's bond goes to the
   prover.

**Capacity is an economic constraint, not a script:** *"a bribery attack is not profitable as long as
the sum value of all REN bonds of all Darknodes in the shard is greater than 3x the value of origin
assets locked in the shard."* The multiple is **`n/t`**, and it falls out of the signing threshold: a
briber must buy `t` of the `n` bonds, so the bonds must be worth `n/t` times the assets. For RenVM
that is `100/34 ≈ 3×`. **It is RenVM's calculation for its own parameters and does not transfer to
us** — see §7. RenVM manages the ratio **by fees, not an oracle**: nodes vote on fees to keep the bond
valuable enough.

---

## 7. What we take, change, add and leave out

### The contribution

> **RenVM trusts its shards to *report* a lock; we *verify* it.**

Under the reference, a mint happens because the shard **witnessed** a lock and signed: *"RenVM will
never produce a minting signature unless it has witnessed a respective lock on the origin chain."*
"Witnessed" means the Darknodes saw it and agreed, and Ethereum takes their signature as proof. **The
source chain is trusted through the federation** — which is why challenge-and-prove is load-bearing
there, and why its slashing rule is *"produce an SPV proof or every bond in the shard is slashed."*
The federation is the oracle; the bond keeps it honest.

**A Solana program verifying BSV inverts that.** Proof of work is checked against the real difficulty
rule, and the deposit against a Merkle path. A fraudulent mint is not provable-after-the-fact; it is
**rejected at the instruction**. **And the reorg case is one the reference does not address at all** —
not differently, *not at all*. If a lock is observed and the block is then reorged away, there is no
mechanism to notice and none to reverse it. **Ours notices on-chain and reverses it**, because a reorg
is a fact about headers the program already stores.

**What it buys:** minting that does not depend on the federation being honest. The federation's trust
assumption narrows to **custody of the reserve and the spent-outpoint record** — not to the truth of
every deposit.

**What it costs:** a header window, cw-144, a seed at `initialize`, a vault, a maturity period, and the
147-record bootstrap that took three audits to get right. **What the reference does not carry, it
avoids by trusting its own shards to report. We chose not to, and we still report the one thing Solana
cannot see.**

### What we take unchanged

1. **The Greycore shape:** a second, appointed quorum with a reputation stake. An attacker *"must
   still attack gateway shards as if there was no Greycore, and then also attack the Greycore
   itself."* The precedent is good because it was deployed by a live bridge, and because the choice
   of members is deliberately accountable rather than anonymous.
2. **Self-proving equivocation slashing** — the one slashing mechanism RenVM actually shipped.
3. **The threshold-key custody primitive** — *"never seen by anyone"*; no single member can move the
   reserve.
4. **The economic framing of safety** — the bond is the real security budget and the fee is the lever.
5. **No price oracle** — RenVM declines an oracle in favour of a discounted-cash-flow valuation by the
   nodes themselves.

### What we change, and why

| RenVM | SOLBEAM | Why |
|---|---|---|
| ~100 Darknodes per shard, corrupted at 1/3+ | **`4-of-N`, `N` variable** | A five-or-so-member federation cannot carry a 1/3 threshold meaningfully, so we lowered `t` to near-unanimity. **The cost is in §9** |
| Greycore co-signs a threshold-key signature | **Greycore co-signs the reserve script** | With threshold-ECDSA custody there is one key and no place to attach a second signature — so the second quorum moved **into the script**. RenVM solves it at the MPC level; we solve it with `OP_CHECKMULTISIG` |
| `3L < B` capacity, enforced by fee curves | **The bond is the float; the Greycore co-signature constrains the reserve** | The vault-ratio version did not reconcile: a symmetric bond gave `H ≤ 0`, and the adopted 3× was RenVM's own `n/t`. Rather than force a ratio, capacity is bounded by the **second quorum's willingness to co-sign** |
| The shard witnesses and reports the lock; the host chain trusts the signature | **The program verifies the BSV chain; the federation reports backing only where Solana cannot see** | A Solana program can check proof of work and inclusion itself, so it does. But Solana cannot see BSV's UTXO set, so spent-outpoint knowledge remains an oracle |
| Key rotation every epoch | **No rotation** | The built `initialize_bridge` fixes `deposit_script` once, so the reserve address cannot change and old-address deposits would become unprovable. **This is an omission, not a free win** |
| Sharding for isolation (`1/N` loss) | **One reserve, no shards** | A small set does not shard. Sharding is the scale path. The cost is that an attack is not contained |
| Node voting on fee curves; continuous fee | **85% supermajority, 30-day delay, live signal, redemptions never pausable** | A small set needs a slow, visible supermajority rather than a continuous fee market |
| Challenge-and-prove is the primary defence against a fraudulent mint | **A fraudulent mint is rejected at the instruction; challenge-and-prove is a backstop** | A mint cannot happen without a verified header. It remains designed for the release side, where **no enforceable predicate is specified** |
| REN-only bond, no oracle | **Two-sided bonds in the asset each side holds** | Each side must be seizable where the liability sits. No external price is consulted |
| Its own consensus chain and a coordination shard | **Solana is the coordination layer** | The program cannot equivocate with itself; no consensus protocol is needed |

### What we deliberately leave out

**1. Key rotation — omitted.** RenVM regenerates every gateway key each epoch and forwards all assets,
*"required so that, as Darknodes exit the network, they do not continue to know shares of actively used
ECDSA private keys."* **Honest cost:** a member who leaves, is removed, or is compromised keeps shares
of a **live** key indefinitely. Rotation would have bounded that to one epoch. The only replacement —
proactive re-sharing — is **not specified anywhere**, and the built program fixes `deposit_script`
once, so rotation is **currently unimplementable**.

**2. Leaver-share invalidation — omitted, and recorded as open for finalisation.** A departing member
**keeps a valid share**, so the **effective threshold degrades with churn**: at `4-of-N`, four former
members together still hold four valid shares, and the threshold is a property of the current member
set only in name. Two remedies, both real work: **key rotation** (the reserve moves on-chain;
`deposit_script` must change) or **proactive re-sharing** (needs the departing member's cooperation).
**The PoC deliberately does not finalise this**, and it is the sharpest open problem in the federation
design.

---

## 8. Row-by-row against RenVM

**Verdicts:** *Take* — same mechanism, same reason. *Change* — RenVM has an analogue, adapted.
*Add* — RenVM has no equivalent. *Omit* — RenVM has it, we deliberately do not. *Weaker* — RenVM does
it better. *Open* — our design is not specified.

| Mechanism | RenVM | SOLBEAM (designed) | Verdict |
|---|---|---|---|
| **How a mint is authorised** | A gateway shard observes the lock, checks the UTXO, its uniqueness and **6 confirmations**, then threshold-signs; the Greycore adds a second signature | The **Solana program verifies the BSV chain itself** in `verify_deposit`. The federation is an **accepted oracle** only for what Solana cannot see | **Add** |
| **How a release is authorised** | A shard observes the **burn** (12 confirmations), threshold-signs, Greycore co-signs | **Designed, not built.** Escrow, individually-signed intents, threshold signature plus Greycore co-signature, settlement proved against the light client, permissionless cancel | **Change** |
| **What the host chain trusts** | A signature plus a uniqueness hash. It does not verify Bitcoin | Solana trusts **its own verification** of BSV for deposits; for **backing** it trusts the federation's report | **Add** for deposits, **Weaker** for backing |
| **Reorg handling** | **Not addressed in the sources** | The client keeps a header window and a stored block hash per staged mint; a followed reorg is visible on-chain and the staged tokens burn. **Fork staging and the reversal are built; maturity ships at 0, so the window is off by parameter** | **Add** |
| **Custody structure** | One threshold ECDSA key per shard, held by 100 nodes, rotated every epoch; the gateway script is a P2PKH-shaped script | `OP_2 <gateway threshold key> <greycore key> OP_2 OP_CHECKMULTISIG`, gateway at `4-of-N`. The reserve never moves when membership changes — re-sharing, not migration | **Change** |
| **Second quorum** | The Greycore, chosen by community governance, co-signing every gateway action | The Greycore is a **2-of-2 co-signer** on the reserve script. Same *shape*, enforced **in the script** rather than by a second MPC signature | **Take** (shape), **Change** (mechanism) |
| **Membership admission** | Bond 100,000 REN, wait until the next epoch; shard selection is random and unbiased | **Open entry** on posting both bonds, admitted by the Greycore. The ceremony that admits a member into the `4-of-N` threshold set is **not specified** | **Open** |
| **Membership exit** | Deregister, wait an epoch plus a full epoch | Announce, wait the unbonding period, withdraw only if still covered on both sides. `fed.unbond_slots` is `open`, and **a departing member keeps a valid share** | **Open / Weaker** |
| **Key rotation** | Every epoch | **Deliberately omitted**, and currently unimplementable | **Omit** |
| **Leaver-share invalidation** | Implied by rotation | **Deliberately unresolved.** The effective threshold degrades with churn | **Weaker** |
| **Slashing — equivocation** | `slashDuplicatePropose` / `Prevote` / `Precommit`; the node's own signatures are the proof | **The same cryptographic half, on intents instead of blocks.** Designed, not built | **Take** |
| **Slashing — bad mint/release** | Challenge-and-prove, shard-wide, with an SPV-proof deadline | **Not needed for a mint** — rejected at the instruction. **For a release there is no enforceable predicate.** The backstop is designed but not built, and its release-side trigger is not specified | **Weaker** |
| **Bond purpose and size** | 100,000 REN; sizes the system toward `B ≥ 3L` | **The bond is the float.** Two-sided, 1,000 BSV per side as a placeholder, **not specified** as a security number | **Change** |
| **Capacity constraint** | `3L < B` and `L < B`, enforced by governed fee curves; no oracle | The bond is the float and the Greycore co-signature constrains the reserve. **No replacement numeric capacity rule is specified** | **Open / Weaker** |
| **Fee model** | 0.1% mint, 0.1% burn, plus a continuous fee, plus chain fees; curves governed | **30 bp each way, governed.** No continuous fee, no order book. One cost exceeds the fee at the minimum deposit — first-time ATA rent — and it is **refundable** | **Change** |
| **Governance** | Darknodes vote on fees and parameters; the Greycore is chosen by community governance | 85% / 30 days / live signal, holding the upgrade authority, redemptions never pausable. **Designed, not built** — except the **authority timelock**, which is built (32 slots) | **Change** |
| **Sharding / isolation** | Randomly sampled, continuously shuffling shards; an attack removes `1/N` | **No sharding.** Blast radius is **100%** of the reserve | **Weaker** |
| **Emergency pause** | Not described as a pause; unresponsive members are removed by governance | **`pause_mints()` only.** Redemptions cannot be paused, ever; the pause lifts automatically. Authority changes have a **32-slot timelock, built** | **Add** |
| **What the user trusts** | A <1/3 threshold of a 100-node shard, the Greycore, and the bond economics | The program verifies deposits; the federation reports backing; the Greycore constrains the reserve. The assumption is that neither a `4-of-N` of the federation nor the Greycore colludes | **Change** |

---

## 9. Where we are weaker than RenVM

**Not softened, because this is the most useful section for a reviewer.**

1. **RenVM ran a BFT consensus with 100-node shards; we run `4-of-N` with `N` a variable.** RenVM's
   safety holds while less than 1/3 of a 100-node shard is adversarial — on the order of **34
   coordinating nodes**. Our threshold is **4 members**, and `N` is not even fixed. A four-person
   collusion is a far smaller, more findable, more bribeable set than 34 anonymous Darknodes.
   Near-unanimity of a small set is **not** equivalent to a supermajority of a large one.
2. **RenVM sharded for isolation; we do not.** With one reserve and no shards, **the blast radius is
   100% of the reserve.** `fed.shards` is `open`, which means this is not a deliberate scope decision
   so much as an unbuilt one.
3. **RenVM's capacity and bond economics are specified; ours are a float.** RenVM states `3L < B`
   and `L < B`, gives governed fee curves that enforce them, and explains how nodes value their bonds
   without an oracle. Our bond is the float, the vault-ratio arithmetic is **superseded**, and **no
   replacement numeric capacity rule is specified. A reviewer cannot compute our capacity from our
   documents, and that is a genuine gap.**
4. **The leaver-share problem is unaddressed.** RenVM guarantees a departed member holds no shares of
   a live key. We guarantee the opposite by omission: a departing member keeps a valid share, the
   effective threshold degrades with churn, and invalidation is deliberately unresolved. **This is the
   single clearest case where RenVM is stronger and we know it.**
5. **RenVM's slashing is enforced on-chain by contracts; half of ours is a social action.** Our
   BSV-side bond sits under the collective key, and seizing it requires the members to *choose* to
   sign. A majority could also seize an honest member's bond.
6. **Our challenge-and-prove has no enforceable predicate on the release side.** We deleted the
   "intent matching no authorised redemption" row because the predicate is undecidable, so for
   releases we have a designed mechanism whose triggering condition is not specified.
7. **RenVM shipped; our second half is designed.** The federation, the Greycore, peg-out and
   governance are **designed, not built**. The vault is now built and `release_mint` / `burn_staged`
   are permissionless — **with the shipped maturity at 0, so the protective window is off by
   parameter.** RenVM ran in production; the implementation gap is real.

**Sharding: none. Collusion: unprevented.** The reference's `n/t` capacity rule does not transfer, and
no numeric rule has replaced it. All four are stated rather than implied away.

---

## 10. What this does **not** resolve

**Key extraction at the gateway threshold is real, and RenVM says so.** *"a signature cannot be
produced, and the underlying ECDSA private key cannot be revealed … unless >=1/3rd Darknodes are
adversarial and coordinating."* So a quorum at the gateway threshold can extract the gateway key.
Slashing does not undo a stolen key. **The Greycore co-signature is what an extracted gateway key
alone cannot satisfy:** the reserve script requires the Greycore's signature too, so extraction
without Greycore collusion does not move the reserve. **A Greycore quorum that colludes with the
gateway is the residual.**

**The reserve is still off-chain.** Publishing it is an assertion by the party that could steal. The
reference does not solve this either; it leans on the bond being larger than the assets — the economic
answer rather than a verification one — and **that economic answer is weaker for us**, because the
bond is the float rather than a capital requirement sized against the reserve.

**The denomination residual.** The `solBSV`-side bond devalues in exactly the scenario it is meant to
protect against — when the reserve is gone, `solBSV` is worth nothing, so seizing or burning it
redistributes loss among holders rather than restoring BSV. The BSV-side bond is under the same key as
the reserve, so if that key moves, it sweeps the bonds too. **The honest statement is that the bond is
the float and it prices provable misbehaviour; it does not restore what is lost.**

**Collusion is unprevented.** A majority of the gateway signers acting together with the Greycore can
take the reserve, and no mechanism here prevents it. The Greycore raises the cost and relocates part
of the assumption into a named, reputational set; it does not remove it.

---

## 11. Open items

1. **`fed.threshold` and `N`** — `4-of-N` is settled as a *shape*; `N`, the admission ceremony and the
   key-generation and signing protocol are not specified.
2. **`fed.greycore_size` / `fed.greycore_threshold`** — both `open`. The Greycore co-signature is what
   constrains the reserve, so these are load-bearing.
3. **Leaver-share invalidation** — deliberately unresolved; no mechanism specified.
4. **Capacity** — no numeric rule replaces the superseded arithmetic.
5. **The release-side challenge predicate** — undecidable as written; the release path has no
   enforceable slashing condition.
6. **The spent-outpoint record** — decided (the federation reports; the program checks), but the
   record's **format and write path are unspecified**, and it is **not built**. The permissionless
   alternative was considered and rejected: making the record writable by anyone turns mint
   availability into an attack surface, not just mint correctness.
7. **The refusal hole** — a threshold that declines to attest leaves no signed artifact, so nothing is
   slashable. A timeout-and-rotate rule is the obvious candidate and is **not in the design**.
8. **`fed.shards`** — unsharded today; no stated threshold at which sharding becomes required.
9. **Genesis** — decided: a BSV-side bond at genesis, so no `solBSV` needs to exist first. A capped,
   explicitly-unbonded first mint is a documented later option, not chosen.

---

## Sources

- [Sharding](https://raw.githubusercontent.com/wiki/renproject/ren/Sharding.md) — gateway /
  coordination / Greycore shards, 100 nodes, epochs, rotation, DKG, random selection, load balancing
- [Safety and Liveliness](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md)
  — assumptions, safety properties, `1/3` thresholds, bonds and slashing, challenge-and-prove, the
  `3L < B` and `L < B` constraints, Sybil/bribery analysis
- [Gateways](https://raw.githubusercontent.com/wiki/renproject/ren/Gateways.md) — lock/mint and
  burn/release flows, confirmation depths, gateway script template, epoch rotation, fee curves
- [Greycore](https://raw.githubusercontent.com/wiki/renproject/ren/Greycore.md) — the Greycore's two
  purposes, selection, safety and liveliness arguments
- [Fees and Economics](https://raw.githubusercontent.com/wiki/renproject/ren/Fees-and-Economics.md) —
  mint/burn/continuous/underlying fees, bonding, economic-security constraints, no price oracle

**Fetched, and 404:** `Darknode-Registry.md`, `Darknode-Slasher.md`, `Epochs.md`. Where this document
states what those pages describe (the 100,000 REN bond, the deregistration delay, the duplicate-propose
slashing functions), the statement is taken from *Safety and Liveliness*, which repeats it. The
**DarknodeSlasher function names and the "placeholder" quotation** are cited from
[`renproject.github.io/ren-client-docs`](https://renproject.github.io/ren-client-docs/contracts/darknode-sol/DarknodeSlasher).
