# 18. Before we code — the critical list

> **Historical, and updated 2026-09-29 for the federation model.** Items are preserved as recorded;
> where one has since been closed it carries a **Fixed** note. Item statuses below are re-based on
> [`13-summary.md`](13-summary.md) (the canonical model) and
> [`23-federation.md`](23-federation.md). **Fixed in code:** **P2** (W1.7), **P4/F7** (W1.6), the
> **window resize** (W1.4) and **A7**. The federation model **makes several P-items moot** — see
> §What the federation model makes moot — and adds four new open items at the end.

Consolidated from three adversarial audits (findings A1–A18, F1–F7, C1–C4, plus the
documentation verification). Organised by **when it has to be dealt with**, not by severity,
because that is the useful question now.

---

## Status at a glance — read this first

| | Item | Status | What it needs |
|---|---|---|---|
| **P1** | Checkpoint race + self-declared difficulty | **Moot for launch under governance** — code unchanged | `initialize` is a governed launch action, not a public race. A private deploy or a governance-gated initialize answers it |
| **P2** | `commit_fork` does not re-anchor | ✅ **Fixed** (W1.7) | `fork_parent_hash` stored at init, linked from, re-checked at commit → `ForkPointMoved`. Commit now also compares **chainwork**, not height |
| **P3** | Deposits have a hard ~32 h life | **Decided — 32 h** | The window cannot hold 48. The app automates the mint; deadline disclosed; unproven receipts published off-chain. **The on-chain refund path is still absent** |
| **P4** | Retarget halts the client (F7) | ✅ **Fixed** (W1.6) | cw-144 per block, verified **324/324** real mainnet headers. **X3 remains**: the rule is hard-coded, and governance can carry a change but not parameterise it |
| **P5** | Replay-list shutdown and hard ceiling | **Decided, not built** | `MIN_PEG_IN = 1 BSV`, **and the fixed list replaced by a nullifier PDA per minted deposit** |
| **P6** | The vault must release the replay entry when it burns (F1) | **Closed by the nullifier decision** | Burning the staged mint closes the PDA, releasing the entry as a side effect |
| **P7** | Detection has no reward (F3) | **Moot — detection is a member job**, funded from fees and bounties | Under the federation the challenger is the node software, run by the parties with the most to lose |
| **P8** | The unbacked path has no backstop (F2) | **Rebased — there is no per-deposit underwriter** | The question is now an exposure bound on the reserve before the bond set is large enough |
| **P9** | Self-dealing profits when detection fails (F5, `k = 1`) | **Moot for per-relayer self-dealing** | No per-relayer underwriting exists. The residual is a colluding threshold at `k = 1`; the exit window is the answer |
| **P10** | The bond must cover staged mints (F4) | **Carried into the federation bond definition** | `owed` includes staged mints and in-flight redemptions; the per-member attribution rule is still to design |
| **P11** | Cross-deployment replay | **Closed by design, not built** | The deposit `OP_RETURN` now carries `cluster_id` and `program_hash`, binding the deposit to this deployment |

**Fixed in code: P2 and P4 (W1.6/W1.7), the window resize (W1.4), and A7.** P1, P7, P8 and P9 are
resolved by the model rather than by code; P3 and P5 are decided but unbuilt; P6 is closed by the
nullifier decision and P10 is carried forward; P11 is closed by design.

## What the federation model makes moot

Several of these items existed **only because there was no operator or governance layer**. They do
not need code; they need the layer that now exists.

| Item | Why it was a problem | Why it is moot now |
|---|---|---|
| **P1** | Unauthenticated `initialize` — the first caller picked the checkpoint | `initialize` is part of a governed launch (D10); the checkpoint is governance-set and published. The code is unchanged; the threat model is |
| **P7** | Advancing and challenging were unpaid chores nobody owned | Detection is a **member job** with a bond behind it, paid from the governed fee (D9, O1) |
| **P8** | A peg-in with no underwriter had no backstop | There is no per-deposit underwriter at all; what backs a mint is the reserve and the bonds. The residual is an exposure bound (see §Still open) |
| **P9** | At `k = 1` a self-dealing staker was roughly break-even | There is no per-relayer underwriting to self-deal against. The analogous case — a colluding threshold — is a governance matter and is answered by the exit window, not by a bigger bond |
| **P6** | Burning a staged mint had to release the replay entry | Still required, but the nullifier PDA releases it as a side effect of closing the account |
| **P10** | The bond had to cover staged mints | Carried into the definition of `owed` (F4); the per-member attribution rule is still to design |

**Not moot:** P3 (the 32-hour deposit life is real), P5 (the ceiling is real and unbuilt), P11
(cross-deployment replay is real and now has a design fix), and the ATA-rent decision.

## Fixed in code, with evidence

| Item | Fix | Evidence |
|---|---|---|
| **P2** — `commit_fork` did not re-anchor | `fork_parent_hash` recorded at `init_staging`, linked from `push_fork_header`, re-checked at `commit_fork` (`ForkPointMoved`); commit compares accumulated chainwork | W1.7; two tests (stale commit refused, uncontested commit succeeds) |
| **P4 / F7** — retarget halted the client | cw-144 from the node's `src/pow.cpp`, computed per block; `bits == expected_bits` is gone for every header with 147 records behind it | W1.6; `difficulty-vectors/` replays 471 mainnet headers — **324/324 exact** |
| **The window resize** | `HeaderRecord` is `hash + chainwork(u128) + time` = 52 B; `WINDOW = 192`; `LIGHT_CLIENT_FIXED = 123`; **`SPACE = 10,107`** of 10,240 (133 B margin) | W1.4; compile-time assertions on `WINDOW > LOOKBACK` and `SPACE <= 10,240` |
| **A7** — replay key included `height` | Identity is `(txid, vout)`; height is stored only so stale entries can be pruned | In code: the comparison is on `txid` and `vout` only |

The on-chain suite reports **27 passing**; Phase 1A is **51/51** synthetic and **21/21** against a
live SV Node.

---

## 1. P1–P5, in detail

### P1 — The checkpoint race · **moot for launch under governance**

`initialize` accepts any 80-byte header that meets **its own declared `bits`**, and the transaction
is front-runnable. Whoever calls it first chooses both the trusted root **and** the difficulty for
the client's entire life.

The checkpoint being trusted is inherent to the design. **The race is not**, and under the
federation model it is a **launch-process** question rather than a code defect: `initialize` is
part of a governed deployment (D10), taken privately or gated on a known key, and the checkpoint is
published. *Decision: initialize as a governed launch action; the code is unchanged, which is a
deliberate acceptance, not an oversight.*

### P2 — `commit_fork` does not re-anchor the staged branch · ✅ **fixed in W1.7**

Linkage is validated when each branch header is *pushed*. At *commit* it used to check only that
the fork height was still in the window, then splice `headers[..=fork_idx] ++ staging.hashes`
**without re-checking that `headers[fork_idx].hash` was still the block the branch links to.** An
intervening commit could splice the window from two different chains, and `verify_deposit` would
then prove against a record whose linkage is broken — **the only invariant the light client has.**

**Fixed:** the expected parent hash is stored in the staging account at `init_staging` and
re-verified at commit (`ForkPointMoved`).

### P3 — A deposit has a hard 32-hour life · **critical, decided: 32 h**

`verify_deposit` requires the deposit's height to be **inside the window** (`index_of(height)`).
The window holds **192** headers and `window_start` advances with every header pushed. (W1 measured
this: 147 records are consumed by cw-144 itself, and 192 is the largest that fits the 10,240-byte
cap with margin.)

So a deposit must be minted within roughly 32 hours of its block. After that the proof can never be
verified again — **and the BSV is already in the federation's deposit script.** An honest depositor
whose mint is delayed — the advancer stalled, a gate closed, or they simply waited — loses the
deposit permanently, with no on-chain refund path (A15).

**Four separate things, taken one at a time:**

**(a) The window advances by design.** Every BSV header pushed moves `window_start` forward. 192
hashes is about 32 hours at ten-minute blocks. Normal operation, not an attack.

**(b) So every deposit has a deadline.** After ~192 blocks the answer is permanently no.

**(c) The BSV does not come back on-chain.** The deposit sits in the federation's script; moving it
needs a threshold signature. **BSV has no timelocks** — `OP_CLTV`/`OP_CSV` are no-ops — so
"refundable after 24 hours" cannot be written into the script. A refund is a **rule, not a
guarantee**, and under the federation it is a member action, not a program one.

**(d) The incentive is now different.** In the old per-relayer model, until a deposit was *proven*
the relayer held the BSV and carried no liability. Under the federation the deposit is already in
the pooled reserve, and members are bonded to the system rather than to a single deposit — so the
"profit by never minting" incentive is gone. **What remains is the 32-hour deadline itself.**

| Option | Extends the deadline? | Cost | Verdict |
|---|---|---|---|
| **Automate the mint** in the app | No — but the deadline stops mattering | trivial | **Do it.** The primary answer |
| **Disclose the deadline** (32 h) in the UI | No | trivial | **Do it** |
| **Publish unproven receipts** off-chain | No | low | **Do it.** Monitoring and reputation, not code |
| Historic-header bridging | By ~12 blocks (tx limit) | medium | Marginal |
| Multiple window accounts (4 × 192) | Yes, 4× | 4× rent (~$22) + complexity | Possible if 32 h proves too short |
| A refund path | — | — | Needs a threshold signature. A rule, not code |
| A covenant | — | research | The structural fix: removes the signing set entirely |

**The residual, stated plainly:** a depositor who does not use our app, does not mint, and does not
watch for 32 hours can lose the deposit. There is **no code fix for that while the reserve is
key-controlled.** It belongs in the trust model explicitly.

### P4 — `FLOOR` and the retarget · **F7, fixed in W1.6**

`push_header` required `bits == expected_bits`, set once at `initialize` and never refreshed.
**Fixed in W1.6:** the target is computed per block by cw-144 — the node's rule, verified against
**324/324** real mainnet headers. The original note is kept for the record: the client halted
permanently at the first difficulty change, invisible on regtest and fatal on testnet or mainnet.
The supposed fix — a stored difficulty-period anchor, on the premise that "BSV retargets every 2016
blocks" — was itself wrong: BSV adjusts **every block**.

**What is still open is X3:** the rule is hard-coded and BSV's own documentation says it may revert
to 2016-block retargeting. Doc 23 calls the DAA governable, which lets governance **carry an
upgrade** — but a parameter vote cannot swap an algorithm, so X3 is closed in the sense that there
is now an owner, not in the sense that no code change is needed. That distinction is worth keeping.

### P5 — The replay list is a cheap shutdown and a hard ceiling · **decided, not built**

`MAX_USED = 200` with `MIN_PEG_IN` unimplemented. Two hundred dust deposits block every peg-in for
the rest of the window, repeatably. Independently, it caps the protocol at **200 peg-ins per
32 hours** with no attacker at all.

**(a) Dust griefing — decided.** `MIN_PEG_IN = 1 BSV` is enforced on-chain. Two hundred
one-satoshi self-deposits previously cost dust; they now cost 200 BSV. **But it is a capital-lockup
attack, not a fee attack:** the attacker *receives tokens* for every deposit. Real, but not
prohibitive.

**(b) The 200-per-window ceiling — NOT fixed.** The account is `8 + 4 + (MAX_USED × 44) + 1` bytes
against a 10,240-byte cap, so one account holds at most **232** entries — about 100 mints a day.
That is a capacity limit, not a safety one, and it would bite in ordinary use.

| Option | Ceiling | Cost | Verdict |
|---|---|---|---|
| Keep the fixed list at 200 | 200 per 32 h | — | Fine for a PoC; not for production |
| Shrink `DepositKey` (u32 height) | ~255 | trivial | Marginal |
| **A nullifier PDA per minted deposit** ✅ | **none** | ~0.001 SOL rent per mint, refundable when closed | **The structural fix.** Solana cannot enumerate PDAs, but it does not need to — replay is checked by looking up a derived address |
| Multiple list accounts | 200 × n | n × rent | Works, more moving parts |

**Decided: adopt the nullifier**, which removes the ceiling, the pruning logic and the accidental
capacity cap at once. **Not built.** Separately, the aggregate mint cap is still unimplemented;
`MAX_MINT_PER_WINDOW` was designed as the safety parameter and the replay list is a *different*
thing that has been accidentally doubling as a capacity cap.

---

## 2. P6–P10 — vault-era items, re-based on the federation

### P6 — Closed by the nullifier decision · **was F1**

**Superseded by P5.** P6 said a burn must release the replay entry so a re-mined deposit can be
minted again. With a nullifier PDA, burning the staged mint **closes the PDA**, so the entry is
released as a side effect.

*On pinning a transaction to a specific block:* BSV has `nLockTime`, but it sets a **lower bound
only** — "not valid before height H". There is no native "expires after" either. Keying replay on
`(txid, vout)` — already done (A7) — plus the nullifier is the answer.

### P7 — Moot · **was F3**

**The earlier framing was wrong and is corrected here.** P7 was written as though the risk were
that someone relays *wrong* information. That was never the risk: the program verifies every
header. A header that does not link to the tip, that declares its own difficulty, or that misses
the target is rejected outright, and the submitter pays the fee for the privilege.

What remained was **liveness** — whether anyone bothers to deliver headers — and **under the
federation that has an owner: it is a member's job**, funded from the governed fee, with a bond
behind it. The cost is about **$0.39 a week** (doc 17). Advancing and fork staging are the two
roles; the node software performs both, and a fraud that goes undetected eats the member's bond.

**The genuine residual is narrower and covered by P2:** a *valid fork* block can be pushed and taken
as the tip, and it is replaced only by a strictly heavier branch — which is where window splicing
became possible, and P2 is the fix.

### P8 — Rebased · **was F2**

The old D6 allowed a peg-in with no underwriter, and then the vault and detection were the *entire*
defence. **Under the federation there is no per-deposit underwriter at all**, so the question
changes: what bounds how much reserve exposure the bond set can carry? With `k = 1`, total value
locked is capped by total bonds pledged — that is the bound, and it is **tight rather than
generous.** Whether an explicit exposure cap is wanted before the bond set is large enough is now
open item 5 below.

### P9 — Moot · **was F5, `k = 1`**

At `k = 1`, a staker underwriting its own fraudulent deposit was roughly break-even **only if the
shortfall was detected.** Under the federation there is no per-relayer underwriting to self-deal
against. The analogous case is **a threshold of members colluding**, and it is not
cryptographically provable (each signs one consistent intent). It is a governance matter, and the
answer is the 30-day live signal plus an exit that cannot be paused — not a larger bond.

### P10 — Carried forward · **was F4**

`owed` must include mints still maturing in the vault. A depositor whose mint is maturing has paid
BSV and holds no tokens, so omitting them leaves exactly that window unbonded. **Carried into the
definition of `owed`** in doc 12. **Still to design:** the rule that turns system-wide `owed` into
a per-member attributed share under a threshold key.

---

## 3. P11 — Cross-deployment replay · **closed by design, not built**

**Yes — redeploying the same code was the bug.** Nothing in a deposit committed to *which* Solana
deployment it was for.

Concretely: the `OP_RETURN` carried the **recipient's Solana pubkey**. A second deployment of the
same program — a fresh program id, its own light client, its own checkpoint — would verify the
same BSV deposit, because the proof is against the BSV chain and the recipient pubkey resolves
identically on both. The same BSV would then back `solBSV` on **two** deployments. It is not a
double-spend of one token: the two deployments have **different mint addresses**, so they are
different tokens, each claiming the same backing. The standard wrapped-asset failure.

**The fix is now part of the canonical payload.** Doc 13's `OP_RETURN` is

```
version ‖ cluster_id ‖ program_hash ‖ flags ‖ recipient
```

so `verify_deposit` rejects a claim whose `cluster_id` or `program_hash` is not this deployment's.
A deposit made for deployment A then cannot mint on B, because B sees a commitment to A.

**Why now rather than later:** the separator has to be in the *deposit*, which is written by the
depositor. Adding it after launch means every depositor must change what they sign, and every
deposit made before the change stays replayable forever. It is cheap to build while
`verify_deposit` is being touched for the vault and the nullifier.

### The first-time ATA rent · **decision needed**

A recipient who has never held `solBSV` needs an associated token account created, which costs
**0.00203928 SOL (~$0.157)** in rent. It is the **largest per-user cost in the system** — against
$0.0004 for the mint that triggers it, and larger than the 30 bp fee on a 1 BSV deposit ($0.09) —
and the only one that does not come back on its own; it is recoverable only by closing the account.

Against a 1 BSV minimum deposit (~$30) that is about **0.52%**, comparable to the fee itself.

| Option | Effect |
|---|---|
| **Submitter pays (status quo)** ✅ | Minting is permissionless, so whoever submits pays. The app absorbs ~$0.16 per new user as an onboarding cost, and the depositor can reclaim it by closing the account |
| Protocol reimburses | Cleaner for users; adds accounting and a withdrawal path |
| Depositor pre-creates the account | Shifts the cost and the rent-reclaim to them, and adds a step before the first deposit |

**Recommended: submitter pays, and say so in the docs.**

## 4. Known and accepted — do not re-litigate

| | |
|---|---|
| **The program upgrade authority** (A5) | Can mint arbitrarily by replacing the program. **Now held by governance** (85% / 30 days / live signal). The residual is the threshold over a small bond set |
| **The reserve is a threshold ECDSA key, not a covenant** (A6) | No covenant, and no timelocks on BSV to fall back on. **The threshold key is the mitigation** — the reserve is an ordinary P2PKH address and the key is never assembled in one place, so the P2PKH check is correct rather than a limitation (audit F10); the covenant track is the destination |
| **The reserve balance is off-chain** (A4) | Monitored, not verified (D8). The **`solBSV`-side bond** is on-chain and seized by the program; the **BSV-side bond** is held under the collective key and seized by the members collectively; the reserve's BSV itself is not readable by the program, which is why **continuous publication is an early deliverable** |
| **Refunds need a signature** (A15) | No on-chain return-to-sender path exists |
| **Detection depends on somebody pushing headers** | A funded member job under the federation — see P7 |

## 5. Audit gaps — not yet examined at all

1. **The vault's instruction set.** Still the largest design gap: release, burn, escrow and return
   do not exist even on paper in a form that has been audited, so there is nothing to audit and
   P8/P10 depend on it. It should be **re-audited against this model**.
2. **The threshold ECDSA key.** Key generation, the signing protocol, what happens during a resharing or a member's exit, and the attribution rule that turns each side's exposure into a per-member share. The *value* is provisional `3-of-5`, `open` (doc 24).
3. **Cross-chain replay.** Design fixed (P11); **no test creates a second deployment and asserts
   rejection.**
4. **The `used_deposits` counterfactual.** X2 argued the missing `seeds` constraint was not
   exploitable. It is now pinned, but **no test creates a counterfeit account and asserts
   rejection** — the closure is by argument, not by evidence.
5. **`verify_deposit`'s parsing.** No adversarial pass on malformed transactions, unusual output
   counts, or `OP_PUSHDATA` handling in the `OP_RETURN`.

*(The order book's economics used to head this list. The book is removed — D2 reversed — so that
gap is deleted rather than paid down.)*

## 6. Still open — the new items

1. **The genesis bootstrap — decided.** Members post a **BSV-side bond at genesis**, so no `solBSV`
   needs to exist first (D16). A capped, explicitly-unbonded first mint is a documented later
   option, not chosen.
2. **Sharding.** One threshold key across all members, or several groups with their own? Shards
   contain both theft and signing latency, at the cost of coordination.
3. **The signing-threshold value.** Provisional `3-of-5`, `open`, and how it trades against signing
   latency and the 30-day exit. It sizes nothing on-chain.
4. **The vault re-audit.** Doc 21 is the current vault design and carries unfixed findings. It
   should be re-audited against the federation model, since several findings came from trying to
   enforce BSV-side behaviour that the federation now handles by different means.
5. **The unbacked exposure bound.** Whether an explicit cap is wanted on reserve exposure before
   the bond set is large enough (the residual of P8).
6. **The ATA rent.** Whichever option is chosen, record it rather than discovering it.

## The decisions this document needs

1. **The ATA rent** — submitter pays, recommended above.
2. **The genesis bootstrap — decided** (D16): a BSV-side bond at genesis.
3. **The signing threshold** — provisional `3-of-5`, `open`; key generation and the attribution rule.

Everything else here is either fixed, decided, moot under the federation model, or explicitly
accepted.
