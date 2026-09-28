# 12. Peg-in and peg-out: the mechanism

> **Start with [`13-summary.md`](13-summary.md)** for the system as it stands, then
> [`14-decisions.md`](14-decisions.md) for the decisions and what they defer. This
> document holds the full reasoning.
>
> **Built or designed?** The shipped program is exactly **light client + `solBSV` token +
> the mint + fork staging**, and it passes 17 on-chain tests. **The vault, the two gates,
> maturity, release, burn, the order book, staking, bonds, `owed_R`, consent, per-relayer
> deposit scripts, all of peg-out, `FLOOR` as a distinct parameter, and the difficulty
> retarget are designed and not built.** The shipped mint goes straight to the depositor's
> token account, so nothing is staged yet and most of this document is a specification
> rather than a property of the code.
>
> One exception to "not built" is worth stating sharply: **DAA is not merely
> unimplemented, it is actively rejected.** `check_daa` has **no caller** and
> `expected_bits` is set once at `initialize` and never refreshed, while `push_header`
> requires `bits == expected_bits`. The client therefore **halts permanently at the first
> difficulty retarget** (F7). On regtest that is invisible because the target never
> changes; on testnet or mainnet the bridge stops minting.

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
| A hashpower or price feed to compute a "safe" confirmation depth | A `FLOOR` fixed in code, plus a depth the depositor chooses and relayers price on the book |
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
*(Designed, not built: the shipped mint uses the fixed `MIN_CONFIRMATIONS` constant and
does not parse a committed depth — A14.)*

**The one residual judgement is `FLOOR` itself.** It is a policy value rather than a
measurement — 12 BSV blocks, **fixed in code** for the PoC (D4), not a vote: there is
**no governance mechanism in the PoC at all** (D7), and the mechanism to change `FLOOR`
is a named gap. The design has no oracles; it does have a parameter somebody had to
choose, and *that* is the thing worth arguing about.

## Recommended state

### Peg-in — BSV → solBSV

```
  1. SEND          user pays ≥ MIN_PEG_IN to a relayer's script,
                   OP_RETURN carries their Solana address
  2. DEPTH         wait the depth the bid named, measured in block time
  3. GATE          no reorg of depth ≥ FLOOR seen recently    ─┐
                   tip not stale                               │ any gate fails
                   client not catching up                      │ -> DELAY, not refund
  4. MINT          solBSV issued into the program's vault      │ — not to the depositor
  5. MATURE        no reorg followed -> the vault releases, fee to the stakers
                   reorg followed    -> the staged tokens are burned. Nobody loses
```

**The gates delay; they do not refund.** A deposit that clears is valid — the only
question is *when* it can be credited. So a gated deposit waits for the gates to
clear and is then minted normally. The funds stay with the bridge throughout, and the
mint is not liquid when it is credited: it lands in the vault and is released only
after maturity.

### Peg-out — solBSV → BSV

```
  1. ESCROW    solBSV moves into the program's vault; a BSV destination is named
  2. GATE      no reorg of depth ≥ FLOOR seen recently         ─┐ any gate fails
               outstanding redemptions ≤ capacity               │ -> request refused,
  3. ACCEPT    a relayer whose bond covers it takes the request  │    escrow not taken
  4. DEADLINE  D, measured in slots, so a Solana halt freezes the clock
  5. PAY       the relayer pays BSV from its own float and proves it against the light client
  6. CHALLENGE W slots for a reorged payout to be caught
  7. SETTLE    burn the escrowed solBSV, pay the fee
               or on failure, return the escrow to the holder. Supply never changes
```

**Peg-out is gated even though a Solana escrow cannot be reorged.** A BSV reorg
cannot invalidate Solana state, so the naive reading is that peg-out needs no reorg
protection. It does — but for a smaller reason than it first appears. Peg-out is one
exit among several, and **gating it is a speed bump rather than the defence.** See
§Where the loss actually lands.

### Parameters

| ID | Parameter | Proposed | Class | Notes |
|---|---|---|---|---|
| **P1** | `FLOOR` — minimum deposit confirmations | 12 (~2 h) | **safety** | The hard bound. Depositors may commit to *more*, never less. **Fixed in code for the PoC** (D4). In the shipped program this is the `MIN_CONFIRMATIONS` constant, not a distinct parameter |
| **P2** | `C_payout` — payout confirmations | 12 | **safety** | Depth before a relayer may claim |
| **P3** | `D` — redemption deadline | 6 h | liveness | Measured in **slots**, not wall-clock |
| **P4** | `W` — challenge window | 24 h | **safety** | Must exceed reorg risk on the payout |
| **P5** | `RECENT_REORG_WINDOW` | 12 h | **safety** | Depth-aware, see §Rule 1 |
| **P6** | `TIP_STALENESS` | 2 h | **safety** | Pause if the tip stops advancing |
| **P7** | `MIN_PEG_IN` | 10 BSV | economic | Fee economics and dust/spam |
| **P8** | `MAX_PEG_IN` | 10,000 BSV | economic | Per *transaction* only; see §aggregate mint cap |
| **P9** | `MAX_PEG_OUT` | 10,000 BSV | economic | **Also bounded by live capacity** |
| **P10** | `HOT_FLOAT_CAP` | config | **safety** | The real bound on what a *successful* fraudulent mint can extract. With per-relayer deposits it is each relayer's own float, not a system hot wallet |
| bond `k` | bond multiple | 1 (D5) | **safety** | **`bond_R ≥ k × owed_R`**, where `owed_R` accumulates only from proofs the program verified itself. Since `bond_R` is `solBSV` the program holds, the inequality is checkable on-chain |

**Safety parameters are not freely loosenable, and nothing can loosen them yet.**
Whoever can set `FLOOR = 0` or `RECENT_REORG_WINDOW = 0` holds a mint voucher; that is
categorically different from whoever sets a fee. Loosening a safety parameter should
require shipping a new program, not flipping a flag. There is **no governance mechanism in
the PoC at all** (D7): `FLOOR` is fixed in code, and the change mechanism is a named gap in
the deferred design rather than a shipped power.

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
| **`FLOOR`** | Consensus — **fixed in code at 12 blocks** (D4) | The hard protection. Binds every mint regardless of who asks. Its change mechanism is deferred (D7) |
| **Committed depth** | The deposit's `OP_RETURN` | The depositor's own risk choice, enforced by the program — *designed, not built* (A14) |
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
  carry the same meaning here. `FLOOR` must be calibrated to **BSV**, not inherited.
- **Depth must move with the value it secures.** A depth adequate for 10 BSV is not
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

### Staking, and a discovered fee

**The fee is discovered, not set.** Underwriting is an **order book**: stakers post sell
orders — how much liquidity, at what fee, at what confirmation depth — and incoming
requests match them **by price, then by time**, partially filled. There is no protocol fee
parameter to argue about, and the fee in a bid is what the staker asks for the risk it is
taking. See §The book, below.

Stakers underwrite the system, so a fraudulent mint lands on **people who chose to
bear that risk** rather than diluting every holder as a discount.

**Depth is a term of the bid, not a protocol constant.** A staker who wants 24 blocks says
so and the book quotes it; `FLOOR` remains a backstop beneath the market term rather than
the price itself. A depositor can see the liquidity and the price before committing, which
is the disclosure the website was already going to provide.

**A book with no bids is a coherent state, not a broken one.** That is what dissolves the
bootstrap circularity: the genesis path (D3) seats the first relayer and its bond, after
which the ordinary path takes over.

**What the book fixes about a fixed fee.** A fixed fee cannot price risk: the round trip
costs the same whether BSV is calm or mid-reorg, so when risk rises stakers want more than
the fee can pay and they leave — precisely when the most backing is wanted. Pricing the fee
and the depth **per bid** is the answer to that, and it is why the fixed-fee model was
replaced rather than layered on later.

**Yield is paid in the asset staked** — BSV for a BSV-side staker, `solBSV` for a
`solBSV`-side one. See §O1.

## The attack this defends against

A reorg is not interesting because it moves blocks. It is interesting because of
one sequence:

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
     │                          │    sold, the loss lands on the buffer
```

The vault is what makes a detected fraud reversible. Two further things make an
undetected one expensive, and **none of them is detection**:

1. **Proof of work.** Forging `FLOOR` blocks must out-mine the honest chain for the
   duration. Depth is the security parameter that makes this cost more than it pays.
2. **The hot float cap.** Even a *successful* fraudulent mint cannot drain more than
   each relayer's own float can pay before its capacity binds.

The vault and maturity are the piece that makes a *detected* fraud reversible: a staged
mint is not liquid, so a followed reorg burns it and the fraud never reaches a market.

The **bond** does not belong on that list: it is sized against `owed_R`, a relayer's
measured liability, and it is not reorg insurance. It answers a relayer's deliberate theft
or abandonment, not the mint.

Detection — the gates above — is a further layer. It reduces the window of
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

**The invariant is monitored, not enforced** (D8). `custodied BSV ≥ outstanding solBSV`
is published and shown as a ratio on the website, and **the protocol cannot enforce it**,
because the reserve is off-chain BSV the program cannot read. The program does not check
it.

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

- **`FLOOR`, the minimum confirmation depth.** It is what makes out-mining the honest
  chain cost more than the fraud is worth. Every other control silently assumes the
  fraudulent mint has already happened.
- **The vault and maturity**, which make a detected fraud reversible rather than merely
  bounded.
- **The hot float cap** bounds what a *successful* attack can extract before a relayer's
  own float runs dry.
- **The bond** is sized against `owed_R` and covers a relayer's deliberate theft or
  abandonment — not the float, not a failed redemption, and not reorg insurance.

The honest consequence, which belongs in the document rather than in a footnote:
**if depth is too low, the loss lands on DEX LPs and exchanges, and the protocol cannot
compensate them.** They are not identifiable from on-chain state, they never
interacted with the bridge, and there is nothing to make them whole with. That is a
stronger argument for setting `FLOOR` conservatively than anything about the exit gate.

### The staking buffer closes the hole

**Stakers bear the fraud loss**, through two things: the **bond** the program holds and can
seize, and each relayer's own **float** of BSV. When detection fails and a fraudulent mint
releases, the surplus those stakers hold absorbs it rather than anyone's backing — and
because deposits pay individual relayers, that surplus is per-relayer rather than one
pooled reserve.

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

**The safety condition to aim at is:**

> `staked buffer  ≥  maximum mintable within one reorg window`

**But only part of the buffer is a protocol quantity, and an earlier draft of this section
wrongly treated the whole of it as checkable on-chain.** Per relayer, `owed_R` is derived
from proofs the program verified itself and `bond_R` is `solBSV` the program holds, so
`bond_R ≥ k × owed_R` is checkable and seizable on-chain. The relayer's BSV float is
off-chain, and the program cannot read, spend or slash it. So the buffer as a whole is
**tracked and published, not verified** (D8), and "stakers bear the fraud loss" reduces in
the worst case to each relayer honouring what it holds. The design is not wrong for that —
it is the honest boundary of what a Solana program can see.

**Three tiers, in order:**

1. **Detected in time** → staged tokens burned. No loss at all.
2. **Detection fails, buffer covers it** → stakers lose; the market does not.
3. **Buffer short** → holders absorb a discount. The one case that cannot be repaired.

Only tier 3 is a real failure, and its likelihood is essentially `1 − buffer/max mint`.
That ratio is the number worth watching.

**This creates one new requirement: unstaking must be delayed.** If stakers can leave
at will, the first to notice a fraud exits before it is confirmed, leaving a buffer
sized for a calmer day. An **unbonding period** — longer than the redemption deadline
plus the challenge window — is not a nicety: it is what keeps the buffer present when it
is needed, and it belongs with the same family of parameters as the redemption deadline.

**And the practical tension, stated plainly:** the buffer has to be large enough to
matter, and whether the book clears at a fee that attracts capital *at that size* is
exactly what the model has to prove. It is measurable, which is the argument for
building it.

### What the bond answers — and what it does not

The bond is a **performance bond sized against `owed_R`**, the liability the program
measured. It answers a relayer's **deliberate theft or abandonment** — of what it owes,
and of the float it holds — making either punishable; the exposure it is sized against is
`owed_R`, not the float. It is a chain fact rather than an attestation, because `owed_R` is
accumulated only from proofs the program verified itself.

It is **not** the answer to a failed redemption, and it is **not reorg insurance**:

- **A failed redemption returns the escrow.** The holder's `solBSV` is still in the
  vault, so on failure it goes back to them and **supply never changes**. Returning the
  escrow makes the holder whole, so the bond is **not additionally transferred** to them.
  What such a failure exposes is the relayer's abandonment; the bond's role is to make
  that abandonment punishable, not to top up a holder who is already whole.
- **The relayer's own float is not covered, in the sense that matters.** `bond_R ≥ k ×
  owed_R` is sized against the liability, not against everything the relayer holds; the
  float is its own capital, and no holder relies on the bond for it. Separating the two is
  what makes the inequality meaningful (F4).
- **A reorged payout is not a slashing matter.** A relayer paying out against a
  fraudulent mint has done nothing wrong, because it cannot tell that mint from a real
  one. A payout later reorged away is handled by the challenge window and the return
  path, not by slashing.

**Return-to-sender is a different mechanism again**, and it does not apply here. RTS is
for a *real* deposit the bridge declines to mint: the funds go back to the address that
funded the deposit transaction, derived from that transaction's own inputs. In a
fraudulent mint there is no aggrieved sender — the victims are whoever ends up holding the
unbacked tokens, which is the market. RTS is not built (A15).

## Reorg detection

### Rule 1 — depth, not wall-clock

The trigger compares two depths, not "did anything reorg in the last N hours":

| Observed reorg depth `R` | Response |
|---|---|
| `R < FLOOR` (e.g. 2-block tip reorg) | **Nothing.** Confirmations already cover it |
| `R ≥ FLOOR` | The reorg response applies: re-examine mints credited within the last `R − FLOOR` blocks, and stop crediting new ones |

A uniform 12-hour freeze on *any* reorg would stall the bridge through ordinary
tip churn — a 2-block reorg is normal BSV behaviour and is harmless to a deposit
that is already 12 deep. The wall-clock window is a convenience bound (12 h ≈ 72
BSV blocks); **depth is the number that means something.** Whether that response is
enforced by the program as an on-chain gate or merely monitored and published is the one
open item here — see Decision 5.

### Rule 2 — read the time from the headers

The 80-byte header carries a 4-byte little-endian Unix timestamp at **offset 68**,
which the program does **not** yet parse — `push_header` reads only offsets 4 and 72, and `HeaderRecord` stores a 32-byte hash only. So the client can derive chain-relative time
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
| S1 | Clean peg-in | Mint after the bid's depth, into the vault; release after maturity | Nobody |
| S2 | Reorg **below** the deposit block, after the mint was staged | The staged tokens are burned. Re-inclusion does **not** restore the mint by itself: the deposit identity stays in the replay list, so the depositor cannot mint again until burning a staged mint also removes its replay entry (**F1**). With that entry removed, the re-mined deposit mints normally | Depositor, until F1 is fixed |
| S3 | Reorg **after** a mint, depth ≥ `FLOOR` | If the mint is **still staged in the vault**, it is burned and the liability reversed. Only a mint that has already **released** cannot be reversed — no freeze authority, by design. Pause new activity; rely on depth and the buffer | The buffer, if depth was too low and release had happened |
| S4 | Clean peg-out | Relayer pays, proves, challenge expires, settles; the escrow is burned and the fee paid | Nobody |
| S5 | Payout reorged during the challenge window | The challenge catches it; the redemption does not settle and the **escrow is returned to the holder**. Supply is unchanged. The relayer has lost the BSV it paid — the bond is not reorg insurance | Relayer (its own float) |
| S6 | Relayer down at the deadline | Deadline expires → the **escrow is returned** to the holder. Nothing mints; supply is unchanged | Nobody loses tokens; the relayer is the party at fault |
| S7 | **The attack** — fraudulent mint then peg-out | The vault stages the mint; gates and the hot float cap bound the loss; a reorg of depth ≥ `FLOOR` burns the staged mint and pauses the exit | The buffer, if detection fails and the mint released; otherwise nobody |
| S8 | Deposit valid but a gate is closed | **Delayed, not refunded.** Funds stay with the bridge; the mint proceeds into the vault once the gates clear | Nobody |

### If a refund is ever needed

Refunds are the exception, and the rule is absolute: **return to sender.** The
destination is the address that funded the deposit transaction, derived from that
transaction's own inputs — never a relayed or user-supplied destination. Anything
else turns "refund" into a way to redirect someone else's coins. This is **not built**
(A15), and it is not the vault's return path, which moves tokens the program already
holds.

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
| Wall-clock **and** depth | Simple, conservative | Still freezes on `R < FLOOR`; the wall-clock part adds nothing once depth is checked |
| No detection — rely on `FLOOR` alone | Simplest; depth is the real security | No protection against a *catching-up* client; exits stay open during instability |

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
| **Order book of staked bids: stakers post liquidity, fee and depth; matched by price then time, partially filled** ✅ | The fee is discovered and depth is a term, so reorg risk is priced; permissionless; losses land on volunteers rather than on every holder | Capacity can be procyclical when risk is mispriced; bids need consent and a seizable bond to be credible |
| Single nominated relayer | Simplest | No fee competition; nomination is a trusted choice |
| Bonded set, open entry | Better liveness | Capital fragmented across bonds; coordination on who pays |
| Fully permissionless | Best liveness | Anyone can attempt payout; no bond means no recourse |

### 5. Bond asset

| Option | Pros | Cons |
|---|---|---|
| **`solBSV` only** ✅ | The same unit as the exposure, so no price move can shrink it relative to what it protects; the program holds it and can seize it; a slash is deflationary | Procyclical only if `solBSV` itself depegs — the case `k = 1` accepts because `solBSV` and BSV are the same asset and any deviation is an arbitrage |
| Mixed: BSV in a timelocked multisig, topped up in `solBSV` | The BSV leg does not depeg with the thing it insures | More moving parts; only the Solana leg is seizable (A16); reintroduces a signer set |
| Stablecoin or SOL | Uncorrelated with the peg | Not BSV-denominated; a stablecoin bond against a BSV liability is a **written call option** on the reserve, struck at `bond ÷ float` — see doc 04 |

### 6. Peg-out capacity control

| Option | Pros | Cons |
|---|---|---|
| **Per-relayer capacity: a redemption is refused unless some relayer's `bond_R ≥ k × owed_R` covers it; per-tx max secondary** ✅ | Matches what can actually be paid; the check is on-chain from verified proofs; prevents a race the relayer cannot win | Capacity must be tracked and published; a relayer's own float still limits what it can pay in practice |
| Per-transaction max only | Trivial | Several large redemptions can all pass and collectively exceed the available float, so escrows are returned rather than paid |
| No cap | Maximum freedom | The float is drainable in one transaction |

### 7. Parameter governance

**There is no governance mechanism in the PoC at all (D7).** No vote, multisig or
timelock exists. The options below are recorded for the deferred design, not chosen:

| Option | Pros | Cons |
|---|---|---|
| **No governance; parameters fixed in code** ✅ *(the PoC position)* | Nothing to capture; `FLOOR` cannot be voted down | Nothing can be adjusted without shipping a new program |
| Multisig + timelock for economic; safety increase-only | Fee/limit agility without a mint-voucher key; timelock lets users exit | Two paths to reason about; not the PoC's position |
| Single multisig, all parameters | Simple | A key compromise can set `FLOOR = 0` and mint against unconfirmed blocks |
| Token vote | Legible | **Safety parameters should not be votable** — a majority can strip its own protection |

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

2. **Bond `k` — settled: `k = 1` (D5).** `solBSV` and BSV are the same asset, so any
   deviation is an arbitrage and closes; `k = 1` is defensible rather than having to
   absorb a standing discount. Self-dealing stakers are an accepted risk: at `k = 1` a
   self-dealing attack is roughly break-even, so what makes it unprofitable is **the
   mining cost of the reorg**, not the bond. The bond's job is covering an honest
   relayer's shortfall.

3. **A failed redemption — return the escrow, and nothing else moves.** The holder's
   `solBSV` is still in the vault, so the settlement is to return it: the holder is
   made whole and **supply is unchanged**. The bond is **not additionally transferred**
   to the holder, because the return has already made them whole. This is the honest
   consequence of refusing a freeze authority: **holders keep their balance through a
   default and absorb any shortfall as a discount rather than a confiscation.** The bond
   protects against a relayer's deliberate theft or abandonment — **not** against the
   reserve being short, **not** against a failed redemption, and **not** against a reorg
   — and it should not be described as if it does.

4. **`FLOOR` is a policy parameter, not a derived one.** Sizing it from the cost of
   reorging `FLOOR` blocks needs a **BSV price feed** to value what is at risk and a
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

   So `FLOOR` is set at **12 blocks, fixed in code** for the PoC (D4), and **disclosed
   before someone transacts**. It does not need to be dynamic. What matters is that it
   is *visible*, not that it is computed. The mechanism to change it is deferred (D4,
   D7); there is nothing to vote with yet.

5. **`RECENT_REORG_WINDOW` — a safety parameter (P5), with one open item.** It is
   classed **safety**, and the depth rule it expresses is settled: a reorg of depth
   `R ≥ FLOOR` is the signal, and `R < FLOOR` is ordinary tip churn and is ignored.
   Whether the program **enforces it as an on-chain gate** or merely **monitors and
   publishes it** is a known open item. Block times are directly available — the header
   carries a Unix timestamp at offset 68, so the observed mining rate over the window is
   computable on-chain, and a mean spacing far below ten minutes is worth warning users
   about — but a genuine hashpower surge looks identical to an attack, so the choice is
   not free. Note the variance is real: a dozen blocks is a small sample.

### Resolved from review

6. **Fee mechanism — settled: the order book.** Fees are discovered on the book, matched
   by price then time (see §The book). The reason no winner has to be attested on-chain
   survives from the earlier discussion: competition is continuous rather than at an
   epoch boundary, so there is no off-chain auction winner whose identity must be
   proven. The alternatives once considered — a user-set fee with open fulfilment, or an
   exclusive epoch right that starts at zero — are recorded as rejected.

7. **Reserve invariant — settled (D8): monitored, not enforced.** `custodied BSV ≥
   outstanding solBSV` is published and monitored, and **the protocol cannot enforce
   it**, because the reserve is off-chain BSV the program cannot read. The earlier worry
   that re-mints make it transiently false is gone: no failure path mints, so supply
   only changes when the vault burns a staged mint or settles a redemption.

### The aggregate mint cap, restated as policy

`MAX_PEG_IN` still does not bound a reorg — a BSV block holds thousands of
transactions, so an attacker fills a fraudulent branch with many deposits under any
per-transaction limit. The bound that works is an **aggregate cap per window**.

But decision 4 applies to it too: it is **set conservatively as policy, not derived**,
because deriving it needs the same two oracles we have excluded. It is a coarse
backstop beneath `FLOOR`, not a calibrated constant — and it is secondary to the two
bounds that need no oracle at all:

- **`FLOOR` set at 12 blocks**, which is what makes out-mining the honest chain expensive;
- **the hot float cap**, which limits what a *successful* attack can actually extract.

All of these are **published and disclosed**. The disclosure matters more than the
value: a user should be able to see `FLOOR`, the cap and the float limit before they
transact. There is nothing to vote with in the PoC (D7).

Monitoring is also where the **external metrics** belong — the ones that must never
enter consensus. A user deciding whether to peg in is better served by seeing the
BSV price, an estimated cost to reorg `FLOOR` blocks, the current hashrate and the
observed block rate alongside the parameters than by seeing the parameters alone.
Published, none of it is a trust assumption; consulted by the program, all of it
would be.

**One qualification on a future vote.** It should not mean freely *loosenable*. Whoever
can vote `FLOOR` down toward zero holds a mint voucher, and no amount of deliberation
makes that safe. The proposed rule for the deferred governance design is **increase-only
under governance**: a vote can make the protocol more conservative at any time, while
making it *less* conservative means shipping a new program. That is a proposal for the
deferred design (D7), not a shipped power — there is nothing to vote with yet.

## O1 — Yield is paid in the asset staked

**Corrected from an earlier draft, which proposed a single `solBSV` pool.** Stakers are
paid in what they staked, because that is what they are providing:

| Side | Stakes | Earns | When |
|---|---|---|---|
| BSV | BSV in a relayer's float | **BSV** | Deducted at peg-in, realised at once |
| Solana | `solBSV` | **`solBSV`** | Deducted at peg-out, realised at once |

Both fees are deducted where the payment happens and accrue to that side's stakers, so
there is no cross-asset conversion, no second pool, and no third party paying anyone.

**Fees mature on the same schedule as the principal.** A fee withdrawable immediately
while the mint behind it is still maturing would be an exit from maturity — the one
thing the gates exist to prevent. So a fee is credited at once and *released* on the
same schedule. That costs nothing in practice: the window is hours and the accounting
is continuous.

**The arithmetic still lands at 1:1.** A peg-in of 100 BSV mints `100 − fee` into the
vault and credits `fee` to that side's stakers, so custody and supply move together for
the deposit; the fee is a transfer between the parties, not new backing. A peg-out burns
the escrowed amount and pays out the same amount less the fee, so both sides fall
together. Neither leg creates a claim on nothing.

**This also collapses "staking" into "relaying".** The BSV-side staker *is* the party
who pays BSV on redemption; the `solBSV`-side staker *is* the party who fronts `solBSV`
on a peg-in before maturity. There is no separate passive staker class needing its own
accounting system — and that supplies the liquidity on both sides with the same
participants bearing the risk.

## O2 — There is no operator, and that was the point

**Corrected.** An earlier draft proposed "a single bonded operator with a cold multisig
reserve". That *contradicts* [`05-relayers.md`](05-relayers.md) and
[`04-trust-model.md`](04-trust-model.md), which describe a relayer as a **role anyone can
run**, bonded in seizable `solBSV`, with a stated roadmap to a signerless reserve. This
document should not have invented a privileged operator. The fix is to restate what those
documents already say rather than to add a new role.

**Minting has no trusted party at all.** Advancing headers is permissionless, and after
the audit fix (A1) the proof-of-work check actually binds, so a forged header costs real
work rather than one hash.

**Redemption's trust is bounded, and already specified elsewhere:** the bond is `solBSV`
held by the program and therefore seizable; the naked-spend residual is answered by
per-relayer isolation and by holding no idle float; and the covenant track removes the hot
key outright. See docs 04 and 05 — this document should not restate them.

### How maturity stops a reorg mint, with no oracle and nobody to trust

This is the part worth stating precisely, because it is where the design succeeds:

1. Minted `solBSV` goes to a **program-owned vault**, not the depositor.
2. The vault releases only once the deposit's block is **still canonical N blocks later**
   — read from headers the program already stores.
3. If BSV reorgs, the program follows the heavier branch through the permissionless
   `commit_fork` path and **burns the still-staged tokens**. The fraud never becomes
   liquid.

No external data is consulted at any step and no person decides anything. The only
requirement is that **honest headers are pushed within the maturity window** — a liveness
condition anyone can satisfy rather than a trust assumption, and one with a built-in
incentive: a fraud that goes undetected eats the staking buffer, so the people with the
most to lose have the most reason to advance the honest chain and notice an orphan.

**What maturity does not fix:** if nobody pushes the honest branch for the whole window,
the staged tokens release and the attacker leaves. That is why `FLOOR` remains — it makes
the reorg expensive whether or not anyone notices — and why the buffer remains, to cover
the case where notice arrives too late.

## A6 — Remove the pooled reserve instead of securing it

The question was what address type the deposit script should be. **The better answer is
to have no shared address at all**, because a pooled reserve is what creates a single key
worth stealing.

### The proposal: deposits pay a relayer, not the bridge

1. Each relayer registers a **BSV script of its own** — P2PKH for a single-key relayer,
   or P2SH multisig if it wants shared custody. The registry lives on Solana and is
   replaceable.
2. A depositor sends BSV **to a relayer's script**, with the usual `OP_RETURN` naming
   their Solana address.
3. `verify_deposit` checks the output pays a **registered relayer's script**, not a
   hardcoded bridge script.
4. The proof itself tells the program which relayer received the deposit, so the program
   **accumulates a per-relayer liability `owed_R` from verified proofs alone**.

**There is then no bridge address**, so no single key whose theft drains everything, and
no committee needed to hold one.

### It also repairs part of A4

A4's complaint was that the staking buffer is an unverifiable off-chain attestation. That
was true of a **pooled** reserve. Per-relayer it stops being true on the side that matters:

- `owed_R` is **derived from proofs the program verified itself**, not attested by anyone.
- `bond_R ≥ k × owed_R` is therefore **checkable on-chain**.
- `bond_R` is `solBSV` held by the program, so it is **seizable on-chain**.

The binding constraint moves from "a custodian promises the buffer exists" to "the program
measures each relayer's exposure and can seize its bond" — the first version of this design
where the buffer is a protocol quantity rather than a promise.

### On multisig

Not needed for the reserve once deposits are per-relayer; a system-wide one would
reintroduce exactly the signer set the design avoids.

| Use | Recommendation |
|---|---|
| A relayer's own shared custody | **Fine** — its own P2SH multisig, its own signers, its own bond. Permissionless |
| A shared system reserve | **Avoid** — recreates the single point of theft and needs a committee |
| Last-resort recovery of stuck funds | **Defer** — Script cannot constrain where a key sends funds, so a recovery key is a backdoor whether or not it is ever used |

### The end state

The covenant track in [`04-trust-model.md`](04-trust-model.md#the-roadmap-to-a-signerless-reserve)
— Script releasing funds only against a proof of the burn, via `OP_CAT`, `OP_MUL` and
in-script verification — removes the spending key entirely. That is the destination;
per-relayer deposits are the path that does not depend on it landing.

## `owed_R` — definition

Used throughout documents 04, 05, 12 and 13 and previously defined nowhere. It is the
quantity the bond is measured against, so it needs to be exact.

> **`owed_R` is the total `solBSV` that relayer `R` has been credited with and has not yet
> discharged — accumulated only from deposits the program has itself verified, and reduced as
> each is discharged.**

It includes, and this is the part that matters:

- **Released mints** — `solBSV` the depositor now holds, which `R` must redeem in BSV if asked.
- **Staged mints still in the vault.** A depositor whose mint is still maturing has paid BSV
  and holds no tokens. If `R` disappears at that moment the depositor has lost the whole
  deposit, so it must be covered. Omitting staged mints would leave exactly that window
  unbonded, which is audit finding **F4**.
- **Escrowed redemptions `R` has accepted** but not yet paid, since `R` holds the obligation.

It excludes `R`'s own float, which is `R`'s capital and not a system liability. The bond is
sized against `owed_R`, not against everything `R` holds — and separating the two is what makes
`bond_R ≥ k × owed_R` meaningful rather than arbitrary.

**Why it is the right unit.** `owed_R` is derived from proofs the program verified itself, not
from an attestation by anyone. That is what makes the inequality **checkable on-chain**
(audit A4), and it is why the buffer is a protocol quantity on the per-relayer design rather
than a promise about off-chain BSV.

## Is the buffer a protocol input? — yes, and it can be checked

Review asked whether the buffer must be a protocol input, and whether mint and redeem
should be gated on it per transaction. **Yes to both, but only once two things are true**,
and the gap is in the second.

### The bond is checkable; the BSV is not. Use the bond.

Per-relayer (see §A6), the program knows two quantities without an oracle:

- **`owed_R`** — what relayer `R` has been credited, accumulated from proofs the program
  verified itself;
- **`bond_R`** — `solBSV` the program holds and can seize.

So the gate is a per-transaction check, and it is one the program can evaluate for itself
from quantities it verified — *designed, not built*:

> A mint naming relayer `R` is refused unless `bond_R ≥ k × (owed_R + this mint)`.
> A redemption is refused unless some relayer with sufficient bond accepts it.

If the buffer is meant as a protocol input, **this is the input.** The off-chain BSV is
not, and does not need to be — the bond is the thing that can actually be seized, so it is
the thing worth gating on.

### The missing piece: the relayer must consent

Without consent the gate is unfair in a way that breaks it. A fraudulent mint names *some*
relayer's script, and if the program credits `owed_R` on the strength of the proof alone,
an innocent relayer's bond is slashed for an attack it never agreed to and could not have
detected.

So a mint against `R` needs **`R`'s signature accepting the liability**. That converts the
reorg risk from an externality into a priced term — `R` knows what it is underwriting and
charges for it — and it is what makes slashing `R` for a liability it accepted defensible
rather than arbitrary. Consent is also what makes the per-transaction gate meaningful: you
cannot check a relayer's capacity if you never asked it whether it was willing.

### The bootstrap path

The gate has a chicken-and-egg problem: nothing can mint until a bond exists, and the bond
is funded by staking, which in turn has nothing to earn from until mints happen. **D3
settles this as G2, the vault-gated genesis mint:** the genesis mint lands in the program
vault and is released only once a matching BSV deposit is verified, so supply exists but is
never liquid until it is backed — no unbacked window to attack, and nothing to keep quiet
about. The first relayer is the team. It needs to be a separate instruction with its own
rules and its own test, not a special case buried in the mint. See §D3 and
[`05-relayers.md`](05-relayers.md).

### On the reserve: do not pool it, rather than securing a pool

Review asked whether the reserve should be a second smart contract, with funds aggregated
and the two-gate lock preventing fraud. The two halves of that need separating, because
they defend against different things and only one of them is a reserve question.

**The two-gate lock does not protect the reserve.** It protects against a **fraudulent
mint** — staged tokens are burned when a reorg is followed, so a fake deposit never becomes
liquid. That is a Solana-side defence and it works without any BSV contract. What it does
*not* do is stop a relayer spending funds it already holds; nothing on the Solana side can
observe that.

**Aggregation is what creates the reserve problem.** One pot means one key worth stealing,
and a key that can be stolen needs either a covenant or a committee. A BSV Script cannot
constrain where a key sends funds, so "the reserve is a contract that only releases against
a valid burn proof" is the covenant track — `OP_CAT`, `OP_MUL`, in-script proof
verification — which is pre-mainnet and unaudited.

| Shape | Reserve key | Needs |
|---|---|---|
| **Pooled** | One key, or a covenant | A committee, or the covenant track |
| **Distributed per relayer** ✅ | Each relayer's own, spent by its own key | Nothing beyond the seizable bond |

**So the recommendation is to not have a pooled reserve at all.** With deposits paying
individual relayers, there is no aggregated pot and therefore no reserve contract to
write, audit or trust — each relayer's exposure is `owed_R`, individually bonded and
seizable on Solana. Aggregation is the thing that creates the problem; removing it is
cheaper than securing it.

The UX does not have to suffer: the website can present one address and rotate it, while
custody stays distributed. What the depositor sees and who holds the key are separate
questions.

**If a pooled reserve is ever wanted anyway**, the covenant is the only non-custodial way
to do it, and it is the reason docs 04 and this document keep the redemption authority
replaceable. That is a destination, not a prerequisite.

## The book: staked bids as an order book

Review proposed replacing the fixed fee and the `FLOOR` debate with **an order book of
underwriting**. Stakers post sell orders — "I will underwrite up to X at such a fee",
"…at such a depth" — matched against incoming requests by price then time, partially
filled, with unused stake returned to the staker's own address.

This is better than what it replaces, for three reasons:

- **The fee is discovered rather than chosen.** No parameter to argue about, and a fixed
  fee stops being load-bearing.
- **The confirmation depth can be a term of the bid**, not a protocol constant. A staker
  who wants 24 blocks says so; the book quotes it. `FLOOR` becomes a backstop rather than
  a price, which is exactly where the earlier discussion wanted it.
- **The depositor can see the liquidity and the price before committing**, which is the
  disclosure the website was already going to have to provide.

It also dissolves the bootstrap problem: a book with no bids is a coherent state, not a
broken one.

### Can a staker auto-approve? **Yes — settled (D2)**

**An earlier draft answered this "no", and the correction is worth keeping. It argued
that a passive bid which fills automatically is a limit order facing informed flow, that
the informed flow here is a miner who deposits, is underwritten, and then reorgs its own
block away, and that a book of auto-filling bids is therefore "a book of sitting ducks".
That framing was wrong.**

**The vault changes who bears the loss.** A fill is not liquid: the mint is staged, so if
the reorg is detected the staged tokens are burned, the liability is reversed, and **the
staker loses nothing.** The staker is harmed only when detection *fails*, which is a
property of the system rather than of any individual fill. The loss is therefore
**systemic, not per-fill** — and a toxic deposit is indistinguishable from an honest one,
so a per-fill approval has little to inspect.

The levers that actually matter are the ones that make detection work: **maturity
length**, **depth** (which keeps the attack rate down), and **the incentive to push the
honest chain**. **Auto-approve is the settled answer for the PoC, to be revisited against
the finished system** — if a last-look window turns out to be cheap insurance, it can be
added then.

### The no-staker path: safe against accident, not against attack

Review proposed a second UX where a transfer proceeds with no staker behind it, on the
basis that the risk is the sender's own. **Half of that holds and half does not.**

It holds for the **deposit**: a reorg takes the sender's BSV back to them, and the two-gate
vault burns the staged tokens, so the sender ends up where they started. Nobody is out of
pocket, and an unseeded book is a perfectly reasonable place to start.

It does **not** hold for the **system**. The attacker in a self-reorg *is* the depositor, so
"the sender bears the risk" describes the attacker bearing the cost of their own attack.
If detection fails and the vault releases, the unbacked `solBSV` dilutes **every** holder,
not just the sender. There is no staker to absorb it because there is no staker.

So the honest framing is: **an unseeded book is safe against accident but not against
attack, and the difference is whether detection works.** It is a reasonable bootstrap and a
reasonable choice for a user who understands it, but "at the sender's risk" understates it
— the tail lands on holders. That is precisely the gap a buffer closes, and precisely why
the buffer's size is the number worth watching.

## Flows at a glance

```
PEG IN — BSV to solBSV
  1  CHOOSE    depositor takes terms from the book, or accepts no backing
  2  SEND      BSV to a relayer's script; OP_RETURN carries the Solana address
  3  DEPTH     wait the depth the bid named, measured in block time
  4  MINT      solBSV issued into the program's vault, not to the depositor
  5  MATURE    if no reorg is followed, the vault releases; fee to the stakers
               if a reorg IS followed, the staged tokens are burned. No loss

PEG OUT — solBSV to BSV
  1  ESCROW    solBSV moves into the program's vault; a BSV destination is named
  2  ACCEPT    a relayer with bond to cover it takes the request
  3  DEADLINE  slots, not wall-clock, so a Solana halt freezes the clock
  4  PAY       the relayer pays BSV and proves it against the light client
  5  CHALLENGE a reorged payout is caught here
  6  SETTLE    burn the escrowed solBSV, pay the fee
               or on failure, return the escrow to the user. Supply never changes
```

Both gates are the same shape: **enter the vault, then leave it either to the counterparty
or back to the sender.** Every failure path is a return rather than a mint.

---

## Decisions D1–D8, in full

All eight are settled for the PoC. [`14-decisions.md`](14-decisions.md) is the register;
the reasoning behind each is here. Several are **decisions to defer**, which is different
from leaving a question open.

### D1 — Who may stake: specialists first

| Option | Pros | Cons |
|---|---|---|
| **Specialists only, with a minimum stake either side** ✅ | Sophisticated parties who can price reorg and custody risk; simpler to reason about | Thin liquidity at launch; excludes everyone else |
| **Delegated staking — retail pledges to a specialist operator** | Deep liquidity; a familiar model; a real product for exchanges and miners to offer, with rewards for retail | Retail cannot assess operator risk, so slashing lands on people who could not evaluate it; needs an operator-selection story |

**Settled: specialists first.** Anyone-may-stake is a phase-2 goal, and **the upgrade path
is a deliverable rather than a maybe** — the design must carry it from the start rather
than have it bolted on. What that path looks like is deferred to final implementation.

### D2 — Can a staker's bid fill automatically? **Yes — auto-approve**

**Settled for the PoC.** The vault reverses a detected fill, so the loss is systemic rather
than per-fill and there is little for a per-fill approval to inspect. The full reasoning is
in §Can a staker auto-approve?, above. To be reviewed against the finished system.

### D3 — Genesis: **G2, the vault-gated genesis mint**

**Settled.** The genesis mint lands in the program vault and is released only once a
matching BSV deposit is verified. No unbacked window exists at any point, so there is
nothing to attack and nothing to keep quiet about. The alternative shapes were considered
and are recorded for the deferred design:

| # | Option | What breaks the loop | Assumption it carries |
|---|---|---|---|
| **G1** | **Self-underwritten genesis.** The team is the first relayer, deposits its own BSV, and mints against it | The team is honest | The same assumption `initialize` already makes when it sets the checkpoint — no *new* trust |
| **G2** ✅ | **Vault-gated genesis mint.** A genesis mint goes straight to the program vault and is released **only when a matching BSV deposit has been verified** | Nothing — the vault means there is no unbacked window to attack | A staged token is not stealable, so it does not matter who knows |
| **G3** | **Genesis bond in SOL, migrated to `solBSV` later** | A non-`solBSV` asset | A price mismatch, which doc 04 argues against — acceptable only for a bounded, short genesis |
| **G4** | **Capped unbacked genesis.** The first `X` BSV of peg-ins need no underwriting, because the amount at risk is bounded and small | A hard cap, nothing else | The cap is genuinely below what anyone would bother attacking |
| **G5** | **Slot-expiring genesis authority.** A named key may seed a bounded amount until a slot, then is permanently dead | A privileged window | The window is short and bounded, and the authority is provably dead afterwards |
| **G6** | **Compile-time test mint** (`#[cfg(feature = "poc")]`) | Nothing — test only | Deliberately not a runtime flag: a runtime flag can leak to mainnet, a compiled-out one cannot |

**G2 composes with G1** — the team self-underwrites, the vault holds the result until the
BSV verifies — which requires no new instruction beyond the ones already specified for
ordinary peg-ins. The instinct behind "mint first, announce later, back it afterwards" is
right that an empty system is not worth attacking, but security by obscurity is the weakest
form of the argument and it is unnecessary here: a mint into the vault is not liquid, so
there is nothing to take even if the whole world knows.

### D4 — Is `FLOOR` still needed? **Yes — 12 blocks, fixed in code**

**Settled.** Once depth is a term of each bid, `FLOOR` is a backstop rather than a price.
It stays low enough never to bind and high enough to catch a bid nobody should accept.
Twelve blocks for the PoC, **fixed in code**; the change mechanism is deferred (D7).

### D5 — The bond multiple `k`: **`k = 1`**

**Settled.** `bond_R ≥ k × owed_R`. `k = 1` covers the principal; anything above covers
the case where the bond is worth less exactly when it is needed, and with `solBSV`
denomination no price move can shrink it relative to the exposure. At `k = 1` a
self-dealing attack is roughly break-even, so what makes it unprofitable is **the mining
cost of the reorg**, not the bond. The bond's job is covering an honest relayer's
shortfall. Self-dealing stakers are an accepted risk rather than a prohibited one.

### D6 — Does the unbacked path exist at all? **Yes, explicitly**

**Settled: allowed.** A peg-in may proceed with no underwriter at all. Whoever does so
**accepts the initial risk of a system with nobody watching while liquidity is seeded**,
and may keep topping up on those terms. The consequence is stated sharply in
[`15-audit-2.md`](15-audit-2.md) (F2): on that path the vault and detection are the entire
defence, because there is no bond and no buffer to absorb a detection failure. Bounding it
later — expiry after `n + 1000` blocks, or a designated initial LP address — is recorded in
the deferred list rather than needed now.

### D7 — Governance: **none in the PoC**

**Settled: there is no governance mechanism at all.** No vote, multisig or timelock
exists. Details are to be figured out on review once the system is better understood and
demonstrably working. Recorded so that "we launched without governance" is a decision
rather than an omission, and so the upgrade path stays a named gap.

### D8 — The reserve invariant: **monitored, not enforced**

**Settled.** `custodied BSV ≥ outstanding solBSV` is published and monitored, and **the
protocol cannot enforce it** — the reserve is off-chain BSV the program cannot read. The
website shows the ratio; the program does not check it.

## Audit findings

An adversarial review ran this document against the implemented program. Severities are
the reviewer's. **Fixed** means addressed in code with a test where one was possible.

| ID | Finding | Severity | Status |
|---|---|---|---|
| **A1** | **The difficulty target was read from the header being checked.** `bits` came from the submitted header and `check_daa` returned `true` unconditionally, so the target was attacker-chosen and proof of work was vacuous — anyone could append headers and mint with no hashpower. Every "forging `FLOOR` blocks must out-mine the chain" claim here was false against the code. **Regtest masked it**: `0x207fffff` is already the largest encodable target, so no easier one exists there. On testnet the target varies and declaring the easiest permitted one is cheap | critical | **Fixed** — target taken from the chain at `initialize`, validated before use. Test added |
| **A2** | `set_checkpoint` and `set_paused` were **unauthenticated**: `authority` was a bare `Signer` compared to nothing, so any key could rewrite the trusted root or halt minting | critical | **Fixed** — authority stored, `has_one` enforced. The deploy-time race on `initialize` remains |
| **A3** | `verify_deposit` never constrained `mint`, so anyone could submit a valid public deposit against a counterfeit mint and burn the replay slot — stranding the real deposit for ~0.002 SOL | critical | **Fixed** — pinned to the `[b"mint"]` PDA |
| **A4** | The staking buffer is off-chain BSV — unverifiable and unseizable, so `S ≥ M` was unenforceable as written | critical | **Partly fixed by A6.** Per-relayer, `owed_R` comes from verified proofs and `bond_R` is seizable, so the constraint is checkable on-chain for the part the program can see; the relayer's float is still off-chain |
| **A5** | The **program upgrade authority is an unconditional mint voucher.** Safety parameters are Rust `const`s, so "loosening needs a new program" and "ship a new program" are the same power | critical | **Out of scope for the PoC** — accepted and recorded. The fix is governance: a vote, or the stakers, possibly holding additional tokens granting that right. **Must not be silently forgotten** |
| **A6** | The peg-in destination is a **P2PKH key, not a covenant** — the entry point for the whole reserve is a raw key | critical | **Proposed** — remove the pooled reserve rather than secure it. See the section above |
| **A7** | **The replay key included `height`**, so a deposit re-included at a different height after a reorg minted twice. Introduced by this document's own pruning change | serious | **Fixed** — identity is `(txid, vout)`; height stored only for pruning |
| **A8** | The staging escrow is **not implemented**, and `UsedDeposits` stores no recipient or amount, so reversing N credited mints needs an off-chain indexer and one paid transaction per depositor | serious | **Open** — proposed only |
| **A9** | `MAX_USED = 200 < WINDOW = 288` is a cheap peg-in shutdown; `MIN_PEG_IN`/`MAX_PEG_IN` are unimplemented, so 200 one-satoshi deposits block all later peg-ins | serious | **Open** |
| **A10** | The aggregate cap is not tied to the buffer, so `S ≥ M` cannot hold by construction | serious | **Open** |
| **A11** | `commit_fork` never re-anchors the staged branch, so an intervening commit can splice the window from two chains with broken linkage | serious | **Open** |
| **A12** | `commit_fork` only emits an event — no depth recorded, no pause, no bounty — so the gate is triggered off-chain and is itself griefable given A1 | serious | **Open** |
| **A13** | "No oracles, by construction" is false for quantities that gate funds: the hot float is off-chain BSV | serious | **Open** — see A4 |
| **A14** | The committed confirmation depth is not parsed, and **would not bind an attacker anyway** — the fraud's depositor *is* the attacker, so they commit exactly `FLOOR`. It is a market term, not a safety one | serious | **Open** |
| **A15** | Return-to-sender is not an on-chain path: no refund instruction, `parse_outputs` reads only outputs, and spending the deposit needs the P2PKH key | serious | **Open** |
| **A16** | The bond asset contradicts across documents, and only the Solana leg is seizable | serious | **Resolved by decision** — the bond is `solBSV` only (D5), held by the program; the mixed BSV/SOL framing is gone |
| **A17** | Pausing freezes `push_header` too, so the tip stalls and unpausing needs the missed headers replayed one transaction at a time | minor | **Open** |
| **A18** | Minor mismatches — `bits_to_target_be` masks the sign bit, and the adversary playbook expects an error that does not exist | minor | **Partly fixed** — `used_deposits` is now pinned to `[b"used_deposits"]` (X2), so that part is closed; the sign-bit mask and the stale playbook expectation remain |

**The lesson worth keeping.** A1 and A2 were invisible to a green suite: the tests asserted
that bad proof of work was rejected, and it *was* — against the target the test itself
supplied. A2 had no test at all. Passing tests demonstrated that the code did what the
tests did, not that the client was secure.

## Deliberately deferred

Recorded so these are not later relitigated as oversights. Most are **decisions to defer**
rather than open questions; each is a known simplification or an unbuilt part of the
current model.

| Deferred | From | Note |
|---|---|---|
| The change mechanism for `FLOOR` | D4 | Voting, or the stakers. `FLOOR` is fixed in code for the PoC |
| The open-staking upgrade path | D1 | Specialists first; anyone-may-stake is a phase-2 goal the design must carry from the start |
| Bounding the unbacked peg-in | D6 | Block-height expiry, or a designated initial LP address |
| Governance generally | D7 | Absent by decision, not by accident. No vote, multisig or timelock exists |
| The program upgrade authority | A5 | It can override every parameter. The fix is governance, possibly tied to staking |
| An independent audit | — | The critical defects found so far were found by our own adversarial review |
| Risk-priced confirmation depth | — | Depth is a term of the bid; whether longer waits actually earn lower rates is an empirical question |
| Differential yields per side | — | BSV-side staking carries custody risk; `solBSV`-side carries reorg-fraud risk. Pricing both the same may misprice one |
| Fee realisation mechanics | — | Whether stakers withdraw from their own float or accrue a claim is unresolved |

## What this changes downstream

The design implies these changes to the implemented program:

- **`MIN_CONFIRMATIONS`** stays 12 and is the code's form of `FLOOR`; **`FLOOR` as a
  distinct parameter is not built.** There is no governance in the PoC (D7), so it is fixed
  in code rather than voted. Loosening it means shipping a new program.
- **The difficulty retarget** must be implemented, or `expected_bits` permitted to advance,
  before testnet: the client halts permanently at the first retarget (F7). This is a
  liveness gap in the built code, not a designed feature still to be written.
- **`commit_fork`** must record reorg depth and block time when it fires, feeding
  the depth rule and the pause.
- **`push_header`** must start using the header timestamp it would need to start parsing, to
  expose the regression, catch-up and staleness signals.
- **The mint path** gains the gate checks, and mints into the vault rather than to the
  depositor; **the redemption path** gains both the gate and the capacity check.
- **The header window** (currently 32 h / 192 blocks, set by what cw-144 needs) is a *liveness* parameter for
  following reorgs and should not be conflated with `RECENT_REORG_WINDOW`, which is
  a *safety* parameter (P5). They are currently the same idea in two places and must be
  named apart.
- **A per-window mint cap** (`MAX_MINT_PER_WINDOW`) is wanted as a coarse backstop,
  set by policy rather than derived — see §The aggregate mint cap, restated as
  policy. It sits beneath `FLOOR` and the hot float cap rather than replacing them.
- **The replay list is no longer a lifetime limit.** It is now pruned by height, so
  it bounds deposits *per window* rather than total usage. The previous fixed list
  of 256 stopped the peg-in path permanently once reached, which ordinary volume
  would have done on its own. A test that mints across a window boundary to
  exercise the prune is still wanted — the current fixture cannot reach one.
- **Burning a staged mint must remove its replay entry** (F1), or a re-included
  deposit can never be re-proven and an honest depositor's BSV is stranded.
- **Tests** in `poc/TEST_PLAN.md` §4 and §5 gain the gate and capacity cases.
