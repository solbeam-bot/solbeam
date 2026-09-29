# 23. The federation — membership, powers, and the floor

The system is run by a **bonded federation** whose members run software, hold the reserve, and are
governed by a slow supermajority. This supersedes decision **D7** ("no governance in the PoC") and
replaces the discovered order-book fee (**D2**) with a governed one.

**Status: designed, not built.**

---

## The two ideas this document rests on

Everything below follows from these, and both were arrived at by correcting earlier drafts.

**1. The floor is the exit, not immutability.** Governance holds the upgrade authority, and can in
principle change anything. That is safe — not because the rules are frozen, but because **a change
takes 30 days and redemptions cannot be paused during it.** A hostile proposal is visible while it
is only a proposal, and anyone who dislikes it leaves. By the time it takes effect there is nothing
left to take. **The protection is the exit window, not a constitution.**

**2. Slashing works by making misbehaviour self-proving.** You cannot deduce who was at fault from
an opaque threshold signature. So you don't: **members sign individually**, and a member who signs
two conflicting things has produced **their own proof of guilt**, which anyone can submit.

---

## Membership

**Open.** Anyone with a **1,000 BSV bond** may join — unlike Zcash's federation, whose membership is
fixed. Open entry with a capital gate, so the set is permissionless but not anonymous.

- The bond is posted as **`solBSV`**, because it must be seizable on Solana
- `bond ≥ k × owed` must always hold, with `k ≥ 1` — **a member cannot leave while owing**
- Fees are earned **pro rata to stake**
- Leaving: announce, wait the unbonding period, withdraw if still covered

**The bond size is the scale limit.** With `k = 1`, total value locked is capped by total bonds
pledged — ten members at 1,000 BSV is roughly $300k of capacity. That is a proof of concept, and it
is better to say so than to imply otherwise.

---

## What a member does — all automatic

A member is **an operator running software**. There is no manual approval of any transaction; the
node watches, verifies, signs and challenges on its own, and the bond prices the risk of that
software being modified. It is closest to **running a staked validator**.

| Job | How |
|---|---|
| Watch both chains and verify **independently** | Each node runs its own light client; it does not take the others' word |
| Hold the reserve under a **threshold key** | No single member can move funds |
| **Sign payout intents individually** | Recorded on Solana, so every approval is attributed |
| Produce the threshold signature for the BSV payout | Only once enough attributed intents exist |
| **Challenge theft** | The node software *is* the challenger |

**Making the challenger part of the node is the important change.** It was previously an unpaid
chore nobody owned; it is now a funded job done by the parties with the most to lose.

---

## Governance

| | |
|---|---|
| **Who may propose** | Any member |
| **To pass** | **85% of pledged coins** |
| **Delay** | **30 days** from passing to taking effect |
| **Signal** | **Live from the moment it is raised**, not only when it passes |
| **Exit** | **Redemptions stay open throughout, and can never be paused** |
| **Scope** | Includes the **upgrade authority** |

All four numbers are **parameters**, not constants — they are defaults.

**Because governance holds the upgrade key, there is no immutable floor** — and that is a
deliberate choice, not an oversight. The floor is the exit window: 30 days of live signal during
which redemptions work, so a change that would harm holders empties the bridge before it lands.

**The residual, stated plainly:** a holder who does not watch and does not act within 30 days is
exposed. That is a disclosure obligation, not a mechanism.

---

## Pause

```
pause_mints()      stops NEW mints only.
                   Redemptions continue, always.
                   Lifts automatically after N days unless renewed.
```

**Pausing inbound is a safety valve. Pausing outbound is taking hostages.** They are deliberately
not bundled — and the exit guarantee is what makes the rest of this document safe, so it is the one
power that stays off the table.

Because the power is bounded, a pause can carry a **lower threshold** than a governance change
(a simple majority of pledged coins) rather than waiting 30 days for an emergency.

---

## Slashing — self-proving misbehaviour

**This is copied from what RenVM actually deployed, not from what it planned.**

RenVM's `DarknodeSlasher` exposes functions such as
([docs](https://renproject.github.io/ren-client-docs/contracts/darknode-sol/DarknodeSlasher)):

```
slashDuplicatePropose(height, round, blockhash1, signature1, blockhash2, signature2)
slashDuplicatePrevote(...)
slashDuplicatePrecommit(...)
```

**The node's own two conflicting signatures are the entire proof.** No judgement, no vote, no
inference — a member who signs two conflicting statements has signed their own evidence, and
**anyone** can submit it for a bounty.

Our equivalent, given that members sign payout intents individually:

| Misbehaviour | Provable? | How |
|---|---|---|
| A member signs **two conflicting payout intents** | **Yes — self-proving** | Two signatures, one member, conflicting statements. Anyone slashes, anyone is paid |
| A member signs an intent matching **no authorised redemption** | **Yes** | Intents are recorded on Solana, so the program checks it against the redemption set |
| A **threshold** of members signs something invalid | Attributable, since each signature is on record | But this is ultimately a governance matter, not a cryptographic one |

**The design rule this implies:** *make misbehaviour produce a self-incriminating signed artifact,
rather than trying to infer guilt from an aggregate.* Attribution stops being a hard cryptographic
problem because nobody has to solve it.

**And the warning worth carrying:** RenVM's own documentation says the slasher *"will become a
voting system for darknodes to deregister other misbehaving darknodes. **Right now, it is a
placeholder.**"* Only the cryptographic half was ever built. **We should copy the half that
shipped and be explicit that the rest has no precedent** — not write it down as though it were
proven.

---

## Governable parameters

Changeable by 85% with a 30-day delay:

| | Default |
|---|---|
| Mint fee | **30 bp** |
| Redeem fee | **30 bp** |
| Bond size | 1,000 BSV |
| Unbonding period | — |
| Governance threshold / delay | 85% / 30 days |
| Pause threshold / duration | majority / N days |
| `FLOOR` (confirmation depth) | 12 blocks |
| `MATURITY` | 144 blocks |
| `WINDOW` | 192 records |
| **DAA parameters** | cw-144 as specified |
| Challenge bounty share | — |

**Making the DAA governable closes X3.** A BSV difficulty change becomes a vote rather than a
program upgrade — previously an orphaned risk with no owner.

---

## What this replaces

| Previously | Now |
|---|---|
| **D7** — no governance in the PoC | 85% / 30 days / live signal, holding the upgrade authority |
| **D2** — fees discovered on an order book | **30 bp, governed** |
| Per-relayer keys, each individually trusted | **Threshold key** over the reserve |
| An unpaid permissionless challenger | **The node software**, funded from fees and bounties |
| An immutable floor | **The exit window** is the floor |
| Slashing with no attribution mechanism | **Self-proving equivocation** on individually-signed intents |

**The order book is redundant.** It solved fee discovery and capacity allocation; a governed fee
plus a bond cap solves both more simply — and deleting it removes the one subsystem that never
received an adversarial review.

---

## Open questions

1. **Genesis bootstrap** — members bond `solBSV`, but none exists until a mint happens. The first
   members need a path (the genesis mint, D3)
2. **Shards** — one threshold key across all members, or several groups with their own? Shards
   contain both theft and signing latency, at the cost of coordination
3. **The unbacked path (P8)** — previously removed by requiring a registered relayer script; that
   decision needs re-examining under the federation model
4. **Vault design** — doc 21 is the current vault design and carries unfixed findings. It should be
   re-audited against this model, since several of its findings were caused by trying to enforce
   BSV-side behaviour that the federation now handles by different means
