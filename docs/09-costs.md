# 09. What it costs to run, and who pays

> **Measured, not recalled.** The rent rate is **5,080 lamports per byte**, measured directly:
> `solana rent 0` = 0.00065024 SOL = 128 × 5,080, and `solana rent 165` = 0.00148844 =
> (165 + 128) × 5,080. The formula is `(bytes + 128) × 5,080` — **the 128-byte overhead is already
> included in that rate.** The commonly-quoted 6,960 is an older toolchain's constant; using it here
> overstates every rent by about 37%. This has been re-litigated twice and the measurement settles it.

> **The fee is 30 bp on each leg, governed** — not 10 bp, and no longer discovered on an order book
> (the book is removed). The **10 bp** numbers that used to run through this document are **stale**.

Figures use the measured constants: **5,000 lamports** per signature on Solana, the rent formula
above, **144 BSV blocks a day**, and a reference price of **$77/SOL** and **$30/BSV**. BSV relay is
taken at the 1 sat/byte minimum. **Priority fees are the only genuinely variable input** and are shown
as a range.

---

## The headline: the header cost is not the expensive part

Keeping Solana current with 144 BSV blocks a day is **1,008 transactions a week** — and the whole
thing costs **about 39 cents a week**.

| Priority fee (µlamports/CU) | lamports per header | SOL/week | **USD/week** | USD/year |
|---|---|---|---|---|
| 0 (base fee only) | 5,000 | 0.00504 | **$0.39** | $20 |
| 1,000 | 5,010 | 0.00505 | $0.39 | $20 |
| 10,000 | 5,100 | 0.00514 | $0.40 | $21 |
| 100,000 | 6,000 | 0.00605 | $0.47 | $24 |

Even at a hundred times a normal priority fee, a **year** of headers costs about $24. The intuition
that header upkeep would be the expensive part does not survive the arithmetic — the account is
written in place, so each header is a base fee and a little compute, not a new account.

**What is actually being paid for** is the `LightClient` account's existence, which is a **one-time,
refundable rent deposit of 0.05199 SOL (~$4.00)**. Running it is nearly free.

---

## Per-operation fees, both chains

| Operation | Chain | Cost |
|---|---|---|
| A BSV deposit transaction (~250 B) | BSV | 250 sats — **$0.000075** |
| A BSV payout transaction (~250 B) | BSV | 250 sats — **$0.000075** |
| `push_header` — one per BSV block | Solana | 5,000 lamports — **$0.00039** |
| Peg-in — verify and stage the mint | Solana | 1 signature — **$0.00039** |
| Peg-in — `release_mint` (permissionless) | Solana | 1 signature — **$0.00039** |
| Peg-out — escrow, one intent per signing member, settle | Solana | `2 + m` signatures. At 7-of-10, 9 signatures — **$0.0035** |

**BSV fees are a rounding error at four decimal places of a cent.** Solana fees are larger but still
thousandths of a cent per operation. **The new line is the intent set:** each member signs its payout
intent in its own Solana transaction, so a redemption carries one signature per member who signs.
Even at ten members that is ~$0.006, which is why it does not appear as a design constraint — but it
is a real per-redemption cost, paid by the members, and it scales with the member set.

---

## The costs that actually dominate

Not transaction fees — **rent on accounts**:

| Account | Size | Nature | Cost |
|---|---|---|---|
| **First-time ATA** for a recipient who has never held `solBSV` | 165 B | **Rent, recoverable only by closing the ATA** | **0.00148844 SOL — $0.115** |
| `LightClient` | **10,107 B** | One-time, refundable | 0.05199 SOL — $4.00 |
| `ForkStaging` per submitter | **10,069 B** | One-time, refundable on commit or abandon | 0.05180 SOL — $3.99 |
| The deposit script record (`DepositScript`) | **84 B** (8 + 4 + 71 + 1) | One-time, refundable | ~0.00257 SOL — ~$0.20 |
| A nullifier PDA per minted deposit (**built**) | ~25 B | Per mint, refundable when closed | ~0.00095 SOL — ~$0.07 |
| `StagedMint` per in-flight mint (**built**) | 97 B | Per item, refundable on release or burn | scales with the record size |
| A `PegOut` / `PayoutIntent` per in-flight redemption *(designed)* | record-sized | Per item, refundable on settle | `m × n` accounts for attribution |

**The first-time ATA is the largest per-user cost in the system — about $0.115, against $0.0004 for
the mint that funds it, and larger than the 30 bp fee on a 1 BSV deposit ($0.09).** It is paid by
whoever submits the mint, under `init_if_needed`, and is only recoverable by closing the account. At a
hundred new users it is $16, and unlike every other cost here it does not come back on its own.

The `LightClient` and `ForkStaging` accounts are the largest single deposits — about **$5.5 each** —
but both are **one-time and refundable**, so they are capital rather than spend. `ForkStaging` is per
submitter and is reclaimed on commit or abandon.

**One correction against older figures:** the deposit-script record grew from 38 to **84 bytes** when
the code began accepting the 71-byte 2-of-2 reserve script. That is a fraction of a cent and does not
change any conclusion here.

---

## Does the 30 bp fee cover the infrastructure?

Yes, comfortably, at any volume at all:

| Swap size | Value | 30 bp each way | Round trip | Covers a week of headers by |
|---|---|---|---|---|
| 1 BSV | $30 | $0.09 | $0.18 | 0.5× |
| **10 BSV** | **$300** | **$0.90** | **$1.80** | **4.6×** |
| 100 BSV | $3,000 | $9.00 | $18.00 | 46× |
| 1,000 BSV | $30,000 | $90.00 | $180.00 | 464× |

**A single 10 BSV round trip pays for about a month and a half of the entire header-advancing cost.**
At ten such swaps a week the infrastructure share is roughly **2.2% of the week's fees** ($0.39 against
$18.00) — which means the header cost is not a design constraint, and pricing it explicitly would be
over-engineering. And because the intent signatures are paid by the members out of the same fee, the
peg-out cost is covered by the same 30 bp.

There is one honest caveat: **at zero volume the $0.39/week is unrecovered.** Someone pays it — the
federation, or the team — and during the PoC that is simply a running cost. It is small enough to
absorb, but it is not zero, and it is the cost that exists *before* any revenue does.

---

## What this says about the design

1. **"Swappers pay the blockchain fees" is easy to honour**, because per-swap fees are thousandths of
   a cent. They already pay them by submitting the transaction.
2. **Header upkeep should not be a line item.** At $0.39/week it is cheaper to absorb than to meter,
   and metering it would add a parameter for no economic benefit. Under the federation it is simply
   part of a member's job, paid from the governed fee.
3. **The ATA rent deserves a decision.** It is the only per-user cost that is not trivially
   recoverable and not negligible, and it is currently shouldered by whoever submits the mint.
   **Recommended: submitter pays, and say so in the docs.**
4. **Priority fees are the only real uncertainty**, and even a hundredfold increase leaves the annual
   cost at about $24.
5. **The intent set is a real, small, new cost.** One Solana signature per signing member per
   redemption is trivial in dollars but it is the only cost that scales with the member set, and it is
   why the signing threshold should not be set larger than the security case requires.
6. **The risk is liveness, not cost.** If nobody advances the headers, minting halts — but that is a
   coordination question worth 39 cents a week, not a funding problem, and under the federation it is
   a member's funded job rather than nobody's.
