# 17. What it costs to run, and who pays

> **The 10 bp used throughout is illustrative, not a parameter.** The fee is **discovered on the
> order book**, so the figures below answer "would any plausible fee cover this?" rather than
> assuming a rate. The point stands for any fee at all.

Figures use the measured constants: **5,000 lamports** per signature on Solana, **5,080
lamports per byte** of account rent (measured from `solana rent` on the droplet), **144 BSV
blocks a day**, and a reference price of **$77/SOL** and **$30/BSV**. BSV relay is taken at the
1 sat/byte minimum. **Priority fees are the only genuinely variable input** and are shown as a
range.

---

## The headline: the header cost is not the expensive part

Keeping Solana current with 144 BSV blocks a day is **1,008 transactions a week** — and the
whole thing costs **about 39 cents a week**.

| Priority fee (µlamports/CU) | lamports per header | SOL/week | **USD/week** | USD/year |
|---|---|---|---|---|
| 0 (base fee only) | 5,000 | 0.0050 | **$0.39** | $20 |
| 1,000 | 5,010 | 0.0051 | $0.39 | $20 |
| 10,000 | 5,100 | 0.0051 | $0.40 | $21 |
| 100,000 | 6,000 | 0.0060 | $0.47 | $24 |

Even at a hundred times a normal priority fee, a **year** of headers costs about $24. The
intuition that header upkeep would be the expensive part does not survive the arithmetic — the
account is written in place, so each header is a base fee and a little compute, not a new
account.

**What is actually being paid for** is the `LightClient` account's existence, which is a
**one-time, refundable rent deposit of 0.048 SOL (~$3.68)** — already covered in
[`docs/02-how-it-works.md`](02-how-it-works.md#what-it-costs-to-run). Running it is nearly free.

---

## Per-operation fees, both chains

| Operation | Chain | Cost |
|---|---|---|
| A BSV deposit transaction (~250 B) | BSV | 250 sats — **$0.000075** |
| A BSV payout transaction (~250 B) | BSV | 250 sats — **$0.000075** |
| `push_header` — one per BSV block | Solana | 5,000 lamports — **$0.00039** |
| Peg-in mint | Solana | 1 signature — **$0.00039** |
| Peg-out (escrow, accept, prove, settle) | Solana | 4 signatures — **$0.0015** |

**BSV fees are a rounding error at four decimal places of a cent.** Solana fees are larger but
still hundredths of a cent per operation.

## The costs that actually dominate

Not transaction fees — **rent on accounts**, and one of them per user:

| Account | Nature | Cost |
|---|---|---|
| **First-time ATA** for a recipient who has never held `solBSV` | **Rent, recoverable only by closing the ATA** | **0.00149 SOL — $0.11** |
| `LightClient` (10,103 B) | One-time, refundable | 0.051 SOL — $3.95 |
| `ForkStaging` per submitter (10,069 B) | One-time, refundable on commit or abandon | 0.051 SOL — $3.94 |
| A resting bid on the book | Per bid, refundable on cancel | scales with the record size |
| An in-flight mint or redemption record | Per item, refundable on settle | scales with the record size |

**The first-time ATA is the largest per-user cost in the system — about $0.11, against
$0.0004 for the mint that funds it.** It is paid by whoever submits the mint, under
`init_if_needed`, and is only recoverable by closing the account. That is worth deciding
deliberately rather than discovering: at a hundred new users it is $11, and unlike every other
cost here it does not come back on its own.

---

## Does the swapper's fee cover the infrastructure?

Yes, comfortably, at any volume at all:

| Swap size | Value | 10 bp each way | Covers the week's headers by |
|---|---|---|---|
| 1 BSV | $30 | $0.06 | 0.2× |
| **10 BSV** | **$300** | **$0.60** | **1.5×** |
| 100 BSV | $3,000 | $6.00 | 15× |
| 1,000 BSV | $30,000 | $60.00 | 155× |

**A single 10 BSV swap pays for a week and a half of the entire header-advancing cost.** At ten
swaps a week the infrastructure share is roughly **6% of one swap's fee** — which means the
header cost is not a design constraint, and pricing it explicitly would be over-engineering.

There is one honest caveat: **at zero volume the $0.39/week is unrecovered.** Someone pays it —
a staker, the team, or the protocol — and during the PoC that is simply a running cost. It is
small enough to absorb, but it is not zero, and it is the cost that exists *before* any revenue
does.

---

## What this says about the design

1. **"Swappers pay the blockchain fees" is easy to honour**, because per-swap fees are
   hundredths of a cent. They already pay them by submitting the transaction.
2. **Header upkeep should not be a line item.** At $0.39/week it is cheaper to absorb than to
   meter, and metering it would add a parameter for no economic benefit.
3. **The ATA rent deserves a decision.** It is the only per-user cost that is not trivially
   recoverable and not negligible, and it is currently shouldered by whoever submits the mint.
4. **Priority fees are the only real uncertainty**, and even a hundredfold increase leaves the
   annual cost at about $24.
5. **The risk is liveness, not cost.** If nobody advances the headers, minting halts — but that
   is a coordination question worth 39 cents a week, not a funding problem.
