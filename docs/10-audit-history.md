# 10. Audit history

**How the design got here, and the honest record of what was wrong.** This document is the audit
trail: the adversarial passes, the findings, the drafts that failed, and — the most informative thing
in the repository — **the errors this project made and corrected.**

It is history. It does not describe the current model; where a finding has a current status, that
status is in [05. Trust model](05-trust-model.md) or [08. Status and roadmap](08-status-and-roadmap.md)
and is repeated here only in summary. Older source documents are deliberately gone: a reader must not
find two versions.

---

## 1. The five errors this project made and corrected

**These matter more than the findings, because they are the pattern repeated.**

### 1. The 2016-blocks premise

The client halted at the first difficulty change (`push_header` required `bits == expected_bits`, set
once at `initialize`). A supposed fix stored a **difficulty-period anchor**, on the premise — stated
in the design without being measured — that *"BSV retargets every 2016 blocks."* It does not. **BSV
adjusts difficulty every block.** The correct rule is `cw-144`, taken from the SV Node's
`src/pow.cpp`, and it was verified only after the premise was discarded: **324/324 real mainnet
headers predicted exactly**, and 147 records of lookback reproduce the result while 146 reproduces
0/324.

**The lesson:** a retrofit built on an unmeasured premise about consensus is worse than no fix,
because it looks like one.

### 2. The rent constant

Solana account rent was taken as **6,960 lamports per byte** — an older toolchain's constant — when
the measured rate is **5,080** (`solana rent 0` = 650,240 = 128 × 5,080). The wrong constant
**overstated every rent figure in the cost document by about 37%**, and it was wrong **twice in the
same direction**, both times by substituting a recalled constant for a measured one. The correct
formula is `(bytes + 128) × 5,080`, with the 128-byte overhead already inside the rate.

**The lesson:** measure the constant, or say it is unmeasured.

### 3. The "threshold script" wording

When audit F10 was first raised — that the deposit script was hard-required to be P2PKH and so could
not be the "threshold script" the documents described — the correction was that the federation uses
**threshold ECDSA**, the reserve address is an ordinary P2PKH address, and therefore `is_p2pkh` and
`DepositScript::SPACE = 38` were **correct**, and the *word* "threshold script" was banned from the
copy.

**Then the Greycore was adopted as a 2-of-2 co-signer on the reserve script, and the reversal
happened:** the deposit script genuinely **is** a 2-of-2 `OP_CHECKMULTISIG`, F10 is reversed, and the
committed code now accepts it (`is_reserve_multisig`, `MAX_SCRIPT_LEN = 71`, `DepositScript::SPACE =
84`). **The wording ban was itself the error** — it had assumed a reserve with no second quorum.

**The lesson:** a wording rule is a design claim in disguise, and it ages the moment the design moves.

### 4. The capacity multiple

Two related errors, both recorded because the arithmetic was already wrong twice:

- **Correction 1.** The bond was first sized against the whole reserve, and a symmetric two-sided bond
  gave `H ≤ 0` — **zero capacity.** That was an impossible inequality, not a conservative one.
- **Correction 2.** The reference's **3×** was then adopted while running a `3-of-5` threshold, where
  the formula would give `5/3 ≈ 1.67×`. Quoting 3× **overstated capacity by 1.8×**, and `fed.k = 1`
  was then `1.67×` short of a rule that did not apply.

**Both are withdrawn**, along with the `~$180k` and `$300k` capacity figures. The `n/t` multiple is
RenVM's own bribery-cost calculation for a 100-node shard at a 1/3 threshold; **it does not transfer
to a bond that is the float.** The honest consequence: **no numeric capacity rule has replaced it**,
and a reviewer cannot compute our capacity from our documents.

**The lesson:** borrowing another system's parameter requires checking that its derivation applies.

### 5. The rotation premise

An earlier revision claimed that rotation was unnecessary because the threshold already handled a
member who had left with shares. **It does not.** A departing member **retains a valid share**, so at
`4-of-N` **four leavers hold four valid shares** — the effective threshold degrades with churn, and
the guarantee is a property of the current member set only in name. Rotation exists precisely to
bound this, and it was omitted for an implementation reason (`deposit_script` is fixed once in
`initialize_bridge`) that is a **cost, not a justification**. It is now recorded as an **open
finalisation item**, alongside proactive re-sharing, which is not specified anywhere.

**The lesson:** "the threshold handles it" is a claim about cryptography; it needs the cryptography.

### And the pattern underneath all five

**A claim outrunning a mechanism.** The project has repeatedly written down a property the code did
not have. Two instances are worth naming separately because they were found late:

- **Verifying a function and claiming a system.** The `324/324` result validated `difficulty.rs` as a
  pure function while the *instruction path* could not advance past the checkpoint at all. "The client
  follows a real chain" was false as a system claim (F1).
- **Describing a design and claiming a build.** Documents described a vault, releases and burns that
  did not exist, then — after the vault landed — kept calling the vault unbuilt. Both directions of
  drift happened. The current discipline is that every document carries the built/designed label, and
  the counts are read from the code.

---

## 2. The audit passes

Four adversarial passes ran over roughly a week of design movement. Their findings are current-status
in [05. Trust model](05-trust-model.md); what follows is what each pass was and what it found.

### Pass 1 — "who loses funds" (the mechanism audit)

A focused pass organised around four questions: who loses in a **natural** reorg, who loses to a
**reorg attacker**, who can **extract funds illicitly**, and who can be **damaged without being
robbed**.

**Its findings, preserved:**

| ID | Finding |
|---|---|
| **F1** | Burning a staged mint would not release its replay entry: pruning was by height, and a reorg re-mines the transaction at a new height, so the depositor could never mint again. Needed: burning must remove the replay entry |
| **F2** | The unbacked path had no backstop: with no underwriter, the vault and detection were the *entire* defence |
| **F3** | **Detection had no reward.** Pushing headers was permissionless and cheap but the incentive was indirect and diffuse, and there was no bounty for detecting a reorg — the case where detection is the whole defence |
| **F4** | The bond covered only the liability (`owed_R`), not a relayer's own float, and staged mints had to count toward `owed_R` or that window was unbonded |
| **F5** | At `k = 1` and no detection, self-dealing profited: "roughly break-even" held *only if the shortfall was detected and the bond slashed* |
| **F6** | `MAX_USED = 200` was a **cheap, repeatable shutdown** and a hard throughput ceiling: 200 dust deposits filled the list and every peg-in failed for the rest of the window — with no attacker, the protocol processed at most 200 peg-ins per window |
| **F7** | **The client could not follow a difficulty retarget**, so it halted permanently — and this was a consequence of the A1 fix, which made the check actually bind |
| **X1** | "The vault does not exist" — correct at the time, which reclassified F1 as *unimplemented* rather than *inherent* |
| **X2** | "Unlimited double-mint via a counterfeit replay list" — **incorrect.** Only the owning program may write an account's data, and the PDA was already bound by owner and type. It was pinned anyway, because the reasoning is subtle and a future instruction creating a second list would make it real |
| **X3** | **BSV changes its difficulty algorithm.** The client hard-codes `cw-144`, and BSV's own documentation says it will revert to 2016-block retargeting. If it does, the client rejects every header from the change point and the bridge halts — a liveness failure caused by a third party. Mitigations: version the rule on-chain; treat it as upgradeable; monitor and warn |

**Also claimed and incorrect:** "a miner can re-mine a signed deposit with a different `OP_RETURN`,
redirecting the mint to themselves." The depositor's own signature covers the `OP_RETURN` output under
`SIGHASH_ALL`, so altering it invalidates the input's signature. Not exploitable unless a depositor
signs with `SIGHASH_NONE`, which no wallet does.

**Recorded because a wrong critical is worse than no critical:** both of the independent pass's
criticals were checked, and one did not survive.

**C3, a residual.** `initialize` accepted any header meeting *its own* declared `bits`, so the
checkpoint's difficulty was unchecked and self-declared. Combined with the deploy-time race, whoever
called `initialize` first chose both the trusted root and its difficulty. The checkpoint being trusted
is inherent; the race is not.

### Pass 2 — the pre-code critical list

Consolidated from three passes into items organised by **when they had to be dealt with**. Its status
at the time, and what happened:

| Item | What it was | Outcome |
|---|---|---|
| **P1** | The checkpoint race and self-declared difficulty | **Moot for launch under governance** — `initialize` is a governed launch action, not a public race; the code is unchanged |
| **P2** | `commit_fork` did not re-anchor the staged branch; an intervening commit could splice the window from two chains | **Fixed** — `fork_parent_hash` recorded at `init_staging`, re-checked at `commit_fork` (`ForkPointMoved`); commit compares chainwork, not height |
| **P3** | Deposits have a hard **32-hour life**, with no on-chain refund path | **Decided — 32 h**, disclosed, app automates the mint. The refund path is still absent; BSV has no timelocks (`OP_CLTV`/`OP_CSV` are no-ops), so a refund is a rule, not a guarantee |
| **P4** | The retarget halted the client (F7) | **Fixed** — cw-144 per block, verified 324/324. **X3 remains** |
| **P5** | The replay-list shutdown and hard ceiling | **Decided and built** — a **nullifier PDA per minted deposit** removes the list, the ceiling and the pruning logic at once |
| **P6** | A burn must release the replay entry (F1) | **Closed by the nullifier decision** — burning can close the PDA |
| **P7** | Detection had no reward (F3) | **Moot under the federation** — detection is a member job, funded from fees and bounties |
| **P8** | The unbacked path had no backstop (F2) | **Rebased** — there is no per-deposit underwriter; the question becomes an exposure bound on the reserve before the bond set is large enough |
| **P9** | Self-dealing profited when detection failed (F5, `k = 1`) | **Moot for per-relayer self-dealing**; the analogous case is a colluding threshold, answered by the exit window, not a bigger bond |
| **P10** | The bond had to cover staged mints (F4) | **Carried** into the definition of exposure; the per-member attribution rule is still to design |
| **P11** | **Cross-deployment replay** — nothing in a deposit committed to *which* Solana deployment it was for, so the same BSV could back `solBSV` on two deployments | **Closed by design, not built** — the payload becomes `version ‖ cluster_id ‖ program_hash ‖ flags ‖ recipient`. It had to be in the *deposit*, which the depositor writes, so adding it after launch would leave earlier deposits replayable forever |

**The first-time ATA rent** was flagged here as a decision needed: it is the largest per-user cost in
the system, recoverable only by closing the account. **Recommended: submitter pays, and say so.**

**Audit gaps named at the time, and still gaps:** the vault's instruction set had never been audited;
the gateway threshold key and Greycore had no key generation or signing protocol; **no test created a
second deployment** to assert rejection; **no test created a counterfeit account** to assert rejection
(the X2 closure is by argument, not evidence); and `verify_deposit`'s parsing had no adversarial pass
on malformed transactions, unusual output counts or `OP_PUSHDATA` handling.

### Pass 3 — the federation audit

**Verdict at the time: not sound to build.** The blocking set was **F1, F3, F4, F5, F7, F8 — and it
was not the set the vault audit named.**

**The single most important result was F1**: the light client, believed built and verified, **could
not run on a real BSV chain.** That is the "verifying a function and claiming a system" error, and it
is the most valuable finding in the repository.

The findings, their fixes, and the two adjudications (F10 reversed; N5 confirmed as not creating
unbacked supply) are in [05. Trust model §The audit findings](05-trust-model.md#the-audit-findings-and-their-status).
The parts that are **narrative** and belong here:

- **F8 was the case of four writers against one.** The "intent matching no authorised redemption" row
  was provably undecidable, and four documents already said so; a single commit had added the row.
  The fix was to delete the row, and an earlier pass deleted the *wrong* row (collusion) and left this
  one standing. **The two rows were different findings.**
- **The minimum blocking set was re-based** as fixes landed: F1 fixed and verified (160 real mainnet
  headers); F3 fixed (a real mainnet branch through `push_fork_header`); F2 fixed (and the instruction
  later removed); F4's initialiser vulnerability fixed, the authority residual specified not built;
  F5/F9's formula superseded by two-sided bonds; F7 given a floor; F8's row deleted; **F10 reversed**;
  genesis decided as a BSV-side bond.
- **N1, N3 and N5 needed restatement, not code.** That correction — that the vault audit's blocking
  list was aimed at the wrong component — is itself part of the record.

### Pass 4 — the vault drafts

Three vault designs, each audited and found broken, before the current build. **Recorded in full
because the pattern is the useful part: most findings were the design failing to enforce BSV-side
behaviour that no longer needs enforcing from Solana.**

**Draft 1 — the first vault design. Four criticals, three inherent to the design as described:**

| ID | Defect |
|---|---|
| **V1** | **The nullifier *was* the pending item, and release closed it** — one deposit minted repeatedly. The author's own error; it defeated replay protection entirely |
| **V2** | `cancel_redeem` raced `settle_redeem`, and `D = 6 h < W = 24 h` made it **certain** rather than unlucky |
| **V3** | The peg-in fee was never deducted, so every release over-drew a commingled vault |
| **V4** | `settle_redeem` bound neither amount nor destination — pay 1 sat, settle 1,000 tokens |
| **V5** | The out-of-window release row meant the real detection budget was `MATURITY`, not the window, and a Solana halt switched every staged mint to unchecked release |
| **V6** | `MATURITY` stored as a Solana **slot** while canonicality is a BSV **height** |
| **V7** | `PendingMint` had no relayer field, so `owed_R` could not be attributed |
| **V8** | **No slash instruction existed anywhere** |
| **V9** | The domain separator's encoding was undefined; a substring test could mint on two deployments |
| **V10** | Rent unclaimable, bond custody undefined, permissionless resolution could burn a good mint during a transient fork, `index_of` conflated "too old" with "too new" |

**Draft 2 — the revision. The fixes did not hold: four were inert or reintroduced the defect.**

| ID | Defect |
|---|---|
| **W1** | **The prune was a replay oracle**: the nullifier stored nothing, so `prune(nullifier, fabricated_height)` removed it while the deposit was still in the window. The fix was one field: store `deposit_height` and check it |
| **W2** | V5 was **inert in the branch it was written for** — with the height out of the window, "the tip has advanced" was automatically true. The real defect was never "did the tip advance" but "is the client's view current": the right fix is **staleness**, not advancement |
| **W3** | `slash` had **no on-chain predicate**: "proves the relayer spent BSV it held against outstanding obligations" is not program-checkable |
| **W4** | **R1's no-bounty argument was unsound**: the attacker is the most motivated pusher, and a light client cannot out-mine them. `commit_fork` also compared height, not chainwork |
| **W5** | Payout proofs were **replayable**: one payment settled every redemption with the same amount and destination |
| **W6** | `claimed` was a **one-way latch with no resolver**; a reorged payout froze the escrow forever |
| **W7** | **Consent destroyed the permissionless remedy**: requiring a per-deposit relayer signature meant a depositor could not mint without the relayer. Nothing decremented `owed`, so it grew monotonically and `withdraw_bond` became unsatisfiable |
| **W8–W11** | The vault-invariant claim was false while the fee sink was undefined; the `UNBACKED` flag labelled a risk without bounding it; the instruction set did not compose; and the closing paragraph — "nothing is trusted" — was false |

**What draft 2 genuinely fixed:** the separate nullifier closes V1 until the prune; minting the net
amount is correct; `D ≥ W + C_payout` names the right ordering; value-and-script binding is real; the
length-prefixed separator is right. **None of it survived the holes above.**

**Draft 3 — the structural redesign, and the current design's ancestor.** It restated four rules (one
account one job; a prunable record contains its own pruning condition; the program checks its own
freshness; no shared token account) and classified **all 33 findings V1–V10, W1–W11, T1–T12**. The
pattern it named: *most findings were the design failing to enforce BSV-side behaviour that no longer
needs enforcing from Solana.* The federation change **dissolved 28 of the 33**, mostly by deleting the
per-deposit underwriter, the `claimed` latch, `owed_R` and the naked-spend challenger; what remained
became **N1–N6** (current status in [05](05-trust-model.md#the-vault-audits--v-w-t-n)).

**The third audit (T1–T12)** found, among others: `slash` was **forgeable** (the spending transaction
need not be in a block); the redesign violated its own class-4 rule; `owed_R` double-counted and grew
monotonically; the settle/cancel race was still live via a caller-supplied `deadline_slot`;
`Marker` was three things at once; and **decision reversals were being made silently** — including a
test count ("17") that was wrong.

### The vault build spec, and the maturity-0 correction

The current build answers the vault's open questions and records three decided parameters:

| | Value |
|---|---|
| `lc.max_staleness_slots` | **54,000 (~6 h)** — a multiple of BSV block time; anything under ~1,500 slots per block is meaningless |
| `v.maturity_blocks` | **0, as a stored parameter** — not a compile-time constant |
| `MIN_CONFIRMATIONS` | **12**, unchanged |

**The correction that matters.** An earlier revision of the spec said **"the burn cannot fire"** at
maturity 0. **That was too strong, and the correction is the useful record:** at maturity 0 the burn
predicate is **satisfied instantly rather than unreachable**, so release and burn are a **race**, and
release normally wins. The honest statement is narrower than it first appears — maturity 0 removes the
*window* in which the reversal is comfortable, not the reversal itself. Someone who sees a reorg and
calls `burn_staged` before anyone releases still burns the tokens.

**And the reason maturity is a parameter rather than a constant:** a constant at 0 could never be
exercised, so `burn_staged` could only be *asserted*. As a parameter it is set non-zero in a test, and
the burn path is **proven**: `NotMatured` is reachable, `DepositHashUnchanged` flips to
`DepositHashChanged`, the burn zeroes the vault with the supply delta asserted, and a later maturity
raise leaves in-flight staged items at the value they were staged with.

**The tests that do NOT cover what they appear to** — `StaleClient` (54,000 slots is unreachable on a
local validator: implemented, **untested**), `AlreadyStaged`, `WrongStagedMint`, and the
`DepositHeightNotInWindow` paths on release and burn — are listed in
[08](08-status-and-roadmap.md#4-what-the-tests-do-not-cover). Naming them is the point: a document
claiming more than it verified is this project's most repeated failure.

---

## 3. The documentation refresh, and why it is kept

A documentation refresh once rewrote the public set around the model of the day — the vault, the order
book, per-relayer deposits, depth as a term of the bid, and "no governance in the PoC". **Every one of
those is now superseded**, and the refresh's own closing caution survived it: *a refresh must be
checked against claims rather than vocabulary.*

The tell that opened that worklist was `grep -l vault docs/*.md` matching only four files. By the end
it matched 22 of 23 — **and matching the word was not the same as matching the design.** The residual
job was cross-document contradictions: bond versus buffer, `bond ≥ k × owed` versus `bond_R ≥ k ×
owed_R`, and "the liveness of the advancer is the load-bearing assumption" versus the self-reporting
redemption deadline. The bond and deadline wording was reconciled; **the general cross-document
discipline remained the open item.**

**Why the record is kept rather than deleted:** the same failure happened again, one model later. The
refresh's warning — that documents deploy with the site, so flipping `main` while a document describes
an old design publishes something that no longer exists — is exactly why this set was consolidated.

**One caution carried forward.** The "naked-spend" analysis in the trust model was not deleted or
hedged: its structure — that a hot float is a **written option**, and that a bond must be sized against
detection rather than against price — remained correct. What changed was the *object*: a pooled hot
wallet became each relayer's own float, and then (under the federation) became **a BSV-side bond held
outside the reserve**. The analysis should be re-read against the current object, not carried forward
by habit.

---

## 4. Notes that did not become dependencies

### Miner-attested hashrate, and the latency question

A 2026 proposal in the BSV community: a miner signs an attestation that a transaction **will be mined
and not double-spent**; if 80% of hashrate has attested, the transaction is finalised before
inclusion; node software would reject competing blocks. It would require coordination between miners,
a leader or tie-breaking endpoint, and a BSV consensus change.

**It helps partly, and not the part we have.** Attestation makes a transaction final earlier; it does
not make a *chain* final. A minority reorg below the attestation threshold remains possible, so the
vault still has to reverse it. It **shrinks the window we must defend against; it does not remove the
need to defend it.** And it is a **new trust assumption, not the removal of one**: an 80% supermajority
can finalise anything and can censor by refusing to attest; the tie-breaking endpoint is a
centralisation point; and it needs a hard fork.

**The part worth recording is that it would be verifiable without a new oracle.** A miner's weight is
observable from the chain itself — that miner's blocks in a recent window over all blocks in the
window — and the light client already stores headers, so a Merkle proof of the coinbase transaction
plus a weight accumulator would let the program compute the weights rather than trust a report. **It
is not buildable today**, because the attestations do not exist to verify.

**The latency problem is real and ours.** 12 confirmations is ~2 hours before a deposit is mintable,
against a market expectation of seconds. **Decision: `MIN_CONFIRMATIONS` stays at 12 for the PoC.** What
is recorded is the **gap between what we do and what the market expects**, so it is a known limitation
rather than an unnoticed one. The vault is how the two decouple — *visible immediately, unlocked at
maturity* — and that decoupling is available now. **Nothing in this design may assume miner attestation
exists**, and no document should describe minting as instant until it does.

### BLS, Alpenglow, and proving Solana back to BSV

Verified against the sources: **BLS pubkey registration is live on mainnet** and a **Validator
Admission Ticket** has been live since July 2026 — validators register a 48-byte compressed BLS pubkey
with a 96-byte proof of possession, and **anyone can read it from the vote account**. But **the
BLS12-381 syscalls are status `Review`, not activated**, and the existing pairing syscall is not
instantiated for that curve: a general BPF program **cannot perform BLS12-381 operations today**. What
exists is `alt_bn128` (BN254), a different curve.

**Why it matters, and only one half does.** Alpenglow targets **~150 ms finality**, and our peg-out
settlement is gated on Solana finality — so a move from seconds to 150 ms materially improves
redemption. **That part is real.** A BLS aggregate would also be the ingredient for a Solana light
client *on BSV*, closing the other direction — **but a pairing check cannot be done in BSV script**,
and none is planned, so it would need an optimistic scheme or the same covenant research track the
reserve already depends on. **Recorded as an improvement to watch; nothing in the design may depend on
it.**

---

## 5. What the audit history teaches

1. **Verify the instruction path, not just the function.** F1 was invisible because a pure-function
   replay passed while the program could not advance.
2. **A green suite proves the code does what the tests do.** A1 and A2 were invisible to tests that
   asserted rejection against a target the test itself supplied.
3. **A wrong critical is worse than no critical.** Both of an independent pass's criticals were
   checked; one did not survive. Recording that is part of the audit.
4. **Delete the finding, do not soften the sentence.** The undecidable slashing predicate was removed;
   the capacity arithmetic was withdrawn; the "burn cannot fire" wording was corrected. Each time, the
   first instinct was a softer sentence rather than the truth.
5. **Say which thing you are claiming.** "Built", "designed", "trusted" and "open" are not hedging —
   they are the distinction this project keeps getting wrong.
6. **The reversals are the record.** Every reversal in this history is dated and given a reason,
   because a silently edited document is how the same error returns.
