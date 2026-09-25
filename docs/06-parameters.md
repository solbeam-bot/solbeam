# 6. Parameters & governance

## Launch parameters

| Parameter | Value | Why |
|---|---|---|
| **Mint confirmations** | **12 BSV confirmations** (~2 hours) | Deep enough that a BSV reorg cannot un-mint a deposit. Larger deposits can be required to wait longer |
| **Redemption deadline** | **6 hours** | Window in which a payout must be proven. Covers reorg and propagation risk; larger redemptions may get a longer window |
| **Light client** | Checkpoint + rolling window, optimistic advancement | Simplest viable design; ZK verification is a later hardening step |
| **Token** | Classic SPL, 8 decimals, no freeze authority | Ordinary Solana token; composable with Raydium/Orca |
| **Fees** | Percentage of redeemed amount | Scales with BSV; keeps relayers incentivised through price moves |
| **Bond asset** | **`solBSV`, locked on Solana** | Exposure is in BSV, so the bond must be too. A stablecoin bond is a written call option on the reserve: as BSV rises it shrinks relative to what it protects, and a large holder can help the price along. Denominating in `solBSV` removes the position instead of hedging it — no oracle, no governor, no window |
| **Unbonding period** | **≥ redemption deadline + challenge window** (e.g. 7 days) | A bond withdrawable on demand is not a bond. Release must outlast the period in which a challenge can still land |
| **Challenge window** | Days | How long after the fact an unmatched-spend proof remains admissible |

## Tiered reserve sizing — the invariant

```
bond  ≥  k × ( hot float  +  releasable tranche )        both sides in solBSV
```

The bond must cover **what can be stolen before anyone can react**, not the whole reserve. Two levers set that number: how much the hot wallet holds, and how much the cold covenant may release per period.

**`k` is a vigilance parameter, not a price parameter.** With both sides in `solBSV`, `k` no longer has to absorb BSV price moves. Its only remaining job is to absorb the chance that a *naked* spend — float taken with no redemption outstanding — is never challenged. An attacker's expected cost is `B × P(slashed)`, so deterrence needs `B > H / P`, which makes `k ≈ 1/P`. That also tells you what to do about it: rather than inflating `k` indefinitely, remove the case. Once the hot wallet holds no idle float and requires a veto-only cosigner (see [Trust model](04-trust-model.md#the-naked-option-attack)), the residual is bounded by arithmetic rather than by watchfulness, and `k` can fall toward **`~1.5–2`**.

**Worked example (illustrative):**

| Reserve | Hot cap | Tranche / 24h | Max exposure | Bond in `solBSV` (k = 5) | TVL ÷ bond |
|---|---|---|---|---|---|
| 10,000 BSV | 50 BSV | 50 BSV | ~100 BSV | **500** | **20×** |
| 10,000 BSV | 20 BSV | 20 BSV | ~40 BSV | **200** | **50×** |
| 10,000 BSV | 100 BSV | 100 BSV | ~200 BSV | **1,000–2,000** | 5–10× |

Note what a `solBSV` bond costs the operator: it is **locked and cannot be redeemed while bonded**, so a relayer at `k = 5` ties up five times the float it serves. That locked capital — not gas — is what the redemption fee has to cover.

Insurance can substitute for part of the bond and raise the multiple further.

### A caution on the first draft numbers

An early sketch proposed **hot 100 BSV / cold 10,000 BSV / bond 20 BSV**. That combination does **not** satisfy the invariant: a relayer could move 100 BSV and forfeit only 20. The bond must be at least `k × (hot + tranche)`, so it needs to be **500–2,000 `solBSV`** for a 100 BSV hot float — or the hot float must come down. The table above gives balanced alternatives; **Option B (hot 20 / tranche 20 / bond ~200–400) is the recommended starting point.**

### A caution on the stablecoin bond

The first draft specified the bond as a **stable unit (e.g. USDC)**, matched to a BSV exposure by a price governor. That was wrong, and it is worth recording why rather than quietly editing it.

A stablecoin bond against a BSV liability is a **written call option on the reserve, struck at `bond ÷ float`**. Post `$500k` against `10,000 BSV` and the relayer is short `5,000 BSV`; at `$100` the option is in the money and absconding is the rational trade. Worse, the attacker does not need to time the move — a large holder can help the price along and manufacture the strike. A price governor cannot fix a written option: it is reactive, it needs an oracle, and the window between the move and the throttle *is* the trade. Denominate the bond in `solBSV`.

The launch numbers will be conservative: small float, slow tranches, modest bond. They rise as the system earns trust.

## Changing parameters as the system grows

Parameters **must** be adjustable — caps and bonds need to grow with usage, and the *shape* of the reserve (how much sits hot versus cold) changes what "safe" means. A BSV price move no longer does: with bond and exposure both in `solBSV`, the invariant is price-invariant.

- **Control:** a **timelocked multisig** (initially the SOLBEAM team, hardware-backed), with a published change log and an on-chain timelock (24–48 hours) so no change is a surprise.
- **Scope:** governance can change fees, caps, deadlines, tranche schedules, the hot cap, the challenge window and the unbonding period, and can pause minting.
- **Critical limit:** **governance cannot move the reserve.** The cold reserve is covenant-locked (it can only pay the hot wallet), and the hot float is bonded and fraud-checkable. Admin power is over *parameters*, not over funds. This is the key separation that keeps SOLBEAM from being a custodian with extra steps.
- **Critical limit (2):** governance **cannot remove the challenger bounty**, nor shorten the unbonding period below the redemption deadline. A bounty that can be voted away is a bounty an attacker can wait out, and an unbonding period shorter than the deadline is not a lock at all.
- **Growth path:** as usage grows, control migrates toward on-chain governance, with parameter changes proposed publicly and executed after a delay.
- **Emergency pause:** freezes new mints; **existing holders can always redeem or be refunded.** A pause can never trap funds.

## Wind-down and LP exit

The cold reserve may be funded by a large liquidity provider, who will want a way out. This is handled honestly rather than cryptographically:

**Wind-down mode** (a governed, timelocked process):

1. **Announce** — a wind-down notice is published with a 30-day notice period. Nobody can rug.
2. **Stop new minting.** Existing `solBSV` continues to redeem.
3. **Drain** — redemption fees drop to zero (or are subsidised) so holders exit at par, with priority on the hot float being replenished from the cold tranches.
4. **Release** — once outstanding supply is **provably zero** (published and independently verifiable on Solana), the cold covenant's accelerated exit path releases the remainder to the LP. Any bond still locked is released only after its unbonding period expires.

**The honest caveat:** the covenant cannot verify Solana state for itself, so the final release depends on a governance attestation plus the 30-day timelock and public verifiability of supply on Solana. A **fully trustless LP exit** requires the same zero-knowledge proof machinery as a signerless redemption — i.e. the roadmap target in [Trust model](04-trust-model.md). Until then, exits are transparent, slow, and gated on supply being zero.

### Entry is fast; exit is slow

This is worth stating plainly, because it is a deliberate choice rather than an oversight. The system is **easy to enter and slow to leave**. Minting is trustless and completes in about two hours. Exit is rate-limited at three separate points: the hot cap limits how much can be paid out at once, the covenant's tranche schedule limits how fast the float can be replenished, and the unbonding period limits how fast relayer capital can leave.

So a large LP cannot pull its whole position out in one move, and a relayer cannot recycle its bond quickly. That is the same machinery that protects holders, and it is why relayer capital has to be genuinely long-term. The asymmetry is acceptable for one reason: **the fast direction is the one that needs no trusted party**, and the slow direction is the one where trust has to be substituted with collateral.

## Parameter change checklist

Any parameter change should be:

1. **Proposed publicly**, with the reasoning and the new invariant arithmetic.
2. **Timelocked** for 24–48 hours.
3. **Published** in a changelog with the effective date.
4. **Reversible**, except where a change has already taken effect on-chain.
5. **Never able to move reserves** — that constraint is enforced by the covenant, not by policy.

---

Next: [Roadmap](07-roadmap.md)
