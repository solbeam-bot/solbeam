# 16. Documentation refresh — the worklist

> **CLOSED — historical record, not a status page.** This document tracked a **specific
> documentation refresh of the two-gate model**: the vault, the order book, per-relayer deposits,
> depth as a term of the bid, and "no governance in the PoC". That model has since been **replaced
> by the federation model** ([`13-summary.md`](13-summary.md),
> [`23-federation.md`](23-federation.md)).
>
> **Nothing below describes the current state.** The order book and discovered fees are
> **removed**; the fee is a **governed 30 bp**; the reserve is held by a **bonded federation under
> a threshold key**; and there **is** governance — 85% of pledged coins, 30 days, live signal,
> holding the upgrade authority, with redemptions never pausable.
>
> It is kept because it is the record of that refresh, and because its closing caution — that a
> refresh must be checked against claims rather than vocabulary — is still the right one. **Use
> [`13-summary.md`](13-summary.md) for status.**

## What this document was

At the time it was written, the refresh had landed: the two-gate design and the audit were in
documents 12–15 and in `poc/TEST_PLAN.md` §0, and the public document set had been brought onto
that design. What that refresh did not cover was listed under *Still outstanding* below.

The work was done in three passes:

- **The public set.** `01`–`11` and both READMEs were rewritten around the model of the day: the
  vault, the order book, per-relayer deposits, depth as a term of the bid, no governance in the
  PoC, and the vocabulary of that model (`owed_R`, `FLOOR`, relayer consent).
- **Doc 12.** Bannered rather than rewritten. Its body still carried the older model; the banner
  named the specific contradictions and declared `13` and `14` authoritative where they disagreed.
- **Doc 13.** A canonical capability table was added, naming every component, whether it was
  built, and the line of code or test that settled it.

The tell that opened this worklist — `grep -l vault docs/*.md` matching only 12–15 — no longer
held at the time: it matched 22 of the 23 files in `docs/` (all but the cost table
[`17-costs.md`](17-costs.md)) and both READMEs. Matching the word is not the same as matching the
design, which is why the residual list below was about claims rather than vocabulary.

---

## What changed, against the documents that came before this refresh

| | Older documents said | After this refresh |
|---|---|---|
| **Where a mint lands** | Straight to the depositor | **Into a program-owned vault**; released after maturity, or burned if a reorg is followed. *Designed, not built* |
| **Reorg defence** | 12 confirmations is the protection | Depth is a **term of the bid**; maturity supplies the detection time; `FLOOR` is a backstop |
| **The reserve** | A pooled hot wallet, bounded by the naked-spend analysis | **No pooled reserve.** Deposits pay individual relayers |
| **The bond** | `bond ≥ k × (hot float + releasable tranche)` | `bond_R ≥ k × owed_R`, derived from proofs the program verified, with `k = 1` |
| **Who bears a fraud** | Holders, by dilution | Underwriters — the **bond** (seizable) first, then the **buffer** (off-chain over-collateralisation, not slashable) — **designed, not built** |
| **Fees** | A percentage set by governance | **An order book**; the fee is discovered, and depth is a term of the bid |
| **Oracles** | Not addressed | Explicit: the program consults only BSV headers and Solana slots |
| **Time** | Wall-clock deadlines | BSV headers and Solana slots |
| **Governance** | Assumed present | **Absent by decision** for the PoC, with the upgrade path a named gap |

*(Every "after this refresh" column above is itself now superseded: the reserve is a federation
threshold, the fee is governed at 30 bp, and governance is 85% / 30 days. Kept as history.)*

---

## Still outstanding at the time

The refresh changed topic sentences and vocabulary; it did not audit every claim. It left four
residual jobs, listed below. **Three were closed:** `13`'s body and the `15` tables in the pass
that produced this revision, the body of `12` in a later rewrite, and the READMEs. The
cross-document job was the residual, and the bond/deadline wording within it was reconciled in the
pass that produced this revision.

| Item | State at the time |
|---|---|
| ~~**The body of [`12-peg-mechanism.md`](12-peg-mechanism.md)**~~ | **Closed.** It was bannered rather than rewritten at first; the body was since brought onto that model — the pooled bond formula, the fixed 10 bp fee, the failure paths that re-mint, minting to the depositor and D2-as-open were gone, and the DAA banner recorded cw-144 as built with X3 open |
| ~~Present-tense claims in the body of `13-summary.md` and both READMEs~~ | **Closed.** `13`'s body was made to read in design voice, and the READMEs' `"the mint lands in the program's vault"` / `"minted into the program's vault"` were reworded |
| **Cross-document contradictions** | The refreshed documents were edited in parallel and did not agree everywhere. Recorded then: bond vs buffer and which losses the bond answers rather than detection; `bond ≥ k × owed` in older text vs `bond_R ≥ k × owed_R`; and "the liveness of the advancer is the load-bearing assumption" vs doc 04's self-reporting redemption deadline. *The bond and deadline wording was reconciled across 04/07/08/12/13/15; the general cross-document discipline remained the open item* |
| **The tables in [`15-audit-2.md`](15-audit-2.md)** | X1 retracted F1's classification, but the Table A row, the F1 heading and the ranking still called it **inherent** and the most likely honest-user loss; the bond-capacity rows ignored relayer consent; the "force a pause" row ignored that `set_paused` is authority-gated; the buffer/bond distinction was blurred (*corrected in the pass that produced this revision*) |

---

## Order

The refresh followed the order this worklist set, and the residual jobs were to follow the same
logic:

1. ~~**The body of `12`**~~ — **closed.** It was rewritten onto that model, so it no longer carried
   the old reasoning as its own.
2. ~~**The two READMEs**~~ — **closed.** Both then read in design voice, and the present tense no
   longer claimed a vault.
3. **The cross-document contradictions** — reconcile the bond/buffer and detection claims to the
   canonical form in `13` §*What exists, exactly* and `15` §*What this adds up to*. *The bond and
   deadline wording was reconciled; the general discipline remained the job.*

## Why this mattered before the merge

**These documents deploy with the site.** Flipping `main` while `12` still described a pooled hot
wallet would have published a design that no longer existed — the exact failure the audit had
already found once, documents describing software that was never built. The point of the refresh
was to stop repeating that mistake at a larger scale; the residual jobs above were the places it
had not yet stopped.

**That warning is why this file is kept closed rather than deleted:** the same failure has since
happened again, one model later, which is the reason this whole set is being rewritten onto the
federation model in [`13-summary.md`](13-summary.md).

## One caution

`04-trust-model.md` was not deleted or hedged. Its **naked-spend analysis remained correct in
structure** — the argument that a hot float is a written option, and that the bond must be sized
against detection rather than against price — and it was the reasoning that led to bonding in
`solBSV`. What changed at that time was the *object*: a pooled hot wallet became each relayer's own
float. **Under the federation model that object changes again** — there is no per-relayer float —
so the analysis should be re-read against the current model rather than carried forward.
