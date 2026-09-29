# 5. The federation

The system is run by a **bonded federation**: members who run software rather than exercising
judgement, hold the BSV reserve under a **2-of-2 script with the Greycore**, relay headers, sign
payouts and challenge theft. **Admission is the Greycore's job** — it is the trusted, non-operational
body, so it finds and admits replacement members — and the gate for a member is a bond plus the
Greycore's standing.

This chapter replaces the earlier **relayers** chapter. That model — per-relayer deposit scripts,
each with its own key and its own float, underwriting an order book of staked bids — is
**superseded**. What changed is recorded in §What this replaces.

> **Status: designed, not built.** The built set is the light client (cw-144), the `solBSV` token,
> the mint and fork staging with chainwork — 27 passing on-chain tests. **No block, bond, member,
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

**Open.** Anyone posting the bonds may join — unlike Zcash's federation, whose membership
is fixed. Open entry with a capital gate, so the set is permissionless but not anonymous.

- **Two bonds, one per direction** (doc 13, *The two bonds*): a **BSV-side bond** held **outside the
  reserve** and sized against the BSV held, and a **`solBSV`-side bond**, because that leg must be
  **seizable on Solana**, sized against the `solBSV` held
- **Neither bond sits inside the reserve**
- Fees are earned **pro rata to stake**
- Leaving: announce, wait the unbonding period, withdraw if still covered on **both** sides

**Members run software, not judgement.** There is no manual approval of any transaction: each node
watches both chains, verifies independently with its own light client, signs, and challenges —
automatically. It is **running a staked node**: pledge a bond, run the software, earn a yield, lose
the bond for misbehaving.

**The BSV-side bond is enforced by the members, not by the Solana program.** It sits under the
**collective (threshold ECDSA) key** — the same primitive as the reserve, pointed at the bond — and
**not under the member's own key**. That is the design requirement: a member cannot move their own
bond, and the federation can, by signing a threshold transaction that moves the bond. **Slashing pays
the slashers from the seized bond**, which is the motive. It is a **collective action by the
majority**, not an automatic rule — nothing on BSV compels the members to sign — and the two
residuals (a majority could seize an honest member's bond; the duty to slash is social) are stated in
doc 13, *The two bonds*.

**The bond is the float, and the reserve is constrained by the Greycore, not the bond.** The bond is
**working capital for transfers** — the float that lets the federation serve redemptions — not a
capital requirement sized against the reserve. The claim that total value locked is capped by bonds
pledged is **withdrawn**, along with its `~$180k` figure (that was the output of RenVM's `n/t`
bribery-cost rule, which does not apply to a bond that is the float). **What constrains the reserve is
the Greycore's co-signature on every reserve spend.** That is a proof of concept, and it is better to
say so than to imply otherwise.

---

## What a member does — all automatic

| Job | How |
|---|---|
| Watch both chains and verify **independently** | Each node runs its own light client; it does not take the others' word |
| Relay BSV headers to Solana | Permissionless and unpaid in itself — but nothing releases, including the member's own deposits, if the tip does not advance |
| Hold the reserve under a **2-of-2 `OP_CHECKMULTISIG`** | The gateway's **threshold ECDSA** key plus the **Greycore**'s. **Both must sign**, so no gateway majority and no Greycore can move funds alone |
| **Sign payout intents individually** | Recorded on Solana, so every approval is attributed |
| Produce the threshold signature for the BSV payout | Only once enough attributed intents exist, and the **Greycore co-signs** it before the script will spend |
| **Report spent deposit outpoints** | Solana cannot read the BSV UTXO set; the software reports which deposit outpoints are spent and the program checks mints against that record (the accepted oracle for what Solana cannot see) |
| **Challenge theft** | The node software *is* the challenger: the `solBSV` bond is seized by the program on proof, and the BSV bond by the members collectively |

**Making the challenger part of the node is the important change.** It was previously an unpaid
chore nobody owned; it is now a funded job done by the parties with the most to lose.

### Rewards

- **Revenue:** the governed fee — **30 bp** — paid **pro rata to pledged stake**. There is no bid to
  post and no price to set: the fee is a parameter, so a member's income is a function of the
  system's volume and its share of the bond, not of its pricing judgement.
- **Costs:** BSV transaction fees (tiny), Solana transaction fees, and the opportunity cost of the
  bonds — the dominant cost, because the `solBSV`-side bond cannot be redeemed while it is pledged
  and the BSV-side bond is capital held outside the reserve.
- **Risk:** the bonds. They cover what each side holds and are lost for **equivocation**, which is
  self-proving. The BSV-side bond sits under the **collective key** and is seized by the members collectively, with the slashers paid from it — a collective action by the majority, not an automatic rule.
- **The coverage floor is not a member's choice.** The two-sided bonds carry a floor — `bsv_bond ≥ k ×
  (BSV held)` and `solbsv_bond ≥ k × (solBSV held)` — checked on-chain at the moment a mint or an exit
  uses it. It is a **solvency check, not a capacity ceiling**.

The floor is **`k = 1`** per side, and **neither bond sits inside the reserve**, so a bond is no longer
funded by a deposit into the very reserve it covers (the old single-bond flaw). **The bond is the
float** — working capital for transfers — and the fee has to clear the cost of the locked capital, not
just gas. It **prices provable misbehaviour**; it does not restore a loss.

---

## The reserve, and the 2-of-2 script

**One reserve, under a 2-of-2 `OP_CHECKMULTISIG`.** The BSV sits at the federation's reserve script:
the gateway's **threshold ECDSA** key (never assembled in one place) **plus** the **Greycore**'s key.
**Both must sign**, so the gateway majority cannot move funds alone, the Greycore cannot move funds
alone, and the Greycore polices every reserve spend. **This reverses audit F10:** the deposit script
genuinely is a multisig, so `is_p2pkh` must change and `DepositScript::SPACE` must grow to ~71 bytes
(from 38). Three consequences, and they are the whole custody story:

1. **No single member — and no gateway majority — can move the reserve.** A compromised key share is
   not a compromised reserve.
2. **The gateway quorum and the Greycore together can.** That is the trust assumption, stated rather
   than hidden, and it is bounded by the co-signature and by **continuous publication** of the reserve
   and supply (doc 07).
3. **Each bond is in the asset its side holds**, so the program can compare each pair on-chain with
   nothing external consulted, and no price move shrinks either bond relative to what it protects.

**What the program cannot see:** the BSV itself. The published invariant — `custodied BSV ≥
outstanding solBSV` — is **monitored, not enforced** (D8), because the reserve is off-chain. What
*is* checkable is each bond against its own side, since those are quantities the program holds or
measures. **The one reported input is the spent-outpoint record** the program checks mints against.

**`fed.threshold` = 4-of-N, with `N` a variable.** The number is arbitrary and deferred (doc 24). It is
the gateway signing threshold; the Greycore's size and threshold are separately `open`
(`fed.greycore_size`, `fed.greycore_threshold`). Changing the gateway's `t` or `n` is a **re-sharing**,
not a migration; **changing the Greycore's key changes the deposit script**, so that one membership
change does move the reserve. **Leaver-shares are an open finalisation item:** a departing member
retains a valid share, so the effective threshold degrades with churn — resolving it needs key rotation
or proactive re-sharing (doc 23, *Open — for finalisation: leaver-shares*).

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
placeholder.**"* Only the cryptographic half was ever built. **We copy the half that shipped and are
explicit that the rest has no precedent** — not write it down as though it were proven.

---

## Governable parameters

Changeable by 85% with a 30-day delay:

| | Default |
|---|---|
| Mint fee | **30 bp** |
| Redeem fee | **30 bp** |
| Bond — `fed.bond_mint` (BSV side) | 1,000 BSV, outside the reserve |
| Bond — `fed.bond_redeem` (`solBSV` side) | 1,000 BSV, seizable on Solana |
| Bond multiple `k` | 1 |
| Gateway signing threshold `fed.threshold` | **4-of-N, `N` open** — the number is deferred |
| Greycore size / threshold | `fed.greycore_size` / `fed.greycore_threshold`, both `open` |
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
| Per-relayer keys, each individually trusted | **A 2-of-2 reserve script** — the gateway's threshold ECDSA key plus the Greycore's key, both required |
| A single `solBSV` bond inside the reserve | **Two-sided bonds, neither inside the reserve** (doc 13, *The two bonds*) |
| Per-relayer deposit scripts and floats | **One reserve address**, one reserve under a threshold |
| An unpaid permissionless challenger | **The node software**, funded from fees and bounties |
| An immutable floor | **The exit window** is the floor |
| "Monitoring" as a late activity | **Continuous publication of reserve and supply as an early deliverable** — the only defence where nothing is enforceable |
| Slashing with no attribution mechanism | **Self-proving equivocation** on individually-signed intents |

**The order book is redundant.** It solved fee discovery and capacity allocation; a governed fee
solves both more simply — and deleting it removes the one subsystem that never received an adversarial
review. **The "bond cap" once cited here is withdrawn:** the bond is the float, and the reserve is
constrained by the Greycore co-signature. Its prose in
[`12-peg-mechanism.md`](12-peg-mechanism.md) is
**superseded**, as is the relayer model in the previous version of this chapter (the Git history of
`docs/05-federation.md` records it; what matters here is that it is not the model).

---

## Open questions

1. **Genesis — decided: a BSV-side bond.** Members post BSV at genesis, so no `solBSV` needs to exist
   first. A capped, explicitly-unbonded first mint is recorded as a documented later option, not
   chosen (`docs/14-decisions.md`, D16)
2. **Shards** — one threshold key across all members, or several groups with their own? Shards
   contain both theft and signing latency, at the cost of coordination
3. **The threshold value** — `fed.threshold` = **4-of-N**, with `N` a variable and the number
   deferred, and `fed.greycore_size` / `fed.greycore_threshold` separately `open` (doc 24). The gateway
   threshold is one leg of the reserve's 2-of-2 script, so every "no single member" claim depends on
   it, and the choice interacts with shard count
4. **Leaver-shares — open, for finalisation.** A departing member retains a valid share of the reserve
   key, so the effective threshold degrades with churn. Key rotation (the reserve moves on-chain and
   `deposit_script` changes) or proactive re-sharing (needs the leaver's cooperation). Doc 23, *Open —
   for finalisation: leaver-shares*
4. **The vault design** — [`21-vault-structural.md`](21-vault-structural.md) is the current vault
   design and carries unfixed findings. It should be re-audited against this model, since several
   of its findings were caused by trying to enforce BSV-side behaviour that the federation now
   handles by different means
5. **The unbonding period and the challenge bounty share** — both are named as parameters above and
   neither has a default

---

Next: [Parameters & governance](06-parameters.md)
