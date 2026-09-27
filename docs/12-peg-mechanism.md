# 12. Peg-in and peg-out: the mechanism

> **Status: proposed, for review.** Nothing below is built beyond what is already
> in Phase 2. This document exists so the mechanism can be argued about on paper
> — with the trade-offs written down rather than rediscovered — before any of it
> is coded. **§Recommended state is the thing to review.** The tables in
> §Considerations record why each choice was made and what was rejected.

---

### Vocabulary

Four things are easy to conflate, so this document names them apart:

| Term | Means |
|---|---|
| **The program** | The on-chain Solana program. Consensus. Consults nothing external |
| **The peg** (or the bridge) | The whole mechanism — program, relayers, deposits, redemptions |
| **The website** | `solbeam.me`. Order entry, the depth recommendation, relayer adverts, the status page. **Nothing here is consensus** |
| **Monitoring** | The published surface: parameters, external metrics, Solana liveness |

The distinction does real work. A recommendation or an advertised preference on the
website can change as often as we like and carries no trust assumption; the same
value influencing a mint decision in the program would be an oracle. "The system"
is not used in this document, because it could mean any of the four.

### No oracles, by construction

The mechanism needs none. It is worth being explicit about where each would
otherwise have appeared and what replaced it:

| Would have needed | Replaced by |
|---|---|
| A hashpower or price feed to compute a "safe" confirmation depth | A voted `FLOOR`, plus a depth the depositor chooses and relayers price |
| A feed to price reorg risk | Relayers pricing it into the fee; the website publishing the metrics |
| A price feed to value the bond | `1 BSV = 1 solBSV` by construction. A deviation is an arbitrage, not an input |
| An external source for *has BSV reorged, and how deep?* | **The BSV headers themselves.** Chain data, not external data |
| Wall-clock to expire a redemption deadline | **Solana slots.** A halt freezes the clock instead of burning the relayer |
| A price oracle to size the mint cap | A conservative policy value, published, with the metrics shown beside it |

Two rules cover every row:

- **Chain-native.** Anything the program must *react* to is read from a chain —
  reorg depth and block time from BSV headers, deadlines from Solana slots.
- **Market-native.** Anything that would need external data becomes a term of
  trade, discovered off-chain and published on the website.

**The market layer is not merely advisory**, and this is the part that makes the
substitution work. The depositor commits their chosen depth in the deposit's
`OP_RETURN`, so the program **enforces** a market-negotiated term while never
knowing an external price. It does not need to understand *why* the depth is what
it is — only that the commitment is honoured and sits at or above `FLOOR`.

**The one residual judgement is `FLOOR` itself.** It is a voted policy value rather
than a measurement: governance, not an oracle. The design has no oracles; it does
have a parameter somebody has to choose, and *that* is the thing worth arguing
about.

## Recommended state

### Peg-in — BSV → solBSV

```
  1. SEND          user pays ≥ MIN_PEG_IN to the bridge P2PKH,
                   OP_RETURN carries their Solana address
  2. MATURE        C confirmations, measured in block time
  3. GATE          no reorg of depth ≥ C seen recently       ─┐
                   tip not stale                               │ any gate fails
                   client not catching up                      │ -> DELAY, not refund
  4. MINT          solBSV issued to the stated address       ─┘
```

**The gates delay; they do not refund.** A deposit that clears is valid — the only
question is *when* it can be credited. So a gated deposit waits for the gates to
clear and is then minted normally. The funds stay with the bridge throughout.

### Peg-out — solBSV → BSV

```
  1. BURN          user burns solBSV, names a BSV destination
  2. GATE          no reorg of depth ≥ C seen recently       ─┐
                   outstanding redemptions ≤ capacity          │ any gate fails
  3. DEADLINE      D hours for a bonded relayer to pay       ─┘ -> request refused,
  4. PAY           relayer pays BSV from its hot float            burn not taken
  5. PROVE         relayer submits SPV proof, C_payout deep
  6. CHALLENGE     W hours to dispute; a reorged payout is caught here
  7. SETTLE        fee to relayer — or, on failure, RE-MINT and slash the bond
```

**Peg-out is gated even though the burn is final.** A reorg cannot invalidate a
Solana burn, so the naive reading is that peg-out needs no reorg protection. It
does — but for a smaller reason than it first appears. Peg-out is one exit among
several, and **gating it is a speed bump rather than the defence.** See
§Where the loss actually lands.

### Parameters

| Parameter | Proposed | Class | Notes |
|---|---|---|---|
| `FLOOR` — minimum deposit confirmations | 12 (~2 h) | **safety** | The hard bound. Depositors may commit to *more*, never less |
| `C_payout` — payout confirmations | 12 | **safety** | Depth before a relayer may claim |
| `D` — redemption deadline | 6 h | liveness | Relayer must pay within this |
| `W` — challenge window | 24 h | **safety** | Must exceed reorg risk on the payout |
| `RECENT_REORG_WINDOW` | 12 h | **safety** | Depth-aware, see below |
| `TIP_STALENESS` | 2 h | **safety** | Pause if the tip stops advancing |
| `MIN_PEG_IN` | 10 BSV | economic | Fee economics and dust/spam |
| `MAX_PEG_IN` | 10,000 BSV | economic | Bounds per-transaction reserve exposure |
| `MAX_PEG_OUT` | 10,000 BSV | economic | **Also bounded by live capacity** |
| `HOT_FLOAT_CAP` | config | **safety** | The real bound on a drain |
| bond `k` | config | **safety** | `bond ≥ k × (hot float + releasable tranche)` |

**Safety parameters may only be moved in the conservative direction by
governance.** Whoever can set `C = 0` or `RECENT_REORG_WINDOW = 0` holds a
mint voucher; that is categorically different from whoever sets a fee. Loosening
a safety parameter should require shipping a new program, not flipping a flag.

**Test overrides are compile-time, not config.** "Remove the minimum during
testing" must be a `#[cfg(feature = …)]`, never a runtime value — a runtime value
can leak to mainnet, a compile-time one cannot.

---

### Confirmation depth is a market term, not just a constant

`FLOOR` is a **floor**, not a fixed term. The depth a deposit actually waits is a
term of the trade — chosen by the depositor, priced by whoever serves them.

**How it is enforced.** The depositor's chosen depth is committed in the deposit's
`OP_RETURN` alongside their Solana address, and the program requires both

> `confirmations ≥ committed_depth` **and** `committed_depth ≥ FLOOR`

The commitment is what makes it more than a promise, and the reason is easy to miss:
**the claim is constructed by whoever submits it.** Without a commitment, a relayer
could agree a long wait and then submit at the floor. Putting the depth in the
deposit makes the depositor's choice binding and impossible to undercut by anyone
downstream.

**Four layers, in order of authority:**

| Layer | Lives in | Role |
|---|---|---|
| **`FLOOR`** | Consensus — voted, increase-only | The hard protection. Binds every mint regardless of who asks |
| **Committed depth** | The deposit's `OP_RETURN` | The depositor's own risk choice, enforced by the program |
| **Relayer preferences** | Off-protocol advert | Preferred depth and size for their best rate |
| **Recommendation** | The website | An adaptive suggestion with no consensus role |

**Why a depositor would ever choose above the floor.** A longer wait is less risky for
whoever fronts the mint, so it buys a better rate — and it is better for the peg,
because fewer mints sit close to the tip where a reorg can reach them. The incentive
has to come through the fee; a depositor has no reason to wait longer for nothing.

**Why relayers advertise.** A relayer publishes the depth and size at which it offers
its lowest rate. That is a **public schema rather than a contract** — published on the
website, consumable by any other front-end, changing nothing on-chain. It lets a depositor
see what is on offer before choosing, and lets competing venues quote against the same
terms.

**Choosing too low** has exactly two honest outcomes: relayers decline, or they ask a
higher rate. A depth below `FLOOR` is not discouraged but *refused* by the program,
and the interface should say so plainly rather than presenting it as a live option —
a warning on something the chain will reject anyway is false reassurance.

## Who this is for, and why the delays are acceptable

The peg is **wholesale infrastructure, not a retail product.** It is built for
market makers, larger LPs and exchanges who move the coin back and forth to earn
a yield and to rebalance inventory on both sides of the fence. Retail is expected
to reach `solBSV` through an exchange or through Raydium, not through the peg
directly. That framing resolves most of the apparent tension in the gates above.

### The security is economic, and it is Bitcoin's own

Bitcoin's double-spend resistance is an *economic* argument, not a cryptographic
one: confirmations are secure when out-mining them costs more than the value they
protect. Nothing here invents a new assumption. The peg extends the same logic one
layer up, deliberately goes **beyond** the whitepaper's six confirmations, and adds
a revert path on top. What is added is not a new trust model — it is a larger
margin on the existing one.

Two calibration warnings, both load-bearing:

- **BSV's hashpower is not Bitcoin's.** The whitepaper's table is parameterised by
  `q`, the attacker's *share* of hashpower. On a chain with less total hashpower,
  reaching a given `q` costs less in absolute terms, so six confirmations do not
  carry the same meaning here. `C` must be calibrated to **BSV**, not inherited.
- **`C` must move with the value it secures.** A depth adequate for 10 BSV is not
  adequate for 10,000.

### The gates protect the backing, not the depositor

This is what makes them not paternalism. A fraudulent mint does not merely
inconvenience the depositor — **it dilutes everyone holding `solBSV`.** The gates
protect the token's backing, which is the market makers' own inventory. They are a
collective protection that the professionals benefit from most, not a judgement
about a user's ability to assess risk.

### Loops must be open to anyone

For the peg to hold, **both directions must be permissionless and open to
anyone** — the loop is what lets the market arbitrage `solBSV` back to BSV. This
has a hard consequence that is easy to miss:

> **Gate symmetry is a peg-integrity requirement, not a fairness one.**

Pausing peg-in while leaving peg-out open pushes the price up; pausing peg-out
while leaving peg-in open pushes it down **and leaves open exactly the exit the
gate existed to close**. Either way the token trades at a basis. When a gate
closes it must close **both ways** — and the secondary market is then free to
price the risk, which is the honest outcome and the reason the pause is not
patronising. We stop the protocol, not the market.

### Staking, and a fixed fee

**The model is deliberately simple, and the simplicity is the point.** Anyone may
stake BSV on either side of the fence and earns a share of transfer fees, fixed at
**10 bp in and 10 bp out**. If it works at that level, price discovery and
risk-pricing can be layered on afterwards. Building them first would mean tuning a
market that has not yet been shown to function.

Stakers underwrite the system, so a fraudulent mint lands on **people who chose to
bear that risk** rather than diluting every holder as a discount.

**What a fixed fee makes clear.** With the fee pinned, the *yield* is the clearing
variable: stakers enter while it beats their cost of capital and leave when it does
not, so the amount staked settles where the two meet.

> Daily fees = 0.002 × daily volume, so annual fees = 0.73 × daily volume.
> At a 10% cost of capital, equilibrium staked capital ≈ **7.3 × daily volume**.

That is the number that decides whether the model is practical, and it is worth
measuring rather than arguing about.

**What 10 bp cannot do is price risk.** The round trip costs the same whether BSV is
calm or mid-reorg. So when risk rises, stakers want more than the fee can pay and
they leave — precisely when the most backing is wanted. That is the accepted price
of simplicity here rather than an oversight, and it is the first thing to revisit
once the loop works end to end.

## The attack this defends against

A reorg is not interesting because it moves blocks. It is interesting because of
one sequence:

```
  ATTACKER                     BSV                         SOLANA
     │                          │                             │
     │ 1. mine a competing branch containing a fake
     │    deposit to themselves (needs hashpower)
     ├─────────────────────────►│
     │                          │ 2. 12 confirmations on the
     │                          │    FAKE branch
     │                          │ 3. mint ───────────────────►│ unbacked solBSV
     │                          │                             │
     │ 4. peg out ◄───────────────────────────────────────────┤
     │                          │                             │
     │ 5. honest chain overtakes the fake branch
     │                          │  → the mint is now unbacked, but the
     │                          │    attacker already holds real BSV
```

Three things make this expensive, and **none of them is detection**:

1. **Proof of work.** Forging `C` blocks must out-mine the honest chain for the
   duration. `C` is the security parameter that makes this cost more than it pays.
2. **The hot float cap.** Even a *successful* fraudulent mint cannot drain the
   whole reserve — only what a relayer can pay out before the cap binds.
3. **The bond.** `bond ≥ k × hot float` means the relayer's stake covers what
   the float can lose.

Detection — the gates above — is the *fourth* layer. It reduces the window of
opportunity and it protects honest users from being caught mid-flight. It is not
what makes the peg safe, and it should not be sold as such: **a gate that
depends on someone submitting a competing branch is a liveness assumption, not a
guarantee.**

---

## Where the loss actually lands

### The invariant, stated the right way round

What is at risk is **unbacked `solBSV`** — more `solBSV` outstanding than BSV
custodied. The invariant is

> `custodied BSV ≥ outstanding solBSV`

and a successful reorg attack violates it by creating supply with nothing behind it.
The reverse (more BSV than supply) is the *safe* direction, merely inefficient.

### A fraudulent mint has more than one exit

The attacker's difficulty is not obtaining `solBSV` — it is turning it into something
else. There are three routes, and **the protocol controls exactly one**:

| Exit | Who absorbs the loss | Gateable? |
|---|---|---|
| **Peg-out** via the protocol | The relayer's hot float, up to the float cap | Yes — gated, and the residual risk is priced into the fee |
| **DEX** (Raydium / Orca) | The **LPs** in the pool, who bought `solBSV` that is not backed | **No.** A permissionless AMM cannot be frozen |
| **CEX** | The exchange, and its users if it cannot cover | **No.** Off-chain entirely |

So gating peg-out closes one door and leaves two open. It is still worth doing — it
removes the deepest, fastest exit and forces the attacker through market slippage,
which costs them real money — but it is **not** what makes the peg safe, and this
document previously came close to saying that it was.

### What actually protects

Only one mechanism attacks the fraud itself rather than its exit:

- **`C`, the confirmation depth.** It is what makes out-mining the honest chain cost
  more than the fraud is worth. Every other control silently assumes the fraudulent
  mint has already happened.
- **The hot float cap** bounds what the *protocol* can lose once it has.
- **The bond** covers the protocol's loss, and guarantees the relayer honours
  redemptions at all.

The honest consequence, which belongs in the document rather than in a footnote:
**if `C` is too low, the loss lands on DEX LPs and exchanges, and the protocol cannot
compensate them.** They are not identifiable from on-chain state, they never
interacted with the bridge, and there is nothing to re-mint them with. That is a
stronger argument for setting `C` conservatively than anything about the exit gate.

### The staking buffer closes the hole

**Stakers bear the fraud loss**, and what makes that work is *where the stake sits*:
**in the reserve, as over-collateralisation**. The reserve then holds more BSV than
there is `solBSV` outstanding, and a fraudulent mint eats the surplus rather than
anyone's backing.

| | Custody | Supply | Peg holds? |
|---|---|---|---|
| Normal | `D` | `D` | Yes — 1:1 |
| With staking buffer `S` | `D + S` | `D` | Yes — over-collateralised by `S` |
| Fraudulent mint of `M` | `D + S` | `D + M` | Yes, **iff `S ≥ M`** |
| Attacker then exits via peg-out | `D + S − M` | `D` | Yes, iff `S ≥ M` — **stakers are down `M`; nobody else is** |

So the market never sees the shortfall. A DEX LP holding the unbacked tokens holds
something that is *still fully backed*, and an exchange never has to book a loss. That
is what fixes the hole: the loss is socialised onto volunteers instead of landing on
people who never chose it.

**The safety condition becomes checkable without an oracle:**

> `staked buffer  ≥  maximum mintable within one reorg window`

Both are protocol quantities — no price feed, no hashpower estimate, no judgement
beyond the cap itself. That is firmer footing than sizing `FLOOR` against an attack
cost nobody can measure.

**Three tiers, in order:**

1. **Detected in time** → staged tokens burned. No loss at all.
2. **Detection fails, buffer covers it** → stakers lose; the market does not.
3. **Buffer short** → holders absorb a discount. The one case that cannot be repaired.

Only tier 3 is a real failure, and its likelihood is essentially `1 − buffer/max mint`.
That ratio is the number worth watching.

**This creates one new requirement: unstaking must be delayed.** If stakers can leave
at will, the first to notice a fraud exits before it is confirmed, leaving a buffer
sized for a calmer day. An unbonding period is not a nicety — it is what keeps the
buffer present when it is needed, and it belongs with the same family of parameters as
the redemption deadline.

**And the practical tension, stated plainly:** the buffer has to be large enough to
matter, and whether a fixed 10 bp attracts capital *at that size* is exactly what the
simple model has to prove. It is measurable, which is the argument for building it.

### Who gets slashed, and who gets paid

The bond is a **performance bond**. It guarantees the relayer pays valid redemptions.
It is *not* reorg insurance: a relayer paying out against a fraudulent mint has done
nothing wrong, because they cannot tell it from a real one.

When a redemption fails, the **supply-conserving** settlement is:

1. **slash the bond and transfer that `solBSV` to the redeemer**, and
2. only if the bond is short, **mint the remainder**.

This makes the redeemer whole while **leaving total supply unchanged** wherever the
bond covers it. Re-minting alone would inflate supply and dilute every holder;
slashing and transferring moves existing tokens instead. Where the bond is short, the
shortfall *is* dilution — which is exactly what `k` exists to bound.

**Return-to-sender is a different mechanism and does not apply here.** RTS is for a
*real* deposit the bridge declines to mint: the funds go back to the address that
funded the deposit transaction. In a fraudulent mint there is no aggrieved sender —
the victims are whoever ends up holding the unbacked tokens, which is the market.

## Reorg detection

### Rule 1 — depth, not wall-clock

The trigger compares two depths, not "did anything reorg in the last N hours":

| Observed reorg depth `R` | Response |
|---|---|
| `R < C` (e.g. 2-block tip reorg) | **Nothing.** Confirmations already cover it |
| `R ≥ C` | Pause; re-examine mints credited within the last `R − C` blocks |

A uniform 12-hour freeze on *any* reorg would stall the bridge through ordinary
tip churn — a 2-block reorg is normal BSV behaviour and is harmless to a deposit
that is already 12 deep. The wall-clock window is a convenience bound (12 h ≈ 72
BSV blocks); **depth is the number that means something.**

### Rule 2 — read the time from the headers

The 80-byte header carries a 4-byte little-endian Unix timestamp at **offset 68**,
which `push_header` already parses. So the client can derive chain-relative time
from the chain itself, with no external clock:

| Signal | Definition | Reads as |
|---|---|---|
| **Regression** | new header `time` well below tip `time` | A fork, not an extension. Sharpest signal available |
| **Catch-up** | blocks arriving fast in wall-clock while their own `time`s span hours | Advancer was down, **or** a withheld branch is being released — indistinguishable, so pause |
| **Staleness** | `now − tip.time` > `TIP_STALENESS` | Advancer down, or a deep reorg in progress |

Only staleness needs a wall-clock, and only for "now" — the chain's own times do
the rest. Note the limit honestly: a privately mined branch released later has
*normal* block-time spacing, so spacing alone will not catch it. What catches it
is the mismatch between **arrival rate and block-time spacing** — signal 2.

---

## Scenarios

| # | Scenario | Handling | Who loses |
|---|---|---|---|
| S1 | Clean peg-in | Mint after `C` | Nobody |
| S2 | Reorg **below** the deposit block, before mint | Proof fails (`HeaderMismatch`); tx is either re-included in the new branch and re-proven, or reorged out and the UTXO is unspent again | Nobody |
| S3 | Reorg **after** a mint is credited, depth ≥ `C` | Already-minted tokens are **not** reversed — no freeze authority, by design. Pause new activity; rely on `C` having made it uneconomic | Reserve, if `C` was too low |
| S4 | Clean peg-out | Relayer pays, proves, challenge expires, settles | Nobody |
| S5 | Payout reorged during the challenge window | Challenge succeeds → bond slashed, user re-minted | Relayer (and the bond) |
| S6 | Relayer down at the deadline | Deadline expires → automatic re-mint | Relayer, via the bond |
| S7 | **The attack** — fraudulent mint then peg-out | Gates + hot float cap + bond bound the loss; ≥`C` reorg pauses the exit | Reserve up to the cap; relayer's bond |
| S8 | Deposit valid but a gate is closed | **Delayed, not refunded.** Funds stay with the bridge; mint proceeds once gates clear | Nobody |

### If a refund is ever needed

Refunds are the exception, and the rule is absolute: **return to sender.** The
destination is the address that funded the deposit transaction, derived from that
transaction's own inputs — never a relayed or user-supplied destination. Anything
else turns "refund" into a way to redirect someone else's coins.

The UX must show this before it can happen, so it is never a surprise: a
**greyed-out box reading "if refunded, it goes back to the sender address"**,
populated with the address derived from the deposit, visible while the deposit is
pending.

---

## Considerations

Each table records the options, the reasoning, and the verdict. The recommended
row is listed first.

### 1. Reorg detection trigger

| Option | Pros | Cons |
|---|---|---|
| **Depth-aware + block-time signals** ✅ | Matches the actual risk; no false freezes from tip churn; no external oracle | Three signals to tune; block-time thresholds need generous tolerance |
| Wall-clock window only (12 h) | Trivial to implement | Freezes on harmless tip reorgs; measures time, not risk |
| Wall-clock **and** depth | Simple, conservative | Still freezes on `R < C`; the wall-clock part adds nothing once depth is checked |
| No detection — rely on `C` alone | Simplest; `C` is the real security | No protection against a *catching-up* client; exits stay open during instability |

### 2. Response mid-transfer

| Option | Pros | Cons |
|---|---|---|
| **Delay until gates clear** ✅ | Deposit is valid, so delaying preserves intent; no refund path to abuse; funds never move | User waits; needs clear UX or it looks like a hang |
| Refund (return to sender) | User gets funds back | New BSV transaction, new custody, new abuse surface; refund only to the funding address or it is a theft vector |
| Freeze and require manual action | Maximum control | Trusted operator action; unusable at scale |

### 3. Refund authorisation

| Option | Pros | Cons |
|---|---|---|
| **Funding address only, derived on-chain** ✅ | Not redirectable; no discretion to abuse; verifiable from the deposit tx | Requires parsing the deposit's inputs |
| Relayer discretion | Flexible | A relayer that can choose the destination can steal the deposit |
| No refunds at all | No surface at all | A genuinely invalid deposit's funds are stuck forever |

### 4. Relayer model

| Option | Pros | Cons |
|---|---|---|
| **Staking pool: anyone stakes BSV on either side and shares a fixed 10 bp in / 10 bp out fee** ✅ | Simple; permissionless; losses land on volunteers rather than on every holder; nothing to attest | A fixed fee cannot price risk, so capacity is procyclical; operator incentives inside a passive pool are unresolved |
| Single nominated relayer | Simplest | No fee competition; nomination is a trusted choice |
| Bonded set, open entry | Better liveness | Capital fragmented across bonds; coordination on who pays |
| Fully permissionless | Best liveness | Anyone can attempt payout; no bond means no recourse |

### 5. Bond asset

| Option | Pros | Cons |
|---|---|---|
| **Mixed: BSV in timelocked multisig, topped up in solBSV** ✅ | The BSV leg does not depeg with the thing it insures | More moving parts to operate |
| `solBSV` only | Relayer had to lock BSV to acquire it, so it is BSV-backed; slashing burns supply | **Procyclical** — on default `solBSV` depegs, so the bond is worth least exactly when it is called on. `k` must absorb an expected depeg |
| SOL or stables | Uncorrelated with the peg | Not BSV-denominated; the relayer must source it separately |

### 6. Peg-out capacity control

| Option | Pros | Cons |
|---|---|---|
| **Per-window cap = f(hot float, bond/k); per-tx max secondary** ✅ | The only control that matches what can actually be paid; prevents a race the relayer cannot win | Capacity must be tracked and published |
| Per-transaction max only | Trivial | Several large redemptions can all pass and collectively exceed the float → mass re-mint and an unfairly slashed relayer |
| No cap | Maximum freedom | The float is drainable in one transaction |

### 7. Parameter governance

| Option | Pros | Cons |
|---|---|---|
| **Multisig + timelock for economic; safety increase-only** ✅ | Fee/limit agility without a mint-voucher key; timelock lets users exit | Two paths to reason about |
| Single multisig, all parameters | Simple | A key compromise can set `C = 0` and mint against unconfirmed blocks |
| Token vote | Legible | **Safety parameters should not be votable** — a majority can strip its own protection |
| Fully immutable | Maximum safety | No fee tuning, no response to changing conditions |

### 8. Exit gating for fresh supply

| Option | Pros | Cons |
|---|---|---|
| **Global peg-out gate after a detected reorg** ✅ | Blocks the exit path the attack depends on; no token-level machinery | Freezes honest redemptions too |
| Per-token maturation (new mints non-redeemable for N blocks) | Targets only fresh supply | **Tokens are fungible** — the attacker sells on Raydium and an innocent buyer holds the unbacked token. Taint cannot follow without a denylist, which we have deliberately ruled out |
| Nothing | No friction | The attack's exit is always open |

---

## Decisions from review

1. **Relayer fallback — permissionless.** A single relayer must not be able to block
   the peg. The auction still sets the reference fee, but **any bonded relayer may
   pay**, so the winning bid is a price and not an exclusive right. Liveness then
   does not depend on any one participant, which is the point of keeping the loop
   open. The single-relayer model in §Considerations is superseded on this point.

2. **Bond `k` — assume no persistent depeg.** `solBSV` and BSV are the same asset,
   so any deviation is an arbitrage and closes; `k = 1` is defensible rather than
   having to absorb a standing discount. The consequence of that assumption is
   recorded in decision 3, because it is what makes the compensation rule coherent.

3. **Slashing destination — the redeemer, in `solBSV`, supply-conservingly.** The BSV
   never reached the end user, so compensating them in BSV is not available; the
   slashed `solBSV` transfers to them instead and they hold the same *amount* on the
   wrong chain. Because the bond's tokens move rather than new ones being minted,
   **total supply is unchanged wherever the bond covers the redemption** — see
   §Who gets slashed, and who gets paid for the settlement order and the shortfall.
   This is the honest consequence of refusing a freeze authority: **holders keep
   their balance through a default and absorb any shortfall as a discount rather
   than a confiscation.** The bond protects against the relayer absconding — it
   does **not** protect against the reserve being short, and it should not be
   described as if it does.

4. **`C` is a policy parameter, not a derived one.** Sizing it from the cost of
   reorging `C` blocks needs a **BSV price feed** to value what is at risk and a
   **hashpower-rental feed** to price the attack.

   The reason it is rejected is **not** that those quantities are unknowable — they
   are entirely knowable and worth showing. It is that they must stay **outside the
   protocol**. Tuning the cap inside consensus would mean consulting an external
   market from the program itself, and the program consults nothing external: by
   construction `1 BSV = 1 solBSV`. Importing oracles to derive one integer would
   trade away the property the whole design rests on, in exchange for a number that
   would still be a guess dressed as a calculation.

   > **A metric is not an oracle.** BSV price, estimated reorg cost, hashrate and the
   > observed block rate are all fair game as *published information* — they help a
   > user judge the risk they are taking, and they can shape how relayers price. What
   > they must never do is change what the program *does*.
   >
   > **Display anything; decide on nothing external.**

   So `C` is set **high and conservatively as a policy choice**, changed by vote, and
   **disclosed before someone transacts**. It does not need to be dynamic. What
   matters is that it is *visible*, not that it is computed.

5. **`RECENT_REORG_WINDOW` — monitoring first, not a gate.** Block times are
   directly available: the header carries a Unix timestamp at offset 68, so the
   observed mining rate over the window is computable on-chain, and a mean spacing
   far below ten minutes is worth **warning** users about. Whether it becomes a gate
   (delay or RTS) is deferred, because a genuine hashpower surge looks identical to
   an attack. Note the variance is real — a dozen blocks is a small sample.

## Resolved and still open

5. **Fee mechanism — the user sets the fee, or the right is exclusive.** Two
   shapes, neither of which needs a winner attested on-chain:

   - **User-set fee, open fulfilment.** The redemption carries a fee and any bonded
     relayer may claim it if it clears their bar. There is no winner to attest,
     because competition is continuous rather than at an epoch boundary.
   - **Exclusive right, transaction starts at 0 fee.** One relayer holds the epoch,
     so the user-facing fee can be zero — their compensation is what they paid for
     the right.

   This **dissolves** the attestation question rather than answering it. Attestation
   is only needed when an off-chain auction produces a winner whose identity must be
   proven on-chain; under open fulfilment there is no such winner, and under
   exclusivity the obligation is on the holder rather than on a claim.

6. **Reserve invariant.** Does `custodied BSV ≥ outstanding solBSV` hold
   *continuously*, or only after settlement? Re-mints make it transiently false, and
   decision 3 sharpens the question: a shortfall is absorbed as a discount by holders
   rather than covered from the bond, so the invariant may be better stated as a
   **target with an explicit failure mode** than as a hard assertion. This is the one
   genuinely unresolved item.

### The aggregate mint cap, restated as policy

`MAX_PEG_IN` still does not bound a reorg — a BSV block holds thousands of
transactions, so an attacker fills a fraudulent branch with many deposits under any
per-transaction limit. The bound that works is an **aggregate cap per window**.

But decision 4 applies to it too: it is **set conservatively as policy, not derived**,
because deriving it needs the same two oracles we have excluded. It is a coarse
backstop beneath `C`, not a calibrated constant — and it is secondary to the two
bounds that need no oracle at all:

- **`C` set high**, which is what makes out-mining the honest chain expensive;
- **the hot float cap**, which limits what a *successful* attack can actually extract.

All of these are **votable and disclosed**. The disclosure matters more than the
value: a user should be able to see `C`, the cap and the float limit before they
transact.

Monitoring is also where the **external metrics** belong — the ones that must never
enter consensus. A user deciding whether to peg in is better served by seeing the
BSV price, an estimated cost to reorg `C` blocks, the current hashrate and the
observed block rate alongside the parameters than by seeing the parameters alone.
Published, none of it is a trust assumption; consulted by the program, all of it
would be.

**One qualification on *votable*.** It should not mean freely *loosenable*. Whoever
can vote `C` down toward zero holds a mint voucher, and no amount of deliberation
makes that safe. The reconciling rule is **increase-only under governance**: a vote
can make the protocol more conservative at any time, while making it *less*
conservative means shipping a new program. That gives the change mechanism the
review asked for without turning the vote itself into the attack path — and it is
the one place where the two positions in this document genuinely differ, so it is
flagged rather than quietly settled.

## Deliberately deferred

Recorded so these are not later relitigated as oversights. Each is a known
simplification of the current model, to be revisited once the loop works end to end.

| Deferred | Why it matters | Revisit when |
|---|---|---|
| Market-cleared fees | A fixed fee cannot price risk, so staking capacity is procyclical | The loop works and the fixed fee has been measured against real flow |
| Risk-priced confirmation depth | Depth is a term users and stakers negotiate, but nothing yet proves it gets priced | Adverts show whether longer waits actually earn lower rates |
| Differential yields per side | BSV-side staking carries custody risk; Solana-side carries reorg-fraud risk. Paying both 10 bp likely misprices one | Stakers reveal which side is short of capital |
| Operator incentives inside a passive pool | An operator can drain a pool of passive stakers, who then eat the loss | Before any pool holds meaningful value |
| Fee realisation mechanics | Whether stakers withdraw from the reserve or accrue a claim is unresolved | Before staking is live |

## What this changes downstream

Once the recommended state is agreed, these flow from it:

- **`MIN_CONFIRMATIONS`** stays 12 but becomes a governed, increase-only parameter.
- **`commit_fork`** must record reorg depth and block time when it fires, feeding
  the depth rule and the pause.
- **`push_header`** must start using the header timestamp it already parses, to
  expose the regression, catch-up and staleness signals.
- **The mint path** gains the gate checks; **the redemption path** gains both the
  gate and the capacity check.
- **The header window** (currently 48 h / 288 blocks) is a *liveness* parameter for
  following reorgs and should not be conflated with `RECENT_REORG_WINDOW`, which is
  a *safety* parameter. They are currently the same idea in two places and must be
  named apart.
- **A per-window mint cap** (`MAX_MINT_PER_WINDOW`) is wanted as a coarse backstop,
  set by policy rather than derived — see §The aggregate mint cap, restated as
  policy. It sits beneath `C` and the hot float cap rather than replacing them.
- **The replay list is no longer a lifetime limit.** It is now pruned by height, so
  it bounds deposits *per window* rather than total usage. The previous fixed list
  of 256 stopped the peg-in path permanently once reached, which ordinary volume
  would have done on its own. A test that mints across a window boundary to
  exercise the prune is still wanted — the current fixture cannot reach one.
- **Tests** in `poc/TEST_PLAN.md` §4 and §5 gain the gate and capacity cases.
