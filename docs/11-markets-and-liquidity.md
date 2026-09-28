# 11. Markets & liquidity

## The core idea

The peg is the **slow, safe rail**. Markets are the **fast one**.

Nobody has to wait on the peg's confirmation depth to get `solBSV`, or on a redemption deadline to get back to BSV, because they never have to touch the peg at all. They trade. The peg exists to define the floor and ceiling of the price and to let arbitrageurs keep it honest — not to be the thing every user walks through.

```
   FAST  ──────────────────────────────────────────────────────►  SLOW
   AMM pool      P2P atomic swap      market maker      peg redemption
   seconds       ~10–60 min          minutes           hours
   no counter-   named counter-       spread, but       cheapest, no
   party risk    party, trustless    instant           counterparty
```

A user picks their point on that line: speed, cost, or trust.

> **Built or designed?** The light client, the token, the mint and fork staging are built and pass 20 on-chain tests. **The vault, the two gates, maturity, the order book, staking, bonds, `owed_R`, consent, per-relayer deposit scripts, `FLOOR` as a distinct parameter and all of peg-out are designed and not built.** The difficulty retarget is **implemented** — cw-144, verified against 324/324 real mainnet headers; the open item is X3, that the rule is hard-coded and BSV may change it — so the peg plumbing described below is the finished specification; the shipped program mints straight to the depositor. The market layer is external (pools, orderbooks, market makers) and is not SOLBEAM's to build.

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

Latency is **~10–60 minutes** (a few BSV confirmations) rather than the peg's full depth and deadline, and there is **no third party** — neither side can be cheated, only delayed. The trade-off: it needs a counterparty with the opposite asset, and both sides (or a watchtower) must act inside the timelock.

**b) Market-maker settlement.** The app matches the order and a market maker settles it instantly, bearing the peg latency. Faster and simpler for users; the cost is a spread.

This is where the earlier atomic-swap work earns its place: it was never needed for the peg itself, but it is the trustless way for two people to trade `solBSV` for BSV **without waiting for the peg**.

## Layer 3 — Market makers and bonded relayers

These are the bigger, more liquid participants, and they exist to **absorb latency**.

- **Market makers** hold inventory of both BSV and `solBSV`. A user selling `solBSV` gets BSV **immediately** from the maker's inventory; the maker later replenishes from the peg or nets it against the opposite flow. The user never waits — the maker does.
- **Bonded relayers** are the ones who actually process peg redemptions: they pay BSV from their own deposits — there is no pooled hot wallet — prove the payout against the light client, and settle. They post a bond in `solBSV` that the program can seize.

They are related but distinct: a market maker is a liquidity business, a relayer is a bonded settlement role, and one operator can be both. **A relayer is a role anyone may run, not a privileged operator**, and minting has no trusted participant at all.

**Relaying is competitive, and fees are discovered rather than set.** Instead of a governance vote or a published fee schedule, an **order book** prices the service:

- Stakers post bids — a **liquidity amount**, a **fee**, and a **confirmation depth** — and the book matches them **by price, then time**, filling partially.
- The redeemer takes the best available bid rather than naming a fee.
- Competition pushes fees toward marginal cost: BSV transaction fee, Solana fee, the cost of bond capital, and a risk premium.
- When demand spikes, fees rise; when liquidity is plentiful, they fall.

**Fees are paid in the asset staked.** A BSV-side staker earns BSV; a `solBSV`-side staker earns `solBSV`. No cross-asset conversion, and nobody has to pay anybody out.

The **refund path is the backstop**: if no bid is willing, the holder gets their `solBSV` back after the deadline. That caps how expensive relaying can get, and it is why the fee is a market — not a policy — parameter.

## Layer 4 — Exchanges

Centralised exchanges give mainstream access: a user buys BSV with a bank transfer, trades `solBSV` against BSV or USDT internally, and withdraws either. Internal settlement is instant; custody sits with the exchange.

This is a **business dependency**, not a technical one — it requires listings, market-maker arrangements, and compliance work. It is also how most users will first meet `solBSV`, so it belongs on the roadmap. Note the pleasing symmetry: a BSV holder can now withdraw `solBSV` to Solana in minutes instead of waiting on a BSV withdrawal queue, which is the exchange-confirmation problem this project set out to solve.

**An honest caveat.** A DEX or exchange exit is **outside the protocol's control** — it is someone else's market. If a fraudulent mint ever succeeded, the loss would land on whoever bought the unbacked token, and **the protocol cannot compensate them.** That is a further reason the design stages every mint in the vault instead of releasing it at once: a token still in the program's custody cannot be sold into a pool.

---

## The peg basis, and why `solBSV` trades slightly below par

`solBSV` should trade a little under BSV. That gap is not a flaw — it is the **price of speed and risk**:

| Component of the basis | Effect |
|---|---|
| Redemption latency (the bid's depth plus the deadline) | Holder is out of pocket while waiting |
| Redemption fee | Direct cost |
| Peg and smart-contract risk | Small discount demanded |
| Liquidity depth | Thinner pools widen the gap |

**Arbitrage bounds the basis.** If `solBSV` trades above BSV by more than the cost of minting, traders mint and sell — pushing it down. If it trades below by more than the cost of redeeming, traders buy and redeem — pushing it up. That is the mechanism that keeps the wrapper honest, and it is why permissionless minting matters more than any price policy could.

What widens the basis: an exhausted side of the book, thin pools, or a higher discovered fee. What narrows it: more market makers, atomic-swap P2P liquidity, and deep AMM pools. **The basis is a live health metric** and should be published and watched — but it is a human metric, not a program input. The design consults **no oracle**: the program reacts only to BSV headers and Solana slots, and external figures like the basis are published on the website and never read by code.

---

## Who does what

| Participant | Supplies | Earns | Waits? |
|---|---|---|---|
| **User** | BSV or `solBSV` | — | No — they trade |
| **P2P trader** | A counterparty leg | Price improvement | No (~10–60 min) |
| **App / orderbook (Twetch-style)** | Matching, UI | Fees / engagement | No |
| **Market maker** | Both-side inventory | Spread | Yes — they absorb it |
| **Bonded relayer** | Own BSV deposits + `solBSV` bond | Discovered fee, paid in the asset staked | Yes — it is their job |
| **Arbitrageur** | Directional flow | Basis | Partly |
| **Challenger / watchtower** | Gas | Bounty for a successful **payout** challenge; **no** bounty for detecting a reorg (F3) | No |
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
| Basis blows out during a shock | Redemption throughput is capped by the liquidity stakers have posted to the book, not by a price governor — a price shock does not trigger throttling, because bond and exposure are both `solBSV`. Expect a wider basis only if posted liquidity is exhausted; say so publicly |
| Exchange listings stall | Do not depend on them for launch; AMM + P2P + market makers work permissionlessly |
| Atomic-swap leg complexity (no P2SH on BSV) | Reuse the bare-script covenant toolchain; audit before enabling P2P swaps |

## What SOLBEAM builds vs what partners build

**SOLBEAM builds:** the light client, token, bridge program, relayer/watchtower tooling, the atomic-swap primitives, and a reference relayer app.

**Partners build:** pools (Raydium/Orca), orderbooks and UIs (Twetch-style apps and wallets), market-making desks, and exchange listings. SOLBEAM's job is to make `solBSV` *easy to trade*; it is not to be the exchange.

---

Next: [Glossary](09-glossary.md)
