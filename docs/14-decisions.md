# 14. Decision register

**Read this first.** [`13-summary.md`](13-summary.md) is the canonical model. This page is the
short version: the flows as they stand, what is settled, and every decision still open — in one
place, so it can be reviewed with fresh eyes. [`12-peg-mechanism.md`](12-peg-mechanism.md) holds
the full reasoning and the audit findings; [`23-federation.md`](23-federation.md) holds
membership, governance and slashing. **Nothing is settled unless its Status says so.**

> **Rewritten for the federation model, 2026-09-29.** The system is now a light client, a
> **vault**, and a **bonded federation** holding the reserve under a **threshold key**. Several
> earlier decisions are **reversed, not edited** — §Reversals records each with what it was, what
> it is now, and why. The reversals are the point of this register.

**Built or designed?** Built: the **light client with cw-144**, the **`solBSV` token**, the
**mint**, and **fork staging**. Everything else is designed and does not exist in code — the vault,
the federation, governance, slashing and all of peg-out. The shipped mint goes straight to the
depositor's token account and charges no fee.

---

## The flows

**As designed** — the vault and everything downstream of it are not built. Both directions have
the same shape: **enter the program's vault, then leave it either to the counterparty or back to
the sender.** No failure path mints; every one returns.

### Peg-in — BSV → solBSV

```
1  CHOOSE   terms; the fee is 30 bp, governed
2  SEND     BSV to the federation's deposit script
            OP_RETURN = version ‖ cluster_id ‖ program_hash ‖ flags ‖ recipient
3  DEPTH    12 confirmations (FLOOR)
4  STAGE    solBSV is minted INTO THE VAULT, not to the depositor, and a record stores
            the block hash the deposit was proven against
5  MATURE   144 blocks
6  RELEASE  permissionless: the tip has advanced past the deposit AND the stored hash
            still matches -> the vault releases to the recipient
            if the hash DIFFERS -> the staged tokens burn, and the depositor keeps the
            BSV the reorg returned. Nobody loses
```

**Step 6 is decided by the program from its own headers.** `release_mint` and `burn_staged` are
permissionless, so no party's cooperation is ever required.

### Peg-out — solBSV → BSV

```
1  ESCROW    solBSV moves into the vault; a BSV destination and a deadline are set
2  ACCEPT    federation members sign payout intents individually, on Solana
3  PAY       once enough attributed intents exist, the threshold key signs the BSV payment
4  SETTLE    the payout is proved against the light client; the escrow burns
   or
4' CANCEL    permissionless after the deadline: the escrow returns to the holder
```

**Failure returns; it never mints.** Supply never changes. **Step 2 is the attribution
mechanism:** each member signs separately, so a member who signs two conflicting intents has
produced their own proof of guilt (D13). **Redemptions are never pausable** (D11, D12).

---

## What is settled

| | |
|---|---|
| **No oracles** | The program reacts only to BSV headers and Solana slots. External metrics — price, hashrate, reorg cost — are published and never consulted by the program |
| **Minting has no trusted participant; the reserve is trusted and bounded** | The program verifies proof of work and inclusion itself, and the mint authority is a program PDA. The reserve is held by a threshold of bonded members — one explicit trust assumption, bounded by seizable bonds and by an exit that cannot be paused. *(Replaces "No operators": there is no privileged operator for minting, but there is now an operator class holding the reserve)* |
| **Reversibility without a freeze authority** | The vault is program-owned, so staged tokens can be burned or returned. No freeze authority, no Token-2022 hooks |
| **Time is chain-native** | BSV depth and block time from headers; Solana deadlines from slots |
| **Fees mature with the principal** | A fee withdrawable earlier than its mint would be an exit from maturity |
| **Fees are governed and paid pro rata to pledged stake** | One fee, 30 bp each way, governed (85% / 30 days). *(Replaces "Same-asset yield": with per-relayer custody removed there are no two staking sides to pay separately)* |
| **One reserve under a threshold ECDSA key** | No single member can move it: the address is an ordinary **P2PKH** address and the key is **never assembled in one place**. **Two-sided bonds, neither inside the reserve** — the `solBSV` side is seized by the **program** automatically, the BSV side by the **members collectively** under the same collective key (not the member's own). *(Reverses "Do not pool the reserve"; corrects audit F10)* |
| **Three security layers** | A **threshold signature** catches a minority moving funds; **individual attestations** catch a minority's equivocation; a **covenant** (research, later) would catch a colluding majority. None catches a consistent majority. "Double threshold" means **a key plus a paper trail**, not a stronger threshold |
| **Collusion accepted, transparency is the mitigation** | A colluding threshold can take the reserve; maximum loss is the non-member supply. Continuous publication of the reserve and supply converts a hidden theft into a visible one — an **early deliverable**, not phase-5 monitoring (doc 07) |
| **Genesis** | Members post a **BSV-side bond**, so no `solBSV` needs to exist first. A capped, explicitly-unbonded first mint is a documented later option, not chosen |
| **Confirmed by test** | 34 on-chain tests, Phase 1A 51/51 synthetic, 21/21 against a live SV Node, cw-144 324/324 real mainnet headers, **160 real mainnet headers through `push_header`** |

---

## Reversals

Each row is a **reversal with a reason**, dated **2026-09-29**, not a silent edit. The full
reasoning is in [`12-peg-mechanism.md`](12-peg-mechanism.md) §Changes in this revision and in
[`23-federation.md`](23-federation.md).

| # | Was | Now | Why |
|---|---|---|---|
| **R1** | **D7** — no governance in the PoC; parameters fixed in code | **D10** — 85% of pledged coins / 30 days / live signal, holding the **upgrade authority** | "No governance" left the upgrade authority as an unowned mint voucher (A5). Governance names who holds it and makes every use visible for 30 days |
| **R2** | **D2** — fees **discovered** on an order book, bids auto-filling | **A governed fee, 30 bp each way**, changed by 85% / 30 days | The book solved discovery and capacity allocation; a governed fee plus a bond cap solves both more simply. It also removes the one subsystem that never received an adversarial review |
| **R3** | **Per-relayer deposit scripts and independent keys** — "do not pool the reserve" | **One reserve under a threshold key** (D9) | Per-relayer isolation removed the single key but also removed the single reserve that can be attested to, and left no operator layer to detect, challenge or govern. A threshold key means no single member can move funds |
| **R4** | **D1** — specialists first; anyone-may-stake a phase-2 goal | **D9** — open membership, **two-sided 1,000 BSV bonds** | A capital gate is objective, seizable and permissionless; nomination is a trusted choice. The phase-2 deferral is gone because the gate is what made it necessary |
| **R5** | **D4** — `FLOOR` 12 blocks, **fixed in code**, change mechanism deferred | **12 blocks, a governed parameter** (D10) | The value stands; the named gap was that there was no way to change it. The floor itself is **not** made immutable — the exit window is the protection (D11) |
| **R6** | **D6** — a peg-in may proceed with no underwriter, explicitly allowed | **Superseded** — there is no per-deposit underwriter to be present or absent | Minting is permissionless and trustless; a deposit pays the federation's script and is backed by the reserve and the bonds. The residual exposure question is re-opened as an open item |
| **R7** | **O1** — same-asset yield: BSV stakers earn BSV, `solBSV` stakers earn `solBSV` | **Fees paid pro rata to pledged stake** | With one pooled reserve there are no two staking sides; there is one member set and one fee |
| **R8** | **Gate symmetry** — a pause must close both directions | **D12** — pause stops **mints only**; redemptions are never pausable | The earlier argument treated the exit as a risk to gate. The exit is what makes governance safe: it is the protection, and 30 days of live signal is what makes a hostile change empty the bridge before it lands |
| **R9** | **A single bond**, `solBSV`, **inside the reserve**; the formula was `aggregate_bond ≥ k × non_bonded_supply` | **Two-sided bonds, neither inside the reserve** (D14; doc 13, *The two bonds*): `bsv_bond ≥ k × (BSV held)` and `solbsv_bond ≥ k × (solBSV held)` | The old formula was a fix for `B ≥ k × total`, which was unsatisfiable — bonded `solBSV` is itself supply, so it demanded `B ≥ B + H`. Its remaining flaw dissolves once the **mint-side bond is not `solBSV` at all**. The `solBSV` side is seized by the **program** automatically; the BSV side sits under the **collective key** and is seized by the **members collectively**, with the slashers paid from it. **Superseded** |
| **R10** | **"Threshold script"** over the reserve — as if a multisig script enforced the quorum | **Threshold ECDSA key** (D15; audit **F10**): the reserve is an ordinary P2PKH address and the key is never assembled in one place | **The code was right; the description was wrong.** With a multisig, adding or removing a member changes the script and the whole reserve must be swept on-chain, requiring the old quorum. With threshold ECDSA it is a **re-sharing** — the reserve never moves and the address never changes. `fed.threshold` sizes nothing on-chain |

---

## Decisions

All are settled for the PoC unless their Status says otherwise. Each records what was decided, and
separately what it defers — because several are **decisions to defer**, which is different from
leaving a question open.

| ID | Decision | Status |
|---|---|---|
| **D1** | Who may join — specialists first, anyone-may-stake phase 2 | ✅ **Reversed by R4** → **D9** |
| **D2** | Fees discovered on the order book, bids auto-approving | ✅ **Reversed by R2** → governed 30 bp |
| **D3** | Genesis — **G2, the vault-gated genesis mint** | ✅ **Shape stands; bootstrap reopened.** G2 answers how the first supply is backed, not how the first members bond |
| **D4** | `FLOOR` — 12 blocks | ✅ **Partially reversed by R5.** The value stands; it is a governed parameter now |
| **D5** | Bond multiple — `k = 1`, self-dealing accepted | ✅ **`k = 1` stands, but the formula is superseded by D14.** The old single-bond cover (`aggregate_bond ≥ k × non_bonded_supply`) is replaced by two-sided bonds. The `k = 1` collusion-loss consequence is restated: the maximum loss is the entire non-member supply |
| **D6** | A peg-in with no underwriter — allowed, explicitly | ✅ **Superseded by R6**; residual re-opened as an open item |
| **D7** | Governance — none in the PoC | ✅ **Reversed by R1** → **D10** |
| **D8** | The reserve invariant — monitored, not enforced | ✅ **Stands.** A threshold key changes who holds the reserve, not what a Solana program can see |
| **D9** | Membership — **open**, bonds required | ✅ **Settled**, new; **updated by D14** — two-sided bonds, neither inside the reserve |
| **D10** | Governance — **85% of pledged coins / 30 days / live signal**, holds the upgrade authority | ✅ **Settled**, new |
| **D11** | The floor — **the exit, not immutability**; redemptions never pausable | ✅ **Settled**, new |
| **D12** | Pause — **mints only**, lower threshold, auto-lifts | ✅ **Settled**, new |
| **D13** | Slashing — **self-proving equivocation** on individually-signed intents | ✅ **Settled**, new |
| **D14** | Bonds — **two-sided**, one per direction, **neither inside the reserve** | ✅ **Settled**, new. Supersedes the single-bond formula (R9) |
| **D15** | Custody — **threshold ECDSA**, not a multisig script | ✅ **Settled**, new. Corrects audit F10 (R10) |
| **D16** | Genesis — **BSV-side bond** | ✅ **Settled**, new. The capped unbonded first mint is recorded as a later option |

### D3 — Genesis: the vault-gated mint, and the BSV-side bond

The genesis mint lands in the program vault and is released only once a matching BSV deposit is
verified. No unbacked window exists at any point, so there is nothing to attack and nothing to keep
quiet about. **The federation model's answer to *how the first members bond* is now decided (D16):**
members post a **BSV-side bond at genesis**, so no `solBSV` needs to exist first. The alternative — a
**capped, explicitly-unbonded first mint** — is recorded as a documented later option, not chosen,
because it leaves the first mint backed by nothing but the members' word.

### D4 — `FLOOR` — 12 blocks

`FLOOR` is the minimum confirmation depth. Twelve blocks for the PoC, now a **governed parameter**
rather than a constant. Depth and maturity are different parameters: depth sets **the cost of
attacking** — a reorg must out-mine it — while maturity sets **the time available to detect**. A
low floor makes attacks cheap and therefore frequent, which raises the number of chances for a
detection failure to slip through.

### D5 — Bond multiple — `k = 1`, under the two-sided formula

**`k = 1` stands, but the single-bond formula is superseded by D14.** The cover is now two-sided:
`bsv_bond ≥ k × (BSV held)` and `solbsv_bond ≥ k × (solBSV held)`, with **neither bond inside the
reserve**. The earlier statement `aggregate_bond ≥ k × non_bonded_supply` — itself a fix for the
unsatisfiable `B ≥ k × total` — is **superseded**: bonded `solBSV` was part of the supply it was
meant to cover, which the BSV-side bond now avoids by not being `solBSV` at all.

**The consequence is restated rather than carried over:** the maximum loss from a colluding
threshold is the **entire non-member supply**. The old `B_h + H` figure and its 1.15× / 1.49×
multiples were computed for the single-bond model, where the bonds sat inside the reserve; the
two-sided bonds supersede that arithmetic. What makes collusion unattractive is the live signal, the
exit, and **transparency** — not the bond's excess size. The bonds' other job is covering a member's
provable misbehaviour and abandonment.

### D8 — The reserve invariant — monitored, not enforced

`custodied BSV ≥ outstanding solBSV` is published and monitored, and **the protocol cannot enforce
it** — the reserve is off-chain BSV the program cannot read. The website shows the ratio; the
program does not check it. **Under D14/D16 this is promoted from a monitoring task to an early
deliverable** (doc 07): for collusion and for an unchallenged outpoint spend, visibility is the only
defence that remains.

### D9 — Membership — open, two-sided bonds

**Anyone who posts the bonds may join.** There are **two bonds, one per direction** (D14): a
**BSV-side bond** held outside the reserve and sized against the BSV held, and a **`solBSV`-side
bond**, seizable on Solana, sized against the `solBSV` held. **Neither sits inside the reserve**, and
a member cannot leave while owing on either side. Leaving requires announcing and waiting the
unbonding period. Fees are earned pro rata to pledged stake. **The bond sizes are the scale limit,
and that is stated rather than implied:** with `k = 1`, total value locked is capped by total bonds
pledged — ten members at 1,000 BSV is roughly **~$180k** of capacity. That is a proof of concept.

### D10 — Governance — 85% / 30 days / live signal

| | Default |
|---|---|
| Who may propose | Any member |
| To pass | **85% of pledged coins** |
| Delay | **30 days** |
| Signal | **Live from the moment it is raised** |
| Includes | **The upgrade authority** |
| Cannot touch | **Redemptions. They are never pausable** |

All of those are **parameters**, not constants.

### D11 — The floor is the exit, not immutability

**There is no immutable floor, deliberately.** A hostile change needs 85% *and* 30 days, and
redemptions run throughout — so a proposal that would harm holders **empties the bridge before it
lands.** The protection was never that the rules are frozen; it is that you can always leave before
they change. **The residual, stated plainly:** a holder who does not watch and does not act within
30 days is exposed. That is a disclosure obligation, not a mechanism.

### D12 — Pause stops mints only

**Mints can be paused. Redemptions cannot.** Pausing inbound is a safety valve; pausing outbound is
taking hostages. The power is bounded and lifts automatically, so a pause carries a lower threshold
than a governance change rather than waiting 30 days for an emergency.

### D13 — Slashing is self-proving equivocation

You cannot deduce who was at fault from an opaque threshold signature, **so the design does not
try to.** Members sign individually, so misbehaviour produces its own evidence:

| Misbehaviour | Provable? |
|---|---|
| A member signs **two conflicting payout intents** | **Yes — self-proving.** Two signatures, one member, conflicting statements. Anyone submits it; anyone can be paid the bounty |
| A **threshold** of members signs something invalid | Attributable, since every signature is on record — but a governance matter, not a cryptographic one |

**One row was deleted, not corrected: "an intent matching no authorised redemption."** A *closed*
`PegOut` is indistinguishable from one that never existed, so the program cannot decide the
predicate, and checking it would false-positive against an honest member who attested before a
cancel. There is no enforceable predicate for an off-chain threshold signature over BSV (audit F8).

**Copied from what RenVM actually shipped.** Only the cryptographic half was ever built; RenVM's own
documentation says the slashing contract *"will become a voting system … Right now, it is a
placeholder."* We copy the half that shipped and say plainly that the rest has no precedent.

### D14 — Two-sided bonds

**Two bonds, one per direction, each denominated in the asset that side holds, and neither inside the
reserve:**

```
mint side     bsv_bond    ≥ k × (BSV held in the reserve)     BSV, OUTSIDE the reserve
redeem side   solbsv_bond ≥ k × (solBSV held)                 solBSV, seizable on Solana
```

This replaces the single-bond formula `aggregate_bond ≥ k × non_bonded_supply` (R9).

| Bond | Where | Who seizes | How |
|---|---|---|---|
| `solBSV` (redeem side) | Solana | **The program** | An instruction, on proof — automatic |
| **BSV** (mint side) | A BSV script under the **collective key** | **The members collectively** | A threshold-signed transaction moving the bond |

**The design requirement that makes the BSV side real:** each member's bond must sit under the
**collective (threshold ECDSA) key, not the member's own.** If a member controls their own bond they
move it the moment they are caught, or before they act, and there is nothing to slash. Under the
collective key a member **cannot** move their own bond and the federation **can** — the same
primitive as the reserve, pointed at the bond. **Slashing pays the slashers from the seized bond**,
which is what makes the collective action happen.

**Two residuals, stated honestly.** (1) A **majority could seize an honest member's bond** — the
symmetric risk of collective custody, resting on the same majority already trusted with the reserve.
(2) The **obligation to slash is social, not on-chain**: nothing on BSV compels the members to sign,
so it rests on the majority being honest, on visibility, and on the bounty. It is a **collective
action by the majority**, not an automatic rule.

### D15 — Threshold ECDSA, not a multisig script

**The reserve address is an ordinary P2PKH address, and the key is a threshold ECDSA key whose shares
are never assembled in one place.** This corrects audit **F10** (R10): the code was right and the
description was wrong.

- `is_p2pkh` requiring a 25-byte P2PKH script is **correct**; `DepositScript::SPACE = 38` is
  **correctly sized**
- **"No single member can move funds" is true because the key is shared**, not because a script
  enforces it
- With a multisig, adding or removing a member changes the script and the whole reserve must be swept
  on-chain, requiring the old quorum. With threshold ECDSA it is a **re-sharing** — the reserve never
  moves and the address never changes
- **`fed.threshold` sizes nothing on-chain.** Provisional **`3-of-5`**, marked `open` (doc 24). It is a
  **signing-protocol** parameter, and it is the number every "no single member" claim depends on

### D16 — Genesis: a BSV-side bond

**Decided: members post a BSV-side bond at genesis**, so no `solBSV` needs to exist first. The
**alternative** — a **capped, explicitly-unbonded first mint** — is recorded as a documented later
option, with the reason it was not chosen: it leaves the first mint backed by nothing but the
members' word.

---

## Not settled, and deliberately out of scope for the PoC

| | |
|---|---|
| **Sharding the threshold key** | One key across all members, or several groups with their own? Shards contain theft and signing latency, at the cost of coordination |
| **The threshold key's shape** | Key generation and signing protocol for the **threshold ECDSA** key, and the attribution rule that turns system-wide `owed` into a per-member share. The *value* is provisional `3-of-5` (`open`) |
| **The vault's re-audit** | The current vault design carries unfixed findings and should be re-audited against this model, since several were caused by trying to enforce BSV-side behaviour the federation now handles differently |
| **The unbacked exposure bound** | Whether an explicit cap is wanted on reserve exposure before the bond set is large enough (the residual of R6/D6) |
| **Independent audit** | The critical defects found so far were found by our own adversarial review, which is not the same as an audit by someone with no stake in the answer |

## Deferred to final implementation

These are named so they cannot be quietly forgotten. None blocks the PoC.

| Deferred | From | Note |
|---|---|---|
| The unbonding period | D9 | Longer than the redemption deadline plus the challenge window; the value is a parameter |
| X3 — the hard-coded DAA | W1/A1 | cw-144 is implemented and verified 324/324, but BSV may change the rule. Governance can carry the upgrade; a parameterisable rule is not built |
| The program upgrade authority | D10/A5 | Now held by governance. The residual is the 85% threshold over a small bond set |
| The aggregate mint cap | P5/A10 | Set by policy rather than derived; not implemented |
| Fee realisation mechanics | D2/O1 | Whether members withdraw from their own balance or accrue a claim is unresolved |
| An independent audit | — | The critical defects found so far were found by our own adversarial review |

## Where to read more

- [`12-peg-mechanism.md`](12-peg-mechanism.md) — full reasoning, scenarios, audit findings A1–A18
- [`13-summary.md`](13-summary.md) — the canonical model
- [`23-federation.md`](23-federation.md) — membership, governance, slashing
- [`18-pre-code-checklist.md`](18-pre-code-checklist.md) — P1–P11 status against the new model
- [`04-trust-model.md`](04-trust-model.md) — what is trusted, and the roadmap to a signerless reserve
