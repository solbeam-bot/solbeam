# 6. Parameters & governance

## Launch parameters

| Parameter | Value | Why |
|---|---|---|
| **Mint confirmations** | **12 BSV confirmations** (~2 hours) | Deep enough that a BSV reorg cannot un-mint a deposit. Larger deposits can be required to wait longer |
| **Redemption deadline** | **6 hours** | Window in which a payout must be proven. Covers reorg and propagation risk; larger redemptions may get a longer window |
| **Light client** | Checkpoint + rolling window, optimistic advancement | Simplest viable design; ZK verification is a later hardening step |
| **Token** | Classic SPL, 8 decimals, no freeze authority | Ordinary Solana token; composable with Raydium/Orca |
| **Fees** | Percentage of redeemed amount | Scales with BSV; keeps relayers incentivised through price moves |
| **Bond asset** | Stable unit (e.g. USDC) | Exposure is in BSV; a stable bond plus a price governor keeps the two matched |

## Tiered reserve sizing — the invariant

```
bond value  ≥  k × ( hot float  +  releasable tranche )        k ≈ 5–10
```

The bond must cover **what can be stolen before anyone can react**, not the whole reserve. Two levers set that number: how much the hot wallet holds, and how much the cold covenant may release per period.

**Worked example (illustrative):**

| Reserve | Hot cap | Tranche / 24h | Max exposure | Bond (k = 5) | TVL ÷ bond |
|---|---|---|---|---|---|
| 10,000 BSV | 50 BSV | 50 BSV | ~100 BSV | **500 BSV-equivalent** | **20×** |
| 10,000 BSV | 20 BSV | 20 BSV | ~40 BSV | **200 BSV-equivalent** | **50×** |
| 10,000 BSV | 100 BSV | 100 BSV | ~200 BSV | **1,000–2,000 BSV-equivalent** | 5–10× |

Insurance can substitute for part of the bond and raise the multiple further.

### A caution on the first draft numbers

An early sketch proposed **hot 100 BSV / cold 10,000 BSV / bond 20 BSV**. That combination does **not** satisfy the invariant: a relayer could move 100 BSV and forfeit only 20. The bond must be at least `k × (hot + tranche)`, so it needs to be **500–2,000 BSV-equivalent** for a 100 BSV hot float — or the hot float must come down. The table above gives balanced alternatives; **Option B (hot 20 / tranche 20 / bond ~200–400) is the recommended starting point.**

The launch numbers will be conservative: small float, slow tranches, modest bond. They rise as the system earns trust.

## Changing parameters as the system grows

Parameters **must** be adjustable — caps and bonds need to grow with usage, and a BSV price move changes what "safe" means.

- **Control:** a **timelocked multisig** (initially the SOLBEAM team, hardware-backed), with a published change log and an on-chain timelock (24–48 hours) so no change is a surprise.
- **Scope:** governance can change fees, caps, deadlines, tranche schedules, the hot cap, the price-governor settings, and can pause minting.
- **Critical limit:** **governance cannot move the reserve.** The cold reserve is covenant-locked (it can only pay the hot wallet), and the hot float is bonded and fraud-checkable. Admin power is over *parameters*, not over funds. This is the key separation that keeps SOLBEAM from being a custodian with extra steps.
- **Growth path:** as usage grows, control migrates toward on-chain governance, with parameter changes proposed publicly and executed after a delay.
- **Emergency pause:** freezes new mints; **existing holders can always redeem or be refunded.** A pause can never trap funds.

## Wind-down and LP exit

The cold reserve may be funded by a large liquidity provider, who will want a way out. This is handled honestly rather than cryptographically:

**Wind-down mode** (a governed, timelocked process):

1. **Announce** — a wind-down notice is published with a 30-day notice period. Nobody can rug.
2. **Stop new minting.** Existing `solBSV` continues to redeem.
3. **Drain** — redemption fees drop to zero (or are subsidised) so holders exit at par, with priority on the hot float being replenished from the cold tranches.
4. **Release** — once outstanding supply is **provably zero** (published and independently verifiable on Solana), the cold covenant's accelerated exit path releases the remainder to the LP.

**The honest caveat:** the covenant cannot verify Solana state for itself, so the final release depends on a governance attestation plus the 30-day timelock and public verifiability of supply on Solana. A **fully trustless LP exit** requires the same zero-knowledge proof machinery as a signerless redemption — i.e. the roadmap target in [Trust model](04-trust-model.md). Until then, exits are transparent, slow, and gated on supply being zero.

## Parameter change checklist

Any parameter change should be:

1. **Proposed publicly**, with the reasoning and the new invariant arithmetic.
2. **Timelocked** for 24–48 hours.
3. **Published** in a changelog with the effective date.
4. **Reversible**, except where a change has already taken effect on-chain.
5. **Never able to move reserves** — that constraint is enforced by the covenant, not by policy.

---

Next: [Roadmap](07-roadmap.md)
