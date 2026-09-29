# 5. The federation

The system is run by a **bonded federation**: members who run software rather than exercising
judgement, hold the BSV reserve under a **threshold key**, relay headers, sign payouts and challenge
theft. Membership is open, and the only gate is a bond.

This chapter replaces the earlier **relayers** chapter. That model — per-relayer deposit scripts,
each with its own key and its own float, underwriting an order book of staked bids — is
**superseded**. What changed is recorded in §What this replaces.

> **Status: designed, not built.** The built set is the light client (cw-144), the `solBSV` token,
> the mint and fork staging with chainwork — 20 passing on-chain tests. **No block, bond, member,
> threshold signature or governance vote described below exists in code yet**, and the shipped
> program mints straight to the depositor's token account, not into a vault. Read this as a
> specification. [`13-summary.md`](13-summary.md) is authoritative where this chapter disagrees.

---

## The two ideas this rests on

Everything below follows from these, and both were arrived at by correcting earlier drafts.

**1. The floor is the exit, not immutability.** Governance holds the upgrade authority and can in
principle change anything. That is safe — not because the rules are frozen, but because **a change
takes 30 days and redemptions cannot be paused during it.** A hostile proposal is visible while it
is only a proposal, and anyone who dislikes it leaves. By the time it takes effect there is nothing
left to take. **The protection is the exit window, not a constitution.**

**2. Slashing works by making misbehaviour self-proving.** You cannot deduce who was at fault from
an opaque threshold signature. So you don't: **members sign payout intents individually**, and a
member who signs two conflicting things has produced **their own proof of guilt**, which anyone can
submit.

---

## Membership

**Open.** Anyone posting a **1,000 BSV bond** may join — unlike Zcash's federation, whose membership
is fixed. Open entry with a capital gate, so the set is permissionless but not anonymous.

- The bond is posted as **`solBSV`**, because it must be **seizable on Solana**
- `bond ≥ k × owed` must always hold, with **`k = 1`** — so **a member cannot leave while owing**
- Fees are earned **pro rata to stake**
- Leaving: announce, wait the unbonding period, withdraw if still covered

**Members run software, not judgement.** There is no manual approval of any transaction: each node
watches both chains, verifies independently with its own light client, signs, and challenges —
automatically. It is **running a staked node**: pledge a bond, run the software, earn a yield, lose
the bond for misbehaving.

**The bond size is the scale limit, and that is stated rather than implied.** With `k = 1`, total
value locked is capped by total bonds pledged. Ten members at 1,000 BSV is roughly **$300k** of
capacity. That is a proof of concept, and it is better to say so than to imply otherwise.

---

## What a member does — all automatic

| Job | How |
|---|---|
| Watch both chains and verify **independently** | Each node runs its own light client; it does not take the others' word |
| Relay BSV headers to Solana | Permissionless and unpaid in itself — but nothing releases, including the member's own deposits, if the tip does not advance |
| Hold the reserve under a **threshold key** | **No single member can move funds** |
| **Sign payout intents individually** | Recorded on Solana, so every approval is attributed |
| Produce the threshold signature for the BSV payout | Only once enough attributed intents exist |
| **Challenge theft** | The node software *is* the challenger, and the bond is seizable on proof |

**Making the challenger part of the node is the important change.** It was previously an unpaid
chore nobody owned; it is now a funded job done by the parties with the most to lose.

### Rewards

- **Revenue:** the governed fee — **30 bp** — paid **pro rata to pledged stake**. There is no bid to
  post and no price to set: the fee is a parameter, so a member's income is a function of the
  system's volume and its share of the bond, not of its pricing judgement.
- **Costs:** BSV transaction fees (tiny), Solana transaction fees, and the opportunity cost of the
  bond — the dominant cost, because the bond is `solBSV` and cannot be redeemed while it is pledged.
- **Risk:** the bond. It answers `owed` — the liability the program credited from proofs it verified
  itself — and it is lost for **equivocation**, which is self-proving.
- **Caps are not a member's choice.** Capacity is set by `bond ≥ k × owed`: the bond is the limit,
  and it is enforced on-chain at the moment it is used.

The bond is `bond ≥ k × owed`, with **`k = 1`**. At `k = 1` it covers the principal and no more. A
self-dealing attack by a member is roughly break-even — what makes it unprofitable is **the mining
cost of the reorg**, not the bond. The bond's job is covering a shortfall, and the fee has to clear
the cost of the locked capital, not just gas.

---

## The reserve, and the threshold key

**One reserve, under a threshold key.** The BSV sits at the federation's deposit script and only a
threshold signature can spend it. Three consequences, and they are the whole custody story:

1. **No single member can move the reserve.** A compromised key is not a compromised reserve.
2. **A quorum that colludes can.** That is the trust assumption, stated rather than hidden, and it
   is bounded by bonds that exceed what a colluding quorum could take.
3. **The bond is `solBSV`**, the same unit as the exposure, so the program can compare the two
   on-chain with nothing external consulted, and no price move can shrink the bond relative to what
   it protects.

**What the program cannot see:** the BSV itself. The published invariant — `custodied BSV ≥
outstanding solBSV` — is **monitored, not enforced** (D8), because the reserve is off-chain. What
*is* checkable is `bond ≥ k × owed`, since both are `solBSV` quantities the program holds or measures.

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

All of those are **parameters**, not constants — they are defaults.

**Because governance holds the upgrade key, there is no immutable floor** — and that is a deliberate
choice, not an oversight. The floor is the exit window: 30 days of live signal during which
redemptions work, so a change that would harm holders empties the bridge before it lands.

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
placeholder.**"* Only the cryptographic half was ever built. **We copy the half that shipped and are
explicit that the rest has no precedent** — not write it down as though it were proven.

---

## Governable parameters

Changeable by 85% with a 30-day delay:

| | Default |
|---|---|
| Mint fee | **30 bp** |
| Redeem fee | **30 bp** |
| Bond size | 1,000 BSV |
| Bond multiple `k` | 1 |
| Unbonding period | — |
| Governance threshold / delay | 85% / 30 days |
| Pause threshold / duration | majority / N days |
| `FLOOR` (confirmation depth) | 12 blocks |
| `MATURITY` | 144 blocks |
| `WINDOW` | 192 records (32 h) |
| **DAA parameters** | cw-144 as specified |
| Challenge bounty share | — |

**Making the DAA governable closes X3.** A BSV difficulty change becomes a vote rather than a
program upgrade — previously an orphaned risk with no owner. **(Nothing above is implemented: the
built program has `MIN_CONFIRMATIONS = 12` fixed in code, and no fee, bond, vote or window
parameter is settable at runtime.)**

---

## What this replaces

| Previously | Now |
|---|---|
| **D7** — no governance in the PoC | 85% / 30 days / live signal, holding the upgrade authority |
| **D2** — fees discovered on an order book | **30 bp, governed** |
| Per-relayer keys, each individually trusted | **Threshold key** over the reserve |
| Per-relayer deposit scripts and floats | **One deposit script**, one reserve under a threshold |
| An unpaid permissionless challenger | **The node software**, funded from fees and bounties |
| An immutable floor | **The exit window** is the floor |
| Slashing with no attribution mechanism | **Self-proving equivocation** on individually-signed intents |

**The order book is redundant.** It solved fee discovery and capacity allocation; a governed fee
plus a bond cap solves both more simply — and deleting it removes the one subsystem that never
received an adversarial review. Its prose in [`12-peg-mechanism.md`](12-peg-mechanism.md) is
**superseded**, as is the relayer model in the previous version of this chapter (the Git history of
`docs/05-federation.md` records it; what matters here is that it is not the model).

---

## Open questions

1. **Genesis bootstrap** — members bond `solBSV`, but none exists until a mint happens. The first
   members need a path (the genesis mint, D3)
2. **Shards** — one threshold key across all members, or several groups with their own? Shards
   contain both theft and signing latency, at the cost of coordination
3. **The threshold itself** — `t` of `n` is not yet specified anywhere, and the choice interacts
   with shard count and the bond multiple `k`
4. **The vault design** — [`21-vault-structural.md`](21-vault-structural.md) is the current vault
   design and carries unfixed findings. It should be re-audited against this model, since several
   of its findings were caused by trying to enforce BSV-side behaviour that the federation now
   handles by different means
5. **The unbonding period and the challenge bounty share** — both are named as parameters above and
   neither has a default

---

Next: [Parameters & governance](06-parameters.md)
