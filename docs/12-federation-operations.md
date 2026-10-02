# 12. Federation operations

[03. The federation](03-the-federation.md) is the **design**: why a federation, the reserve script,
governance, slashing, and the RenVM comparison. This is the **operational** companion. It is written
for two readers: someone who might **be** a member, and someone assessing whether to **trust** one. It
covers what a member is expected to do, how a member joins and leaves, what the economics are, what a
member is exposed to — and, in the same voice, how much of any of it exists.

**What this chapter is not.** It is not a second design document. Where 03 argues for a mechanism, this
one asks only: *is it built, is it enforced, and what happens to a member who does not do it?* For most
of the federation the answer is that it is neither built nor enforced, and saying so is the point.

**Status: designed, not built — all of it.** The built program is **27 instructions and 79 passing
tests**, and **none of the federation exists in it**: no membership account, no bond, no Greycore, no
reserve script keys, no governance, no slashing (§8). Everything in this chapter is therefore an
**expectation of how the system is supposed to work**, not a description of a system that runs. The
instruction count was taken by counting the handlers in `poc/solana/programs/solbeam/src/lib.rs` and the
test count from the suite, in this review — neither was copied from another document. The suite's
recorded run is **79 passing / 0 failing**.

---

## 1. Who the members are, and what they do

A member is one seat in the **gateway threshold key** — the `4-of-N` key that is one leg of the
reserve's 2-of-2 script — plus a bonded operator of the node software. The intended jobs are in
[03 §2](03-the-federation.md#2-membership). Split into **obligations** and **expectations**:

| Job | Nature | Enforced today? |
|---|---|---|
| **Hold a share of the reserve key** | Obligation of the seat — it *is* the seat | **No.** No key exists, and no share has been issued |
| **Sign reserve movements** (the threshold leg of a payout) | Obligation — a payout cannot happen without `t` of `n` | **No.** No signing protocol, no reserve, no key |
| **Serve peg-outs** — pay BSV from the reserve when a redemption is due | Obligation — the design says a peg-out requires a member to act ([11 §6](11-peg-out-spec.md#6-what-this-does-not-solve)) | **No.** `initiate_redeem`/`cancel_redeem`/`claim_payout`/`settle_redeem` are built, but **membership — who is obliged to pay — is not** ([11 §7](11-peg-out-spec.md#7-build-order)) |
| **Run a BSV node** | Expectation — how the member sees the chain it reports on | **No.** No node software exists |
| **Run a Solana node** | Expectation — how the member watches the programs and signs intents | **No.** No node software exists |
| **Watch, verify independently, relay headers** | Expectation — permissionless and unpaid in itself ([03 §2](03-the-federation.md#2-membership)) | **No.** The instruction to relay exists; no obligation to call it does |

**"Obligation" here means the design requires it; it is not an enforceable duty.** There is no
membership account that records a member, no bond the program can seize for failing at any of these,
and no instruction that can name a member at all. The distinction between obligations and expectations
is a distinction in the intended design. **In the built system it has no consequence, because nothing
is enforced.**

The one duty that *is* on-chain in the design is the **spent-outpoint report** ([03 §3](03-the-federation.md#3-the-reserve-script-threshold-ecdsa-plus-a-greycore-co-signature)):
Solana cannot read the BSV UTXO set, so the software reports which deposit outpoints the reserve has
spent. In the built program that report exists as `report_spent`, but its signer is the program's
upgrade authority, not a federation (§8).

---

## 2. Joining

**The intended path**, in order:

1. **The Greycore vets and admits.** The Greycore — the trusted, non-operational body — *"finds and
   admits replacement members"* ([03 §2](03-the-federation.md#2-membership), [07 D9](07-decisions.md#d9--membership--the-greycore-admits-two-sided-bonds)). Entry has a capital gate, so the
   set is permissioned but not anonymous.
2. **The bond is posted** — two-sided, one per direction, each in the asset its side holds, neither
   inside the reserve (§5). At genesis the member posts the **BSV-side** bond, so no `solBSV` needs to
   exist first ([07 D16](07-decisions.md#d16--genesis-a-bsv-side-bond)).
3. **The key share is issued** — the new member becomes a participant in the gateway's threshold ECDSA
   key, which is one leg of the reserve script ([03 §3](03-the-federation.md#3-the-reserve-script-threshold-ecdsa-plus-a-greycore-co-signature)).

**Then the specification stops.** Three things a member or an applicant needs are **not specified**:

- **The re-sharing ceremony.** Adding a holder to a threshold key is a re-sharing, and the ceremony —
  key generation, share distribution, verification — **is not specified anywhere**. [03 §11](03-the-federation.md#11-open-items) records
  it as open: *"`N`, the admission ceremony and the key-generation and signing protocol are not
  specified."* Not specified.
- **Who may be admitted.** The design says the Greycore admits and that there are *"objective bond
  requirements"* ([07 R4](07-decisions.md#reversals)). It does not say what else qualifies or
  disqualifies an applicant — competence, jurisdiction, conflicts — because nothing does. **Not
  specified.**
- **Whether admission can be compelled.** Nothing in the design obliges the Greycore to admit anyone,
  or obliges existing members to accept a re-sharing with a new holder. **Not specified.** A refusal is
  unobservable: no artifact is produced by declining.

**And none of it is built.** There is no admission instruction, no bond account, no key generation and
no re-sharing. A party who wanted to join today would have nothing to join.

---

## 3. Leaving, and the sharpest open problem

**The intended path** is short: announce, wait the unbonding period, withdraw — but only if the bonds
still cover what their sides hold. *"A bond withdrawable on demand is not a bond"* ([05 invariant
7](05-trust-model.md#core-invariants)); a **member cannot leave while owing**. The unbonding period
`fed.unbond_slots` is `open` and *"must exceed the redemption deadline plus the challenge window"*
([06](06-parameters.md#federation)).

**The problem is what the departing member keeps.** A member who leaves — or is removed, or is
compromised — **retains a valid share of the reserve key**, because nothing invalidates it:

- the threshold is a property of the **current** member set only in name;
- the **effective threshold degrades with churn**;
- at `4-of-N`, **four former members together still hold four valid shares**, and can sign as if they
  were still seated.

This is a **stated omission, not a design**. [03 §7](03-the-federation.md#what-we-deliberately-leave-out) records it as deliberately left out and
*"the sharpest open problem in the federation design"*; [08 §5](08-status-and-roadmap.md#5-the-honest-limitations) states it as a limitation; the
status page repeats it. **The PoC does not finalise it.**

**The two remedies, both real work, neither built:**

1. **Key rotation.** Move the reserve on-chain to a newly generated key and script, so old shares no
   longer sign for live funds. This **changes the deposit script**, which is the blocker: a rotation
   would strand deposits made to the old address unless the program can follow the change, and
   `initialize_bridge` accepts `deposit_script` **once**. There is no instruction that changes it
   (verified in `lib.rs`: `DepositScript` is written only by `initialize_bridge`). **Key rotation is
   currently not even implementable.**
2. **Proactive re-sharing.** Re-share the live key without the departed member, so their shares are
   useless against the new sharing. This **needs the departing member's cooperation** — a member who
   was removed for cause, or compromised, is exactly the one who will not cooperate. **Not specified**,
   and it does not answer the case it is most needed for.

RenVM's answer is rotation every epoch ([03 §6](03-the-federation.md#6-the-reference-renvm-in-one-page)); we deliberately omit it. **The honest
summary: a departing member keeps a working share, and there is no mechanism in the design or the code
that stops them using it.**

---

## 4. The economics

**The bond is the float, not capital sized against the reserve.** It is **working capital** that lets
the federation serve transfers and redemptions. It is **not** a capital requirement sized against the
reserve, and **not** a capacity ceiling ([07 D9](07-decisions.md#d9--membership--the-greycore-admits-two-sided-bonds), [06](06-parameters.md#the-bond-is-the-float-deliberately)).
The earlier reading — that bond size caps total value locked — is **withdrawn**, along with the
`~$180k` and `$300k` capacity figures and the `n/t` arithmetic that produced them ([03 §8](03-the-federation.md#8-row-by-row-against-renvm),
[07 R9](07-decisions.md#reversals)).

**What constrains the reserve is the Greycore co-signature**, not the bond. Every reserve spend needs
both legs of the 2-of-2 script to sign, so a gateway majority alone cannot move funds ([03 §3](03-the-federation.md#3-the-reserve-script-threshold-ecdsa-plus-a-greycore-co-signature),
[07 D15](07-decisions.md#d15--a-2-of-2-reserve-script-the-gateway-threshold-key-plus-the-greycore)).

**The coverage floor is a solvency check, not a capacity rule:** `bsv_bond ≥ k × (BSV held)` and
`solbsv_bond ≥ k × (solBSV held)`, with `k = 1`, checked on mint and on exit. **Designed, not built**
([07 D14](07-decisions.md#d14--two-sided-bonds), [06](06-parameters.md#federation)).

**The fee is 30 bp each way, and where it physically lands differs by side:**

- **Redeem side — built.** `REDEEM_FEE_BP = 30` is in the program. The holder redeems `A` and is paid
  `A − fee`; the whole `A` is burned. The reserve falls by `A − fee` and the supply by `A`, so the
  **reserve-to-supply ratio improves by `fee`** and **no distribution account exists or is needed** —
  the fee is BSV the reserve did not have to pay out ([11 §3](11-peg-out-spec.md#3-why-the-fee-works-out-and-where-it-physically-lands), verified in
  `redeem_fee`).
- **Mint side — designed.** The 30 bp peg-in fee is decided ([07 R2](07-decisions.md#reversals)) but its **physical
  location is unspecified**: the deposit arrives at the reserve address and the program reads only
  `claim.amount`, so it can mint `net` and credit a claim, but it cannot make the BSV-side fee true.
  This is **N1, recorded as unresolved and critical** in [05](05-trust-model.md#the-vault-audits--v-w-t-n). **Not specified, and not built.** The
  same reasoning as the redeem side does **not** transfer unchanged: there, the fee is BSV retained;
  here, the BSV has already arrived and the program cannot move it.

**No numeric capacity rule has replaced the withdrawn arithmetic, and this is a genuine gap.** A
reviewer cannot compute our capacity from our documents ([03 §9.3](03-the-federation.md#9-where-we-are-weaker-than-renvm),
[08 §5](08-status-and-roadmap.md#5-the-honest-limitations)). The reserve is bounded by the second quorum's willingness to co-sign,
which is a policy, not a number. **Not specified.**

---

## 5. The two bonds

Two bonds, one per direction, each in the asset its side holds, and **neither inside the reserve**
([07 D14](07-decisions.md#d14--two-sided-bonds), [06](06-parameters.md#federation)). 1,000 BSV per side is a **placeholder** ([03 §8](03-the-federation.md#8-row-by-row-against-renvm)),
not a number derived from any security calculation.

| | BSV-side bond (mint side) | `solBSV`-side bond (redeem side) |
|---|---|---|
| **Denomination** | Native BSV | `solBSV` |
| **Where** | A BSV script, **outside the reserve** | On Solana, **outside the reserve** |
| **Custody** | Under the **collective (threshold ECDSA) key**, not the member's own | The program holds it |
| **Who can seize it** | **The members collectively**, by signing a threshold transaction | **The program**, by an instruction, on proof — automatic in the design |
| **Is that enforced?** | **No — a social duty.** Nothing on BSV compels the members to sign | **No — the instruction is not built** |
| **Built?** | **No** | **No** |

**The honest limits, stated rather than glossed:**

- **The `solBSV` side is seizable by the program** — in the design. That makes the redeem-side bond the
  enforceable half, and the asymmetry is stated, not hidden ([05 A16](05-trust-model.md#the-mechanism-audit--a1a18)).
- **The BSV side is seized by the members collectively, and the duty to slash is social rather than
  on-chain.** A member cannot move their own bond — that is the design requirement that makes slashing
  possible — but the members must *choose* to sign, and nothing compels them
  ([03 §2](03-the-federation.md#2-membership), [07 D14](07-decisions.md#d14--two-sided-bonds)).
- **A majority could seize an honest member's bond.** The symmetric risk of collective custody, resting on
  the same majority already trusted with the reserve ([07 D14](07-decisions.md#d14--two-sided-bonds)).
- **No bond prevents a majority.** The bonds are the float and cover only what their own sides hold. A
  gateway majority acting with the Greycore can take the reserve, and the bonds do not answer it; the
  maximum loss is the **entire non-member supply** ([05](05-trust-model.md#the-residual-the-bond-cannot-close), [07 D5](07-decisions.md#d5--bond-multiple--k--1-under-the-two-sided-formula)). Bonding
  prices provable misbehaviour; it does not restore a loss and it does not stop a quorum.

---

## 6. The Greycore

**The Greycore is trusted third parties, not node operators** — *"people with reputations to lose, who
do not run the reserve"*, mirroring RenVM's own second quorum ([03 §3](03-the-federation.md#3-the-reserve-script-threshold-ecdsa-plus-a-greycore-co-signature)). It holds the second key in
the reserve script:

```
OP_2  <gateway threshold key>  <greycore key>  OP_2  OP_CHECKMULTISIG
```

**Both must sign**, so the gateway majority cannot move funds alone, the Greycore cannot move funds
alone, and **the Greycore polices every reserve spend** ([07 D15](07-decisions.md#d15--a-2-of-2-reserve-script-the-gateway-threshold-key-plus-the-greycore)).

**Its intended roles:**

- **co-sign every reserve spend** — this is what constrains the reserve (§4);
- **admit members** — *"finds and admits replacement members"* (§2);
- **manage change** — the Greycore is appointed by community governance in the reference, and in our
  design it is the body that carries the trust the bond cannot ([03 §6](03-the-federation.md#6-the-reference-renvm-in-one-page)).

**And the honest state, which is thin:**

- **It does not exist in code.** No account, no instruction, no parameter (§8). It is verified absent
  from `lib.rs`.
- **Its size and threshold are `open`.** `fed.greycore_size` and `fed.greycore_threshold` are both
  `open` ([06](06-parameters.md#federation)). The co-signature is what constrains the reserve, so these are
  load-bearing and unfixed.
- **At genesis it cannot be disjoint from the founding set.** The founding members appoint the
  Greycore, so the two quorums are not independent at the start; [08 §6.3](08-status-and-roadmap.md#6-what-we-would-attack)
  names this directly — *"Does it genuinely make the two quorums independent, given the founders may
  appoint both?"* **No mechanism makes them disjoint, and none is specified.**

A 2-of-2 script raises the cost of moving the reserve — two quorums must collude instead of one — and
it relocates part of the trust into a named, reputational set. **It does not remove the trust.**

---

## 7. What a member is exposed to

| Exposure | What it is | Can it be taken? | By whom | Built? |
|---|---|---|---|---|
| **The bond** | 1,000 BSV per side, the float | **`solBSV` side: yes**, in the design, by the program. **BSV side: yes**, by the members collectively signing a threshold transaction — a social duty, not an on-chain rule | The program; the members collectively | **No** |
| **The reserve** | The pooled BSV under the 2-of-2 script, which the members hold | **Yes — unprevented.** A majority of the gateway signers acting **with the Greycore** can take it. The 2-of-2 raises the bar; it does not close the case | A gateway majority plus the Greycore | **No** |
| **Reputation** | Not a bond and not on-chain; the reason the Greycore is trusted at all | **Not seizable** — which also means it cannot be posted, checked or slashed | — | **No** |
| **The fee stream** | 30 bp each way, pro rata to pledged stake; the only revenue ([03 §2](03-the-federation.md#2-membership)) | **Not an asset the program holds.** Redeem-side fee is built into the payout arithmetic; mint-side is designed and its location unspecified (N1) | — | **Redeem side only** |

**Collusion is an accepted, unmitigated risk.** A majority of the reserve signers acting together with
the Greycore can take the reserve, and **no mechanism here prevents it** ([03 §10](03-the-federation.md#10-what-this-does-not-resolve),
[05](05-trust-model.md#the-residual-the-bond-cannot-close)). The Greycore raises the cost and relocates the assumption; the
bonds price provable misbehaviour; individual intents make a minority's equivocation self-proving; and
a published reserve makes a theft visible. **None of those is prevention**, and a consistent majority
who sign the same fraudulent thing and never equivocate is caught by none of the three layers
([03 §5](03-the-federation.md#5-slashing--self-proving-misbehaviour)).

---

## 8. What the programme actually does today

**The built set is 27 instructions and 79 passing tests.** The handler count was taken by counting the
`pub fn`s inside the `#[program]` module of `poc/solana/programs/solbeam/src/lib.rs` (lines 412–2053):
`initialize`, `push_header`, `seed_headers`, `init_staging`, `push_fork_header`, `abandon_staging`,
`commit_fork`, `initialize_token`, `initialize_bridge`, `report_spent`, `verify_deposit`,
`release_mint`, `burn_staged`, `propose_authority_change`, `execute_authority_change`,
`cancel_authority_change`, `prune_nullifier`, `initiate_redeem`, `cancel_redeem`, `claim_payout`,
`settle_redeem`.

**None of the federation is among them.** Verified against the source:

- **No membership account.** No `Member` account and no instruction that adds, removes or lists one.
- **No bond.** No bond account, no bond instruction, no custody of either side.
- **No Greycore.** No account, no key, no instruction.
- **No reserve script keys.** The program accepts the *shape* only: `is_acceptable_deposit_script`
  takes a 25-byte P2PKH or the 71-byte 2-of-2 `OP_CHECKMULTISIG` (`is_reserve_multisig`,
  `MAX_SCRIPT_LEN = 71`, `DepositScript::SPACE = 84`). No key generation, no sharing, no signing, and
  **no deposit has ever paid one**.
- **No governance.** The only change path is the **timelocked authority** (`TIMELOCK_SLOTS = 32`)
  held by the program's **upgrade authority — one key**, not the designed 85% / 30 days. The
  checkpoint, pause and `maturity_blocks` changes go through `propose`/`execute`/`cancel_authority_change`.
- **No slashing.** No `slash_equivocation` instruction and no intent account to equivocate on.

**The one stand-in a reader must read as a stand-in.** `report_spent` — the spent-outpoint record —
is written by the **program's upgrade authority**, verified on-chain against the loader's `ProgramData`
account. It **stands in for the federation, which does not exist yet**. When a federation exists the
signer becomes its key or quorum; nothing else about the instruction needs to change (verified in
`ReportSpent` and `create_spent_outpoint`). Until then the hole is closed **in mechanism and unchanged
in trust**: it is still one key's assertion, and that key is the upgrade authority.

**On the counts.** The suite stands at **79 passing / 0 failing** and the program at **27 instructions**. Two documents said "37 passing" until this note was written — the phrase was split across a line break, so a text-level find-and-replace silently matched nothing while reporting success, and a plain `grep "37 passing"` reported the files clean for the same reason. **Prefer grepping with a whitespace-tolerant pattern**, or the check will confirm a correction that never happened.

[11 §6](11-peg-out-spec.md#6-what-this-does-not-solve)).

---

## 9. Monitoring as the mitigation

**The reserve is off-chain BSV the program cannot read, so `custodied BSV ≥ outstanding solBSV` is
monitored, not enforced** ([07 D8](07-decisions.md#d8--the-reserve-invariant--monitored-not-enforced), [05 invariant 1](05-trust-model.md#core-invariants)). A threshold key changes who holds
the reserve; it does not change what a Solana program can see.

**Publishing the reserve and the supply is the only defence left for the cases nothing enforces:** a
colluding threshold, and a spent deposit outpoint nobody challenges. It is promoted from a phase-5
monitoring task to an **early deliverable**, *"and it must exist before real value does"*
([08 §9](08-status-and-roadmap.md#open--remaining-work), [07 D8](07-decisions.md#d8--the-reserve-invariant--monitored-not-enforced)).

**What exists, and what it is for:**

- **[`poc/PHASE5_MONITORING.md`](../poc/PHASE5_MONITORING.md)** is the plan for the public monitoring page
  — **"Status: plan only."** Its first rule is that every number is a derivation with its inputs shown,
  not a claim; its core metric is reserve against supply, published with the addresses so anyone can
  recompute the invariant; it also plans the absolute cost to attack BSV, the rentable hash power, and
  observed block spacing. **Nothing in it is implemented.**
- **[`website/status/index.html`](../website/status/index.html)** carries the same item as
  **"Phase 5 · Public monitoring — Plan only."** It is a status/roadmap page, not a data feed, and it
  publishes no reserve or supply figure today.

**The monitoring page is [`website/monitor/`](../website/monitor/)**, and it is the mitigation this section describes. It publishes BSV block height, hashrate and observed block spacing, BSV and SOL prices, and Solana validator stake and throughput — all fetched live, keyless, in the reader's browser, with the source and fetch time shown.

**The figures that matter cannot be fetched, and are published by hand.** BSV held in the reserve, `solBSV` minted, and the reserve-to-supply ratio are all `null` in `data.json` and render as **"NOT PUBLISHED"** — never as zero and never as a dash, because a reader must not mistake *"we have not published this"* for *"this is zero."* Each says what would populate it and when. **The federation does not exist, so there is nothing to publish.**

**It also computes the cost to out-mine the chain** — live hashrate against a stated machine and energy assumption, printed with units and labelled an electricity-only floor. **That is the number the design leans on**, and it is an assumption rather than a measurement, so the page says so.

**The availability figure, and why it matters more than the price.** NiceHash's public API exposes the SHA-256 rental market in **three separate markets that must be summed** — `SHA256AsicBoost` (BTC), `SHA256AsicBoost_USDT` (USDT), and the legacy `SHA256`. **Consolidated, the rentable supply is roughly 25 EH/s.**

**Against BSV's network of about 0.21 EH/s, that is a surplus of ~120×.**

> **BSV can be 51%-attacked by renting.** Matching the network's hashrate costs on the order of **$9,000 a day** at the current leased rate; a comfortable majority about twice that. **Attack cost is not a defence, and nothing here treats it as one.**

**What answers it is the vault.** A rented majority can reorg a deposit, but **the program verifies the chain itself and burns a mint whose deposit is reorged away** — which no amount of rented hashrate prevents. **The defence is the reversal, not the price of the attack.**

**And the limit of the mitigation stands:** publication makes a theft **visible**. It does not prevent
one, and a published reserve balance is an assertion by the party that could steal it
([03 §10](03-the-federation.md#10-what-this-does-not-resolve)). For a member and for a holder, that is the honest position: the
part of the system that enforces is the program; the part that does not is published.
