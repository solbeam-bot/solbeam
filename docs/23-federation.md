# 23. The federation — membership, powers, and the floors

This replaces the earlier operator-less framing. The system is run by a **bonded federation**
whose members run software, hold the reserve, and are governed by a slow supermajority.

**Status: designed, not built.** This supersedes decision **D7** ("no governance in the PoC") and
replaces the discovered order-book fee with a governed one.

---

## What the federation is

A set of **operators running a node**, each bonded, each doing the same four jobs automatically.
There is no manual approval of anything: the software watches, verifies, signs and challenges on
its own, and the bond is what prices the risk of it being modified.

It is closest to **running a staked validator** — pledge a bond, run the software, earn a yield,
lose the bond if you misbehave.

| Job | Automatic? |
|---|---|
| Watch both chains and verify independently | Yes — each node runs its own light client |
| Hold the BSV reserve under a threshold key | Yes — **no single member can move funds** |
| Sign redemption payouts | Yes — k-of-n |
| **Challenge theft** | Yes — the node software is the challenger |
| Relay headers to Solana | Yes — one of the four, not a separate role |

**Making the challenger part of the node is the important change.** It was previously an unpaid
chore nobody owned; it is now a funded job done by the parties with the most to lose.

---

## Membership

**Open.** Anyone with a **1,000 BSV bond** may join — unlike Zcash's federation, whose membership
is fixed. Open entry with a capital gate, so the set is permissionless but not anonymous.

- The bond is **seizable on Solana**, so it is posted as `solBSV`, not raw BSV
- Bond must always satisfy `bond ≥ k × owed` — **a member cannot leave while owing**
- Members earn the fee **pro rata to stake**
- Leaving: announce, wait the unbonding period, withdraw if still covered

**The bond size is the scale limit, and it is honest about it.** With `k = 1`, total value locked
is capped by total bonds pledged. Ten members at 1,000 BSV is roughly $300k of capacity. That is a
proof of concept, not a competitor to a centralised custodian, and it is better to say so.

---

## Governance

| | |
|---|---|
| **Who may propose** | Any member |
| **To pass** | **85% of pledged coins** |
| **Delay** | **30 days** from passing to taking effect |
| **Signal** | **Live from the moment it is raised** — not just when it passes |
| **Exit** | **Redemptions stay open throughout, and cannot be paused** |

**The 30-day delay is the entire protection, and it only works because exit is guaranteed.** A
proposal is visible before it passes and visible for a month after, so anyone who dislikes it can
redeem and leave. That is what makes a supermajority tolerable rather than a seizure mechanism.

### The floor — what 85% cannot change

Without a floor, a supermajority can amend anything, including what counts as a supermajority.

**Immutable in the program:**

1. **The exit guarantee** — redemptions can never be paused. This is the one that makes the rest safe
2. **Proof verification** — the light client's checks; no vote can make an invalid header valid
3. **The vault rules** — no path mints on failure; a failed redemption returns the escrow
4. **`bond ≥ k × owed`, with `k ≥ 1`** — a member can never be less covered than it owes
5. **The 85% threshold itself** — otherwise it is not a threshold, just a delay

**The honest caveat:** these are immutable *in the program*, and the program has an **upgrade
authority** (A5) which can change anything. So the real floor is whoever holds that key. The options
are to burn it (fully immutable, no bug fixes), hold it in a multisig (a trust point, but an
explicit one), or hand it to governance (which makes the floor illusory). **This is unresolved and
is the largest remaining trust question.**

---

## Pause

```
pause_mints()      stops NEW mints only.
                   Redemptions continue, always.
                   Lifts automatically after N days unless renewed.
```

**Pausing inbound is a safety valve. Pausing outbound is taking hostages.** They were previously
bundled and are now deliberately not.

A mint-only pause is bounded — the worst it does is delay new deposits, and a depositor who is
already staged is unaffected. **That boundedness is why it can be a lower threshold than a
governance change** (a simple majority of pledged coins), rather than waiting 30 days for an
emergency.

---

## Governable parameters

All variables, changeable by 85% with a 30-day delay:

| | Default |
|---|---|
| Mint fee | **30 bp** |
| Redeem fee | **30 bp** |
| Bond size | 1,000 BSV |
| Unbonding period | — |
| `FLOOR` (confirmation depth) | 12 blocks |
| `MATURITY` | 144 blocks |
| **DAA parameters** | cw-144 as specified |
| `WINDOW` | 192 records |
| Challenge bounty share | — |

**Making the DAA governable partly closes X3.** A BSV difficulty change becomes a vote rather than
a program upgrade — which was previously an orphaned risk with no owner.

---

## Slashing, and the attribution problem

**This is the largest open technical question and it should not be glossed.**

The reserve is held under a **threshold key**, so no single member can move funds. But if funds
move wrongly, **a plain threshold signature does not reveal which members signed.** You cannot
slash the guilty without knowing who they are, and slashing everyone punishes the honest and
destroys the incentive to join.

Options, none yet chosen:

1. **Per-signer signatures** — each member's signature is recorded, so guilt is attributable.
   More expensive and more on-chain data.
2. **Identifiable threshold schemes** (FROST with identifiable abort) — attribution built in.
3. **A Solana-side consent record** — members record their intent on Solana before contributing to
   the BSV signature, so the destination chain has the list. Two-phase, but attributable.
4. **Slash on the *outcome*** — funds moved with no authorising redemption means at least `k`
   members are complicit, and the bond is seized proportionally from all of them. Blunt, and
   punishes honest signers.

**Until this is decided, "the bond gets slashed" is a claim without a mechanism.** That is the same
failure the earlier vault audits found, and it should not be repeated.

---

## What this replaces

| Previously | Now |
|---|---|
| **D7** — no governance in the PoC | 85% / 30 days / live signal, with an immutable floor |
| **D2** — fees discovered on an order book | **30 bp, governed** |
| Per-relayer keys, each individually trusted | **Threshold key** over the reserve |
| An unpaid permissionless challenger | **The node software**, funded from fees and bounties |
| Relayers as unrelated parties | A **federation** sharing one reserve |

**The order book is likely redundant.** It solved fee discovery and capacity allocation; a governed
fee plus a bond cap solves both more simply. Deleting it also removes the one subsystem that never
received an adversarial review.

---

## Open questions

1. **The upgrade authority** — burn it, multisig it, or give it to governance? The honest floor depends on this
2. **Slashing attribution** — one of the four options above must be chosen
3. **Genesis bootstrap** — members must bond `solBSV`, but none exists until a mint happens. The first members need a path (the genesis mint, D3)
4. **Does the order book survive at all**, or is the fee simply governed?
5. **Shards** — one threshold key for all members, or several groups of members with their own? Shards contain both theft and signing latency
