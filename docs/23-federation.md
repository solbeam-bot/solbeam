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

**Admission is the Greycore's job.** The **Greycore** — the trusted, non-operational body — **finds
and admits replacement members**. Members post the bonds; entry has a capital gate, so the set is
permissioned but not anonymous, and the Greycore is the body that decides who is in it. That is the
same shape as the reference, where the second set is chosen by governance rather than by chance.

- **Two bonds, one per direction** (doc 13, *The two bonds*): a **BSV-side bond** held **outside the
  reserve**, and a **`solBSV`-side bond**, because that leg must be seizable on Solana. **The bond is
  the float** — working capital for transfers — not a capital requirement sized against the reserve
- **Neither bond sits inside the reserve**
- Fees are earned **pro rata to stake**
- Leaving: announce, wait the unbonding period, withdraw if still covered on **both** sides
- **A leaver's shares are an open finalisation item** — see §Open — for finalisation: leaver-shares

**The bond is the float, and the reserve is constrained by the Greycore, not the bond.** The bond is
**working capital that lets the federation serve redemptions**, sized for transfer throughput, not
against the reserve. The reading that bond size caps total value locked is **withdrawn**, and the
`~$180k` capacity figure computed from it is **withdrawn** (doc 26 §5). **What constrains the reserve
is the Greycore's co-signature on every reserve spend.** That is a proof of concept, and it is better
to say so than to imply otherwise.

**The BSV-side bond is enforced by the members, not by the Solana program.** It sits under the
**collective (threshold ECDSA) key** — the same primitive as the reserve, pointed at the bond — and
**not under the member's own key**. That is the design requirement the mechanism rests on: a member
cannot move their own bond, and the federation can, by signing a threshold transaction that moves the
bond. **Slashing pays the slashers from the seized bond**, which is the motive. It is a **collective
action by the majority**, not an automatic rule — nothing on BSV compels the members to sign. Two
residuals are stated in doc 13, *The two bonds*: a **majority could seize an honest member's bond**,
and the **obligation to slash is social, not on-chain**.

---

## The reserve script: threshold ECDSA **plus** a Greycore co-signature

**The reserve script is a 2-of-2 `OP_CHECKMULTISIG`:**

```
OP_2  <gateway threshold key>  <greycore key>  OP_2  OP_CHECKMULTISIG
```

**One leg is the gateway's threshold ECDSA key.** As RenVM used: shares are held by the members, a
payout needs `t` of `n` of them to cooperate, and the key is **never assembled in one place**. The
threshold group emits **one** signature however many members signed.

**The other leg is the Greycore's key.** **Both must sign**, so:

- the gateway majority **cannot move funds alone** — this fixes the collective-key hostage problem
- the Greycore cannot move funds alone
- **the Greycore polices every reserve spend**, which is what the reference does

**The Greycore is trusted third parties, not node operators.** RenVM's own words are *"Darknodes that
have developed reputations with the community"*, chosen by governance and with a stake in the
system's safety: **people with reputations to lose, who do not run the reserve.** We mirror the
reference deliberately, because the precedent is good.

**This reverses audit F10, and the reversal is explicit.** With a Greycore the deposit script
genuinely **is** a multisig, so:

- `is_p2pkh` **must change** — it can no longer require a 25-byte P2PKH script
- `DepositScript::SPACE` **must grow** to ~71 bytes for the 2-of-2, against the current 38
- F10's conclusion — *"the code is right, the docs are wrong"* — **was itself too quick and is
  reversed**: the code is right only for a P2PKH reserve, and the reserve is no longer P2PKH
- The earlier claim that "the P2PKH check is correct, not a limitation" is **superseded**

**What still holds.** The *gateway* key is threshold ECDSA, so changing the gateway's `t` or `n` is a
**re-sharing**, not a migration — the gateway key's address does not change and the reserve does not
move for that reason. **Changing the Greycore's key does change the deposit script**, so that one
membership change does require moving the reserve.

**`fed.threshold` = 4-of-N, with `N` a variable.** The number is arbitrary and deferred. It is the
gateway signing threshold; the Greycore's **size** and **threshold** are separately `open`
(`fed.greycore_size`, `fed.greycore_threshold`, doc 24).

---

## What a member does — all automatic

A member is **an operator running software**. There is no manual approval of any transaction; the
node watches, verifies, signs and challenges on its own, and the bond prices the risk of that
software being modified. It is closest to **running a staked validator**.

| Job | How |
|---|---|
| Watch both chains and verify **independently** | Each node runs its own light client; it does not take the others' word |
| Hold the reserve under the **2-of-2 `OP_CHECKMULTISIG`** | The gateway's **threshold ECDSA** key plus the **Greycore** key. **Both must sign**, so no gateway majority and no Greycore can move funds alone |
| Hold the **mint-side bonds** under the **same collective key** | The bond is *not* under the member's own key, so a caught member cannot move it. The members seize it by signing a threshold transaction, and the slashers are paid from it — a collective action by the majority, not an automatic rule |
| **Sign payout intents individually** | Recorded on Solana, so every approval is attributed |
| Produce the threshold signature for the BSV payout | Only once enough attributed intents exist, and the **Greycore co-signs** it before the 2-of-2 script will spend |
| **Report spent deposit outpoints** | Solana cannot read the BSV UTXO set, so the software reports which deposit outpoints have been spent and the program checks mints against that record. **Not a new trust assumption:** the federation is already trusted with the reserve (doc 13, *What is trustless*) |
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
| Gateway signing threshold — `fed.threshold` | **4-of-N, `N` open.** The number is **arbitrary and deferred**; it is the gateway's threshold signer count |
| Greycore size — `fed.greycore_size` | **`open`** |
| Greycore threshold — `fed.greycore_threshold` | **`open`** — the second signature on the reserve script |
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
| Per-relayer keys, each individually trusted | **A 2-of-2 reserve script** — the gateway's threshold ECDSA key plus the Greycore's key, both required |
| A single `solBSV` bond inside the reserve | **Two-sided bonds, neither inside the reserve** (doc 13, *The two bonds*) |
| An unpaid permissionless challenger | **The node software**, funded from fees and bounties |
| An immutable floor | **The exit window** is the floor |
| Slashing with no attribution mechanism | **Self-proving equivocation** on individually-signed intents |
| "Monitoring" as a late phase-5 activity | **Continuous publication of reserve and supply as an early deliverable** — the only defence where nothing is enforceable |

**The order book is redundant.** It solved fee discovery and capacity allocation; a governed fee
solves both more simply — and deleting it removes the one subsystem that never received an
adversarial review. **The "bond cap" that used to be cited here is withdrawn:** the bond is the
float, not a capacity ceiling, and the reserve is constrained by the Greycore co-signature.

---

## Open — for finalisation: leaver-shares

**The problem.** A member who leaves **retains a valid share of the reserve key**, because nothing
invalidates it. The effective threshold therefore **degrades with churn**: at **4-of-N, four former
members together still hold four valid shares**, so the threshold is a property of the current member
set only in name.

**Two remedies, both real work:**

1. **Key rotation** — the reserve moves on-chain to a newly generated key, so **`deposit_script` must
   change**. This is RenVM's answer, every epoch. The built program fixes `deposit_script` once in
   `initialize_bridge` with no instruction to change it, so rotation is currently unimplementable.
2. **Proactive re-sharing** — a protocol that invalidates old shares of the *same* key. It **needs
   the departing member's cooperation**, and it is not specified anywhere.

**The Greycore is the natural admission body**, and admission is where the standards belong. Members
will need **equipment requirements, sufficient stake, and standing with exchanges and miners**.

**The PoC deliberately does not finalise this.** It is recorded as an open finalisation item rather
than resolved.

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
5. **Leaver-shares — open, for finalisation.** A departing member retains a valid share, so the
   effective threshold degrades with churn. **Key rotation** (the reserve moves and `deposit_script`
   changes) or **proactive re-sharing** (needs the leaver's cooperation). See §Open — for
   finalisation: leaver-shares
6. **The Greycore's composition and standards** — size (`fed.greycore_size`), threshold
   (`fed.greycore_threshold`) and admission criteria are all `open`. Members will need equipment
   requirements, sufficient stake and standing with exchanges and miners
7. **The spent-outpoint record** — the federation reports spent deposit outpoints to Solana (doc 21,
   N5 resolved by this); the record's format and the write path are unspecified
