# 11. Markets & liquidity

## The core idea

The peg is the **slow, safe rail**. Markets are the **fast one**.

Nobody has to wait two hours to get `solBSV`, or up to six hours to get back to BSV, because they never have to touch the peg at all. They trade. The peg exists to define the floor and ceiling of the price and to let arbitrageurs keep it honest — not to be the thing every user walks through.

```
   FAST  ──────────────────────────────────────────────────────►  SLOW
   AMM pool      P2P atomic swap      market maker      peg redemption
   seconds       ~10–60 min          minutes           up to 6 hours
   no counter-   named counter-       spread, but       cheapest, no
   party risk    party, trustless    instant           counterparty
```

A user picks their point on that line: speed, cost, or trust.

---

## Layer 1 — AMM pools (the default)

`solBSV` is an ordinary SPL token, so it trades in permissionless pools on **Raydium** and **Orca** the moment it exists. Pairs worth seeding:

- `solBSV` / USDC — the price reference.
- `solBSV` / SOL — for Solana-native traders.
- `solBSV` / USDT — for stablecoin arbitrage.

Trades settle in seconds with no counterparty and no permission. The only requirement is liquidity, which is a bootstrapping problem, not a technical one (see below).

## Layer 2 — P2P and orderbooks (Twetch-style apps)

Apps and wallets can host a **`solBSV` : BSV orderbook** so two traders swap directly, on their own terms, without waiting for the peg.

Because the two assets live on different chains, the orderbook needs a settlement method. There are two honest options:

**a) Atomic swap (trustless).** A shared secret locks both legs:

- **BSV leg:** a bare-script covenant — hashlock via `OP_SHA256`, refund after a timeout enforced by inspecting the transaction's own `nLockTime` (BSV's old timelock opcodes are disabled, so this is done with preimage introspection).
- **Solana leg:** an escrow program with the same hashlock and an expiry enforced by the `Clock` sysvar.
- The secret-holder claims one leg, revealing the secret, which lets the counterparty claim the other.

Latency is **~10–60 minutes** (a few BSV confirmations) rather than six hours, and there is **no third party** — neither side can be cheated, only delayed. The trade-off: it needs a counterparty with the opposite asset, and both sides (or a watchtower) must act inside the timelock.

**b) Market-maker settlement.** The app matches the order and a market maker settles it instantly, bearing the peg latency. Faster and simpler for users; the cost is a spread.

This is where the earlier atomic-swap work earns its place: it was never needed for the peg itself, but it is the trustless way for two people to trade `solBSV` for BSV **without waiting for the peg**.

## Layer 3 — Market makers and bonded relayers

These are the bigger, more liquid participants, and they exist to **absorb latency**.

- **Market makers** hold inventory of both BSV and `solBSV`. A user selling `solBSV` gets BSV **immediately** from the maker's inventory; the maker later replenishes from the peg or nets it against the opposite flow. The user never waits — the maker does.
- **Bonded relayers** are the ones who actually process peg redemptions: pay BSV from the hot wallet, prove the payout, settle. They are bonded because they hold the float.

They are related but distinct: a market maker is a liquidity business, a relayer is a bonded settlement role, and one operator can be both.

**Relaying is competitive.** Fees are market-driven, and any bonded participant can fulfil a redemption:

- The redeemer offers a fee (or a maximum fee).
- Relayers compete to fulfil — the first to present a valid payout proof wins, or the cheapest bid wins if the redemption is auctioned.
- Competition pushes fees toward marginal cost: BSV transaction fee, Solana fee, the cost of bond capital, and a risk premium.
- When demand spikes, fees rise; when liquidity is plentiful, they fall.

The **automatic refund path is the backstop**: if no relayer is willing at the offered fee, the holder gets their `solBSV` back after the deadline. That caps how expensive relaying can get, and it is why the fee is a market — not a policy — parameter.

## Layer 4 — Exchanges

Centralised exchanges give mainstream access: a user buys BSV with a bank transfer, trades `solBSV` against BSV or USDT internally, and withdraws either. Internal settlement is instant; custody sits with the exchange.

This is a **business dependency**, not a technical one — it requires listings, market-maker arrangements, and compliance work. It is also how most users will first meet `solBSV`, so it belongs on the roadmap. Note the pleasing symmetry: a BSV holder can now withdraw `solBSV` to Solana in minutes instead of waiting on a BSV withdrawal queue, which is the exchange-confirmation problem this project set out to solve.

---

## The peg basis, and why `solBSV` trades slightly below par

`solBSV` should trade a little under BSV. That gap is not a flaw — it is the **price of speed and risk**:

| Component of the basis | Effect |
|---|---|
| Redemption latency (up to 6 hours) | Holder is out of pocket while waiting |
| Redemption fee | Direct cost |
| Peg and smart-contract risk | Small discount demanded |
| Liquidity depth | Thinner pools widen the gap |

**Arbitrage bounds the basis.** If `solBSV` trades above BSV by more than the cost of minting, traders mint and sell — pushing it down. If it trades below by more than the cost of redeeming, traders buy and redeem — pushing it up. That is the mechanism that keeps the wrapper honest, and it is why permissionless minting matters more than any price policy could.

What widens the basis: a throttled peg during a price shock, thin liquidity, or a redemption-fee increase. What narrows it: more market makers, atomic-swap P2P liquidity, and deep AMM pools. **The basis is a live health metric** and should be published and watched.

---

## Who does what

| Participant | Supplies | Earns | Waits? |
|---|---|---|---|
| **User** | BSV or `solBSV` | — | No — they trade |
| **P2P trader** | A counterparty leg | Price improvement | No (~10–60 min) |
| **App / orderbook (Twetch-style)** | Matching, UI | Fees / engagement | No |
| **Market maker** | Both-side inventory | Spread | Yes — they absorb it |
| **Bonded relayer** | Hot float + bond | Competitive fee | Yes — it is their job |
| **Arbitrageur** | Directional flow | Basis | Partly |
| **Challenger / watchtower** | Gas | Slashed-bond share | No |
| **Exchange** | Custody + order book | Fees | No (internally) |

---

## Bootstrapping liquidity

1. **Seed the AMM pools** at launch — a credible initial `solBSV`/USDC depth is the single highest-leverage action for adoption.
2. **Sign market makers** for instant redemption at a published spread, so the "fast path" exists on day one.
3. **Open the relayer role** publicly (desktop and mobile apps) so redemption fees stay competitive.
4. **Pursue listings** sequentially: permissionless pools first, then tier-2 exchanges, then majors.
5. **Encourage BSV apps and wallets** — Twetch-style orderbooks, wallet swaps, and P2P desks — because they reach users who never touch a peg UI.
6. **Publish the basis** and pool depth, so liquidity is transparent and improvable.

## Risks

| Risk | Mitigation |
|---|---|
| Thin pools at launch | Seed liquidity; incentivise market makers; start with one deep pair rather than many shallow ones |
| Market makers withdraw | Multiple makers; atomic-swap P2P as a fallback that needs no inventory; publish depth |
| Basis blows out during a shock | Redemption throughput is capped by the hot float and tranche schedule, not by a price governor — a price shock no longer triggers throttling, because bond and exposure are both `solBSV`. Expect a wider basis only if the float is exhausted; say so publicly |
| Exchange listings stall | Do not depend on them for launch; AMM + P2P + market makers work permissionlessly |
| Atomic-swap leg complexity (no P2SH on BSV) | Reuse the bare-script covenant toolchain; audit before enabling P2P swaps |

## What SOLBEAM builds vs what partners build

**SOLBEAM builds:** the light client, token, bridge program, relayer/watchtower tooling, the atomic-swap primitives, and a reference relayer app.

**Partners build:** pools (Raydium/Orca), orderbooks and UIs (Twetch-style apps and wallets), market-making desks, and exchange listings. SOLBEAM's job is to make `solBSV` *easy to trade*; it is not to be the exchange.

---

Next: [Glossary](09-glossary.md)
