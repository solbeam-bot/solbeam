# 28. State of the project — a summary for review

**What this is.** SOLBEAM issues `solBSV` on Solana, backed 1:1 by BSV held by a bonded
federation, following the architecture of **RenVM** (the bridge that issued wrapped ZEC) and adding
**reorg protection that RenVM does not have**. Peg only: there is no order book and no exchange
mechanism. Eight decimals, MIT, proof-of-concept.

**How to read this document.** It is the honest state: what is **built and verified**, what is
**designed and not built**, what is **deliberately open**, and **where we are weaker than the
reference**. If you are reviewing, the section "What we would attack" is the one worth your time.

---

## 1. What is actually built

| | |
|---|---|
| **Light client** | cw-144 difficulty implemented on-chain and **verified against 324/324 real mainnet headers**; 160 real mainnet headers verified through the `push_header` instruction itself |
| **`solBSV`** | SPL Token, 8 decimals, PDA mint authority |
| **The mint** | Deposit verified by Merkle inclusion; replay prevented by a **nullifier PDA per `(txid, vout)`** |
| **Fork staging** | Chainwork-based, so a branch below the tip can be staged and force a burn |
| **Authority** | Two-step, **timelocked** checkpoint and pause; the immediate setters were removed |
| **Tests** | **34 passing / 0 failing**, including negative controls, on a local validator |

**The instruction set is 14 instructions.** Everything else in this document is **designed, not
built.**

### The three findings that mattered

**F1 — the client could not follow a real chain.** `required_bits()` returned `None` while the
window held fewer than 147 records, and the caller fell back to "the difficulty matches", which it
could not on mainnet. The client **deadlocked at `initialize`**. Fixed with a trusted 147-record
seed, and verified against real headers.

**F2 — a stale re-anchor.** `set_checkpoint` updated the height and hash but left the expected
difficulty, the retargeting flag and the proof-of-work limit stale — so a client initialised on
regtest would accept **everything** at the easiest target. Now re-derived through one shared path.

**F3 — a branch could not be staged.** Branch headers were pinned to the incumbent tip's target, so
no fork below the tip could ever stage, and `burn_staged` was **unreachable**. Now computed at the
header's own height. This required an allocation-free in-place window reader, because the borsh
decode of a 192-entry window costs ~28 KB of a 32 KB per-instruction heap.

---

## 2. What is designed, and NOT built

**Nothing below exists in code. Do not read it as a feature list.**

| | |
|---|---|
| **The vault** | Every mint should land in a program-owned account, released after maturity, and **burned if a reorg is followed**. There is **no `release_mint` and no `burn_staged`** — today `verify_deposit` mints straight to the depositor |
| **The reserve script** | Should be `OP_2 <gateway threshold key> <greycore key> OP_2 OP_CHECKMULTISIG`. `is_p2pkh` is still hard-required and `DepositScript::SPACE` is still 38 bytes |
| **Peg-out** | Entirely unbuilt |
| **The Greycore** | Unbuilt. No account, no instruction, no parameter |
| **Governance** | Unbuilt |
| **Slashing** | Unbuilt |

---

## 3. The contribution

**RenVM trusts its shards to *report* a lock; we *verify* it.**

Under the reference, a mint happens because the Darknodes **witnessed** a lock and signed. The host
chain takes that signature as proof. **The source chain is trusted through the federation** — which
is why challenge-and-prove is load-bearing there, and why its slashing rule is *"produce an SPV proof
or every bond in the shard is slashed."* The federation is the oracle and the bond keeps it honest.

**A Solana program verifying BSV inverts that.** Proof of work is checked against the real difficulty
rule, and the deposit against a Merkle path. A fraudulent mint is not provable-after-the-fact; it is
**rejected at the instruction**.

**And the reorg case is one the reference does not address at all** — not differently, *not at all*.
If a lock is observed and the block is then reorged away, there is no mechanism to notice and none to
reverse it. **Ours notices on-chain and reverses it**, because a reorg is a fact about headers the
program already stores.

**That is the addition, and it is why the light client and the vault exist.**

---

## 4. What we take from the reference

**The federation shape, deliberately, because the precedent is good:** a bonded member set; a
**second quorum of trusted third parties** which must co-sign every reserve movement; epochs and
scheduled membership changes; challenge-and-prove as the slashing frame; and fees — not an oracle —
to hold the economic ratio.

**The Greycore is trusted figures with reputations to lose, NOT node operators.** In the reference's
own words, *"Darknodes that have developed reputations with the community."* A 2-of-2 with the
gateway's threshold key means **neither side can move funds alone**, and the Greycore **polices every
reserve spend**.

---

## 5. What is deliberately open

**These are unaddressed, and saying so is the point.**

**Leaver-shares.** A member who leaves **retains a valid share of the reserve key**, so the effective
threshold **degrades with churn** — at 4-of-`N`, four former members together still hold four valid
shares. Resolving it needs **key rotation** (the reserve moves on-chain; `deposit_script` must change)
or **proactive re-sharing** (needs the departing member's cooperation). **The PoC does not finalise
this.**

**Capacity.** The bond is the **float for transfers**, not capital sized against the reserve. **No
numeric rule has replaced the superseded arithmetic** — the reference's `3×` is its own calculation
for a 100-node shard and does not transfer.

**Threshold.** `4-of-N`, with `N` a variable and the values deferred.

**Backing.** Solana cannot read the BSV UTXO set. **The federation's software reports spent outpoints
and the program checks mints against that record.** This is an accepted oracle, and it is not a new
trust: the federation is already trusted with the reserve. The honest sentence is two sentences —
*"The program verifies deposits. The federation reports backing."*

---

## 6. Where we are weaker than RenVM

**Listed plainly, because a reviewer should not have to find these.**

| | |
|---|---|
| **Sharding** | **None.** Our blast radius is **100%**; the reference isolates to `1/N` |
| **Consensus** | They ran BFT with 100-node shards at a 1/3 threshold (~34 nodes); ours is `4-of-N` |
| **Capacity economics** | Specified there, a float here — **no numeric rule** |
| **Leaver-shares** | **Open**, and it degrades the threshold |
| **Key rotation** | Omitted, and currently **unimplementable** — `deposit_script` is fixed once |
| **Slashing a bad release** | **No enforceable predicate.** A threshold signature reveals nothing about who signed |
| **Backing verification** | An accepted oracle, as above |

**And collusion is unprevented.** A majority of the reserve signers acting **together with** the
Greycore could take the reserve. The 2-of-2 raises the bar; it does not remove the trust.

---

## 7. What we would attack

**If you are reviewing, these are the places we would look first.**

1. **The seed at `initialize`.** 147 records of trusted difficulty history, supplied by the
   authority. If that seed is wrong, everything after it is wrong. **What verifies the seed?**
2. **The reported spent-outpoint record.** The mint checks it, but it is a federation assertion.
   **What stops a false report, and what does a holder actually rely on?**
3. **The 2-of-2.** Does it genuinely make the two quorums independent, given the founders may appoint
   both, and given that a 2-of-2 can be replaced by the upgrade authority?
4. **The upgrade authority.** It is timelocked now, but it is still one key. **What does it control,
   and what is the timelock worth if the holder waits it out?**
5. **`cw-144` itself.** Verified against 324 real headers — **what is the 325th doing?**
6. **The vault.** Designed and unbuilt. **Every reorg-reversal claim rests on code that does not
   exist.**

---

## 8. The honest bottom line

**The light client is real, verified, and survived three audits.** It is the part that took the most
work and it is the part we would stand behind.

**The mint path's real hole is closed** — a deposit could previously be minted after its output had
been spent, and replay was a fragile list with a 200-entry ceiling. Both are fixed and tested.

**The federation half is a specification, not a system**, and it is smaller and more honest than it
was a week ago: the mechanism has a shape, and **the economics that would size it are unresolved and
recorded as such.**

**The vault is the contribution and it is unbuilt.** If you take one thing from this document: **the
reorg reversal is what makes this different from the reference, and it is the part that does not
exist yet.**
