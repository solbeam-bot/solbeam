# 12. Peg-in and peg-out: the mechanism

> **Start with [`13-summary.md`](13-summary.md)** — it is the canonical model, and where this
> document disagrees with it, doc 13 is right. Then [`14-decisions.md`](14-decisions.md) for the
> register of what is settled and what each decision defers, and
> [`23-federation.md`](23-federation.md) for membership, governance and slashing. This document
> holds the full reasoning.
>
> **Rewritten for the federation model, 2026-09-29.** The system is now three parts: a **light
> client** on Solana, a **vault** every mint lands in, and a **bonded federation** holding the
> reserve under a **2-of-2 script with the Greycore**. This revision removes the order book and discovered fees, the
> per-relayer independent keys, and the "no governance" posture, and records each as a **reversal
> with a reason** rather than a silent edit — see §Changes in this revision. The reasoning that
> still holds is kept: reorg handling, block-time signals, the loss-landing analysis, the tiered
> buffer argument, and the audit findings A1–A18 as history.
>
> **Built or designed?** Built and passing: **the light client with cw-144** (324/324 real mainnet
> headers), **the `solBSV` token**, **the mint**, and **fork staging** — 34 on-chain tests, 51/51
> Phase 1A synthetic checks, 21/21 against a live SV Node. Everything else in this document —
> the vault, maturity, release, burn, the federation, threshold custody, governance, slashing and
> all of peg-out — is **designed, not built**. The shipped mint goes straight to the depositor's
> token account, so nothing is staged, no fee is charged and no maturity exists in code yet. Most
> of this document is therefore a specification rather than a property of the program.
>
> **The difficulty rule is no longer the open item it was.** cw-144 from the node's `src/pow.cpp`
> is implemented in `difficulty.rs` and verified at **324/324 exact**. The old
> `bits == expected_bits` check, which rejected *every* header after the checkpoint (F7), is gone.
> **X3 remains in one form:** the algorithm is hard-coded, and BSV's own documentation says the
> rule will change, so a change needs a program upgrade — which governance can now carry (D10).

---

### Vocabulary

Four things are easy to conflate, so this document names them apart:

| Term | Means |
|---|---|
| **The program** | The on-chain Solana program. Consensus. Consults nothing external |
| **The peg** (or the bridge) | The whole mechanism — program, federation, deposits, redemptions |
| **The federation** | The bonded member set that holds the reserve under a **2-of-2 script — the gateway threshold key plus the Greycore's** — relays headers, signs payout intents and challenges. Members run software, not judgement |
| **The website** | `solbeam.me`. Parameters, the published surface, status. **Nothing here is consensus** |
| **Monitoring** | The published surface: parameters, external metrics, reserve ratio, Solana liveness |

The distinction does real work. A metric shown on the website carries no trust assumption; the
same value influencing a mint decision in the program would be an oracle. "The system" is not used
in this document, because it could mean any of the four.

### The trust division, and why it is stated this way

**This is the honest boundary and it should not be blurred.**

| | |
|---|---|
| **Minting** | **The deposit is verified; the backing is reported.** The program verifies BSV proof of work and Merkle inclusion directly, and **the federation reports spent deposit outpoints** because Solana cannot read the BSV UTXO set. **The program verifies deposits; the federation reports backing** — not a new trust assumption, since the federation is already trusted with the reserve. **The program's upgrade authority is the other exception** — it can re-anchor the checkpoint, so in production it must be threshold-held and timelocked, and the exit window is the real guarantee |
| **Reversal** | **Trustless.** The program compares its own stored header hash against the one a deposit was proven with. A reorg is a fact about headers, not a report from anyone |
| **The reserve** | **Trusted, and bounded.** The BSV is held under a **2-of-2 `OP_CHECKMULTISIG`** — the gateway's threshold ECDSA key plus the **Greycore**'s, and both must sign. No gateway majority and no Greycore can move it alone. What protects a holder is a **bond anyone can seize by proving misbehaviour on-chain**, not the absence of trust |

**The one trust assumption: the gateway threshold and the Greycore do not collude.** Everything else is
verified, except the **reported spent-outpoint record**. That assumption is not eliminated — it is **bounded**, by the Greycore co-signature, by bonds and by proofs anyone can
submit. §The federation's bonds close the hole states the bound exactly, including where it is
weaker than an earlier draft claimed.

## Changes in this revision

Every row below is a **reversal**, dated **2026-09-29**, with the reason. [`14-decisions.md`](14-decisions.md)
carries the same reversals in register form.

| Previously | Now | Why |
|---|---|---|
| **The order book**, with the fee **discovered** per bid (D2) | **A governed fee, 30 bp each way** | The book solved fee discovery and capacity allocation; a governed fee solves both more simply (the "bond cap" once cited here is **withdrawn** — the bond is the float), and the book was the one subsystem that never received an adversarial review |
| **Per-relayer deposit scripts and independent keys** (the A6 posture: "do not pool the reserve") | **One reserve under a 2-of-2 script with the Greycore** | Per-relayer isolation removed the single key but also removed the thing that makes a reserve auditable. Under a 2-of-2 no gateway majority and no Greycore can move funds, and the bond remains seizable on-chain. The reversal is honest about the cost: there is now a reserve to secure |
| **D7 — no governance in the PoC** | **85% of pledged coins / 30 days / live signal**, holding the **upgrade authority** | "No governance" left the upgrade authority as an unowned mint voucher (A5). Governance does not remove that power; it names who holds it and makes every use visible for 30 days |
| **Specialists-only staking** (D1) | **Greycore-admitted membership, two-sided 1,000 BSV bonds** | The Greycore is the trusted, non-operational body that finds and admits replacement members; the bonds are the float and the capital gate is objective |
| **`FLOOR` fixed in code** (D4) | **12 blocks, a governed parameter** | The value stands; the change mechanism that was a named gap is now governance |
| **Detection paid by nobody** (P7) | **Detection is a member job**, funded from fees and bounties | The challenger was an unpaid chore nobody owned; under the federation it is done by the parties with the most to lose |
| Anything resting on **enforcing BSV-side behaviour from Solana** (per-deposit consent, the hot float cap, a relayer-script registry) | **Handled by the federation or dropped** | The program cannot see or spend off-chain BSV. Where a mechanism assumed it could, the mechanism was wrong, not merely unbuilt |
| **Gate symmetry** — pause must close both directions | **Pause stops mints only; redemptions are never pausable** | Reversal of the earlier argument: the exit is the protection, not a symmetric freeze. See §The exit is the floor |

### No oracles, by construction

The mechanism needs none. It is worth being explicit about where each would otherwise have
appeared and what replaced it:

| Would have needed | Replaced by |
|---|---|
| A hashpower or price feed to compute a "safe" confirmation depth | **`FLOOR`, 12 blocks**, a governed parameter (D10) |
| A feed to price reorg risk | The **governed fee**; the metrics are published on the website and never consulted |
| A price feed to value the bond | `1 BSV = 1 solBSV` by construction. A deviation is an arbitrage, not an input |
| An external source for *has BSV reorged, and how deep?* | **The BSV headers themselves.** Chain data, not external data |
| Wall-clock to expire a redemption deadline | **Solana slots.** A halt freezes the clock instead of stranding the holder |
| A price oracle to size the mint cap | A conservative policy value, published, with the metrics shown beside it |

Two rules cover every row:

- **Chain-native.** Anything the program must *react* to is read from a chain — reorg depth and
  block time from BSV headers, deadlines from Solana slots.
- **Published, never consulted.** Anything that would need external data is displayed and shapes
  what people choose; it never changes what the program *does*. *(This replaces the earlier
  "market-native" rule, which existed because the order book turned a published term into an
  enforced one. With the book removed, there is no market layer inside consensus at all.)*

**The one residual judgement is `FLOOR` itself** — a policy value, not a measurement: 12 BSV
blocks, and since this revision a **governed parameter** rather than a constant (D4-revised, D10).
The design has no price oracle; it does have parameters somebody has to choose, and *that* is the thing
worth arguing about.

## Recommended state

### Peg-in — BSV → solBSV

```
  1. CHOOSE    terms; the fee is 30 bp, governed
  2. SEND      BSV to the federation's deposit script
               OP_RETURN = version ‖ cluster_id ‖ program_hash ‖ flags ‖ recipient
  3. DEPTH     12 confirmations (FLOOR)
  4. STAGE     solBSV is minted INTO THE VAULT, not to the depositor, and a record stores
               the block hash the deposit was proven against
  5. MATURE    144 blocks
  6. RELEASE   permissionless, and requires:
                 the tip has ADVANCED past the deposit, and
                 the stored hash at that height STILL MATCHES
               → vault to recipient
               if the hash DIFFERS, the deposit was reorged: the staged tokens BURN,
                 and the depositor keeps the BSV the reorg returned
```

**Step 6 is decided by the program from its own headers.** No oracle, no reporter, no discretion.
`release_mint` and `burn_staged` are **permissionless** — anyone may resolve a pending item and
reclaim its rent, so **no party's cooperation is ever required.**

**Step 4 is what makes a fraudulent mint unsellable.** A staged token is not in anyone's wallet, so
there is nothing to dump and no innocent buyer to inherit the loss.

### Peg-out — solBSV → BSV

```
  1. ESCROW    solBSV moves into the vault; a BSV destination and a deadline are set
  2. ACCEPT    federation members sign payout INTENTS individually, on Solana
  3. PAY       once enough attributed intents exist, the gateway threshold key signs and the
               Greycore co-signs the BSV payment (both are required by the 2-of-2 script)
  4. SETTLE    the payout is proved against the light client; the escrow burns
     or
  4'. CANCEL   permissionless after the deadline: the escrow returns to the holder
```

**Failure returns; it never mints.** Supply is unchanged and the holder is whole without asking
anyone. **Step 2 is also the attribution mechanism:** because each member signs separately, a
member who signs two conflicting intents has produced **their own proof of guilt** (D13; see
[`23-federation.md`](23-federation.md)).

**Nothing here is pausable.** There is no instruction that stops a redemption; the deadline and the
cancel path are the only time limits, and they run in the holder's favour.

### Parameters

| ID | Parameter | Value | Class | Notes |
|---|---|---|---|---|
| **P1** | `FLOOR` — minimum deposit confirmations | **12 blocks (~2 h)** | **safety** | Governed (D4-revised). In the shipped program this is the `MIN_CONFIRMATIONS` constant; `FLOOR` as a distinct parameter is not built |
| **P2** | `MATURITY` | **144 blocks (~24 h)** | **safety** | How long a staged mint must stay unreorged before release |
| **P3** | `WINDOW` | **192 records (32 h)** | liveness | **Fixed by arithmetic, not chosen** — cw-144 needs hash + chainwork + time per record (52 B), so 192 is the largest that fits the 10,240-byte account cap with margin. `LIGHT_CLIENT_FIXED` 123 + 192 × 52 = **10,107** (W1.4) |
| **P4** | `D` — redemption deadline | 6 h | liveness | Measured in **slots**, not wall-clock |
| **P5** | `MIN_PEG_IN` | **1 BSV** | economic | Decided (P5, doc 18): makes dust griefing a capital-lockup attack rather than a fee attack |
| **P6** | `MAX_PEG_IN` | 10,000 BSV | economic | Per *transaction* only; see §The aggregate mint cap |
| **P7** | Mint fee / redeem fee | **30 bp each** | economic | **Governed** (D2-replaced). Deducted where the value moves; paid to members pro rata to pledged stake |
| **P8** | Membership bond | **1,000 BSV per side** (D14): BSV on the mint side, **outside the reserve**; `solBSV` on the redeem side | **safety** | Admission is by the **Greycore** (D9/D15). **The bond is the float** — working capital for transfers — not a capital requirement sized against the reserve. Each side carries a coverage floor, so a member cannot leave while owing |
| bond `k` | bond coverage floor | `k = 1` (D5) | **safety** | `bsv_bond ≥ k × (BSV held)` and `solbsv_bond ≥ k × (solBSV held)`. **A solvency floor, not a capacity ceiling:** the `n/t` arithmetic and the `~$180k` / `$300k` figures are **withdrawn** (doc 26 §5). **The old single `bond ≥ k × owed` is superseded.** The `solBSV` side is checkable and seized on-chain; the BSV side sits under the **collective key** and is seized by the **members collectively** |
| `fed.threshold` | gateway signing threshold | **4-of-N, `N` open** — number deferred | **safety** | `t` of `n` for the gateway **threshold ECDSA** key — one leg of the reserve's **2-of-2** with the Greycore |
| `fed.greycore_size` / `fed.greycore_threshold` | Greycore size / threshold | **`open`** | **safety** | The second signer set on the reserve script: **trusted third parties, not node operators** — people with reputations to lose who do not run the reserve. The Greycore **co-signs every reserve spend** and **admits members** |
| **P9** | Governance | **85% of pledged coins / 30 days / live signal** | **safety** | Holds the upgrade authority (D10). All four numbers are parameters |
| **P10** | Pause | **mints only**, lower threshold, auto-lifts | **safety** | Redemptions are never pausable (D12) |

**Safety parameters are not freely loosenable, and this is now enforced by process rather than by
immutability.** Whoever can set `FLOOR = 0` holds a mint voucher, which is categorically different
from whoever sets a fee. Under the federation model the answer is **not** an immutable floor: it is
85% **and** 30 days **and** a live signal, during which redemptions keep working, so a proposal
that would harm holders empties the bridge before it lands (D11). All of those numbers are
parameters, and changing them is itself a governance change. The old "increase-only" proposal is
**superseded** — see §Decisions from review 4.

**Test overrides are compile-time, not config.** "Remove the minimum during testing" must be a
`#[cfg(feature = …)]`, never a runtime value — a runtime value can leak to mainnet, a compile-time
one cannot.

---

### Confirmation depth is a governed floor, not a market term

**This reverses the earlier design**, in which the depositor committed a depth in the deposit's
`OP_RETURN` and relayers priced it. Depth was a term of the bid; with the book removed it is a
governed parameter again.

That is not a loss, and audit finding **A14** is why: *the committed depth would not bind an
attacker anyway* — the fraud's depositor **is** the attacker, so they commit exactly `FLOOR`. A
market term that the party at risk chooses is not a safety parameter. So `FLOOR` is a governed
floor, at or above which every mint is checked, and depositors do not negotiate it.

**What remains true from the earlier reasoning:** depth and maturity do different jobs. Depth sets
**the cost of attacking** — a reorg must out-mine it — while maturity sets **the time available to
detect**. A low floor makes attacks cheap and therefore frequent, which raises the number of
chances for a detection failure to slip through. Both values are governed.

## Who this is for, and why the delays are acceptable

The peg is **wholesale infrastructure, not a retail product.** It is built for market makers,
larger LPs and exchanges who move the coin back and forth to earn a yield and to rebalance
inventory on both sides of the fence. Retail is expected to reach `solBSV` through an exchange or
through Raydium, not through the peg directly. That framing resolves most of the apparent tension
in the gates above.

### The security is economic, and it is Bitcoin's own

Bitcoin's double-spend resistance is an *economic* argument, not a cryptographic one:
confirmations are secure when out-mining them costs more than the value they protect. Nothing here
invents a new assumption. The peg extends the same logic one layer up, deliberately goes **beyond**
the whitepaper's six confirmations, and adds a revert path on top. What is added is not a new trust
model — it is a larger margin on the existing one.

Two calibration warnings, both load-bearing:

- **BSV's hashpower is not Bitcoin's.** The whitepaper's table is parameterised by `q`, the
  attacker's *share* of hashpower. On a chain with less total hashpower, reaching a given `q` costs
  less in absolute terms, so six confirmations do not carry the same meaning here. `FLOOR` must be
  calibrated to **BSV**, not inherited.
- **Depth must move with the value it secures.** A depth adequate for 10 BSV is not adequate for
  10,000. Governance is what lets the value move without shipping a program.

### The gates protect the backing, not the depositor

This is what makes them not paternalism. A fraudulent mint does not merely inconvenience the
depositor — **it dilutes everyone holding `solBSV`.** The gates protect the token's backing, which
is the market makers' own inventory. They are a collective protection that the professionals
benefit from most, not a judgement about a user's ability to assess risk.

### The exit is the floor

**An earlier draft argued the opposite of what follows, so the reversal is stated rather than
absorbed.** It held that a gate must close **both** directions, because pausing peg-in while
peg-out stays open pushes the price up, and pausing peg-out while peg-in stays open leaves open
exactly the exit the gate existed to close. The conclusion was *gate symmetry is a peg-integrity
requirement*.

**The federation model rejects symmetry, deliberately, for one direction only:**

```
pause_mints()   stops NEW mints. Redemptions continue, always. Lifts automatically.
```

The reason is that the old argument treated the exit as a risk to be gated. It is the opposite:
**the exit is what makes the rest of the system safe.** Governance holds the upgrade authority, so
in principle it can change anything — and that is tolerable **only** because redemptions run
throughout the 30-day delay. A change that would harm holders empties the bridge before it lands.

> **The floor is the exit window, not a constitution.** The protection was never that the rules
> are frozen. It is that you can always leave before they change.

Pausing inbound is a **safety valve** — it buys time when something is wrong with BSV or with the
program, without touching anyone's ability to leave. Pausing outbound is **taking hostages**. They
are deliberately not bundled, and because the pause power is bounded it can carry a lower threshold
than a governance change rather than waiting 30 days for an emergency.

**The residual, stated plainly:** a holder who does not watch and does not act within 30 days is
exposed. That is a disclosure obligation, not a mechanism.

### The reserve is trusted and bounded, and it is one reserve

**Second reversal, and the larger one.** The earlier design distributed custody: each relayer
registered a BSV script of its own, deposits paid individual relayers, and the program accumulated
a per-relayer liability `owed_R`. The point was that **a pooled reserve is a single key worth
stealing**, so the design removed the pool rather than securing it. There was then no reserve
contract to write, audit or trust.

**The federation model pools the reserve under a threshold ECDSA key, and this is the cost of the
change:** there is now a reserve to secure, and the trust assumption is explicit rather than
engineered away.

What the change buys, and why it is worth stating rather than hiding:

- **No gateway majority — and no Greycore — can move the reserve.** The reserve script is a **2-of-2
  `OP_CHECKMULTISIG`**: the gateway's threshold ECDSA key (never assembled in one place) plus the
  **Greycore**'s, and both must sign. The failure mode is *the gateway quorum colluding with the
  Greycore*, not *one key stolen*. **This reverses audit F10:** the deposit script genuinely is a
  multisig, so `is_p2pkh` must change and `DepositScript::SPACE` must grow to ~71 bytes.
- **The bonds are a capital gate.** Members post two-sided bonds, one per direction and **neither
  inside the reserve**; the `solBSV` side is seizable on-chain, so the member set is permissionless
  but not anonymous. The BSV side is held under the **collective key** and seized by the **members
  collectively**, with the slashers paid from it — a collective action by the majority, not an
  automatic rule, since nothing on BSV compels them to sign.
- **There is exactly one reserve to attest to.** With distributed custody, "is the reserve whole?"
  is a question about every relayer at once. With one 2-of-2 reserve it is one address, one
  attested balance and one published ratio.
- **The peg-out path has a signing set, not a discretion.** Members sign intents individually, the
  gateway threshold key signs and the **Greycore co-signs** only once enough attributed intents exist,
  so no member chooses *whether* to pay.

**What is not claimed:** the reserve is still **trusted** — that is the honest word in doc 13's
table, and it should not be softened to "trustless". What bounds it is the two-sided bonds and the
exit, and **continuous publication of the reserve and supply** makes a theft visible (doc 07).
§The federation's bonds close the hole says precisely how far.

## The attack this defends against

A reorg is not interesting because it moves blocks. It is interesting because of one sequence:

```
  ATTACKER                     BSV                         SOLANA
     │                          │                             │
     │ 1. mine a competing branch containing a fake
     │    deposit to themselves (needs hashpower)
     ├─────────────────────────►│
     │                          │ 2. FLOOR confirmations on the
     │                          │    FAKE branch
     │                          │ 3. mint ───────────────────►│ solBSV staged in the vault
     │                          │                             │    (not liquid, not sellable)
     │ 4. release after maturity ─────────────────────────────►│ (only if detection fails)
     │ 5. peg out ◄───────────────────────────────────────────┤
     │                          │                             │
     │ 6. honest chain overtakes the fake branch
     │                          │  → if still staged, the mint is burned and the
     │                          │    liability is reversed. If already released and
     │                          │    sold, the loss lands on the federation's bonds
```

The vault is what makes a detected fraud reversible. Three further things make an undetected one
expensive, and **none of them is detection**:

1. **Proof of work.** Forging `FLOOR` blocks must out-mine the honest chain for the duration.
   Depth is the security parameter that makes this cost more than it pays.
2. **The vault and maturity.** A staged mint is not liquid, so a followed reorg burns it and the
   fraud never reaches a market.
3. **The bond.** Every member's `solBSV` bond is held by the program and seizable on proof.

The **2-of-2 script** — the gateway threshold key plus the **Greycore** — is what stops one member
moving the reserve; it is not reorg insurance and
does not bound a fraudulent mint. The **bond** is the float and answers a member's
provable misbehaviour, not the mint. **The earlier claim that the total bonded stake is the scale
limit is withdrawn:** the bond is the float, and the reserve is constrained by the **Greycore
co-signature**. See §The federation's bonds close the hole.

Detection — the gates above — is a further layer. It reduces the window of opportunity and it
protects honest users from being caught mid-flight. It is not what makes the peg safe: **a gate
that depends on someone submitting a competing branch is a liveness assumption, not a guarantee.**
Under the federation, that liveness assumption has an owner: **a member's job is to watch and
challenge**, and it is funded.

---

## Where the loss actually lands

### The invariant, stated the right way round

What is at risk is **unbacked `solBSV`** — more `solBSV` outstanding than BSV custodied. The
invariant is

> `custodied BSV ≥ outstanding solBSV`

and a successful reorg attack violates it by creating supply with nothing behind it. The reverse
(more BSV than supply) is the *safe* direction, merely inefficient.

**The invariant is monitored, not enforced** (D8). `custodied BSV ≥ outstanding solBSV` is
published and shown as a ratio on the website, and **the protocol cannot enforce it**, because the
reserve is off-chain BSV the program cannot read. The program does not check it. D8 **stands**
under the federation model: a **2-of-2 script with the Greycore** changes who holds the reserve, and
the program still cannot read it. **The one input the federation reports is the spent-outpoint
record**, because Solana cannot read the BSV UTXO set.

### A fraudulent mint has more than one exit

The attacker's difficulty is not obtaining `solBSV` — it is turning it into something else. There
are three routes, and **the protocol controls exactly one**:

| Exit | Who absorbs the loss | Gateable? |
|---|---|---|
| **Peg-out** via the protocol | The federation's reserve, bounded by the bonds behind it | Partly — the intent set refuses an unauthorised payout, and the reserve is finite |
| **DEX** (Raydium / Orca) | The **LPs** in the pool, who bought `solBSV` that is not backed | **No.** A permissionless AMM cannot be frozen |
| **CEX** | The exchange, and its users if it cannot cover | **No.** Off-chain entirely |

So gating peg-out closes one door and leaves two open. It is still worth doing — it removes the
deepest, fastest exit and forces the attacker through market slippage, which costs them real money
— but it is **not** what makes the peg safe, and an earlier draft came close to saying that it was.

### What actually protects

Only one mechanism attacks the fraud itself rather than its exit:

- **`FLOOR`, the minimum confirmation depth.** It is what makes out-mining the honest chain cost
  more than the fraud is worth. Every other control silently assumes the fraudulent mint has
  already happened.
- **The vault and maturity**, which make a detected fraud reversible rather than merely bounded.
- **The 2-of-2 script** — the gateway threshold key plus the Greycore — which means no gateway
  majority and no Greycore can move the reserve alone.
- **The bonds.** The float, with a coverage floor; the earlier "total bonded stake caps the value the
  system can hold" is **withdrawn** — the reserve is constrained by the **Greycore co-signature**.

The honest consequence, which belongs in the document rather than in a footnote: **if depth is too
low, the loss lands on DEX LPs and exchanges, and the protocol cannot compensate them.** They are
not identifiable from on-chain state, they never interacted with the bridge, and there is nothing
to make them whole with. That is a stronger argument for setting `FLOOR` conservatively than
anything about the exit gate.

### The federation's bonds close the hole

**This is the same tiered argument the earlier "staking buffer" made, with the buffer identified as
the member bonds.** Members bear the fraud loss through the `solBSV` the program holds and can
seize. When detection fails and a fraudulent mint releases, the bonded stake absorbs it rather than
anyone's backing.

| | Custody | Supply | Peg holds? |
|---|---|---|---|
| Normal | `D` | `D` | Yes — 1:1 |
| With bonded stake `S` | `D + S` | `D` | Yes — over-collateralised by `S` |
| Fraudulent mint of `M` | `D + S` | `D + M` | Yes, **iff `S ≥ M`** |
| Attacker then exits via peg-out | `D + S − M` | `D` | Yes, iff `S ≥ M` — **the federation is down `M`; nobody else is** |

So the market never sees the shortfall. A DEX LP holding the unbacked tokens holds something that
is *still fully backed*, and an exchange never has to book a loss. That is what fixes the hole: the
loss is socialised onto parties who posted a seizable bond instead of landing on people who never
chose it.

**The safety condition to aim at is:**

> `bonded stake  ≥  maximum mintable within one reorg window`

**The earlier "scale limit is the same number" claim is withdrawn.** With `k = 1` it used to say
total value locked is capped by total bonds pledged — ten members at 1,000 BSV is roughly `~$180k` —
but **the bond is the float**, not a capital requirement sized against the reserve, and the `~$180k`
/ `$300k` figures are **withdrawn** (doc 26 §5). **What constrains the reserve is the Greycore
co-signature.** §What the bond answers says what that implies.

**Three tiers, in order:**

1. **Detected in time** → staged tokens burned. No loss at all.
2. **Detection fails, bonds cover it** → the federation loses; the market does not.
3. **Bonds short** → holders absorb a discount. The one case that cannot be repaired.

Only tier 3 is a real failure, and its likelihood is essentially `1 − bonded stake / max mint`.
That ratio is the number worth watching.

**This creates one requirement: unbonding must be delayed.** If members can leave at will, the
first to notice a fraud exits before it is confirmed, leaving a bond set sized for a calmer day. An
**unbonding period** — longer than the redemption deadline plus the challenge window — is what keeps
the bonds present when they are needed.

### What the bond answers — and what it does not

The bonds are **the float**, with a **coverage floor** of what each side holds — `bsv_bond ≥ k × (BSV held)`
and `solbsv_bond ≥ k × (solBSV held)` (D14; the older "sized against `owed`" is superseded). They
answer a member's **provable misbehaviour** — self-proving equivocation on a payout intent — making
it punishable: the `solBSV` side is a chain fact the program seizes; the BSV side is seized by the
members collectively under the collective key.

**It does not answer "an intent matching no authorised redemption."** That predicate is
undecidable — a *closed* `PegOut` is indistinguishable from one that never existed — and checking it
would false-positive against an honest member who attested before a cancel (audit F8).

It is **not** the answer to a failed redemption, and it is **not reorg insurance**:

- **A failed redemption returns the escrow.** The holder's `solBSV` is still in the vault, so on
  failure it goes back to them and **supply never changes**. Returning the escrow makes the holder
  whole, so the bond is **not additionally transferred** to them. What such a failure exposes is a
  member's abandonment; the bond's role is to make that abandonment punishable, not to top up a
  holder who is already whole.
- **A reorged payout is not a slashing matter.** A member paying out against a fraudulent mint has
  done nothing wrong, because it cannot tell that mint from a real one. A payout later reorged away
  is handled by the deadline and the return path, not by slashing.
- **A threshold of members colluding is not cryptographically provable.** Each member signs one
  consistent intent, so there is no equivocation to exhibit. That case is a governance matter, and
  the exit window is the answer to it — see D13 and [`23-federation.md`](23-federation.md).

**Return-to-sender is a different mechanism again**, and it does not apply here. RTS is for a *real*
deposit the bridge declines to mint: the funds go back to the address that funded the deposit
transaction, derived from that transaction's own inputs. In a fraudulent mint there is no aggrieved
sender — the victims are whoever ends up holding the unbacked tokens, which is the market. RTS is
not built (A15).

## Reorg detection

### Rule 1 — depth, not wall-clock

The trigger compares two depths, not "did anything reorg in the last N hours":

| Observed reorg depth `R` | Response |
|---|---|
| `R < FLOOR` (e.g. 2-block tip reorg) | **Nothing.** Confirmations already cover it |
| `R ≥ FLOOR` | The reorg response applies: re-examine mints credited within the last `R − FLOOR` blocks, and stop crediting new ones |

A uniform 12-hour freeze on *any* reorg would stall the bridge through ordinary tip churn — a
2-block reorg is normal BSV behaviour and is harmless to a deposit that is already 12 deep. The
wall-clock window is a convenience bound (12 h ≈ 72 BSV blocks); **depth is the number that means
something.** Whether the response is enforced on-chain or published is Decision 5 below; under the
federation the node software is the challenger, so it is both at once: the program decides the
release from its own headers, and the node watches and acts.

### Rule 2 — read the time from the headers

The 80-byte header carries a 4-byte little-endian Unix timestamp at **offset 68**. **This is now
parsed and stored:** the W1 rework made every `HeaderRecord` `hash + chainwork + time` (52 bytes),
because cw-144's clamps are functions of the timestamp difference. So the client derives
chain-relative time from the chain itself, with no external clock:

| Signal | Definition | Reads as |
|---|---|---|
| **Regression** | new header `time` well below tip `time` | A fork, not an extension. Sharpest signal available |
| **Catch-up** | blocks arriving fast in wall-clock while their own `time`s span hours | Advancer was down, **or** a withheld branch is being released — indistinguishable, so pause |
| **Staleness** | `now − tip.time` > `TIP_STALENESS` | Advancer down, or a deep reorg in progress |

Only staleness needs a wall-clock, and only for "now" — the chain's own times do the rest. Note
the limit honestly: a privately mined branch released later has *normal* block-time spacing, so
spacing alone will not catch it. What catches it is the mismatch between **arrival rate and
block-time spacing** — signal 2.

---

## Scenarios

| # | Scenario | Handling | Who loses |
|---|---|---|---|
| S1 | Clean peg-in | Deposit proven at `FLOOR`; mint staged in the vault; released after 144 blocks; fee 30 bp to the federation | Nobody |
| S2 | Reorg **below** the deposit block, while the mint is staged | The staged tokens **burn**. Re-inclusion does **not** restore the mint by itself: the deposit identity `(txid, vout)` stays in the replay list, so it cannot be minted again until the burn also removes its replay entry (**F1**). With that entry removed, the re-mined deposit mints normally | Nobody, once F1 is implemented — the depositor keeps the BSV the reorg returned |
| S3 | Reorg **after** a mint, depth ≥ `FLOOR` | If the mint is **still staged**, the program burns it. Only a mint that has already **released** cannot be reversed — no freeze authority, by design. New mints pause; depth and the bonds carry the rest | The federation's bonds, if depth was too low and release had happened |
| S4 | Clean peg-out | Members sign intents; the gateway threshold key signs and the Greycore co-signs; the payout is proved; the escrow burns; the fee is paid | Nobody |
| S5 | Payout reorged after it was proved | The payout proof no longer matches the client's window; settlement does not complete and the **escrow returns to the holder** after the deadline. Supply is unchanged. The federation has lost the BSV it paid — the bond is not reorg insurance | The federation (its reserve) |
| S6 | No threshold signature before the deadline | Deadline expires → the **escrow is returned** to the holder, permissionlessly. Nothing mints; supply is unchanged | Nobody loses tokens; the members who failed to sign are at fault |
| S7 | **The attack** — fraudulent mint then peg-out | The vault stages the mint; a reorg of depth ≥ `FLOOR` burns it; the intent set refuses an unauthorised payout; the bonds bound what a release can extract | The bonds, if detection fails and the mint released; otherwise nobody |
| S8 | Deposit valid but the tip is stale | **Delayed, not refunded.** Funds stay in the federation's script; the mint proceeds once the tip advances | Nobody |

### If a refund is ever needed

Refunds are the exception, and the rule is absolute: **return to sender.** The destination is the
address that funded the deposit transaction, derived from that transaction's own inputs — never a
relayed or user-supplied destination. Anything else turns "refund" into a way to redirect someone
else's coins. This is **not built** (A15), and it is not the vault's return path, which moves tokens
the program already holds.

The UX must show this before it can happen, so it is never a surprise: a **greyed-out box reading
"if refunded, it goes back to the sender address"**, populated with the address derived from the
deposit, visible while the deposit is pending.

---

## Considerations

Each table records the options, the reasoning, and the verdict. The recommended row is listed first.

### 1. Reorg detection trigger

| Option | Pros | Cons |
|---|---|---|
| **Depth-aware + block-time signals** ✅ | Matches the actual risk; no false freezes from tip churn; no external oracle | Three signals to tune; block-time thresholds need generous tolerance |
| Wall-clock window only (12 h) | Trivial to implement | Freezes on harmless tip reorgs; measures time, not risk |
| Wall-clock **and** depth | Simple, conservative | Still freezes on `R < FLOOR`; the wall-clock part adds nothing once depth is checked |
| No detection — rely on `FLOOR` alone | Simplest; depth is the real security | No protection against a *catching-up* client; exits stay open during instability |

### 2. Response mid-transfer

| Option | Pros | Cons |
|---|---|---|
| **Delay until the tip is current** ✅ | Deposit is valid, so delaying preserves intent; no refund path to abuse; funds never move | User waits; needs clear UX or it looks like a hang |
| Refund (return to sender) | User gets funds back | New BSV transaction, new custody, new abuse surface; refund only to the funding address or it is a theft vector |
| Freeze and require manual action | Maximum control | Trusted operator action; unusable at scale |

### 3. Refund authorisation

| Option | Pros | Cons |
|---|---|---|
| **Funding address only, derived on-chain** ✅ | Not redirectable; no discretion to abuse; verifiable from the deposit tx | Requires parsing the deposit's inputs |
| Member discretion | Flexible | A member that can choose the destination can steal the deposit |
| No refunds at all | No surface at all | A genuinely invalid deposit's funds are stuck forever |

### 4. Custody model

| Option | Pros | Cons |
|---|---|---|
| **Bonded federation, one reserve under a 2-of-2 script with the Greycore** ✅ | No gateway majority and no Greycore can move funds alone; Greycore-admitted members with a 1,000 BSV float per side; one reserve to attest to; detection is a funded member job | The gateway quorum plus the Greycore can collude; there is now a reserve to secure; the bond is the float, not a scale limit |
| Per-relayer deposits, independent keys *(the previous design)* | Removes the pooled reserve, so there is no single key worth stealing and no reserve contract to audit | Fragments custody and liability across N scripts; "is the reserve whole?" becomes a question about every relayer; has no threshold or governance layer |
| Single nominated custodian | Simplest | Nomination is a trusted choice; one key worth stealing |
| Fully trustless reserve (covenant) | Removes the spending key entirely | Pre-mainnet and unaudited; `OP_CAT`/`OP_MUL` in-script verification. Still the destination, not a prerequisite |

### 5. Bond asset

| Option | Pros | Cons |
|---|---|---|
| **`solBSV` only** ✅ | The same unit as the exposure, so no price move can shrink it relative to what it protects; the program holds it and can seize it; a slash is deflationary | Procyclical only if `solBSV` itself depegs — the case `k = 1` accepts because `solBSV` and BSV are the same asset and any deviation is an arbitrage |
| Mixed: BSV in a timelocked multisig, topped up in `solBSV` | The BSV leg does not depeg with the thing it insures | More moving parts; only the Solana leg is seizable (A16); reintroduces a second signer set |
| Stablecoin or SOL | Uncorrelated with the peg | Not BSV-denominated; a stablecoin bond against a BSV liability is a **written call option** on the reserve — see doc 04 |

### 6. Peg-out capacity control

| Option | Pros | Cons |
|---|---|---|
| **The reserve's balance and the intent set** ✅ | A payout requires enough attributed intents *and* the gateway threshold key plus the Greycore co-signature; there is no way to pay out more than the reserve holds; redemptions are never paused | Capacity is one pooled number; a redemption larger than the reserve returns the escrow rather than paying |
| Per-transaction max only | Trivial | Several large redemptions can all pass and collectively exceed the reserve, so escrows are returned rather than paid |
| No cap | Maximum freedom | The reserve is drainable in one transaction |
| Pause redemptions | "Bounds" the outflow | **Rejected.** Pausing outbound is taking hostages; it destroys the exit that makes governance safe (D11, D12) |

### 7. Parameter governance

**Reversed.** The PoC position used to be "no governance; parameters fixed in code". Governance now
exists by decision (D10): **85% of pledged coins, 30 days, live from the moment it is raised**, and
it **holds the upgrade authority**.

| Option | Pros | Cons |
|---|---|---|
| **85% / 30 days / live signal, holding the upgrade authority** ✅ | Names who holds A5 instead of leaving it unowned; a hostile proposal is visible while it is only a proposal; redemptions run throughout, so holders can leave | 85% of a small bond set is a small number of members; a holder who does not watch for 30 days is exposed — disclosed, not solved |
| **No governance; parameters fixed in code** *(the previous position)* | Nothing to capture; `FLOOR` cannot be voted down | Nothing can be adjusted without shipping a new program; the upgrade authority is still an unowned mint voucher (A5) |
| An immutable floor, with safety increases only | Bounds the damage a vote can do | There is no floor to make immutable while governance holds the upgrade key; it would be a promise, not a mechanism. The exit window is the real protection |
| Single multisig, all parameters | Simple | A key compromise can set `FLOOR = 0` and mint against unconfirmed blocks |
| Token vote on safety parameters | Legible | **Safety parameters should not be votable on their own** — a majority can strip its own protection; they are governed as part of a change that must also survive the exit window |

### 8. Exit gating for fresh supply

| Option | Pros | Cons |
|---|---|---|
| **Nothing — the exit is never gated** ✅ | The exit is what makes governance safe; redemptions are never pausable (D11, D12) | A released fraudulent mint can be exited; `FLOOR`, maturity and the bonds are what bound it |
| Per-token maturation (new mints non-redeemable for N blocks) | Targets only fresh supply | **Tokens are fungible** — the attacker sells on Raydium and an innocent buyer holds the unbacked token. Taint cannot follow without a denylist, which we have deliberately ruled out |
| Global peg-out pause after a detected reorg | Blocks the exit path the attack depends on | **Rejected** — it takes hostages and removes the protection that makes governance tolerable |

---

## Decisions from review

1. **No privileged operator for minting; the reserve is the one trusted component.**
   *(Reversed from an earlier "no operator at all" framing.)* Advancing headers is permissionless,
   and minting has no trusted participant: the program verifies every header and every Merkle proof
   itself, and mints to a PDA authority. The **reserve** is different — it is held by a threshold of
   bonded members, and that is a trust assumption, stated as one. The single-relayer model is
   superseded; the bond set is open to anyone with 1,000 BSV.

2. **Bond `k` — settled: `k = 1` (D5).** `solBSV` and BSV are the same asset, so any deviation is an
   arbitrage and closes; `k = 1` is defensible rather than having to absorb a standing discount.
   Self-dealing is not the framing any more — there is no per-relayer underwriting to self-deal
   against — but the consequence is recorded plainly: at `k = 1` the bonded stake **equals** what a
   colluding threshold could take rather than exceeding it, so what makes collusion unattractive is
   the 30-day exit and the visibility of a live proposal, not the bond alone.

3. **A failed redemption — return the escrow, and nothing else moves.** The holder's `solBSV` is
   still in the vault, so the settlement is to return it: the holder is made whole and **supply is
   unchanged**. The bond is **not additionally transferred** to the holder, because the return has
   already made them whole. This is the honest consequence of refusing a freeze authority:
   **holders keep their balance through a default and absorb any shortfall as a discount rather
   than a confiscation.** The bond protects against a member's provable misbehaviour — **not**
   against the reserve being short, **not** against a failed redemption, and **not** against a
   reorg.

4. **`FLOOR` is a policy parameter, not a derived one — and it is now governed.**
   *(Partially reversed: the value stands, the change mechanism does not.)* Sizing it from the cost
   of reorging `FLOOR` blocks needs a **BSV price feed** to value what is at risk and a
   **hashpower-rental feed** to price the attack.

   The reason it is rejected is **not** that those quantities are unknowable — they are entirely
   knowable and worth showing. It is that they must stay **outside the protocol**. Tuning the cap
   inside consensus would mean consulting an external market from the program itself, and the
   program consults nothing external: by construction `1 BSV = 1 solBSV`. Importing oracles to
   derive one integer would trade away the property the whole design rests on, in exchange for a
   number that would still be a guess dressed as a calculation.

   > **A metric is not an oracle.** BSV price, estimated reorg cost, hashrate and the observed
   > block rate are all fair game as *published information* — they help a user judge the risk they
   > are taking. What they must never do is change what the program *does*.
   >
   > **Display anything; decide on nothing external.**

   So `FLOOR` is set at **12 blocks**, disclosed before someone transacts, and changeable only by
   85% with 30 days of live signal (D10). **The earlier "increase-only under governance" proposal
   is superseded:** with governance holding the upgrade authority there is no immutable floor to
   protect, and the protection is the exit window (D11).

5. **Reorg detection is depth-aware and the program decides release from its own headers.** The
   depth rule is settled: a reorg of depth `R ≥ FLOOR` is the signal, and `R < FLOOR` is ordinary
   tip churn and is ignored. `commit_fork` now re-anchors the branch (`fork_parent_hash`, W1.7) and
   compares **chainwork**, not height. Block times are directly available — every `HeaderRecord`
   stores `time` — so the observed mining rate over the window is computable on-chain, and a mean
   spacing far below ten minutes is worth warning users about. A genuine hashpower surge looks
   identical to an attack, so the choice is not free; note the variance is real, since a dozen
   blocks is a small sample.

### Resolved from review

6. **Fee mechanism — reversed: a governed fee, 30 bp each way.** The order book and discovered
   fees are **removed**. A governed fee solves what the book solved — discovery and
   capacity allocation — more simply (the "bond cap" once cited here is **withdrawn**: the bond is
   the float), and the book was the one subsystem that never received an
   adversarial review. The fee is a parameter, changed by 85% / 30 days, and it is disclosed. The
   alternatives once considered — the book, a user-set fee with open fulfilment, an exclusive epoch
   right — are recorded as rejected.

7. **Reserve invariant — settled (D8): monitored, not enforced.** `custodied BSV ≥ outstanding
   solBSV` is published and monitored, and **the protocol cannot enforce it**, because the reserve
   is off-chain BSV the program cannot read. The earlier worry that re-mints make it transiently
   false is gone: no failure path mints, so supply only changes when the vault burns a staged mint
   or settles a redemption.

### The aggregate mint cap, restated as policy

`MAX_PEG_IN` still does not bound a reorg — a BSV block holds thousands of transactions, so an
attacker fills a fraudulent branch with many deposits under any per-transaction limit. The bound
that works is an **aggregate cap per window**.

But decision 4 applies to it too: it is **set conservatively as policy, not derived**, because
deriving it needs the same two oracles we have excluded. It is a coarse backstop beneath `FLOOR`,
not a calibrated constant — and it is secondary to the two bounds that need no price oracle at all:

- **`FLOOR` set at 12 blocks**, which is what makes out-mining the honest chain expensive;
- **the Greycore co-signature**, which stops the gateway majority moving funds alone; the bonded
  stake is the **float** and is no longer a capacity bound.

All of these are **published and disclosed**. The disclosure matters more than the value: a user
should be able to see `FLOOR`, maturity, the cap and the bonded stake before they transact.

Monitoring is also where the **external metrics** belong — the ones that must never enter
consensus. A user deciding whether to peg in is better served by seeing the BSV price, an estimated
cost to reorg `FLOOR` blocks, the current hashrate and the observed block rate alongside the
parameters than by seeing the parameters alone. Published, none of it is a trust assumption;
consulted by the program, all of it would be.

## O1 — Fees are governed, and paid pro rata to pledged stake

**Reversed from "yield is paid in the asset staked".** With per-relayer custody removed there are no
two staking sides to pay separately. There is one fee, 30 bp on each leg, and it accrues to the
members pro rata to their pledged stake.

| Leg | Fee | Paid when | To |
|---|---|---|---|
| Peg-in | 30 bp | Deducted where the value moves | Members, pro rata to pledged stake |
| Peg-out | 30 bp | Deducted where the value moves | Members, pro rata to pledged stake |

**Fees mature on the same schedule as the principal.** A fee withdrawable immediately while the
mint behind it is still maturing would be an exit from maturity — the one thing the vault exists to
prevent. So a fee is credited at once and *released* on the same schedule. That costs nothing in
practice: the window is hours and the accounting is continuous.

**The arithmetic still lands at 1:1.** A peg-in of 100 BSV mints `100 − fee` into the vault and
credits `fee` to the members, so custody and supply move together for the deposit; the fee is a
transfer between the parties, not new backing, and if anything it leaves the system
over-collateralised. A peg-out burns the escrowed amount and pays out the same amount less the fee,
so both sides fall together. Neither leg creates a claim on nothing.

**This also makes the node a funded job.** Advancing headers, verifying independently, signing
intents and challenging theft are all work, and the fee is what pays for it. The earlier design had
detection as an unpaid chore nobody owned (P7); that is now a member's job with a member's bond
behind it.

## O2 — There is no privileged operator, but the reserve is trusted

**Reversed, and worth being exact about.** An earlier draft of this section said "there is no
operator, and that was the point" — that a relayer is a role anyone may run, with a roadmap to a
signerless reserve. That is no longer the whole picture: there **is** an operator class now, the
federation, and it **holds the reserve**.

What has and has not changed:

- **Minting still has no privileged participant.** Advancing headers is permissionless, the program
  verifies proof of work and inclusion itself, and the mint authority is a program PDA. A forged
  header costs real work rather than one hash (A1).
- **The reserve is a 2-of-2, and the gateway leg is a threshold.** No gateway majority and no
  Greycore can move it alone, and the bond that backs each member is `solBSV` the program holds and can seize.
- **Redemption trust is bounded and explicit.** Members sign payout intents individually; the
  gateway threshold key signs and the **Greycore co-signs** only once enough attributed intents exist; and a member who signs two
  conflicting intents has produced their own proof of guilt. A **threshold that colludes** is the
  residual, and it is a governance matter with the 30-day exit as the answer — not something to be
  argued away.

### How maturity stops a reorg mint, from the program's own headers

This is the part worth stating precisely, because it is where the design succeeds:

1. Minted `solBSV` goes to a **program-owned vault**, not the depositor.
2. The vault releases only once the deposit's block is **still canonical N blocks later** — read
   from headers the program already stores.
3. If BSV reorgs, the program follows the heavier branch through the permissionless `commit_fork`
   path and **burns the still-staged tokens**. The fraud never becomes liquid.

No external data is consulted at any step and no person decides anything. The only requirement is
that **honest headers are pushed within the maturity window** — a liveness condition, and under the
federation one that a funded member job satisfies: a fraud that goes undetected eats the bonded
stake, so the parties with the most to lose have the most reason to advance the honest chain and
notice an orphan.

**What maturity does not fix:** if nobody pushes the honest branch for the whole window, the staged
tokens release and the attacker leaves. That is why `FLOOR` remains — it makes the reorg expensive
whether or not anyone notices — and why the bonds remain, to cover the case where notice arrives
too late.

## Why the order book was removed

**Recorded because it was a real design, not a draft.** Underwriting used to be an order book:
members posted sell orders — how much liquidity, at what fee, at which confirmation depth — and
incoming requests matched them by price then time, partially filled. It solved two things: the fee
was **discovered** rather than chosen, and **capacity allocation** fell out of matching.

**Both are now handled elsewhere:**

- **The fee** is a governed parameter, 30 bp each way, changeable by 85% with 30 days of signal.
- **Capacity** is the bonded stake. With `k = 1`, total value locked is capped by total bonds
  pledged, and the cap is the same number that absorbs a fraud. There is nothing to allocate.

**And it removes a subsystem that had never been adversarially reviewed** — the audit-gap list in
doc 18 flagged the book's economics as unexamined, and D2's auto-approve rested on reasoning that
had not been tested. Deleting it removes the gap rather than paying down the debt.

One piece of the book's reasoning survives and is worth keeping, because it was a correction to an
even earlier draft: **the vault, not per-fill approval, is what reverses a reorged fill.** A fill
was never liquid, so a detected reorg reversed the liability and the staker lost nothing. That
argument is still right; it is the reason auto-approval was defensible. What the federation model
changes is not the argument but whether there is a per-fill decision to make at all — there is not,
because there is no per-deposit underwriter.

## The bond and `owed` — definitions

Used throughout documents 04, 13, 14 and 23 and previously defined only for the retired per-relayer
design. It is the quantity the bond is measured against, so it needs to be exact.

> **`owed` is the total `solBSV` obligation the program has recognised and not yet discharged,
> accumulated only from proofs the program has itself verified, and reduced as each is discharged.**
> A member's bond must cover `k ×` its attributed share of `owed`.

It includes:

- **Staged mints still in the vault.** A depositor whose mint is still maturing has paid BSV and
  holds no tokens. If the system stopped at that moment the depositor would have lost the whole
  deposit, so it must be covered. Omitting staged mints leaves exactly that window unbonded, which
  is audit finding **F4** (P10).
- **Released mints** — `solBSV` a holder may redeem in BSV.
- **Escrowed redemptions in flight**, since the obligation to pay exists until the payout settles
  or the escrow returns.

It excludes the reserve's own BSV, which is the asset behind the obligation rather than a liability.

**Why it is the right unit.** `owed` is derived from proofs the program verified itself, not from
an attestation by anyone, and the single bond was `solBSV` the program held, so the inequality
`bond ≥ k × owed` was **checkable on-chain** (audit A4). **Superseded by the two-sided bonds (D14):**
the single `owed` counter and the single `bond ≥ k × owed` formula are replaced by
`bsv_bond ≥ k × (BSV held)` and `solbsv_bond ≥ k × (solBSV held)`, so that a bond is no longer the
same asset, or inside the same reserve, as the liability it covers. **What is new in this revision,
and what is not yet designed:** the exact rule that turns each side's exposure into a per-member
attributed share. Under the 2-of-2 script every member is exposed to the whole reserve's obligations,
so the attribution rule is part of the key design, and it is listed as an open item.

## Is the buffer a protocol input? — yes, and it can be checked

> **The `owed`-based formulation below is historical.** It is preserved because it is why the gate
> exists; the current gate is the **two-sided bond check** (D14). The `solBSV`-side bond is seized by
> the program; the BSV-side bond is seized by the members collectively under the collective key.

Review asked whether the buffer must be a protocol input, and whether mint and redeem should be
gated on it per transaction. **The bonds are; the BSV reserve is not.** The program knows the
quantities without an oracle:

- **what each side holds**; and
- **each bond** — the `solBSV` side is program-held and seizable, the BSV side is held under the
  collective key.

So the gate is a check the program can evaluate for itself from quantities it verified — *designed,
not built*:

> Each bond must cover its side; the `solBSV` bond is seized by the program when it does not, and the
> BSV bond by the members collectively. A redemption is paid only once enough attributed intents
> exist; a redemption that cannot be paid returns the escrow.

If the buffer is meant as a protocol input, **this is the input.** The off-chain BSV reserve is not,
and does not need to be — the bonds are the things that can be seized, so they are the things worth
gating on.

### The bootstrap path

The gate has a chicken-and-egg problem, and **the federation model makes it sharper, not weaker**:
nothing can mint until the reserve address exists, and the address is held by members whose redeem
bond is `solBSV` — which does not exist until a mint happens. **D16 settles the path: members post a
BSV-side bond at genesis**, so no `solBSV` needs to exist first. `D3` still settles the *shape* of
the first supply as G2, the vault-gated genesis mint: the genesis mint lands in the program vault and
is released only once a matching BSV deposit is verified, so supply exists but is never liquid until
it is backed — no unbacked window to attack, and nothing to keep quiet about. The alternative — a
**capped, explicitly-unbonded first mint** — is recorded as a documented later option, not chosen,
because it leaves the first mint backed by nothing but the members' word (`docs/14-decisions.md`).
The bootstrap still needs to be a separate instruction with its own rules and its own test, not a
special case buried in the mint.

### On the reserve: pool it under a threshold, rather than distributing it

**Reversed.** Review once asked whether the reserve should be an aggregated pot; the answer was no,
because aggregation creates one key worth stealing, and a stolen key needs either a covenant or a
committee. Deposits were sent to individual relayers and there was no bridge address at all.

**The federation model takes the other branch of that fork deliberately.** A BSV Script cannot
constrain where a key sends funds, so an aggregated reserve does need either a committee or the
covenant track. The design now chooses the **committee, with teeth**:

| Shape | Reserve key | Needs |
|---|---|---|
| **Pooled, 2-of-2 with the Greycore** ✅ | The gateway threshold key plus the Greycore | Two-sided bonds (the float), Greycore-admitted membership, self-proving slashing, the exit window, and continuous publication of the ratio |
| Pooled, single key | One key | Unacceptable — one theft drains everything |
| **Distributed per relayer** *(the previous design)* | Each relayer's own key | Nothing beyond a seizable bond, but there is no threshold, no governance layer and no single reserve to attest to |
| Pooled, covenant | Script enforces the burn proof | `OP_CAT`, `OP_MUL`, in-script verification — pre-mainnet and unaudited |

**The honest accounting of the trade:** the previous design had no reserve to secure and no
threshold to trust, but it also had no operator layer to detect, challenge, govern or pay. The
federation adds all of that and pays for it with one explicit trust assumption — **a threshold of
members do not collude** — bounded by seizable bonds and by an exit that cannot be paused.

**The covenant track remains the destination.** Script releasing funds only against a proof of the
burn, via `OP_CAT`, `OP_MUL` and in-script verification, removes the spending key entirely. That is
still the end state, and it is the reason the redemption authority stays replaceable.

## Flows at a glance

```
PEG IN — BSV to solBSV
  1  CHOOSE    terms; the fee is 30 bp, governed
  2  SEND      BSV to the federation's deposit script
               OP_RETURN = version ‖ cluster_id ‖ program_hash ‖ flags ‖ recipient
  3  DEPTH     12 confirmations (FLOOR)
  4  STAGE     solBSV issued into the program's vault, not to the depositor
  5  MATURE    144 blocks
  6  RELEASE   permissionless: tip advanced past the deposit and the stored hash still matches
               -> vault releases to the recipient; fee to the members
               if the hash DIFFERS -> the staged tokens burn; the depositor keeps the reorged BSV

PEG OUT — solBSV to BSV
  1  ESCROW    solBSV moves into the program's vault; a BSV destination and deadline are set
  2  ACCEPT    members sign payout intents individually, on Solana
  3  PAY       enough attributed intents -> the gateway threshold key signs and the Greycore co-signs
  4  SETTLE    the payout is proved against the light client; the escrow burns; fee to the members
     or
  4' CANCEL    permissionless after the deadline: the escrow returns to the holder
```

Both directions have the same shape: **enter the vault, then leave it either to the counterparty or
back to the sender.** Every failure path is a return rather than a mint.

---

## Decisions D1–D8, in full — and the reversals

[`14-decisions.md`](14-decisions.md) is the register; the reasoning behind each is here, with
reversals marked. Every reversal is dated **2026-09-29**.

### D1 — Who may join: **REVERSED — Greycore-admitted membership, 1,000 BSV bond**

*Was:* specialists first, anyone-may-stake a phase-2 goal.

*Now:* **the Greycore — the trusted, non-operational body — finds and admits replacement members**, and
members post the **two-sided 1,000 BSV bonds (the float)**. The
phase-2 deferral is gone because the gate is the thing that made it necessary.

| Option | Pros | Cons |
|---|---|---|
| **Greycore-admitted members with two-sided 1,000 BSV float** ✅ | Trusted admission with objective bond requirements; fees pro rata to stake | Requires a trusted admission body; the bond is the float, not a scale limit |
| Specialists-only, nominated | Sophisticated parties who can price reorg and custody risk | Nomination is a trusted choice; excludes everyone else; no objective entry rule |
| Delegated staking (retail pledges to an operator) | Deep liquidity; a familiar model | Retail cannot assess operator risk, so slashing lands on people who could not evaluate it |

### D2 — Fees: **REVERSED — a governed fee, 30 bp each way**

*Was:* fees discovered on an order book, with auto-approving bids.

*Now:* **30 bp on each leg, governed** (85% of pledged coins, 30 days, live signal). The book is
removed for the reasons in §Why the order book was removed. The auto-approve question dissolves:
there is no per-bid fill to approve.

### D3 — Genesis: **G2, the vault-gated genesis mint — shape stands, bootstrap reopened**

**Settled in shape.** The genesis mint lands in the program vault and is released only once a
matching BSV deposit is verified. No unbacked window exists at any point, so there is nothing to
attack and nothing to keep quiet about. The alternative shapes were considered and are recorded for
the deferred design:

| # | Option | What breaks the loop | Assumption it carries |
|---|---|---|---|
| **G1** | **Self-underwritten genesis.** The founding members deposit their own BSV and mint against it | The founders are honest | The same assumption `initialize` already makes when it sets the checkpoint — no *new* trust |
| **G2** ✅ | **Vault-gated genesis mint.** A genesis mint goes straight to the program vault and is released **only when a matching BSV deposit has been verified** | Nothing — the vault means there is no unbacked window to attack | A staged token is not stealable, so it does not matter who knows |
| **G3** | **Genesis bond in SOL, migrated to `solBSV` later** | A non-`solBSV` asset | A price mismatch, which doc 04 argues against — acceptable only for a bounded, short genesis |
| **G4** | **Capped unbacked genesis.** The first `X` BSV of peg-ins need no bond, because the amount at risk is bounded and small | A hard cap, nothing else | The cap is genuinely below what anyone would bother attacking |
| **G5** | **Slot-expiring genesis authority.** A named key may seed a bounded amount until a slot, then is permanently dead | A privileged window | The window is short and bounded, and the authority is provably dead afterwards |
| **G6** | **Compile-time test mint** (`#[cfg(feature = "poc")]`) | Nothing — test only | Deliberately not a runtime flag: a runtime flag can leak to mainnet, a compiled-out one cannot |

**What changed:** G2 answers *how the first supply is backed*. **How the first members bond is now
decided (D16): a BSV-side bond at genesis**, so no `solBSV` needs to exist first. The **capped,
explicitly-unbonded first mint is the documented alternative** — it is G4 above — not chosen,
because it leaves the first mint backed by nothing but the members' word. G2 composes with G1 — the
founders self-underwrite, the vault holds the result until the BSV verifies — which requires no new
instruction beyond the ordinary peg-in path.

### D4 — `FLOOR`: **PARTIALLY REVERSED — 12 blocks stands; "fixed in code" does not**

*Was:* 12 blocks, fixed in code, no change mechanism.

*Now:* **12 blocks, a governed parameter.** The value was never the problem; the named gap was that
there was no way to change it. Governance (D10) is the change mechanism, and **the floor itself is
not made immutable** — the exit window is the protection (D11).

### D5 — The bond multiple `k`: **`k = 1` stands; the single-bond formula is superseded**

**Settled by D14 as two-sided bonds.** `k = 1` per side, with
`bsv_bond ≥ k × (BSV held)` and `solbsv_bond ≥ k × (solBSV held)`, **neither bond inside the
reserve**. The single formula `bond ≥ k × owed` — and the `aggregate_bond ≥ k × non_bonded_supply`
that replaced it — are **superseded**: a bond denominated in the asset it protected, and held inside
the reserve it covered, was funded by a deposit into the very thing it was meant to cover. **The
consequence is recorded rather than softened:** the maximum loss from a colluding threshold is the
**entire non-member supply**. The old `B_h + H` figure and its 1.15× / 1.49× multiples were computed
for the single-bond model where the bonds sat inside the reserve, and the two-sided bonds supersede
that arithmetic. What makes collusion unattractive is the live signal, the exit, and **continuous
publication of the reserve and supply**, not the bond's excess size. The BSV-side bond cannot be
seized by the Solana program; the `solBSV` side can.

### D6 — The unbacked path: **SUPERSEDED — there is no per-deposit underwriter**

*Was:* a peg-in may proceed with no underwriter at all, explicitly allowed.

*Now:* the question does not arise in the same form. Minting is permissionless and trustless, and
there is no per-deposit underwriter to be present or absent — a deposit pays the federation's
deposit script, the program verifies it, and what backs it is the reserve and the bonds. **The
residual is re-opened as an open item:** doc 23, question 3 asks whether the federation model needs
an explicit bound on how much of the reserve may be exposed before the bond set is large enough.

### D7 — Governance: **REVERSED — 85% / 30 days / live signal**

*Was:* none in the PoC.

*Now:* see D10 below. "No governance" is reversed because it left the program upgrade authority as
an unowned mint voucher (A5) and because "we launched without governance" is not a position that
survives contact with a reserve. The full reasoning is in [`23-federation.md`](23-federation.md).

### D8 — The reserve invariant: **monitored, not enforced — stands**

**Settled, and unchanged by the federation.** `custodied BSV ≥ outstanding solBSV` is published and
monitored, and **the protocol cannot enforce it** — the reserve is off-chain BSV the program cannot
read. The website shows the ratio; the program does not check it. A **2-of-2 script with the Greycore**
changes who holds the reserve, not what a Solana program can see — though the **reported
spent-outpoint record** is what the program checks a mint against.

### D9–D13 — new and settled under the federation model

| ID | Decision | Note |
|---|---|---|
| **D9** | **Greycore-admitted membership, two-sided bonds** (1,000 BSV per side, the float), **neither inside the reserve** | Reverses D1; updated by **D14**. A member cannot leave while owing on either side; **leaver-shares are an open finalisation item** |
| **D10** | **Governance: 85% of pledged coins, 30 days, live signal, holds the upgrade authority** | Reverses D7. All four numbers are parameters |
| **D11** | **The floor is the exit, not immutability** | No immutable floor, deliberately; redemptions are never pausable. The residual — a holder who does not watch for 30 days — is disclosed |
| **D12** | **Pause stops mints only** | A lower threshold than a governance change, and it lifts automatically. Pausing outbound is taking hostages |
| **D13** | **Slashing is self-proving equivocation** on individually-signed intents | Copied from what RenVM actually shipped; the cryptographic half only. A colluding threshold is a governance matter |
| **D14** | **Two-sided bonds, the float**, one per direction, **neither inside the reserve**; the BSV side under the **collective key**, seized by the members collectively | Supersedes the single-bond formula. The `k × (held)` line is a coverage floor, **not a capacity ceiling**; slashing pays the slashers from the seized bond |
| **D15** | **A 2-of-2 `OP_CHECKMULTISIG`** — the gateway threshold key plus the **Greycore**'s | **Reverses audit F10:** with the Greycore co-signing, the deposit script genuinely is a multisig, so `is_p2pkh` must change and `DepositScript::SPACE` must grow to ~71 bytes. `fed.threshold` = 4-of-N, `N` open |
| **D16** | **Genesis: a BSV-side bond** | No `solBSV` needs to exist first. A capped, explicitly-unbonded first mint is a documented later option, not chosen |

## Audit findings

An adversarial review ran this document against the implemented program. Severities are the
reviewer's. **Fixed** means addressed in code with a test where one was possible. **The table is
kept as history** — it is a record of the design as it stood when it was written — with a note where
the federation model changes what the finding means.

| ID | Finding | Severity | Status |
|---|---|---|---|
| **A1** | **The difficulty target was read from the header being checked.** `bits` came from the submitted header and `check_daa` returned `true` unconditionally, so the target was attacker-chosen and proof of work was vacuous — anyone could append headers and mint with no hashpower. Every "forging `FLOOR` blocks must out-mine the chain" claim here was false against the code. **Regtest masked it**: `0x207fffff` is already the largest encodable target, so no easier one exists there | critical | **Fixed** — the target is now computed per block by cw-144, verified 324/324 against real mainnet headers. Test added |
| **A2** | `set_checkpoint` and `set_paused` were **unauthenticated**: `authority` was a bare `Signer` compared to nothing, so any key could rewrite the trusted root or halt minting | critical | **Fixed** — authority stored, `has_one` enforced. The deploy-time race on `initialize` remains (P1) |
| **A3** | `verify_deposit` never constrained `mint`, so anyone could submit a valid public deposit against a counterfeit mint and burn the replay slot — stranding the real deposit for ~0.001 SOL | critical | **Fixed** — pinned to the `[b"mint"]` PDA |
| **A4** | The staking buffer is off-chain BSV — unverifiable and unseizable, so `S ≥ M` was unenforceable as written | critical | **Superseded by the federation model.** The bonded stake is `solBSV` the program holds and can seize — and it is the **float**, not a scale limit (that claim is withdrawn); the reserve's BSV itself is still off-chain and unreadable, and is still monitored rather than enforced (D8) |
| **A5** | The **program upgrade authority is an unconditional mint voucher.** Safety parameters are Rust `const`s, so "loosening needs a new program" and "ship a new program" are the same power | critical | **Owned by governance now (D10).** Governance holds the upgrade authority, so the power is named rather than unowned; the 30-day live signal and the never-pausable exit are what bound it. It is not removed |
| **A6** | The peg-in destination is a **P2PKH key, not a covenant** — the entry point for the whole reserve is a raw key | critical | **Reversed by design decision, and then the P2PKH shape was itself reversed.** The reserve is pooled under a **2-of-2 `OP_CHECKMULTISIG`** (D9/D15) — the gateway threshold key plus the **Greycore**'s, both required. **F10 is reversed:** `is_p2pkh` must change and `DepositScript::SPACE` must grow to ~71 bytes. The covenant track remains the destination |
| **A7** | **The replay key included `height`**, so a deposit re-included at a different height after a reorg minted twice | serious | **Fixed** — identity is `(txid, vout)`; height stored only for pruning. Confirmed in code |
| **A8** | The staging escrow is **not implemented**, and `UsedDeposits` stores no recipient or amount, so reversing N credited mints needs an off-chain indexer | serious | **Open** — the vault is designed, not built |
| **A9** | `MAX_USED = 200` against a `WINDOW` of 192 is a cheap peg-in shutdown; `MIN_PEG_IN`/`MAX_PEG_IN` are unimplemented | serious | **Decided (P5)** — `MIN_PEG_IN = 1 BSV` plus a **nullifier PDA** per minted deposit, which removes the ceiling. Not built |
| **A10** | The aggregate cap is not tied to the buffer, so `S ≥ M` cannot hold by construction | serious | **Open** — the aggregate cap is still policy, not implemented, and the bonded stake is the only bound in force |
| **A11** | `commit_fork` never re-anchors the staged branch, so an intervening commit can splice the window from two chains with broken linkage | serious | **Fixed** (W1.7) — `fork_parent_hash` is recorded at `init_staging`, linked from `push_fork_header`, and re-checked at `commit_fork` (`ForkPointMoved`). Commit also compares chainwork, not height |
| **A12** | `commit_fork` only emits an event — no depth recorded, no pause, no bounty — so the gate is triggered off-chain and is itself griefable | serious | **Partly addressed by the federation model** — detection is a funded member job, and the program decides release from its own headers; recording depth on commit is still to do |
| **A13** | "No oracles, by construction" is false for quantities that gate funds: the reserve is off-chain BSV | serious | **Open, and now stated** — the trust division in doc 13 names the reserve as trusted rather than pretending otherwise; it is bounded, not verified |
| **A14** | The committed confirmation depth is not parsed, and **would not bind an attacker anyway** — the fraud's depositor *is* the attacker, so they commit exactly `FLOOR` | serious | **Closed by reversal** — the committed-depth market term is removed with the order book; `FLOOR` is a governed floor. Nothing to parse |
| **A15** | Return-to-sender is not an on-chain path: no refund instruction, `parse_outputs` reads only outputs, and spending the deposit needs a key | serious | **Open** — see §If a refund is ever needed |
| **A16** | The bond asset contradicts across documents, and only the Solana leg is seizable | serious | **Resolved by decision** — **two-sided bonds, neither inside the reserve** (D14): the redeem side is `solBSV`, seized by the program; the mint side is native BSV outside the reserve, **held under the collective key and seized by the members collectively** (the slashers paid from it). The asymmetry is stated, not hidden |
| **A17** | Pausing freezes `push_header` too, so the tip stalls and unpausing needs the missed headers replayed one transaction at a time | minor | **Open** — and under D12 the pause is mints-only, so it should not touch `push_header` at all |
| **A18** | Minor mismatches — `bits_to_target_be` masks the sign bit, and the adversary playbook expects an error that does not exist | minor | **Partly fixed** — `used_deposits` is now pinned to `[b"used_deposits"]` (X2); the sign-bit mask and the stale playbook expectation remain |

**The lesson worth keeping.** A1 and A2 were invisible to a green suite: the tests asserted that bad
proof of work was rejected, and it *was* — against the target the test itself supplied. A2 had no
test at all. Passing tests demonstrated that the code did what the tests did, not that the client
was secure.

## Deliberately deferred

Recorded so these are not later relitigated as oversights. Most are **decisions to defer** rather
than open questions; each is a known simplification or an unbuilt part of the current model.

| Deferred | From | Note |
|---|---|---|
| **The genesis bootstrap — decided** | D16 | Members post a **BSV-side bond at genesis**, so no `solBSV` needs to exist first. A capped, explicitly-unbonded first mint is a documented later option, not chosen |
| **Sharding the gateway key** | D9 | One gateway key across all members, or several groups with their own? Shards contain theft and latency at the cost of coordination |
| **The key design** | D9/D15 | Key generation and signing protocol for the gateway **threshold ECDSA** key and the **Greycore**'s, and the attribution rule that turns each side's exposure into a per-member share. `fed.threshold` = **4-of-N** with `N` deferred; `fed.greycore_size` / `fed.greycore_threshold` `open` |
| **Leaver-shares** | D9 | A departing member retains a valid share, so the effective threshold degrades with churn. Key rotation (the reserve moves and `deposit_script` changes) or proactive re-sharing (needs the leaver's cooperation). Not finalised in the PoC |
| **The vault's re-audit** | — | The current vault design carries unfixed findings; it should be re-audited against this model, since several findings came from trying to enforce BSV-side behaviour the federation now handles differently |
| **The unbonding period** | D9 | Longer than the redemption deadline plus the challenge window; the value is a parameter |
| **Bounding the unbacked path** | D6 | Whether an explicit exposure cap is wanted before the bond set is large enough (doc 23, question 3) |
| **X3 — the hard-coded DAA** | A1/W1 | cw-144 is implemented, but BSV may change the rule. Governance can carry the upgrade; a parameterisable rule is not built |
| **The program upgrade authority** | A5 | Now held by governance (D10); the residual is the 85% threshold over a small bond set |
| **An independent audit** | — | The critical defects found so far were found by our own adversarial review |
| **Fee realisation mechanics** | O1 | Whether members withdraw from their own balance or accrue a claim is unresolved |

## What this changes downstream

The design implies these changes to the implemented program. **The shipped program is light client
+ token + mint + fork staging; everything below is designed, not built.**

- **`MIN_CONFIRMATIONS`** stays 12 and is the code's form of `FLOOR`; **`FLOOR` as a distinct
  governed parameter is not built.**
- **The difficulty retarget** is **implemented** (W1.6): cw-144 per block, 324/324 exact. **X3
  remains** — the rule is hard-coded and needs a way to change without a redeploy, which governance
  can now carry.
- **The vault** is the largest build item: staged mints, `release_mint`, `burn_staged`, escrow and
  return, all permissionless where the program can decide.
- **The federation** — membership, the two-sided bonds, the **threshold ECDSA key**, individually-signed
  intents, governance and slashing — is designed in [`23-federation.md`](23-federation.md).
- **The mint path** gains the gate checks, the fee, and mints into the vault rather than to the
  depositor; **the redemption path** is new end to end.
- **`commit_fork`** should record reorg depth and block time when it fires, feeding the depth rule
  and the pause.
- **`push_header`** now stores the header timestamp, so the regression, catch-up and staleness
  signals are computable on-chain. **The pause must not touch `push_header`** (D12, A17).
- **The header window** (32 h / 192 records, set by what cw-144 needs) is a *liveness* parameter and
  should not be conflated with `FLOOR`/`MATURITY`, which are *safety* parameters. They are named
  apart above.
- **A per-window mint cap** (`MAX_MINT_PER_WINDOW`) is wanted as a coarse backstop, set by policy
  rather than derived.
- **The replay list should become a nullifier PDA** (P5), which removes the 200-per-window ceiling
  and the pruning logic at once.
- **Burning a staged mint must release its replay entry** (F1), or a re-included deposit can never
  be re-proven and an honest depositor's BSV is stranded.
- **The deposit `OP_RETURN`** should carry `version ‖ cluster_id ‖ program_hash ‖ flags ‖
  recipient`, which closes cross-deployment replay (P11) by binding the deposit to this deployment.
- **Tests** in `poc/TEST_PLAN.md` §4 and §5 gain the vault, fee and capacity cases.
