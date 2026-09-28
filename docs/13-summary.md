# 13. The system, in summary

What SOLBEAM is, how the mechanism works end to end, and what it asks anyone to trust.
This is a description of the design as it stands. The reasoning behind each choice is in
[`12-peg-mechanism.md`](12-peg-mechanism.md); the questions still open are in
[`14-decisions.md`](14-decisions.md).

---

## What it is

A one-way peg. A user sends BSV and receives **`solBSV`** on Solana; a user burns `solBSV`
and receives BSV back. `solBSV` is a classic SPL token with eight decimals — one base unit
per satoshi — and **no freeze authority**, which is a design commitment rather than an
omission: nobody, including the operators, can confiscate a balance.

The peg is wholesale infrastructure. It is built for market makers, exchanges and miners who
move the coin back and forth to rebalance and to earn a fee. Retail reaches `solBSV` through
an exchange or a pool rather than through the peg directly — which is why a peg that takes
hours is acceptable, and why the interesting questions are economic rather than interactive.

## The parts

| | |
|---|---|
| **The light client** | A Solana program holding a trusted checkpoint and a rolling window of BSV block hashes. It answers one question: *is this transaction in this block, and is that block still canonical?* This is what makes minting trustless |
| **The vault** | A program-owned token account. **Every mint lands here first, never with the user.** This is the mechanism that makes the whole design work, and it is why no freeze authority is needed |
| **The book** | An order book of underwriting. Stakers post sell orders — so much liquidity, at such a fee, at such a confirmation depth — matched by price then time, partially filled |
| **Relayers** | A role, not a company. Anybody may run one. A relayer holds BSV, pays redemptions, and lodges a bond in `solBSV` that the program can seize |
| **The website** | Order entry, published parameters, and the external metrics. **No consensus role at all** — see *What is trusted* |

## Peg-in — BSV to `solBSV`

A depositor chooses terms from the book — how much liquidity, at what fee, waiting how many
confirmations — and sends BSV to the relayer's script, attaching an `OP_RETURN` naming their
Solana address. After the agreed depth, `solBSV` is **minted into the vault**, not to the
depositor. It is released to them once a maturity window passes with the deposit still
canonical.

If a reorg is followed in the meantime, the staged tokens are **burned**. The depositor's BSV
returns to them because the deposit itself was reorged away, and they end exactly where they
started. Nobody else is affected.

## Peg-out — `solBSV` to BSV

A holder escrows `solBSV` **into the vault** and names a BSV destination. A relayer whose
bond covers the amount accepts the request and has a deadline — measured in Solana slots — to
pay BSV and prove it against the light client. A challenge window follows, during which a
payout that is later reorged away can be caught.

On success the escrowed `solBSV` is burned and the relayer keeps its fee. On failure the
escrow is **returned to the holder**. Supply is unchanged either way: no failure path mints.

## The idea that holds it together

Both directions have the same shape — **enter the vault, then leave it either to the
counterparty or back to the sender** — and the vault is program-owned. That single property
does three things at once:

- **It makes a transfer reversible without a freeze authority.** The program can burn or
  return tokens in its *own* account. That is not confiscation; it is disposing of what it
  holds. No freeze authority, no Token-2022 transfer hooks, plain SPL.
- **It removes the window in which a fraudulent mint could be sold.** A staged token is not
  liquid, so there is nothing to dump on a market and no innocent buyer to inherit the loss.
- **It conserves supply.** A failed peg-out returns the escrow instead of re-minting it, so
  no failure dilutes anyone.

## Reorg protection

This is the core of the design, and it needs no oracle and nobody to trust.

**Depth is a term of the trade, not a constant.** A bid names the confirmation depth its
staker will accept, so the market prices reorg risk instead of a committee guessing at it. A
`FLOOR` remains as a backstop — set low enough never to bind, high enough to catch a bid
nobody should accept.

**Detection comes from the chain itself.** A reorg is visible in the BSV headers the client
already stores: depth from the headers, timing from the timestamps at offset 68 of each
header. Nothing external is consulted.

**Detection requires someone to advance the honest chain** — which is permissionless, cheap,
and *incentivised*. An undetected fraud eats the staking buffer, so the parties with the most
to lose have the most reason to push the honest headers and notice an orphan. Nobody has to
be appointed as a watcher; the economics do it.

**Provable fraud is reversed, not compensated.** Because the tokens are still in the vault
when a reorg is followed, they can simply be burned. That is a real repair rather than a
payout, and it is only possible because the tokens never left the program's custody.

## Time

Everything is measured on a chain, never against a wall clock. BSV depth and block time come
from BSV headers; redemption deadlines come from Solana slots, so a cluster halt **freezes**
the clock rather than burning the relayer who could not act.

## Economics

**Fees are paid in the asset staked.** A BSV-side staker earns BSV; a `solBSV`-side staker
earns `solBSV`. No cross-asset conversion, and nobody has to pay anybody out.

**Stakers underwrite the system**, which means a fraudulent mint lands on **volunteers who
chose the risk** rather than diluting every holder. Their stake sits as
over-collateralisation, so a shortfall is absorbed before it can touch anyone else's
backing.

**The bond is seizable.** `bond ≥ k × owed`, denominated in `solBSV`, held by the program.
Because the amount a relayer has been credited is derived from proofs the program verified
itself, that inequality is checkable on-chain — no attestation, no oracle.

**Fees mature with the principal.** A fee that could be withdrawn earlier than the mint
behind it would be an exit from maturity, which is the one thing the gates exist to prevent.

## What is trusted

Deliberately short.

| | |
|---|---|
| **The checkpoint** | The light client starts from a block hash taken on faith. It is published, buried deep, and the only thing not proven |
| **The BSV reserve keys** | BSV sits under keys that can spend it. There is no covenant, and BSV has no timelocks to fall back on |
| **The program upgrade authority** | It can override every parameter. Out of scope for the proof of concept, and recorded rather than hidden |
| **That honest headers get pushed within the maturity window** | A liveness condition anyone can satisfy, not a trust assumption — and one with a built-in incentive |

Everything else — deposits, backing, minting, reorg detection and the payout proof — is
enforced by code.

## Where to read next

- [`12-peg-mechanism.md`](12-peg-mechanism.md) — the full reasoning, scenarios, and the
  audit findings against the implemented program
- [`14-decisions.md`](14-decisions.md) — **every decision still open**, with options
- [`04-trust-model.md`](04-trust-model.md) — what is trusted, and the roadmap to a
  signerless reserve
