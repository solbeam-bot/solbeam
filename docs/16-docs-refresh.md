# 16. Documentation refresh — the worklist

**Status: scoped, not done.** The two-gate design and the audit landed in documents 12–15 and
in `poc/TEST_PLAN.md` §0. **Eleven documents predate both** and still describe the earlier
model. This is the worklist, so the update can be done systematically rather than piecemeal.

The tell: `grep -l vault docs/*.md` matches **only** 12, 13, 14 and 15. Neither the vault nor
maturity nor the order book has propagated anywhere else.

---

## What changed that the older documents do not know about

| | Older documents say | Now |
|---|---|---|
| **Where a mint lands** | Straight to the depositor | **Into a program-owned vault**; released after maturity, or burned if a reorg is followed |
| **Reorg defence** | 12 confirmations is the protection | Depth is a **term of the bid**; maturity supplies the detection time; `FLOOR` is a backstop |
| **The reserve** | A pooled hot wallet, bounded by the naked-spend analysis | **No pooled reserve.** Deposits pay individual relayers |
| **The bond** | `bond ≥ k × (hot float + releasable tranche)` | `bond_R ≥ k × owed_R`, derived from proofs the program verified, with `k = 1` |
| **Who bears a fraud** | Holders, by dilution | Stakers, through the buffer — **designed, not built** |
| **Fees** | A percentage set by governance | **An order book**; the fee is discovered, and depth is a term of the bid |
| **Oracles** | Not addressed | Explicit: the program consults only BSV headers and Solana slots |
| **Time** | Wall-clock deadlines | BSV headers and Solana slots |
| **Governance** | Assumed present | **Absent by decision** for the PoC, with the upgrade path a named gap |

---

## Per file

| File | What is stale |
|---|---|
| [`01-problem.md`](01-problem.md) | Frames the problem around an operator and a bond |
| [`02-how-it-works.md`](02-how-it-works.md) | The flow is "12 confirmations, then mint to you". No vault, no maturity, no book |
| [`03-architecture.md`](03-architecture.md) | The component list predates the vault, the book and per-relayer deposits |
| [`04-trust-model.md`](04-trust-model.md) | **The largest job.** Built around a pooled hot wallet, the naked-option attack and `bond ≥ k × float`. The naked-spend reasoning is still sound in spirit, but **the object it bounds no longer exists** |
| [`05-relayers.md`](05-relayers.md) | Describes a hot wallet and bond custody. Needs per-relayer deposits, `owed_R`, consent, and the D2 auto-approve position |
| [`06-parameters.md`](06-parameters.md) | Predates the `P1`–`P10` IDs and decisions `D1`–`D8` |
| [`07-roadmap.md`](07-roadmap.md) | Sequencing predates the reset in `TEST_PLAN.md` §0 |
| [`08-faq.md`](08-faq.md) | Answers assume mint-to-user and 12-confirmation finality |
| [`09-glossary.md`](09-glossary.md) | No entries for vault, maturity, the book, `owed_R`, or the two gates |
| [`10-brand.md`](10-brand.md) | Cites 12 confirmations as a fixed number |
| [`11-markets-and-liquidity.md`](11-markets-and-liquidity.md) | The market framing holds; the fee sections need the book |
| [`README.md`](../README.md), [`docs/README.md`](README.md) | Summarise the old flow |

---

## Order

1. **`02` and `13`** — the user-facing "how it works", which is what most readers meet first.
2. **`04` and `05`** — the trust model and the relayer role. The two most load-bearing, and the
   two most wrong.
3. **`03`, `06`, `07`, `09`** — architecture, parameters, roadmap, glossary.
4. **`01`, `08`, `10`, `11`, the READMEs** — framing and reference.

## Why this comes before the merge

**These documents deploy with the site.** Flipping `main` while `04` still describes a pooled
hot wallet and `02` still says a mint lands in your wallet would publish a design that no longer
exists — which is the exact failure the audit already found once, documents describing software
that was never built. Doing it in the other order repeats that mistake at a larger scale.

## One caution

`04-trust-model.md` should not simply be deleted or hedged. Its **naked-spend analysis remains
correct in structure** — the argument that a hot float is a written option, and that the bond
must be sized against detection rather than against price — and it is the reasoning that led to
per-relayer deposits and to bonding in `solBSV`. What changes is the *object*: a pooled hot
wallet becomes each relayer's own float. Rewrite it around that, rather than discarding it.
