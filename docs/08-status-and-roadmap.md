# 08. Status and roadmap

**This is the reviewer entry point.** It is the honest state: what is **built and verified**, what is
**designed and not built**, what is **deliberately open**, and **where we are weaker than the
reference**. If you are reviewing, **§6 "What we would attack"** is the section worth your time.

**What this is.** SOLBEAM issues `solBSV` on Solana, backed 1:1 by BSV held by a bonded federation,
following the architecture of **RenVM** and adding **reorg protection that RenVM does not have**. Peg
only: no order book, no exchange mechanism. Eight decimals, MIT, proof of concept.

**The distinction this document never crosses.** The single failure this project repeats is a claim
outrunning a mechanism. Every statement below is labelled **built**, **designed**, **trusted** or
**open**, and the labels are not decorative.

---

## 1. What is actually built

Counts verified against `poc/solana/programs/solbeam/src/lib.rs` and the test suite, not copied from a
document.

| | |
|---|---|
| **Light client** | cw-144 difficulty implemented on-chain and **verified against 324/324 real mainnet headers**; 160 real mainnet headers verified through the `push_header` instruction itself |
| **`solBSV`** | SPL Token, 8 decimals, PDA mint authority, no freeze authority |
| **The mint** | Deposit verified by Merkle inclusion; replay prevented by a **nullifier PDA per `(txid, vout)`** |
| **Fork staging** | Chainwork-based, so a branch below the tip can be staged; `commit_fork` re-checks the recorded fork point |
| **Authority** | Two-step, **timelocked** (`TIMELOCK_SLOTS = 32`) propose/execute/cancel. The immediate setters (`set_checkpoint`, `set_paused`) were **removed** |
| **The vault** | Every mint lands in a program-owned account. `release_mint` and `burn_staged` are **permissionless**, so a recipient does not depend on a relayer to release their own tokens. `set_maturity` goes through the timelocked authority path |
| **Tests** | **45 passing / 0 failing**, including negative controls, on a local validator, plus 4 in-program unit tests |

**The instruction set is 17 instructions.** The handlers, in the program module:

```
initialize, push_header, seed_headers                         (light client)
init_staging, push_fork_header, abandon_staging, commit_fork  (fork staging)
propose_authority_change, execute_authority_change,
  cancel_authority_change                                     (authority)
initialize_token, initialize_bridge                           (token / bridge)
verify_deposit, release_mint, burn_staged, prune_nullifier    (mint / vault)
```

**Everything else in this document is designed, not built.**

### The three findings that mattered (F1/F2/F3)

**F1 — the client could not follow a real chain.** `required_bits()` returned `None` while the window
held fewer than 147 records, and the caller fell back to "the difficulty matches", which it could not
on mainnet. The client **deadlocked at `initialize`**. Fixed with a trusted 147-record seed, and
verified against real headers.

**F2 — a stale re-anchor.** `set_checkpoint` updated the height and hash but left the expected
difficulty, the retargeting flag and the proof-of-work limit stale — so a client initialised on
regtest would accept **everything** at the easiest target. Now re-derived through one shared path.

**F3 — a branch could not be staged.** Branch headers were pinned to the incumbent tip's target, so no
fork below the tip could ever stage, and `burn_staged` was **unreachable**. Now computed at the
header's own height. This required an allocation-free in-place window reader, because the borsh decode
of a 192-entry window costs ~28 KB of a 32 KB per-instruction heap.

---

## 2. The vault is built. Its protective window is off by parameter.

This is the single most important caveat in the document, and it is stated rather than left to be
discovered.

**Maturity ships at 0**, as a **stored parameter** (`Config::maturity_blocks`), not a compile-time
constant. At maturity 0:

- the vault is a **pass-through**: tokens minted into it become releasable in the same instant;
- the burn predicate is **satisfied instantly rather than unreachable** — `burn_staged` requires the
  hash to differ *and* the height to have matured, and the second condition is always met;
- so **release and burn are a race**, and in practice **release wins**, because the recipient wants
  their tokens and a reorg is the unusual case.

**Someone who sees a reorg and calls `burn_staged` before anyone releases still burns the tokens.**
Maturity 0 removes the *window* in which the reversal is comfortable, not the reversal itself.

**What protects a deposit today is `MIN_CONFIRMATIONS = 12` — roughly two hours. That is prevention,
not reversal.** The reversal remains **available, tested, and racy** rather than disabled: the burn
path is exercised by test at a non-zero maturity, with a followed reorg, and a later maturity raise
leaves in-flight staged items at the value they were staged with.

**Do not write "reorg-reversible" as a property of the running system.** The honest sentence is: *the
vault is built; the protective window is currently 0 and can be raised.*

---

## 3. What is designed, and NOT built

**Nothing below exists in code. Do not read it as a feature list.**

| | |
|---|---|
| **The federation** — membership, bonds, epochs | **NOT built.** No block, bond, member, threshold signature or governance vote |
| **The Greycore** | **NOT built.** No account, no instruction, no parameter |
| **The reserve script** | **The shape is accepted by code** — `is_acceptable_deposit_script` takes a 25-byte P2PKH or the 71-byte 2-of-2 `OP_CHECKMULTISIG` (`is_reserve_multisig`), with `MAX_SCRIPT_LEN = 71` and `DepositScript::SPACE = 84`, all committed. **The gateway threshold key and the Greycore behind it do not exist**, and no deposit has ever paid one |
| **Peg-out** | **Entirely unbuilt.** No escrow, no intent, no settlement, no cancel |
| **Governance** | **NOT built.** Only the timelocked authority path for the checkpoint/pause and `maturity_blocks` is built |
| **Slashing** | **NOT built.** No `slash_equivocation` instruction |
| **The reported spent-outpoint record** | **NOT built.** Decided (the federation reports; the program checks), format and write path unspecified |
| **The aggregate mint cap** | **NOT built.** Policy, not derived |

---

## 4. What the tests do NOT cover

Recorded because the alternative is a document claiming more than it verified.

| | |
|---|---|
| **`StaleClient`** | The ~54,000-slot staleness bound **cannot be exercised on a local validator** in a test run of minutes. The condition is implemented and **untested** |
| **`AlreadyStaged`** | Untested — a replay is refused earlier, by `AlreadyMinted`, so the second guard never fires on its own |
| **`WrongStagedMint`** | Untested |
| **`DepositHeightNotInWindow` on release and burn** | Untested |
| **The burn bounty** | Not built, and not sized. `fee.bounty_share` is `open` |
| **A real BSV node** | All on-chain verification runs against a local validator and fixtures. F1/F2/F3 are fixed and verified against **real mainnet headers**, but not against a live node in the test suite |
| **A second deployment** | The cross-deployment replay fix (P11) is designed; no test creates a second deployment and asserts rejection |
| **The counterfeit-list counterfactual** | Closed by argument, not by a test that fabricates an account and asserts rejection |

**`burn_staged` IS covered** at a non-zero maturity (see §2).

---

## 5. The honest limitations

These are the things this project has repeatedly got wrong by softening. They are limitations, not
features.

- **The federation is an accepted oracle for spent outpoints.** The mint checks the record, but it is
  a federation assertion. The honest sentence is two sentences: *"The program verifies deposits. The
  federation reports backing."* It is not a new trust — the federation already holds the reserve — but
  it is a trust, and the mint path depends on it.
- **Leaver-shares are open and degrade the threshold.** A departing member retains a valid share, so
  the effective threshold degrades with churn — at 4-of-N, four former members together still hold
  four valid shares. Resolving it needs **key rotation** (the reserve moves on-chain; `deposit_script`
  must change — currently unimplementable because the program fixes it once) or **proactive
  re-sharing** (needs the departing member's cooperation). **The PoC does not finalise this.**
- **There is no numeric capacity rule.** The bond is the **float** for transfers, not capital sized
  against the reserve. **No numeric rule has replaced the superseded arithmetic** — the reference's
  `3×` is its own calculation for a 100-node shard and does not transfer. **A reviewer cannot compute
  our capacity from our documents.**
- **Collusion is unprevented.** A majority of the reserve signers acting **together with** the
  Greycore could take the reserve. The 2-of-2 raises the bar; it does not remove the trust.
- **Sharding: none.** Our blast radius is **100%** of the reserve; the reference isolates to `1/N`.
- **The vault's protective window is off** (§2). `MIN_CONFIRMATIONS = 12` is what protects a deposit.
- **No independent audit.** The critical defects found so far were found by our own adversarial
  review, which is not the same as an audit by someone with no stake in the answer.

**Where we are weaker than RenVM, listed plainly** (full comparison in
[03. The federation](03-the-federation.md#9-where-we-are-weaker-than-renvm)):

| | |
|---|---|
| **Sharding** | **None.** Blast radius **100%**; the reference isolates to `1/N` |
| **Consensus** | They ran BFT with 100-node shards at a 1/3 threshold (~34 nodes); ours is `4-of-N` |
| **Capacity economics** | Specified there, a float here — **no numeric rule** |
| **Leaver-shares** | **Open**, and it degrades the threshold |
| **Key rotation** | Omitted, and currently **unimplementable** |
| **Slashing a bad release** | **No enforceable predicate.** A threshold signature reveals nothing about who signed |
| **Backing verification** | An accepted oracle, as above |

---

## 6. What we would attack

**If you are reviewing, these are the places we would look first.**

1. **The seed at `initialize`.** A trusted 147-record difficulty history, supplied by the authority.
   If that seed is wrong, everything after it is wrong. **What verifies the seed?**
2. **The reported spent-outpoint record.** The mint checks it, but it is a federation assertion.
   **What stops a false report, and what does a holder actually rely on?**
3. **The 2-of-2's independence.** Does it genuinely make the two quorums independent, given the
   founders may appoint both, and given that a 2-of-2 can be replaced by the upgrade authority?
4. **The upgrade authority.** It is timelocked now (`TIMELOCK_SLOTS = 32`), but it is still one key.
   **What does it control, and what is the timelock worth if the holder waits it out?**
5. **`cw-144` itself.** Verified against 324 real headers — **what is the 325th doing?**
6. **The vault's window being off.** The reversal mechanism exists, but at maturity 0 it is a race
   that release normally wins. **Every "reorg-reversible" claim rests on a parameter, not on shipped
   behaviour.**

---

## 7. The contribution

**RenVM trusts its shards to *report* a lock; we *verify* it.**

Under the reference, a mint happens because the Darknodes **witnessed** a lock and signed. The host
chain takes that signature as proof. **The source chain is trusted through the federation** — which is
why challenge-and-prove is load-bearing there, and why its slashing rule is *"produce an SPV proof or
every bond in the shard is slashed."* The federation is the oracle and the bond keeps it honest.

**A Solana program verifying BSV inverts that.** Proof of work is checked against the real difficulty
rule, and the deposit against a Merkle path. A fraudulent mint is not provable-after-the-fact; it is
**rejected at the instruction**.

**And the reorg case is one the reference does not address at all** — not differently, *not at all*.
If a lock is observed and the block is then reorged away, there is no mechanism to notice and none to
reverse it. **Ours notices on-chain and reverses it**, because a reorg is a fact about headers the
program already stores.

**That is the addition, and it is why the light client and the vault exist.** The detection half is
built and verified; the reversal half is built, and **its safety window is off by parameter.**

**What we take from the reference, deliberately, because the precedent is good:** a bonded member set;
a **second quorum of trusted third parties** (the Greycore) which must co-sign every reserve spend; a
challenge-and-prove framing; and fees — not an oracle — to hold the economic ratio.

---

## 8. The honest bottom line

**The light client is real, verified, and survived three audits.** It is the part that took the most
work, and it is the part we would stand behind.

**The mint path's real hole is closed** — a deposit could previously be minted after its output had
been spent, and replay was a fragile list with a 200-entry ceiling. The list is now a nullifier PDA.
The spent-outpoint hole is closed *in principle* by the reported record, **which is not built**.

**The federation half is a specification, not a system**, and it is smaller and more honest than it was
a week ago: the mechanism has a shape, and **the economics that would size it are unresolved and
recorded as such.**

**The vault is the contribution and it is built.** If you take one thing from this document: **the
reorg reversal is what makes this different from the reference — the mechanism exists and is tested,
and at the shipped maturity of 0 it is a race rather than a window.**

---

## 9. Roadmap

The order is deliberate: **the built system now follows a real chain**, then the component the rest of
the design rests on, then the federation and the parts that need bonds and adjudication.

### Closed — fixed in code and verified

| | What closed it |
|---|---|
| **F1/F2/F3 — the light client could not follow a real chain** | Seed the difficulty bootstrap and verify the instruction path: **160 real mainnet headers through `push_header`** and a real mainnet branch through `push_fork_header` |
| **F4's initialiser vulnerability** | `initialize` and `initialize_bridge` require the program's **upgrade authority**, and checkpoint/pause changes go through a **timelocked** propose/execute/cancel path. The authority *threshold* residual is specified, not built |
| **The difficulty retarget** | **cw-144**, from the SV Node's `src/pow.cpp`, replayed against real mainnet headers: **324/324 predicted exactly** |
| **The fork re-anchor (P2/A11)** | `init_staging` records `fork_parent_hash`; `commit_fork` requires the chain still to hold it and fails `ForkPointMoved` otherwise. Commit compares accumulated chainwork |
| **A7 — double-minting one deposit** | Deposit identity is `(txid, vout)`, and the **nullifier PDA** replaces the fixed replay list |
| **The replay-list ceiling** | The nullifier removes the 200-per-window cap, the pruning logic and the accidental capacity limit at once |
| **The window resize** | **192 records of 52 bytes**, `SPACE` **10,107** of 10,240, deposit lifetime **32 hours**. The old 288 × 32-byte, 48-hour layout was arithmetically impossible once cw-144 was implemented |
| **The vault** | `release_mint`, `burn_staged` and `set_maturity` are built; the burn path is tested at a non-zero maturity |

**Testnet is no longer blocked on the retarget.** What remains open is **X3**: the DAA rule is
hard-coded and BSV says it will change, so it must become changeable without a redeploy — which the
federation model turns into a **governance parameter** rather than an orphaned risk.

### Open — remaining work

| Step | What ships | Why this order |
|---|---|---|
| **Raise the vault's maturity from 0** | A non-zero `maturity_blocks`, through the timelocked authority path that already exists | The mechanism is built; the protective window is a parameter. Until it is non-zero, the reversal is a race |
| **Transparency — reserve and supply published continuously** | The reserve balance and the `solBSV` supply published as a live, public backing ratio | **Promoted from a phase-5 monitoring task to an early deliverable.** For the two cases nothing can enforce — a colluding threshold, and an unspent-outpoint spend nobody challenges — **visibility is the only remaining defence**, and it must exist before real value does |
| **The reported spent-outpoint record** | The format and the write path, then the check in the mint | Closes N5 in practice. Until then the mint cannot tell a spent deposit output from an unspent one |
| **The federation** | Greycore-admitted membership, two-sided bonds (the float), members running software, and the reserve under a **2-of-2 `OP_CHECKMULTISIG`** | Turns individually-trusted keys into a threshold over the reserve **with a Greycore co-signature**, and makes the parties with the most to lose the ones who watch |
| **Governance** | 85% / 30 days / live signal, holding the upgrade authority; redemptions never pausable; pause stops mints only | Gives the system a change mechanism slower than its exit |
| **Slashing** | **Self-proving equivocation** — members sign payout intents individually | The proof is on-chain and needs no judgement |
| **Peg-out** | Escrow into the vault, individually-signed payout intents, the threshold signature plus Greycore co-signature, settlement proved against the light client, and a permissionless cancel | Turns a one-way wrapper into a two-way peg. Failure returns; it never mints |
| **The node and user software** | The member's node, and the surfaces that submit headers and resolve pending items | **No consensus role.** Last because it exposes a system rather than completing one |

**Named as open, and not dressed up:** the vault's *design* failed two audits before the current build;
**sharding** is undecided; **leaver-shares** are unfinalised; **the DAA is hard-coded**; and there is
**no numeric capacity rule**.

### Launch sequencing

1. **Testnet, mint only.** No longer blocked on the retarget; no real value.
2. **Transparency, alongside the testnet.** Publish the reserve balance and the supply continuously,
   so the backing ratio is public **before** any real value is at stake.
3. **Testnet, vault.** Raise maturity, so mints stage, mature and release.
4. **Testnet, federation and governance.** Bonds, threshold custody, governance and slashing exercised
   end to end. Still no real value.
5. **Mainnet, mint only, small caps.** Deposits proven end to end. Redemption handled manually and
   transparently while peg-out is finished.
6. **Mainnet, peg-out live, small caps.** Escrow, payout intents, the threshold signature and the
   cancel path exercised in production.
7. **Raise caps.** The bond is the **float**, not a capacity ceiling, so scaling means growing the
   float, adding members (admitted by the Greycore) and enlarging the Greycore — and only after audits
   and after the invariants hold in production for a sustained period.

### Milestones

- The light client verifies mainnet BSV headers, **including the per-block difficulty**, against
  independent reference data. *(Built: cw-144, 324/324.)*
- A deposit is minted on mainnet with no human in the loop: staged in the vault, then released after a
  non-zero maturity.
- A reorg followed inside the maturity window burns the staged tokens, and the depositor ends exactly
  where they started. *(The burn is tested at a non-zero maturity; the shipped maturity is 0.)*
- A federation is formed from **Greycore-admitted members** at the two-sided float, and holds the
  reserve under a **2-of-2 `OP_CHECKMULTISIG`**.
- A governance proposal passes at **85% of pledged coins** and takes effect after **30 days**, with
  redemptions live throughout.
- A redemption completes end-to-end: escrow → signed payout intents → BSV payout → proof → settlement.
- A failed redemption returns the escrow: supply unchanged, and the bond **not** additionally
  transferred.
- A member that signs two conflicting payout intents is slashed by anyone submitting its own two
  signatures.

### Phase 2 — beyond the proof of concept

**Delegated staking.** Lets non-members delegate `solBSV` to a federation member and share its fee,
growing the bond base without new operators. **Activatable by governance.** Deliberately deferred: a
staking layer has its own incentive problems, and it should not be designed until the thing it is
meant to scale has been shown to work.

**Miner attestations.** A proposal in the BSV community would let a miner attest that a transaction
will be mined and not double-spent, and node software would reject competing blocks. It would shrink
the reorg window we must defend against; it would **not** remove the need to defend it (a minority
reorg below the attestation threshold remains possible), so the vault is still required. A Solana
program could verify such an attestation **without a new oracle**, because a miner's weight is
derivable from the headers we already store — coinbase inclusion proofs plus a weight accumulator. It
is **not buildable today** because the attestations do not exist to verify. **Nothing in this design
may assume miner attestation exists.**

Latency remains a real product problem: 12 confirmations is ~2 hours, against a market expectation of
seconds. The vault is how the two decouple — *visible immediately, unlocked at maturity* — and that
decoupling is available now.
