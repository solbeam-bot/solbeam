# 12. Peg-in and peg-out: the mechanism

> **Status: proposed, for review.** Nothing below is built beyond what is already
> in Phase 2. This document exists so the mechanism can be argued about on paper
> — with the trade-offs written down rather than rediscovered — before any of it
> is coded. **§Recommended state is the thing to review.** The tables in
> §Considerations record why each choice was made and what was rejected.

---

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

**Peg-out is gated even though the burn is final.** This is the correction from
review: a reorg cannot invalidate a Solana burn, so the naive reading is that
peg-out needs no reorg protection. That reading is wrong, and the reason is
§The attack, not the mechanism — **peg-out is the exit**. If a fraudulent mint
succeeds, peg-out is how the attacker turns it into real BSV. Gating the exit
is therefore the defence that matters most.

### Parameters

| Parameter | Proposed | Class | Notes |
|---|---|---|---|
| `C` — deposit confirmations | 12 (~2 h) | **safety** | Increase-only under governance |
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

### The fee is whatever the market clears at

The fee is **not a parameter to be chosen** — it is discovered by the bidding, and
it can settle well above any cost floor. Anchoring on a figure like 10 bp is a
mistake: there is no reason bidders would stop at 50 bp if that is where demand
against scarce capacity puts it.

What the bidding prices is a **risk-adjusted return on bond capital**, and the
arithmetic is why the range is wide. Capital is committed for roughly half a day
per cycle, so about two cycles a day. Because the bond is a multiple `k` of the
float, the return falls on total committed capital rather than on volume moved:

| Fee per cycle | Annualised on committed capital (`k` = 1) |
|---|---|
| 5 bp | ~18% |
| 10 bp | ~37% |
| 25 bp | ~91% |
| 50 bp | ~183% |
| 100 bp | ~365% |

Those are large numbers for a half-day lockup, and they cut both ways. They attract
capital, which compresses the fee — but three things stop it collapsing to cost:

- **Capacity is the scarce good, not capital.** The hot float caps how much can be
  paid out per cycle, so fee pressure comes from the ratio of demand to *bonded
  capacity*, not from the number of willing bidders.
- **The risk is real and hard to price.** Custody of the reserve, BSV reorgs during
  the lockup, and Solana downtime all sit on the relayer.
- **Exclusivity is a trade, not a free win.** A single relayer per epoch can be held
  to one bond and a clear SLA, but holds pricing power between auctions. Open entry
  to any bonded relayer is what actually compresses the fee, at the cost of
  coordination on who pays.

So the protocol does not set the fee. What it sets is **who may bid, how much
capacity the limits permit, and how exclusive the slot is** — and those levers are
what decide where the fee lands. A cost-of-capital figure is the floor beneath the
auction, not the outcome of it.

An exchange cross is a natural pressure valve: an exchange with a balance sheet on
both sides can quote tighter than the peg. That is healthy and it caps the fee.
It is worth being clear about what it does *not* do — a cross is a **centralised**
substitute, and it works only because it can fall back on the trustless peg to
rebalance. If the peg breaks, the exchange's `solBSV` is unbacked too. That is why
the peg being wholesale and boring does not make it unimportant: **it is the
settlement layer everything else derives from.**

---

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
what makes the system safe, and it should not be sold as such: **a gate that
depends on someone submitting a competing branch is a liveness assumption, not a
guarantee.**

---

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
| **Bonded set — auction sets the reference fee, any bonded relayer may pay** ✅ | One bad relayer cannot block the peg; competition still sets the price | More bonds to track; the auction remains an off-chain trust point for the benchmark |
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

3. **Slashing destination — the victims, in `solBSV`.** The BSV never reached the
   end user, so compensating them in BSV is not available; the slashed `solBSV`
   returns to them instead, and they hold the same *amount* on the wrong chain.
   This is the honest consequence of refusing a freeze authority: **holders keep
   their balance through a default and absorb any shortfall as a discount rather
   than a confiscation.** The bond protects against the relayer absconding — it
   does **not** protect against the reserve being short, and it should not be
   described as if it does.

4. **`C` calibration — open by design.** Arbitrary at PoC, measured in Phase 5, and
   needing a change mechanism that is itself a proposal. Getting the mechanism right
   matters more than the starting number, because the number decays as BSV's
   hashrate moves.

5. **`RECENT_REORG_WINDOW` — monitoring first, not a gate.** Block times are
   directly available: the header carries a Unix timestamp at offset 68, so the
   observed mining rate over the window is computable on-chain, and a mean spacing
   far below ten minutes is worth **warning** users about. Whether it becomes a gate
   (delay or RTS) is deferred, because a genuine hashpower surge looks identical to
   an attack. Note the variance is real — a dozen blocks is a small sample.

## Still open

6. **Auction attestation.** Is the off-chain fee auction acceptable as a trusted
   component, or must the winning relayer be attested on-chain per epoch? Now that
   the auction sets a price rather than an exclusive right, the trusted part is
   smaller — but not gone, since the benchmark still influences what everyone
   charges.

7. **Reserve invariant.** Does `custodied BSV ≥ outstanding solBSV` hold
   *continuously*, or only after settlement? Re-mints make it transiently false, and
   decision 3 makes the question sharper: a shortfall is absorbed as a discount by
   holders rather than covered from the bond, so the invariant may be better stated
   as a **target with an explicit failure mode** than as a hard assertion.

### The cap that actually bounds a reorg: per window, not per transaction

This answers the open question about `MAX_PEG_IN`. The concern is right — a reorg
can mint unbacked `solBSV`, and that supply exits through the peg-out path — but a
**per-transaction** maximum does not bound it. A BSV block holds thousands of
transactions, so an attacker building a fraudulent branch simply fills it with many
deposits, each comfortably under the limit.

The instrument that works is an **aggregate cap per window**:

> Total minted within the last `C` blocks ≤ `MAX_MINT_PER_WINDOW`, chosen so that
> `MAX_MINT_PER_WINDOW × BSV value < cost of reorging C blocks`.

That is the safety parameter, and it is what makes `C` meaningful rather than
decorative. `MAX_PEG_IN` remains useful — reserve management and fat-finger
protection — but it is **operational**. Without the aggregate cap, `C` alone carries
the whole economic argument, and `C` is the number nobody has measured yet
(decision 4).

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
- **A per-window mint cap** (`MAX_MINT_PER_WINDOW`) must be enforced, not just a
  per-transaction maximum — see §The cap that actually bounds a reorg.
- **The replay list is no longer a lifetime limit.** It is now pruned by height, so
  it bounds deposits *per window* rather than total usage. The previous fixed list
  of 256 stopped the peg-in path permanently once reached, which ordinary volume
  would have done on its own. A test that mints across a window boundary to
  exercise the prune is still wanted — the current fixture cannot reach one.
- **Tests** in `poc/TEST_PLAN.md` §4 and §5 gain the gate and capacity cases.
