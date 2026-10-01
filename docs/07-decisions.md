# 07. Decisions

**The register.** What is settled, what each decision defers, and every **reversal with its reason**.
Flows, mechanisms and open items live in [02. How it works](02-how-it-works.md),
[03. The federation](03-the-federation.md) and [06. Parameters](06-parameters.md); this page is the
short version, so it can be reviewed with fresh eyes.

**Nothing is settled unless its Status says so.**

> **Rewritten for the federation model.** The system is a light client, a **vault**, and a **bonded
> federation** holding the reserve under a **2-of-2 script with the Greycore**. Several earlier
> decisions are **reversed, not edited** — §Reversals records each with what it was, what it is now,
> and why. The reversals are the point of this register.

**Built or designed?** Built: the **light client with cw-144**, **`solBSV`**, the **mint**, **fork
staging**, the **nullifier**, the **timelocked authority** and **the vault** — 27 instructions, 77
passing / 0 failing. **Designed, not built: the federation, the Greycore, peg-out and governance.**
**The vault's protective window is a stored parameter at 144 BSV blocks (~24 hours)**, so a
followed reorg has a window in which it can be reversed.

---

## The flows

Both directions have the same shape: **enter the program's vault, then leave it either to the
counterparty or back to the sender.** No failure path mints; every one returns. Full detail in
[02. How it works](02-how-it-works.md).

### Peg-in — BSV → `solBSV`

```
1  CHOOSE   terms; the fee is 30 bp, governed
2  SEND     BSV to the federation's deposit script
            OP_RETURN = version ‖ cluster_id ‖ program_hash ‖ flags ‖ recipient
3  DEPTH    12 confirmations (FLOOR = MIN_CONFIRMATIONS)
4  STAGE    solBSV is minted INTO THE VAULT, not to the depositor, and a record stores
            the block hash the deposit was proven against and maturity_at_deposit
5  MATURE   144 BSV blocks (~24 hours), a stored parameter
6  RELEASE  permissionless: the client is fresh, the tip has advanced past the deposit,
            AND the stored hash still matches
            → the vault releases to the recipient
            if the hash DIFFERS → the staged tokens burn, and the depositor keeps the
            BSV the reorg returned. Nobody loses
```

**Step 6 is decided by the program from its own headers.** `release_mint` and `burn_staged` are
permissionless, so no party's cooperation is ever required.

### Peg-out — `solBSV` → BSV

```
1  ESCROW    solBSV moves into the vault; a BSV destination and a deadline are set
2  ACCEPT    federation members sign payout intents individually, on Solana
3  PAY       once enough attributed intents exist, the threshold key signs and the
             Greycore co-signs the BSV payment
4  SETTLE    the payout is proved against the light client; the escrow burns
   or
4' CANCEL    permissionless after the deadline: the escrow returns to the holder
```

**Failure returns; it never mints.** Supply never changes. **Step 2 is the attribution mechanism:**
each member signs separately, so a member who signs two conflicting intents has produced their own
proof of guilt (D13). **Redemptions are never pausable** (D11, D12).

---

## What is settled

| | |
|---|---|
| **No oracles** | The program reacts only to BSV headers and Solana slots. External metrics — price, hashrate, reorg cost — are published and never consulted |
| **Minting has no trusted participant; the reserve is trusted and bounded** | The program verifies proof of work and inclusion itself, and the mint authority is a program PDA. The reserve is held by a threshold of bonded members — one explicit trust assumption, bounded by seizable bonds and by an exit that cannot be paused |
| **Reversibility without a freeze authority** | The vault is program-owned, so staged tokens can be burned or returned. No freeze authority, no Token-2022 hooks |
| **Time is chain-native** | BSV depth and block time from headers; Solana deadlines from slots |
| **Fees mature with the principal** | A fee withdrawable earlier than its mint would be an exit from maturity |
| **Fees are governed and paid pro rata to pledged stake** | One fee, 30 bp each way, governed (85% / 30 days) |
| **One reserve under a 2-of-2 `OP_CHECKMULTISIG`** | No gateway majority — and no Greycore — can move it alone. Two-sided bonds, neither inside the reserve: the float, with a coverage floor. The `solBSV` side is seized by the program; the BSV side by the members collectively |
| **Three security layers** | A **threshold signature** catches a minority moving funds; **individual attestations** catch a minority's equivocation; a **covenant** (research, later) would catch a colluding majority. None catches a consistent majority. "Double threshold" means **a key plus a paper trail**, not a stronger threshold |
| **Collusion accepted, transparency is the mitigation** | A colluding threshold can take the reserve; maximum loss is the non-member supply. Continuous publication of the reserve and supply converts a hidden theft into a visible one — an **early deliverable** |
| **Genesis** | Members post a **BSV-side bond**, so no `solBSV` needs to exist first. A capped, explicitly-unbonded first mint is a documented later option, not chosen |
| **Confirmed by test** | 77 on-chain tests / 0 failing, cw-144 324/324 real mainnet headers, **160 real mainnet headers through `push_header`**, and the burn path exercised at a non-zero maturity |
| **`MIN_CONFIRMATIONS = 12` and a 144-block maturity window protect a deposit** | The 12-confirmation delay is **prevention**; the 144-block (~24 hour) window is the time in which a followed reorg can be **reversed** |

---

## Reversals

Each row is a **reversal with a reason**, not a silent edit.

| # | Was | Now | Why |
|---|---|---|---|
| **R1** | **D7** — no governance in the PoC; parameters fixed in code | **D10** — 85% of pledged coins / 30 days / live signal, holding the **upgrade authority** | "No governance" left the upgrade authority as an unowned mint voucher (A5). Governance names who holds it and makes every use visible for 30 days |
| **R2** | **D2** — fees **discovered** on an order book, bids auto-filling | **A governed fee, 30 bp each way**, changed by 85% / 30 days | The book solved discovery and capacity allocation; a governed fee solves both more simply, and it removes the one subsystem that never received an adversarial review |
| **R3** | **Per-relayer deposit scripts and independent keys** — "do not pool the reserve" | **One reserve under a 2-of-2 script** (D9/D15) | Per-relayer isolation removed the single key but also removed the single reserve that can be attested to, and left no operator layer to detect, challenge or govern |
| **R4** | **D1** — specialists first; anyone-may-stake a phase-2 goal | **D9** — **Greycore-admitted** membership, **two-sided 1,000 BSV bonds (the float)** | The Greycore — the trusted, non-operational body — finds and admits replacement members, with objective bond requirements. The phase-2 deferral is gone because the gate is what made it necessary |
| **R5** | **D4** — `FLOOR` 12 blocks, **fixed in code**, change mechanism deferred | **12 blocks, a governed parameter** (D10) | The value stands; the named gap was that there was no way to change it. The floor itself is **not** made immutable — the exit window is the protection (D11) |
| **R6** | **D6** — a peg-in may proceed with no underwriter, explicitly allowed | **Superseded** — there is no per-deposit underwriter to be present or absent | Minting is permissionless; a deposit pays the federation's script and is backed by the reserve and the bonds. The residual exposure question is re-opened as an open item |
| **R7** | **O1** — same-asset yield: BSV stakers earn BSV, `solBSV` stakers earn `solBSV` | **Fees paid pro rata to pledged stake** | With one pooled reserve there are no two staking sides; there is one member set and one fee |
| **R8** | **Gate symmetry** — a pause must close both directions | **D12** — pause stops **mints only**; redemptions are never pausable | The earlier argument treated the exit as a risk to gate. The exit is what makes governance safe |
| **R9** | **A single bond**, `solBSV`, **inside the reserve**; the formula was `aggregate_bond ≥ k × non_bonded_supply` | **Two-sided bonds, neither inside the reserve** (D14); `bsv_bond ≥ k × (BSV held)` and `solbsv_bond ≥ k × (solBSV held)` as a **coverage floor** | The old formula was a fix for `B ≥ k × total`, which was unsatisfiable — bonded `solBSV` is itself supply, so it demanded `B ≥ B + H`. Its remaining flaw dissolves once the **mint-side bond is not `solBSV` at all**. The bond is the float; the `n/t` capacity arithmetic is **withdrawn** |
| **R10** | **"Threshold script"** over the reserve — as if a multisig script enforced the quorum | **REVERSED AGAIN: it IS a multisig script** (D15). The reserve is `OP_2 <gateway threshold key> <greycore key> OP_2 OP_CHECKMULTISIG`, and F10 is reversed | The earlier "threshold ECDSA key, not a script" conclusion assumed a reserve with **no second quorum**. With the Greycore co-signing, a multisig script is exactly what the design needs. The committed code now **accepts** the 71-byte reserve script. The *gateway* key is still threshold ECDSA, so its `t`/`n` changes remain a re-sharing |

---

## Decisions

All are settled for the PoC unless the Status says otherwise. Each records what was decided, and
separately what it defers — because several are **decisions to defer**, which is different from
leaving a question open.

| ID | Decision | Status |
|---|---|---|
| **D1** | Who may join — specialists first, anyone-may-stake phase 2 | ✅ **Reversed by R4** → **D9** |
| **D2** | Fees discovered on the order book, bids auto-approving | ✅ **Reversed by R2** → governed 30 bp |
| **D3** | Genesis — **G2, the vault-gated genesis mint** | ✅ **Shape stands; bootstrap decided by D16.** G2 answers how the first supply is backed, not how the first members bond |
| **D4** | `FLOOR` — 12 blocks | ✅ **Partially reversed by R5.** The value stands; it is a governed parameter in the design |
| **D5** | Bond multiple — `k = 1`, self-dealing accepted | ✅ **`k = 1` stands, but the formula is superseded by D14.** The maximum loss from a colluding threshold is the entire non-member supply |
| **D6** | A peg-in with no underwriter — allowed, explicitly | ✅ **Superseded by R6**; the residual is re-opened as an open item |
| **D7** | Governance — none in the PoC | ✅ **Reversed by R1** → **D10** |
| **D8** | The reserve invariant — monitored, not enforced | ✅ **Stands.** A threshold key changes who holds the reserve, not what a Solana program can see |
| **D9** | Membership — **open**, bonds required, Greycore-admitted | ✅ **Settled**; **updated by D14** — two-sided bonds, neither inside the reserve |
| **D10** | Governance — **85% of pledged coins / 30 days / live signal**, holds the upgrade authority | ✅ **Settled** |
| **D11** | The floor — **the exit, not immutability**; redemptions never pausable | ✅ **Settled** |
| **D12** | Pause — **mints only**, lower threshold, auto-lifts | ✅ **Settled** |
| **D13** | Slashing — **self-proving equivocation** on individually-signed intents | ✅ **Settled** |
| **D14** | Bonds — **two-sided**, one per direction, **neither inside the reserve** | ✅ **Settled.** Supersedes the single-bond formula (R9) |
| **D15** | Custody — **a 2-of-2 `OP_CHECKMULTISIG` reserve script**, the gateway threshold key plus the Greycore's | ✅ **Settled.** Reverses audit F10 (R10). The script's *shape* is accepted by the committed code; the keys behind it are not built |
| **D16** | Genesis — **BSV-side bond** | ✅ **Settled.** The capped unbonded first mint is recorded as a later option |

### D3 — Genesis: the vault-gated mint, and the BSV-side bond

The genesis mint lands in the program vault and is released only once a matching BSV deposit is
verified. No unbacked window exists at any point, so there is nothing to attack and nothing to keep
quiet about. **How the first members bond is D16:** a **BSV-side bond at genesis**, so no `solBSV`
needs to exist first. The alternative — a **capped, explicitly-unbonded first mint** — is recorded as
a documented later option, not chosen, because it leaves the first mint backed by nothing but the
members' word.

### D4 — `FLOOR` — 12 blocks

`FLOOR` is the minimum confirmation depth, now a **governed parameter** rather than a constant. Depth
and maturity do different jobs: depth sets **the cost of attacking** — a reorg must out-mine it —
while maturity sets **the time available to detect**. A low floor makes attacks cheap and therefore
frequent, which raises the number of chances for a detection failure to slip through. **In the built
code it remains `MIN_CONFIRMATIONS = 12`, with no setter.**

### D5 — Bond multiple — `k = 1`, under the two-sided formula

**`k = 1` stands, but the single-bond formula is superseded by D14.** The cover is two-sided:
`bsv_bond ≥ k × (BSV held)` and `solbsv_bond ≥ k × (solBSV held)`, with neither bond inside the
reserve. The earlier `aggregate_bond ≥ k × non_bonded_supply` — itself a fix for the unsatisfiable
`B ≥ k × total` — is **superseded**.

**The consequence is restated rather than carried over:** the maximum loss from a colluding threshold
is the **entire non-member supply**. The old `B_h + H` figure and its 1.15× / 1.49× multiples were
computed for the single-bond model, where the bonds sat inside the reserve. What makes collusion
unattractive is the live signal, the exit, and **transparency** — not the bond's excess size.

### D8 — The reserve invariant — monitored, not enforced

`custodied BSV ≥ outstanding solBSV` is published and monitored, and **the protocol cannot enforce
it** — the reserve is off-chain BSV the program cannot read. **Promoted from a monitoring task to an
early deliverable:** for collusion and for an unchallenged outpoint spend, visibility is the only
defence that remains.

### D9 — Membership — the Greycore admits; two-sided bonds

**The Greycore finds and admits replacement members.** There are **two bonds, one per direction**: a
BSV-side bond held outside the reserve, and a `solBSV`-side bond seizable on Solana. **Neither sits
inside the reserve**, and a member cannot leave while owing on either side. **The bond is the float** —
working capital that lets the federation serve redemptions — not a capital requirement sized against
the reserve, and not a scale limit. The earlier "total value locked is capped by bonds pledged" claim,
and its `~$180k` figure, are **withdrawn**: the reserve is constrained by the **Greycore co-signature**.
**Leaver-shares are an open finalisation item.**

### D10 — Governance — 85% / 30 days / live signal

| | Default |
|---|---|
| Who may propose | Any member |
| To pass | **85% of pledged coins** |
| Delay | **30 days** |
| Delay floor | `gov.delay_min`, **`open`** — 7 days proposed |
| Signal | **Live from the moment it is raised** |
| Includes | **The upgrade authority** |
| Cannot touch | **Redemptions. They are never pausable** |

All of those are **parameters**, not constants.

### D11 — The floor is the exit, not immutability

**There is no immutable floor, deliberately.** A hostile change needs 85% *and* 30 days, and
redemptions run throughout — so a proposal that would harm holders **empties the bridge before it
lands.** **The residual, stated plainly:** a holder who does not watch and does not act within the
delay is exposed. That is a disclosure obligation, not a mechanism.

### D12 — Pause stops mints only

**Mints can be paused. Redemptions cannot.** Pausing inbound is a safety valve; pausing outbound is
taking hostages. The power is bounded and lifts automatically, so pause carries a lower threshold than
a governance change.

### D13 — Slashing is self-proving equivocation

You cannot deduce who was at fault from an opaque threshold signature, **so the design does not try
to.** Members sign individually, so misbehaviour produces its own evidence:

| Misbehaviour | Provable? |
|---|---|
| A member signs **two conflicting payout intents** | **Yes — self-proving.** Two signatures, one member, conflicting statements. Anyone submits it; anyone can be paid the bounty |
| A **threshold** of members signs something invalid | Attributable, since every signature is on record — but a governance matter, not a cryptographic one |

**One row was deleted, not corrected: "an intent matching no authorised redemption."** A *closed*
`PegOut` is indistinguishable from one that never existed, so the program cannot decide the predicate,
and checking it would false-positive against an honest member who attested before a cancel.

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
move it the moment they are caught, or before they act, and there is nothing to slash. **Slashing pays
the slashers from the seized bond**, which is what makes the collective action happen.

**Two residuals, stated honestly.** (1) A **majority could seize an honest member's bond** — the
symmetric risk of collective custody, resting on the same majority already trusted with the reserve.
(2) The **obligation to slash is social, not on-chain**: nothing on BSV compels the members to sign. It
is a **collective action by the majority**, not an automatic rule.

### D15 — A 2-of-2 reserve script: the gateway threshold key plus the Greycore

**The reserve script is a 2-of-2 `OP_CHECKMULTISIG`:**

```
OP_2  <gateway threshold key>  <greycore key>  OP_2  OP_CHECKMULTISIG
```

One leg is the gateway's **threshold ECDSA** key, whose shares are never assembled in one place and
which emits **one** signature however many members signed; the other is the **Greycore**'s key. **Both
must sign**, so the gateway majority cannot move funds alone, the Greycore cannot move funds alone, and
**the Greycore polices every reserve spend.** **The Greycore is trusted third parties, not node
operators** — people with reputations to lose, who do not run the reserve.

**This reverses F10 (and R10).** The earlier decision said the code was right and the description was
wrong, because the reserve was presumed to be a single-key P2PKH address. With a Greycore the deposit
script genuinely **is** a multisig, and the committed code now accepts it:

- `is_p2pkh` is **no longer the only accepted script**: `is_acceptable_deposit_script` accepts either a
  25-byte P2PKH or the 71-byte 2-of-2 reserve script (`is_reserve_multisig`);
- `MAX_SCRIPT_LEN = 71` and `DepositScript::SPACE = 84` (8 discriminator + 4 length + 71 + 1);
- The **gateway** key is still threshold ECDSA, so changing its `t` or `n` is a **re-sharing**.
  **Changing a Greycore key changes the deposit script**, so that change does move the reserve;
- **`fed.threshold` = 4-of-N**, with `N` a variable and the number deferred; `fed.greycore_size` and
  `fed.greycore_threshold` are separately `open`.

**And leaver-shares are an open finalisation item.** A departing member retains a valid share, so the
effective threshold degrades with churn — at 4-of-N, four former members together hold four valid
shares. Key rotation (the reserve moves and `deposit_script` changes) or proactive re-sharing (needs
the leaver's cooperation). The PoC deliberately does not finalise it.

### D16 — Genesis: a BSV-side bond

**Decided: members post a BSV-side bond at genesis**, so no `solBSV` needs to exist first. The
**alternative** — a **capped, explicitly-unbonded first mint** — is recorded as a documented later
option, with the reason it was not chosen: it leaves the first mint backed by nothing but the members'
word.

---

## Not settled, and deliberately out of scope for the PoC

| | |
|---|---|
| **Sharding the threshold key** | One key across all members, or several groups with their own? Shards contain theft and signing latency, at the cost of coordination. **There is no sharding today: blast radius 100%** |
| **The threshold key's shape** | Key generation and signing protocol for the gateway threshold ECDSA key and for the Greycore's key, and the attribution rule that turns system-wide exposure into a per-member share. `fed.threshold` is `4-of-N` with `N` deferred; `fed.greycore_size` / `fed.greycore_threshold` are `open` |
| **Leaver-shares** | A departing member retains a valid share, so the effective threshold degrades with churn. **Key rotation** (the reserve moves and `deposit_script` changes) or **proactive re-sharing** (needs the leaver's cooperation). The PoC deliberately does not finalise it |
| **The reported spent-outpoint record** | Chosen over the permissionless alternative; the **format and write path are unspecified and it is not built** |
| **The unbacked exposure bound** | Whether an explicit cap is wanted on reserve exposure before the bond set is large enough (the residual of R6/D6). **No numeric capacity rule has replaced the superseded arithmetic** |
| **The refusal hole** | A threshold that declines to attest leaves no signed artifact, so nothing is slashable. A timeout-and-rotate rule is not in the design |
| **Independent audit** | The critical defects found so far were found by our own adversarial review, which is not the same as an audit by someone with no stake in the answer |

## Deferred to final implementation

These are named so they cannot be quietly forgotten. None blocks the PoC.

| Deferred | From | Note |
|---|---|---|
| The unbonding period | D9 | Longer than the redemption deadline plus the challenge window; the value is a parameter |
| X3 — the hard-coded DAA | W1/A1 | cw-144 is implemented and verified 324/324, but BSV may change the rule. Governance can carry the upgrade; a parameterisable rule is not built |
| The program upgrade authority | D10/A5 | Now held by governance *by design*. The residual is the threshold over a small bond set, and in the built code it remains one timelocked key |
| The aggregate mint cap | P5/A10 | Set by policy rather than derived; not implemented |
| The burn bounty | — | Suggested, not sized; `fee.bounty_share` is `open` |
| Fee realisation mechanics | D2/O1 | Whether members withdraw from their own balance or accrue a claim is unresolved |
| An independent audit | — | — |
