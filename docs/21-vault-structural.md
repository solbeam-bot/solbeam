# 21. The vault — current design against the federation model

> **This is the current vault design.** It supersedes the chain of drafts, and it is written
> against [`23-federation.md`](23-federation.md), whose model is itself canonical alongside
> [`13-summary.md`](13-summary.md). Findings V1–V10, W1–W11 and T1–T12 are the historical
> record; **[`19-vault.md`](19-vault.md) and [`20-vault-revised.md`](20-vault-revised.md) stay
> as written**, and the previous state of this document is preserved in §What dissolved, below.
>
> **Why this is a re-audit and not a third patch.** The first two redesigns kept trying to
> enforce BSV-side behaviour from Solana — who holds which deposit, which relayer owes what —
> and each attempt failed on a different edge. The new model stops trying: a **bonded
> federation** holds the reserve under a **threshold key**, members record **individually
> signed payout intents**, and misbehaviour produces its own evidence. Attribution, which was
> the unsolved problem underneath V7, W3, T1 and T9, is no longer a cryptographic question.

**Status: designed, not built.** The vault, the federation, threshold custody, bonds and
peg-out are **designs**. What exists in the program is the light client, the token, the mint
and fork staging: **11 instructions, 24 on-chain tests**, cw-144 verified against **324/324**
real mainnet headers. The shipped `verify_deposit` **mints straight to the depositor's token
account** — there is no vault in the code, no staged item, no maturity and no bond.

---

## What is built, and what this document adds

`[built]` means it is in `poc/solana/programs/solbeam/src/lib.rs` today. `[designed]` means it
is specified here and nowhere in code.

| | |
|---|---|
| **Built** | `initialize`, `push_header`, `verify_deposit`, `init_staging`, `push_fork_header`, `abandon_staging`, `commit_fork`, `initialize_token`, `initialize_bridge`, `set_checkpoint`, `set_paused` |
| **Designed — vault** | `release_mint`, `finalize_mint`, `burn_staged`, `prune_marker`, `request_redeem`, `accept_intent` *(name provisional)*, `finalize_redeem`, `cancel_redeem` |
| **Designed — federation** | `stake`, `announce_unbond`, `withdraw_bond`, `attest_payout`, `slash_equivocation`, `propose`, `vote`, `execute`, `pause_mints` |

**Every instruction in the second and third rows is a proposal.** None is a commitment to a
name, an account layout or a wire format. The point of tracing the set here is that the
federation instructions are not optional decoration around the vault: `stake` is how a member
exists, `attest_payout` is the attribution mechanism, and `slash_equivocation` is the only
thing that makes the bond real.

---

## The four rules this design obeys

The two failed drafts failed the same four ways. Each class keeps its rule; what changed is
that the federation removes the reason three of them were hard.

| Class | Why it kept happening | **The rule** |
|---|---|---|
| **1. Identity confusion** | One account held two jobs, so closing it for one destroyed the other (V1, T9) | **One account, one job.** No account is reused |
| **2. Expiry** | A prunable record must not trust a caller to say when (W1) | **A record that can be pruned must contain the condition for its own pruning** |
| **3. Liveness treated as cryptography** | "Has this been reorged?" is really "has anyone told the client?" (V5, W2, W4, T8) | **The program checks its own freshness, and detection is a funded job, not a hope** |
| **4. Uncheckable invariants** | A commingled vault's "balance = sum of items" cannot be verified on Solana (V3, W8, T2) | **No shared token account.** Every escrow belongs to exactly one item |

**Class 4 is the rule that survives the model change, and it is the one the federation makes
harder to state.** The federation *does* hold a shared reserve — that is its purpose — so
"no pooled value" is now true only of the **Solana-side vault**. The BSV reserve is pooled
off-chain and controlled by a threshold key, and the security argument for it is different in
kind: not "no one can steal a pool" but "no single member can move it, and a member who
equivocates is provably guilty." §The pooled reserve, restated below says exactly what that
does and does not buy.

---

## Accounts — every one has a single job

### Solana side (designed)

| Account | Seeds | Its one job | Closed by |
|---|---|---|---|
| **`PegIn`** | `[b"in", txid, vout]` | Metadata for one staged mint | `release_mint`, `burn_staged` |
| **`PegInEscrow`** | token account of the `PegIn` PDA | Holds **that one item's** staged tokens | with its `PegIn` |
| **`Marker`** | `[b"used", txid, vout]` | Replay record. Stores **`deposit_height`** — the condition for its own pruning | `prune_marker`, once `deposit_height < window_start` |
| **`PegOut`** | `[b"out", holder, nonce]` | Metadata for one redemption, including `amount`, `fee`, `bsv_destination`, `deadline_slot` | `finalize_redeem`, `cancel_redeem` |
| **`PegOutEscrow`** | token account of the `PegOut` PDA | Holds **that one item's** escrowed tokens | with its `PegOut` |
| **`PayoutIntent`** | `[b"intent", redemption_id, member]` | One member's signed statement: amount, destination, deadline. **Attribution and equivocation evidence** | with the redemption, or on slash |
| **`Member`** | `[b"member", key]` | `{ bond, script, attestations, state }` — one bonded member | `withdraw_bond` |
| **`Bond`** | token account of the `Member` PDA | That member's staked `solBSV`, **seizable only by `slash_equivocation`** | `withdraw_bond` |
| **`Proposal`** | `[b"prop", id]` | One governance proposal: payload hash, weight, `effective_slot` | `execute` |
| **`FeeAccount`** | `[b"fees"]` | Mint-side fee accrual, distributed pro rata to members | — |

**Why class 1 is answered:** `PegIn` is not the replay marker. `PayoutIntent` is not the
redemption. Closing a resolved item cannot destroy the record that would prove a member
equivocated, because they are different accounts with different lifetimes — the T9 conflation
cannot recur, because the slash evidence is now a signed statement rather than a field on a
shared record.

**Why class 2 is answered:** `Marker` stores `deposit_height` and `prune_marker` reads it.
`PayoutIntent` carries its own `deadline_slot`, so nothing has to be told when it expired.

**Why class 4 is answered:** each escrow holds one item's tokens. `FeeAccount` is the one
deliberate exception, and it is discussed under §New defect N3 rather than waved through.

### BSV side (designed)

**One deposit script for the whole federation** — the threshold script the reserve sits in.
That replaces the per-relayer registered scripts of docs 19–21 and reverses decision **P8**
("every deposit must pay a registered relayer's script"); the reversal is recorded in
§What dissolved, T12. The script is a **parameter under governance**, and changing it changes
which deposits are acceptable to a future mint — which is the new hazard recorded as
§New defect N5.

---

## Peg-in

```
verify_deposit(claim)            [built, but not as the vault needs it]
    • parses the OP_RETURN, which must commit to version ‖ cluster_id ‖
      program_hash ‖ recipient
    • the deposit's output script must equal the federation deposit script
    • today: mints the GROSS amount straight to the recipient's token account
    • designed delta: mint NET into a NEW PegInEscrow owned by a NEW PegIn,
      mint the fee into FeeAccount, and create Marker{deposit_height}

release_mint(item)               [designed] permissionless
    • the client is FRESH — the last accepted header is within MAX_STALENESS_SLOTS
    • tip_height >= deposit_height + MATURITY_BLOCKS
    • deposit_height still in the window AND hash matches, or
      deposit_height < window_start (survived the whole window)
    • escrow -> recipient; closes PegIn and its escrow

burn_staged(item)                [designed] permissionless
    • client fresh
    • deposit_height in the window AND the stored hash DIFFERS
    • the replacement hash has MATURITY_BLOCKS behind it, so a transient
      fork cannot burn a good mint
    • burns the escrow; closes PegIn, its escrow and the Marker

prune_marker(marker)             [designed] permissionless
    • requires marker.deposit_height < window_start   <- a STORED field
```

The three corrections from the drafts are kept because they were right: **net is minted**
(V3), **identity is `(txid, vout)` and never height** (V1/A7), and **freshness replaces
"the tip advanced"** (W2). One relayer-consent step is deleted: **the federation holds the
deposit script, so no member signs off on a deposit.** That is what dissolves W7.

### Detecting a reorg is a header property, not a report

`PegIn`/`Marker` records the **block hash the deposit was proven against**, not just its
height. The program then decides from its own window:

| | Condition | Result |
|---|---|---|
| Height still in the window, stored hash matches | canonical | `release_mint` succeeds |
| Height still in the window, hash differs | reorged | `burn_staged` succeeds |
| Height has left the window | survived the window | `release_mint` succeeds; burning is no longer possible |

**The detection budget is `MATURITY_BLOCKS`, not the window, and it is narrower than it
looks.** With the built window at **192 records / 32 hours** and `FLOOR = 12`, a deposit is
provable at 12 confirmations and becomes releasable at `MATURITY = 144` blocks after
inclusion, while its hash stays checkable until `WINDOW = 192` blocks after inclusion. The
margin between "releasable" and "no longer checkable" is therefore
`WINDOW − MATURITY` = **48 blocks, about 8 hours** — and a deposit that commits to a *deeper*
`FLOOR` in its `OP_RETURN` has correspondingly **less**: at the maximum committed depth of
144 the margin is zero, because the two clocks coincide. This is V5's finding, unchanged in
kind by the model change: the window is the ceiling, and the margin is small. It is a
**parameter decision with a security consequence**, not a mechanism, and §Open items records
it as such.

---

## Peg-out

```
request_redeem(amount, bsv_destination, deadline_slot)   [designed]
    • holder -> a NEW PegOutEscrow owned by a NEW PegOut

attest_payout(item, preimage, signature)                 [designed] one per member
    • the member signs THIS redemption: amount, destination, deadline
    • the program verifies the signature over the recorded preimage
    • recorded in PayoutIntent[redemption_id, member] but NOT acted on

finalize_redeem(item, proof)                             [designed]
    • slot <= deadline_slot
    • the payout transaction pays exactly `amount` to `bsv_destination`
      and its OP_RETURN carries THIS redemption's id     <- binds one payment to one item
    • payout depth >= C_payout
    • burns the escrow; fee -> FeeAccount; closes PegOut; fee split pro rata

cancel_redeem(item)                                      [designed] permissionless
    • requires slot > deadline_slot, and no finalized settlement
    • escrow -> holder; closes PegOut. Supply is unchanged
```

**No `claimed` latch.** W6 was right that a one-way state with no resolver freezes the
escrow forever. There is no latch: the two instructions are mutually exclusive by time, so
there is no intermediate state to get stuck in.

**The threshold key is not consulted on Solana.** Members attest individually; the threshold
signature for the BSV payment is produced **off-chain** once enough attributed intents exist.
The program's job is to record the attributions and to bind the eventual payment to this
item. Anything else would be asserting a mechanism Solana cannot enforce.

**Where the payout proof comes from is an open item.** `verify_deposit` already proves Merkle
inclusion with built machinery; proving a payout with the same machinery requires the payout
transaction to be in a block **and** requires the destination address to be recoverable from
the output script. That is designed here, not built, and it is the largest single piece of
peg-out work. See §Open items 4.

---

## The federation side, which the vault does not own

```
stake(amount, script)            [designed] posts the bond; 1,000 BSV default
announce_unbond()                [designed] starts the unbonding period
withdraw_bond()                  [designed] requires bond >= k * owed AFTER; a member
                                 with outstanding obligations cannot leave
attest_payout(...)               [designed] the per-member signature above
slash_equivocation(member, redemption_id, two_signed_intents)   [designed]
    • verifies BOTH signatures are the member's
    • verifies both are over the SAME redemption_id
    • verifies the signed statements CONFLICT
    • seizes the bond; the challenger takes the bounty
propose / vote / execute / pause_mints   [designed] see 23
```

**This is the whole of the slashing story, and it is deliberately narrow.** It copies the
half of RenVM that shipped: two conflicting signatures from one node are the entire proof
(§Slashing in doc 23). It does **not** cover a threshold of members signing something invalid
— that is attributable because every signature is on record, but it is a governance matter
and this document does not dress it up as a cryptographic one.

---

## What dissolved, and what changed — all 33 findings

Recorded in full, because the pattern is the useful part: **most findings were the design
failing to enforce BSV-side behaviour that no longer needs enforcing from Solana.**
"Dissolved" below always names the mechanism; where a problem merely moved, it says so.

### V1–V10 (first audit, doc 19)

| | Old defect | Now | Why |
|---|---|---|---|
| **V1** | The nullifier *was* the pending item, and release closed it — one deposit minted repeatedly | **Dissolved** | Fixed by the two-account split in doc 20/21 (`Marker` outlives `PegIn`) and kept. The federation change does not touch it: replay is still a Solana-side identity question, and identity is still `(txid, vout)` |
| **V2** | `cancel_redeem` raced `settle_redeem`; `D = 6 h < W = 24 h` made it certain | **Dissolved** | Time-exclusive `finalize`/`cancel` plus nested deadlines. The deadline is still a Solana slot, so a cluster halt freezes it rather than burning anyone |
| **V3** | The peg-in fee was never deducted, so every release over-drew a commingled vault | **Dissolved** | The *instance* is fixed: `net` is minted into the item's own escrow, so a release can no longer over-draw anyone. The fee's remaining home is a **different** defect with a different mechanism — the fee is now governed at 30 bp rather than deducted from anything, and where the BSV physically sits is unspecified. That is T3, and it is **N1** |
| **V4** | `settle_redeem` bound neither amount nor destination — pay 1 sat, settle 1,000 tokens | **Dissolved** | `finalize_redeem` verifies value, script **and** the redemption id. The model change strengthens it: the amount and destination are what the members signed, so the payment can no longer be anything else |
| **V5** | The third canonical row released an item whose height had left the window, so the real detection budget is `MATURITY`, not the window | **Dissolved** (the release-logic defect); **changed** (the budget it exposed) | The defect — an unchecked release with no lookback — is gone: release requires client **freshness**, and the out-of-window row is an explicit, stated branch rather than a vacuous hash comparison. The budget it exposed persists as a **parameter property**: `WINDOW − MATURITY` is 48 blocks at the proposed values, and zero at the maximum committed depth, quantified in §Peg-in. The old extra defect — a Solana halt switching every item to unchecked release — is gone with the same fix |
| **V6** | `MATURITY` stored as a Solana slot while canonicality is a BSV height | **Dissolved** | Maturity is `MATURITY_BLOCKS`, a BSV height. Slots are used only where the wanted property is "a Solana halt freezes the clock" (the redemption deadline). The built `MIN_CONFIRMATIONS = 12` is already height-based |
| **V7** | `PendingMint` had no relayer field, so `owed_R` could not be attributed; a fraudulent mint would slash an innocent relayer | **Dissolved** | **The liability this modelled no longer exists.** Deposits are not underwritten by a member: they pay the federation's script, so there is no per-deposit relayer to attribute, and `owed_R` has no peg-in term to accumulate. The remaining liability is per-redemption and is attributed by the member's own signature |
| **V8** | No slash instruction existed anywhere | **Dissolved** | `slash_equivocation` exists in the design, and unlike the drafts it has an implementable predicate: two signatures, one member, conflicting statements |
| **V9** | The domain separator's encoding was undefined; a substring test could mint on two deployments; the program id is identical across clusters | **Dissolved** | The `OP_RETURN` is a **fixed framed parse** — `version ‖ cluster_id ‖ program_hash ‖ recipient` — and `cluster_id`/`program_hash` are compared. **The built code does not yet do this**: it only requires that the payload equal the recipient. So the *design* closes V9 and the *code* does not, and the built behaviour is the weaker one |
| **V10** | Rent unclaimable, bond custody undefined, permissionless resolution could burn a good mint during a transient fork, `index_of` conflated "too old" with "too new" | **Dissolved, with one part standing** | Rent: `prune_marker` is the answer. Bond custody: now a `Bond` token account held by the `Member` PDA, separate from every escrow. Transient fork: `burn_staged` requires the replacement hash to have depth behind it. "Too new": `index_of` is refused explicitly. **Standing:** nothing verifies the escrow sum against the item set on-chain, and nothing can — it is a test, and §Open items keeps it |

### W1–W11 (second audit, doc 20)

| | Old defect | Now | Why |
|---|---|---|---|
| **W1** | The prune was a replay oracle: `prune(nullifier, fabricated_height)` because the nullifier stored nothing | **Dissolved** | `Marker` stores `deposit_height`; `prune_marker` reads the stored field and never an argument. Fixed in doc 21 and untouched by the model |
| **W2** | V5 was inert in the branch it was written for — a stalled advancer compared the window to itself | **Dissolved** | Release requires **freshness** (`now − last_push_slot ≤ MAX_STALENESS_SLOTS`), so a stalled client blocks release outright. Stated limitation, kept: freshness measures update recency, not honesty (see T8) |
| **W3** | `slash` had no on-chain predicate: "spent BSV it owed" is not program-checkable | **Dissolved** | The predicate changed substrate. It is no longer "prove a spend was unauthorised" but "prove this member signed two conflicting statements" — checkable from two signatures and two recorded preimages |
| **W4** | R1's no-bounty argument was unsound: the attacker is the most motivated pusher, and a light client cannot out-mine them | **Dissolved** | The honest pusher is now a **funded role**: members run nodes, push headers, and earn fees. The design no longer asks an unpaid third party to beat an attacker with hashpower it does not have |
| **W5** | Payout proofs were replayable: one payment settled every redemption with the same amount and destination | **Dissolved** | The payout carries **this redemption's id**, and the id is inside the statement the members signed. Two redemptions to one exchange address are now two distinguishable payments |
| **W6** | `claimed` was a one-way latch with no resolver; a reorged payout froze the escrow forever | **Dissolved** | The latch is deleted. `finalize` and `cancel` are mutually exclusive by time. A reorged payout simply does not finalize, and the deadline returns the escrow |
| **W7** | Requiring the relayer's per-deposit signature made minting non-permissionless and let a relayer strand a deposit | **Dissolved** | Per-deposit consent is gone. The deposit pays the federation's script and **anyone** may call `verify_deposit`. Minting is permissionless again, which is what P3's remedy depended on |
| **W8** | The vault-invariant claim was false while the fee sink and the shared payout were undefined | **Changed** | The claim is no longer made: this document says the on-chain sum is unverifiable and needs a test. The fee sink is now named (`FeeAccount`) rather than undefined — but naming it does not remove the exception it creates unless it is specified, hence **N3** |
| **W9** | The `UNBACKED` flag labelled an unbacked mint without bounding it; the loss landed on holders who never chose it | **Dissolved** | The whole path is removed: every deposit pays one **federation** script, so there is no per-deposit underwriter and nothing for a flag to distinguish. There is no unbacked mode to label and no bounding bit that pretends to bound it. **Cost of the removal, stated:** it reverses D6 and P8 |
| **W10** | The instruction set did not compose: no resolver for `claimed`, two-phase settle specified as one, `OP_RETURN` contradicting R4, `D ≥ W + C_payout` mixing slots and blocks, `nonce` in seeds but not the record, no `owed` decrement, non-atomic inits | **Dissolved, with two items standing** | Resolver: deleted latch. Two-phase: `finalize` is one instruction with a depth requirement. `OP_RETURN`: R4's `flags` field is deleted, so the contradiction is gone. Slots-vs-blocks: `deadline_slot` is a slot and `C_payout` is BSV depth — the two units are now named where they are used rather than in one inequality. `nonce` and `owed`: the record now carries `nonce`, and the shared `owed` counter is replaced by per-intent attribution. **Standing:** the atomicity of `verify_deposit`'s account creations is still unverified and still untested |
| **W11** | "Nothing is trusted" was false: the checkpoint is trusted, the upgrade authority overrides every parameter, a relayer signature was required, and the client halted at the first retarget | **Dissolved** | F7 is fixed (cw-144, 324/324). The upgrade authority is now governance at **85% / 30 days / live signal**, so it is a disclosed power with a delay rather than an unconditional one. The relayer signature is gone. **The checkpoint is still trusted**, and doc 23's floor argument — the exit window, not immutability — is what covers it. The paragraph that made the false claim is not in this document |

### T1–T12 (third audit, doc 21 as it stood)

| | Old defect | Now | Why |
|---|---|---|---|
| **T1** | `slash` was **forgeable**: the spending transaction need not be in a block, so anyone could craft bytes consuming a public outpoint and burn any relayer's bond; the predicate was a universal quantifier over a set Solana cannot enumerate | **Dissolved** | The predicate no longer contains a spending transaction or a universal quantifier. It is two signatures by one member over conflicting statements about one redemption. The forgery route — a Merkle-free spend — is not part of the new predicate |
| **T2** | The redesign violated its own class-4 rule: `slash` paid into a pooled "holders' reserve account"; also, burning a bond leaves nothing to transfer | **Changed** | The pooled reserve is now **intentional**: the federation holds it under a threshold key. The self-contradiction is gone because slashing **seizes a live, seizable asset** rather than burning it. The class-4 rule is restated to apply only to the Solana-side escrow — see §The pooled reserve, restated |
| **T3** | The peg-in fee had no home: the `OP_RETURN` has no amount or fee field, so "mint NET" was undefined, and trusting the relayer's declared fee trusts the party with the incentive to overstate it | **Changed** | The fee is no longer discovered or declared — it is **30 bp, governed** — so the fee is computable from the amount the program verifies. What remains is where the 30 bp physically is: the BSV arrived in the federation's threshold script. See **N1** |
| **T4** | Recorded P2 was unanswered: `commit_fork` spliced without re-checking the fork point | **Dissolved** | `fork_parent_hash` is recorded at `init_staging` and re-checked at `commit_fork` (`ForkPointMoved`). It is in the built code, with a test both ways |
| **T5** | `owed_R` double-counted at mint and accept with only settlement decrementing, so the counter grew monotonically and the bond locked | **Dissolved** | The shared counter is gone. Liability is per-redemption and attributed by the member's own signature; there is no accumulation across mint and redeem to double-count, because member liability does not arise at mint |
| **T6** | The settle/cancel race was still live: `deadline_slot` was caller-supplied with no `>= now + D` requirement, so a holder could choose a past deadline, keep the BSV and reclaim the escrow | **Changed** | Still a real class, in a new form. The escrow and the deadline are now on the redemption record, and the holder can still choose a short deadline in the hope of being paid **and** refunded. The fix is a **minimum deadline** at `request_redeem` (`deadline_slot >= now + D_MIN`, with `D_MIN` covering payout plus depth), and it must be in the instruction. §Open items 5 |
| **T7** | Post-settle reorg left the holder with nothing: the escrow was burned and the item closed, so neither instruction could resolve it | **Dissolved** | The payout is now made **from the federation's own reserve**, not from a floating relayer's float, and `finalize_redeem` requires `C_payout` blocks behind it before the escrow burns. A payout that is not buried does not finalize, and the deadline path still returns the escrow. The reserve does not depend on the payout surviving — doc 13's original point, which the per-relayer model then contradicted |
| **T8** | Freshness was satisfied by the attacker (`last_push_slot` advances on any accepted header), and a mint staged on a losing branch became releasable once its height left the window — the honest chain advancing destroyed the only evidence | **Dissolved** | The dependency is inverted: members are **required by their job** to keep the client current, and a stalled client blocks release rather than forcing it. There is no longer a design that relies on an unpaid party out-pushing an attacker. The honesty half of the old finding is **not** solved cryptographically and is not claimed to be: freshness still measures recency |
| **T9** | `Marker` was three things at once (replay record, slash evidence, closeable object) with contradictory field lists | **Dissolved** | `Marker` is now one thing (replay) and `PayoutIntent` is another (evidence). The slash evidence is not a field on a shared record but a signature the member produced themselves |
| **T10** | Chainwork arithmetic underspecified: 256-bit division, the `initialize` baseline (cumulative from genesis, never listed as trusted), and `commit_fork` never specified to write the new state | **Dissolved** | Each record stores its own cumulative chainwork; `commit_fork` compares chainwork and writes the rebuilt window, tip, hash and `last_push_slot`. The baseline is the checkpoint. The arithmetic is verified against **324/324** real mainnet headers. Remaining, and preserved: the comparison itself is exercised only on a constant-difficulty fixture, and `work_from_bits` must **saturate** rather than wrap |
| **T11** | Rent rose to ~$0.11 extra per item, and no `stake`/`announce_unbond`/`withdraw_bond` instruction existed though the table said `Bond` was closed by one | **Changed** | Rent is worse, not better: one escrow token account per item became the structural answer to class 4, and `PayoutIntent` adds one account per member per redemption. The bond instructions are no longer missing from the design — they are in §The federation side — but they are still designed, not built |
| **T12** | Silent decision reversals: requiring a registered relayer script reversed D6/P8; "genesis is the ordinary bonded path" reversed D3/G2; docs claimed a strictly-heavier commit while the commit compared height; the suite was said to be 17 tests | **Dissolved, with the record widened** | The chainwork claim is now true and the suite is **20** tests. The reversals are real and are now stated rather than silent: **D6/P8** are superseded by the single federation deposit script, **D2/D7** are superseded by doc 23, and **genesis is still open** — members bond `solBSV` and none exists until a mint happens |

---

## The pooled reserve, restated

The old drafts treated "no pooled reserve" as a security property. Under the federation model
that property is **deliberately given up**, and the replacement argument has to be stated
precisely because it is weaker in one direction and stronger in another.

| | |
|---|---|
| **What is gone** | The claim that there is no single key worth stealing. There is now one reserve, and a threshold of members can move it |
| **What replaces it** | No **single** member can move it; misbehaviour by an individual member produces their own signed proof; and the bond must satisfy `bond ≥ k × owed`, so a member cannot leave while owing |
| **What it does not cover** | A **threshold** of members signing something invalid. Every signature is on record, so it is attributable — but it is a governance matter, not a cryptographic one, and doc 23 says so |
| **The honest second gap** | A threshold can also simply **refuse to sign** a legitimate redemption. Silence leaves no signed artifact, so nothing is slashable and no challenger can prove anything. The holder is not robbed — `cancel_redeem` returns the escrow after the deadline — but they are denied exit-to-BSV while the deadline runs. Iterated, that is a soft pause on redemptions, which doc 13 says can never be paused. See §New defect N2 |

**Solana-side, class 4 stands unchanged:** the vault is a rule, not an address. Every escrow
holds one item's tokens, so "balance = sum of items" is vacuous rather than unverifiable.

---

## New defects this re-audit exposes

These are not in V, W or T. They exist **because** of the new model, and the first three are
structural rather than unspecified.

### N1 — The peg-in fee's physical location is unspecified, and `OP_RETURN` still has no amount · **critical, unspecified** · *the live successor to V3 and T3*

The fee is **30 bp, governed** — but the BSV arrived in the federation's threshold script, and
the program mints `net` while crediting `FeeAccount`. Two consequences follow, and neither is
currently resolved:

1. **Precedence.** If paying the deposit script is what triggers minting, then a non-member
   can never receive the fee, and the fee is decided by the federation's *signing policy* over
   a pooled script, not by the program. The program can mint `net` and credit `FeeAccount`,
   but it cannot make the BSV-side 30 bp true.
2. **Repudiation.** A depositor can prove a deposit to the script and be minted `net`. The
   30 bp is a Solana-side accrual against a pool the members already hold. If a member group
   ever pays a depositor out of the reserve outside the redemption path, the accrual and the
   reserve diverge.

T3 identified the shape of this. The model change did not close it; it moved it from "the
relayer declares a fee" to "the federation's signing policy is the fee." **This is the first
thing to specify before any vault code.**

### N2 — Redemption can be stalled by member inaction, and nothing is slashable · **high, structural**

Every escalation in the design assumes a signed artifact: equivocation, invalid
attestation, an unauthorised spend. **Refusal to sign is not an artifact.** A member — or
enough members to fall below the threshold — can decline to attest, and the holder's only
remedies are to wait out the deadline and to take `solBSV` back.

Because `solBSV` is still redeemable in principle and still backed by the reserve, this is not
theft; it is a **denial of the peg-out service**. But doc 13's floor argument rests on
redemptions being the exit, and a persistent stall makes the exit the *token* rather than BSV.
The failure is bounded today only by the fact that members are paid per completed redemption
and by governance ejection. **A timeout-and-rotate rule — a redemption that fails attestation
inside `D_MIN` cannot be attested by the same quorum — is the obvious candidate, and it is
not in the design.** Without it, "redemptions are never pausable" is a statement about the
instruction set, not about the system.

### N3 — `FeeAccount` is a shared pool on the Solana side, which class 4 forbids · **medium, self-inflicted**

Class 4 says no shared token account, and `FeeAccount` is one. It is bounded — it accrues
0.3% of verified deposits and is distributed pro rata — but "bounded" is not the rule the
document set, and every previous draft died of exactly this: writing a rule and then making an
exception to it without saying so. Either the fee is distributed per item at
`release_mint`/`finalize_redeem` (no shared account, member entitlement recorded per item, at
the cost of per-item bookkeeping), or class 4 is restated to say that shared accounts are
permitted where the value is a fee accrual and not escrowed principal. **The document should
pick one and say which.**

### N4 — `PayoutIntent` is one account per member per redemption · **medium, cost**

Attribution is the model's central mechanism, and its cost is now explicit: at `m` members and
`n` concurrent redemptions, attribution costs `m × n` accounts, each paying rent, plus one
instruction per member per redemption. This interacts with T11's rent increase and with
`MAX_USED`-style caps. It is the price of self-proving misbehaviour and it is probably worth
paying — but the design should state the cost and the cap, because the retired per-relayer
counter was an attempt to avoid precisely this and it failed for other reasons.

### N5 — The deposit script is the mint gate, and it is only a script · **high, unspecified**

`verify_deposit` proves an output paid the deposit script; it does not prove that output is
**still unspent**. That was tolerable when each relayer had its own float and a bond. Now the
deposit script is the pool itself: a single threshold script holding the reserve. If the
members sign a reserve transaction that spends a deposit output — a consolidation, a payout,
anything — the deposit remains provable and mintable, and there is no on-chain record that its
BSV has left. The mint is then unbacked.

The fix is to **anchor minting to an unspent output**, which means either a Solana record of
spent outpoints that the federation must maintain, or a rule that reserve spends never touch
unreleased deposit outputs. **Neither is designed.** This is the residual of the old
naked-spend challenge (W3): the challenger was removed with the per-relayer model, and
nothing replaced the invariant it protected. It is the most serious defect in this re-audit
after N1.

### N6 — The bond is denominated in the thing it protects · **medium, structural**

`bond = 1,000 BSV in solBSV`. The bond and the liability are the same asset, which doc 6
argues is right because there is no price margin to defend — and for a per-relayer shortfall
that argument holds. It is weaker for the federation case: a large loss **is** a devaluation of
`solBSV`, so the bond's purchasing power falls at the exact moment it is being seized. If the
reserve is impaired enough to matter, the seized bond is worth less in BSV terms by the same
proportion. This does not make slashing useless — it is the difference between recovery and
full recovery — but the capacity claim ("ten members at 1,000 BSV is roughly $300k") assumes a
price that an incident can move. **The bond answers attribution; it does not fully answer
loss, and the document should not imply that it does.**

---

## Open items

1. **`MAX_STALENESS_SLOTS`, `MATURITY_BLOCKS`, `FLOOR`, `D_MIN`, `C_payout` are unset.**
   `WINDOW − MATURITY` is the detection budget and is only 48 blocks at the proposed
   values — zero at the maximum committed depth, because the clocks coincide.
2. **`N1` — the peg-in fee's physical location** must be specified before vault code.
3. **`N5` — the deposit-script spend problem**: minting is not anchored to an unspent output.
4. **The payout proof.** The destination address must be recoverable from the payout output
   script for `finalize_redeem` to verify it, and the same Merkle machinery must be reused.
   Not designed in detail.
5. **`request_redeem` must require `deadline_slot >= now + D_MIN`** (T6) or a holder can be
   paid and refunded.
6. **Atomicity.** `verify_deposit`'s account creations are still not stated to be atomic and
   there is no test for rollback. Adding `PegInEscrow` and `Marker` makes this three inits.
7. **Rent.** `PegIn`, `PegInEscrow`, `PegOut`, `PegOutEscrow`, `Marker`, `PayoutIntent` per
   member, `Member`, `Bond`, `Proposal` and `FeeAccount` all carry rent. T11's estimate is
   now a floor, not a total.
8. **`owed` / bond accounting on the federation side** — the counter is gone, but
   `bond >= k × owed` still needs a definition of `owed` that does not double-count (T5) and
   does not grow monotonically (W7's second half).
9. **`prune_marker` and `PayoutIntent` closure.** Intents must be closeable after their
   redemption, or attribution rent accumulates forever.
10. **Genesis.** Members bond `solBSV`, and no `solBSV` exists until a mint happens. Doc 23
    leaves this open; this document cannot close it, and the "ordinary bonded path" answer
    from doc 21 is withdrawn as circular.

---

## What this document does not claim

- It does not claim the vault is sound to build. It classifies the history, states the design
  the model implies, and names six new defects, of which **N1 and N5 are unresolved and
  critical**.
- It does not claim the federation exists. **Nothing in §The federation side is built.**
- It does not claim the built code implements the vault. `verify_deposit` today mints gross,
  straight to the depositor, with no escrow, no marker, no maturity and no fee.
- It does not claim the old findings dissolved because the model is better. Each dissolved
  item above names the mechanism, and where the mechanism is a deletion (per-deposit consent,
  the `claimed` latch, `owed_R`, the naked-spend challenger) the cost of deleting it is stated
  in the same row or in §New defects.
