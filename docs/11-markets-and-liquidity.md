# 11. Markets & liquidity

> **This document has lost its subject.** It was written around an **order book**: stakers posting
> liquidity, matching by price then time, and a **discovered fee**. All of that has been
> **removed** from the design. The fee is now a **governed 30 bp**, and capacity is capped by
> **bonds pledged**. SOLBEAM runs no market, so nothing below should be read as describing a
> component.
>
> **Recommendation: delete this document, or fold its two surviving facts — the governed fee and
> the bond cap — into [`06-parameters.md`](06-parameters.md).** It is kept for now only to record
> what replaced the book, and the rest of the documentation should stop treating "markets &
> liquidity" as something the protocol provides.

## What replaced the order book

| The order book did | Now |
|---|---|
| Discover the fee | **30 bp, governed** — mint and redeem |
| Allocate capacity by bid | **Capped by bonds pledged**, with `k = 1` |
| Make depth a term of the bid | **`FLOOR`, 12 BSV confirmations**, with maturity at 144 blocks |

**Fee.** 30 bp, and a parameter rather than a price. It is changed the same way anything else is:
**85% of pledged coins, a 30-day delay, live signal from the moment a proposal is raised.** There
is no book, no matching, no market-making, no fee discovery and no depth auction.

**Capacity.** With `k = 1`, total value locked is capped by total bonds pledged. Ten federation
members at the canonical **1,000 BSV** bond is roughly **$300k** of capacity. That is the scale
limit of the proof of concept, and it is a consequence of the bond rule — not a policy dial that
trading demand can move.

**Depth.** A deposit waits **`FLOOR`, 12 confirmations**, and then **144 blocks** of maturity
before the vault releases it. Depth is no longer a term anyone commits to, and there is no bid to
name a higher one.

## Trading, which is not SOLBEAM's

`solBSV` is an ordinary SPL token, so it can be held and traded wherever SPL tokens trade. That is
**external to the protocol and outside its control**: SOLBEAM deploys no pool, provides no
liquidity, quotes no price, and the program consults no oracle. Any price for `solBSV` is an
observation, not a program input; nothing in the design targets or bounds a basis against BSV.

**The honest caveat, unchanged.** If a fraudulent mint ever succeeded, the loss would land on
whoever bought the unbacked token, and **the protocol cannot compensate them.** That is a further
reason the design stages every mint in the vault instead of releasing it at once: a token still in
the program's custody cannot be sold.

---

Next: [Glossary](09-glossary.md)
