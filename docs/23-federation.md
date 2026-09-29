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

**Open.** Anyone who posts the bonds may join — unlike Zcash's federation, whose membership is
fixed. Open entry with a capital gate, so the set is permissionless but not anonymous.

- **Two bonds, one per direction** (doc 13, *The two bonds*): a **BSV-side bond** held **outside the
  reserve** and sized against the BSV held, and a **`solBSV`-side bond**, because that leg must be
  seizable on Solana, sized against the `solBSV` held
- **Neither bond sits inside the reserve**
- Fees are earned **pro rata to stake**
- Leaving: announce, wait the unbonding period, withdraw if still covered on **both** sides

**The bond size is the scale limit.** With `k = 1`, total value locked is capped by total bonds
pledged — ten members at 1,000 BSV is roughly ~$180k of capacity. That is a proof of concept, and it
is better to say so than to imply otherwise.

**The BSV-side bond is enforced by the members, not by the Solana program.** It sits under the
**collective (threshold ECDSA) key** — the same primitive as the reserve, pointed at the bond — and
**not under the member's own key**. That is the design requirement the mechanism rests on: a member
cannot move their own bond, and the federation can, by signing a threshold transaction that moves the
bond. **Slashing pays the slashers from the seized bond**, which is the motive. It is a **collective
action by the majority**, not an automatic rule — nothing on BSV compels the members to sign. Two
residuals are stated in doc 13, *The two bonds*: a **majority could seize an honest member's bond**,
and the **obligation to slash is social, not on-chain**.

---

## Threshold ECDSA, not a multisig script

**The reserve address is an ordinary P2PKH address.** The key is a **threshold ECDSA** key, as RenVM
used: shares are held by the members, and a payout needs `t` of `n` of them to cooperate. The key is
**never assembled in one place.** This is the correct reading of the reserve, and it corrects audit
**F10** — the P2PKH check was right and the description was wrong (doc 25, F10).

- `is_p2pkh` requiring a 25-byte P2PKH script is **correct**, not a limitation
- `DepositScript::SPACE = 38` is **correctly sized** and does not need to grow
- **"No single member can move funds" is true because the key is shared**, not because a script
  enforces it
- The phrase **"threshold script"** is wrong and is removed everywhere: there is a threshold **key**,
  not a threshold script. A threshold script would imply a multisig, which is a different and much
  worse design

**Why it matters practically.** With a multisig, adding or removing a member changes the script, so
**the entire reserve must be swept on-chain to the new script**, requiring the old quorum to
cooperate. With threshold ECDSA, a member joins or leaves by **re-sharing the key** — a DKG-style
ceremony — and **the reserve never moves, the address never changes, and no on-chain migration
happens.** That is the whole reason to use threshold ECDSA rather than a script.

**So `fed.threshold` sizes nothing on-chain.** It is a parameter of the **signing protocol**, not of
a script. **Provisional `3-of-5`, marked `open`** (doc 24) — it is the number every "no single
member" claim depends on, and it sizes no account and no script. Changing `t` or `n` later is a
re-sharing, not a migration; larger `n` costs only coordination, not space.

---

## What a member does — all automatic

A member is **an operator running software**. There is no manual approval of any transaction; the
node watches, verifies, signs and challenges on its own, and the bond prices the risk of that
software being modified. It is closest to **running a staked validator**.

| Job | How |
|---|---|
| Watch both chains and verify **independently** | Each node runs its own light client; it does not take the others' word |
| Hold the reserve under a **threshold ECDSA key** | No single member can move funds: the reserve is an ordinary P2PKH address, but the key is never assembled in one place |
| Hold the **mint-side bonds** under the **same collective key** | The bond is *not* under the member's own key, so a caught member cannot move it. The members seize it by signing a threshold transaction, and the slashers are paid from it — a collective action by the majority, not an automatic rule |
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
| A **threshold** of members signs something invalid | Attributable, since each signature is on record | But this is ultimately a governance matter, not a cryptographic one |

**One row was deleted, not corrected: "an intent matching no authorised redemption."** A *closed*
`PegOut` is indistinguishable from one that never existed, so the program cannot decide the
predicate, and checking it would false-positive against an honest member who attested before a
cancel (audit F8).

**The design rule this implies:** *make misbehaviour produce a self-incriminating signed artifact,
rather than trying to infer guilt from an aggregate.* Attribution stops being a hard cryptographic
problem because nobody has to solve it.

**And the warning worth carrying:** RenVM's own documentation says the slasher *"will become a
voting system for darknodes to deregister other misbehaving darknodes. **Right now, it is a
placeholder.**"* Only the cryptographic half was ever built. **We should copy the half that
shipped and be explicit that the rest has no precedent** — not write it down as though it were
proven.

### Three security layers

| Layer | Catches |
|---|---|
| **Threshold signature** over the BSV payout | A **minority** moving funds |
| **Individual attestations** on Solana | A **minority's equivocation** — a member signing two conflicting intents produces its own proof of guilt |
| **Covenant** (research, later) | A **colluding majority** — enforced by miners, not by members |

**A threshold signature does not reveal who signed**, which is exactly why the individual
attestation layer exists: it creates **attribution**. And **none of the three catches a consistent
majority**, who simply sign the same fraudulent thing and never equivocate. The phrase **"double
threshold"** must not be read as a stronger threshold — it means **a key plus a paper trail.**

**Collusion is accepted, and the mitigation is transparency.** A colluding threshold can take the
reserve and nothing prevents it; the maximum loss is the entire non-member supply. Publishing the
reserve and the supply continuously, so the backing ratio is public, converts a hidden theft into a
visible one. For the two cases nothing can enforce — collusion, and an unspent-outpoint spend nobody
challenges — **visibility is the only remaining defence** (doc 13, *Collusion*; doc 07).

---

## Governable parameters

Changeable by 85% with a 30-day delay:

| | Default |
|---|---|
| Mint fee | **30 bp** |
| Redeem fee | **30 bp** |
| Bond — `fed.bond_mint` (BSV side) | 1,000 BSV, **outside the reserve** |
| Bond — `fed.bond_redeem` (`solBSV` side) | 1,000 BSV, **seizable on Solana** |
| Signing threshold — `fed.threshold` | **3-of-5 provisional, `open`.** A **signing-protocol** parameter; sizes nothing on-chain |
| `gov.delay_min` (exit floor) | **`open`** — 7 days proposed. With a floor a majority cannot take the warning away; without one the window is whatever the majority allows |
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
| Per-relayer keys, each individually trusted | **Threshold ECDSA key** over the reserve — an ordinary P2PKH address, the key never assembled in one place |
| A single `solBSV` bond inside the reserve | **Two-sided bonds, neither inside the reserve** (doc 13, *The two bonds*) |
| An unpaid permissionless challenger | **The node software**, funded from fees and bounties |
| An immutable floor | **The exit window** is the floor |
| Slashing with no attribution mechanism | **Self-proving equivocation** on individually-signed intents |
| "Monitoring" as a late phase-5 activity | **Continuous publication of reserve and supply as an early deliverable** — the only defence where nothing is enforceable |

**The order book is redundant.** It solved fee discovery and capacity allocation; a governed fee
plus a bond cap solves both more simply — and deleting it removes the one subsystem that never
received an adversarial review.

---

## Open questions

1. **Genesis — decided: a BSV-side bond.** Members post BSV at genesis, so no `solBSV` needs to
   exist first. The alternative — a **capped, explicitly-unbonded first mint** — is recorded as a
   documented later option, not chosen, because it leaves the first mint backed by nothing but the
   members' word (`docs/14-decisions.md`, D3)
2. **Shards** — one threshold key across all members, or several groups with their own? Shards
   contain both theft and signing latency, at the cost of coordination
3. **The unbacked path (P8)** — previously removed by requiring a registered relayer script; that
   decision needs re-examining under the federation model
4. **Vault design** — doc 21 is the current vault design and carries unfixed findings. It should be
   re-audited against this model, since several of its findings were caused by trying to enforce
   BSV-side behaviour that the federation now handles by different means
