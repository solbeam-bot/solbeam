# 16. Documentation refresh — status and residual worklist

**Status: the refresh has landed.** The two-gate design and the audit are in documents 12–15 and
in `poc/TEST_PLAN.md` §0, and the public document set has been brought onto that design. What the
refresh did not cover is listed under *Still outstanding* below.

The work was done in three passes:

- **The public set.** `01`–`11` and both READMEs were rewritten around the current model: the
  vault, the order book, per-relayer deposits, depth as a term of the bid, no governance in the
  PoC, and the new vocabulary (`owed_R`, `FLOOR`, relayer consent).
- **Doc 12.** Bannered rather than rewritten. Its body still carries the old model; the banner
  names the specific contradictions and declares `13` and `14` authoritative where they disagree.
- **Doc 13.** A canonical capability table added, naming every component, whether it is built, and
  the line of code or test that settles it.

The tell that opened this worklist — `grep -l vault docs/*.md` matching only 12–15 — no longer
holds: it now matches 17 of the 19 files in `docs/` (all but the cost table
[`17-costs.md`](17-costs.md) and the contents file [`SUMMARY.md`](SUMMARY.md)) and both READMEs.
Matching the word is not the same as matching the design, which is why the residual list below is
about claims rather than vocabulary.

---

## What changed that the older documents did not know about

| | Older documents said | Now |
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

---

## Still outstanding

The refresh changed topic sentences and vocabulary; it did not audit every claim. It left four
residual jobs, listed below. The `13` half of the second job and the fourth job (the `15` tables)
were closed in the pass that produced this revision; the rest remain open.

| Item | State |
|---|---|
| **The body of [`12-peg-mechanism.md`](12-peg-mechanism.md)** | Bannered, not rewritten. The whole body still carries the old model — the pooled bond formula, a fixed 10 bp fee, failure paths that re-mint, minting to the depositor, D2 as open. **The largest remaining job** |
| **Present-tense claims in the body of [`13-summary.md`](13-summary.md) and both READMEs** | `13`'s capability table is the canonical answer, but prose around it still states the vault, the book and peg-out as running code (*the body of `13` was corrected in the pass that produced this revision; the READMEs were not*). `README.md` still says "the mint lands in the program's vault", and [`docs/README.md`](README.md) still says a mint "is minted into the program's vault" |
| **Cross-document contradictions** | The refreshed documents were edited in parallel and do not yet agree everywhere. Known: bond vs buffer and which losses the bond answers rather than detection; `bond ≥ k × owed` in older text vs `bond_R ≥ k × owed_R`; and "the liveness of the advancer is the load-bearing assumption" vs doc 04's self-reporting redemption deadline |
| **The tables in [`15-audit-2.md`](15-audit-2.md)** | X1 retracted F1's classification, but the Table A row, the F1 heading and the ranking still called it **inherent** and the most likely honest-user loss; the bond-capacity rows ignored relayer consent; the "force a pause" row ignored that `set_paused` is authority-gated; the buffer/bond distinction was blurred (*all corrected in the pass that produced this revision*) |

---

## Order

The refresh followed the order this worklist set, and the residual jobs should follow the same
logic:

1. **The body of `12`** — the full reasoning is what every other document links to, so it is now
   the most misleading artefact in the set.
2. **The two READMEs** — the first thing a reader meets, and the last place the present tense
   still claims a vault.
3. **The cross-document contradictions** — reconcile the bond/buffer and detection claims to the
   canonical form in `13` §*What exists, exactly* and `15` §*What this adds up to*.

## Why this matters before the merge

**These documents deploy with the site.** Flipping `main` while `12` still describes a pooled hot
wallet would publish a design that no longer exists — the exact failure the audit already found
once, documents describing software that was never built. The point of the refresh was to stop
repeating that mistake at a larger scale; the residual jobs above are the places it has not yet
stopped.

## One caution

`04-trust-model.md` was not deleted or hedged. Its **naked-spend analysis remains correct in
structure** — the argument that a hot float is a written option, and that the bond must be sized
against detection rather than against price — and it is the reasoning that led to per-relayer
deposits and to bonding in `solBSV`. What changed is the *object*: a pooled hot wallet became each
relayer's own float. The refresh rewrote it around that new object rather than discarding it, and
any later reconciliation of the bond/detection claims should do the same.
