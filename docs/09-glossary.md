# 9. Glossary

Plain-English definitions of the terms used in this documentation.

> **Specification, not shipped behaviour.** The light client with cw-144, the `solBSV` token, the
> mint and fork staging are **built and tested**. **The vault, the bonded federation, threshold
> custody, governance, slashing and all of peg-out are designed and not built.** The entries for
> those are the vocabulary of the design, not a description of running code. A closing section
> marks the terms of earlier models that the design no longer uses.
> [`13-summary.md`](13-summary.md) is authoritative.

**Atomic (in "atomic wrapper")** — here it means the wrapper is 1:1 and minting is authorised purely by a proof: the deposit either verifies on Solana or it doesn't, with no discretionary approval. It does **not** mean redemption is instantaneous.

**Bond** — the collateral a **federation member** lodges on Solana to join: **1,000 BSV, posted as `solBSV`**, so it is seizable on-chain by the program. **`bond ≥ k × owed` must always hold**, with `k ≥ 1`, which is what stops a member leaving while it still owes — a bond withdrawable on demand is not a bond. Because `k = 1`, total value locked is capped by total bonds pledged: ten members at 1,000 BSV is roughly **$300k** of capacity, and that is the proof of concept's scale limit. Fees are earned **pro rata to stake**.

**Burn** — destroying `solBSV`. A reorg that is followed causes the **vault** to burn tokens that were minted but never released; a settled redemption burns the escrow. Supply on a failed path is unchanged.

**Challenger** — the party that proves misbehaviour on-chain. Under the federation model the **node software is the challenger**: watching both chains is a funded job done by the members with the most to lose, not an unpaid chore. **Anyone** may still submit self-proving evidence and be paid the bounty.

**Checkpoint (light client)** — a recent, deeply buried BSV block header that the on-chain light client treats as its starting point, so it does not have to store the entire header chain. The chainwork baseline is checkpoint-relative and re-anchored by `set_checkpoint`.

**DAA (Difficulty Adjustment Algorithm)** — BSV's rule for adjusting mining difficulty, recalculated **every block** over a 144-block window. The light client verifies it per block: the rule is **cw-144**, taken from the SV Node's `src/pow.cpp` (median-of-three "suitable blocks" 144 apart, the work difference, the 72×/288× time clamps, `(-work)/work`), and it is verified against real mainnet headers at **324/324 exact**. The client must verify it or a fake header could be accepted, and it stores a hash, cumulative chainwork and timestamp per header (52 bytes) to do so — which is what fixed the window at 192 records. **Hard-coded, and that is an open item:** BSV says the rule will change, so it must become governable without a redeploy (X3; the federation model makes it a governance parameter).

**Equivocation** — a member signing **two conflicting payout intents**. It is the design's answer to attribution: because each member signs individually and the intents are recorded on Solana, the two signatures are the entire proof. See *slashing*. (Copied from the half of RenVM's slasher that actually shipped.)

**Exit window** — the **floor of the system**, and deliberately not a constitution. Because redemptions **can never be paused**, a governance change takes **30 days** with **live signal from the moment it is raised**, so a proposal that would harm holders empties the bridge before it lands. The residual, stated plainly: a holder who does not watch and does not act within 30 days is exposed.

**Federation member** — **an operator running software**, not a person exercising judgement. Anyone with a **1,000 BSV bond** may join; there is no manual approval of any transaction. Each node watches both chains, verifies independently with its own light client, signs payout intents individually, and challenges theft automatically. Holders of the reserve do so under a **threshold key**. Closest analogue: running a staked validator.

**FLOOR** — the **minimum deposit confirmation depth**, **12 BSV blocks**. It is the depth a deposit waits: depth is not a term of a bid, because there is no book. `FLOOR` is a **governable parameter** (85% of pledged coins, 30 days), which is why it can be raised against a changing hash rate without a program redeploy. See *maturity*.

**Governance delay** — the **30 days** between a proposal passing at **85% of pledged coins** and taking effect. Its signal is **live from the moment the proposal is raised**, not only when it passes; the delay is what makes the signal useful. All the numbers are parameters with defaults, not constants.

**Light client** — a program that verifies a chain's headers and transaction inclusion without downloading the whole chain. SOLBEAM runs a BSV light client **on Solana**, holding a checkpoint and a rolling window of **192 records** (52 bytes each, `SPACE` **10,107** of 10,240) that carry a hash, cumulative chainwork and time per header.

**Maturity** — the **144 blocks a staged mint must stay unreorged before the vault releases it** to the depositor. Depth and maturity do different jobs: **depth sets the cost of attacking**, while **maturity sets the time available to detect** one. If the program's own stored hash for the deposit's height no longer matches, the staged tokens are burned instead of released. See *vault*.

**Merkle proof** — a short cryptographic path showing that a transaction is included in a block, without needing all the block's transactions.

**Mint** — creating `solBSV` on Solana against a proven BSV deposit. Under the design the new tokens land in the **vault**, not in the depositor's wallet.

**OP_RETURN** — a BSV output that can carry arbitrary data. A SOLBEAM deposit attaches `version ‖ cluster_id ‖ program_hash ‖ flags ‖ recipient` so the program knows which cluster and which Solana address the deposit is for.

**Pause** — an emergency stop for **new mints only**. **Redemptions continue, always**, and the pause lifts automatically after N days unless renewed. Pausing inbound is a safety valve; pausing outbound is taking hostages, so the two are deliberately not bundled. Because the power is bounded it can carry a lower threshold (a simple majority of pledged coins) than a governance change.

**Payout intent** — a federation member's **individual signature**, cast on Solana, approving a specific BSV payout for a redemption. Intents are recorded, so every approval is **attributed** to a member. Once enough attributed intents exist, the **threshold key** produces the BSV payout. Because members sign individually rather than as one opaque group, a member that signs two conflicting intents has produced its own proof of guilt — see *equivocation*.

**Peg-in / peg-out** — moving value into the wrapper (BSV → `solBSV`) and back out (`solBSV` → BSV). Peg only: there is no exchange mechanism, no order book and no leverage.

**Pledged coins** — the `solBSV` members have pledged as bonds. Governance thresholds are counted against these: **85% of pledged coins** to pass, and a simple majority for a bounded pause.

**Proof-of-reserves** — a published, independently checkable statement that the BSV held by the peg matches the `solBSV` supply. **The model does not specify one:** the reserve is native BSV under a threshold key and the Solana program cannot read it.

**Reserve** — the native BSV backing `solBSV`, held by the federation under a **threshold key**. It is **trusted, and bounded**: no single member can move it, and what protects a holder is a bond that anyone can seize by proving misbehaviour on-chain.

**SIGHASH_FORKID** — the signature-hash scheme BSV requires for transaction signing. SOLBEAM implements it directly.

**Slashing** — seizing a member's bond for **self-proving misbehaviour**: signing two conflicting payout intents (equivocation). It is **not** the settlement for a failed redemption — there the escrow is returned to the holder, supply is unchanged, and the bond is **not** additionally transferred, because the returned escrow already makes the holder whole. It also does **not** cover "an intent matching no authorised redemption": that predicate is undecidable (a closed `PegOut` looks like one that never existed) and would false-positive against an honest member who attested before a cancel (audit F8). A threshold of members signing something invalid is attributable from the record but is a **governance matter, not a cryptographic one**.

**solBSV** — the wrapped BSV token on Solana: a classic SPL token, 8 decimals, no freeze authority.

**SPV (Simplified Payment Verification)** — verifying that a transaction is in a chain by checking block headers and Merkle proofs, rather than validating the whole chain yourself.

**Threshold key** — the key over the reserve, which requires a **threshold of federation members** to sign. No single member can move the funds. **The threshold value and whether the key is sharded into several groups are undecided** — sharding contains theft and signing latency at the cost of coordination.

**Trustless** — correct without trusting any participant: verification rests on cryptography and on-chain code. **Minting and reversal qualify.** The reserve does not: it is trusted, and bounded by bonds and proofs.

**Trust-minimised** — a small, bounded trust assumption remains, but it is enforced economically (bonds, slashing) and by on-chain proofs rather than by promises.

**Unbonding period** — the notice a member must serve before its bond is released. Its length is an **undecided parameter** (doc 23 lists it with no default). Without it a member could take on obligations, withdraw its bond, and be gone before anyone could slash; `bond ≥ k × owed` must still hold at withdrawal.

**Upgrade authority** — the program's upgrade key, which can in principle change anything. Under the federation model it is **held by governance** — 85% of pledged coins and a 30-day delay with live signal — rather than being an unowned gap. It cannot pause redemptions.

**Vault** — a **program-owned token account that every mint lands in first, never with the user**. Because the tokens sit in the program's *own* account, the program can **burn them if a reorg is followed** or release them on maturity, all without a freeze authority and without Token-2022 hooks. It is also why a staged token is not liquid, so there is no window in which a fraudulent mint can be sold to an innocent buyer. Peg-out escrows into the same vault before the escrow is burned or returned. Designed, not built.

---

## Terms of earlier models — removed or superseded

These are kept only so that older documents can be read. **None of them is part of the current design.**

- **Order book / "the book"** — **removed.** Stakers posted liquidity and the book matched by price then time. The fee is now a **governed 30 bp**, and capacity is capped by bonds pledged, which solves both jobs more simply.
- **Discovered fee** — **removed** with the book. The fee is a governed parameter.
- **Bonded relayer** — **superseded** by *federation member*. There are no per-relayer independent keys and no per-relayer deposit scripts; the reserve is under one threshold key.
- **`owed_R` / exposure / relayer consent** — **superseded.** The per-relayer liability accounting was the old model's way of bonding individual deposits. The federation bond secures `owed` at the member level.
- **Hot wallet / naked spend** — **superseded.** There is no hot float and no pooled reserve; the earlier analysis that a hot float is a written option is the reasoning that led to bonding in `solBSV`, and it is historical.
- **The two gates** — **superseded** by the vault's own release-and-burn rule: the program decides from its own stored headers, with no separate gate parameters to enforce.
- **Cold reserve / covenant, tranche, veto-only cosigner** — **superseded.** There is no pooled, covenant-locked reserve and no tranche schedule.

---

Next: [Brand & visual language](10-brand.md)
