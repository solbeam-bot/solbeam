# 6. Parameters & governance

> **Specification, not shipped behaviour.** The light client, the token, the mint and fork
> staging are **built and tested** (17 on-chain tests). **The vault, the two gates, maturity, the
> order book, staking, bonds, `owed_R`, consent, per-relayer deposit scripts, `FLOOR` as a
> distinct parameter and all of peg-out are designed and not built.** DAA is **actively rejected
> in the built client** (F7), not merely absent. The shipped program mints straight to the
> depositor's token account, so the parameters below that depend on the vault or on a book are,
> at this moment, a specification rather than a property of the code.

## Launch parameters

These are the parameters of the two-gate design, with the stable IDs from
[`12-peg-mechanism.md` §Parameters](12-peg-mechanism.md#parameters). A **safety** parameter is
one that bounds a fraud; an **economic** parameter sets price and size. The distinction is
load-bearing, because the two classes cannot be governed the same way — see §Changing
parameters.

| ID | Parameter | Proposed | Class | Why |
|---|---|---|---|---|
| **P1** | `FLOOR` — minimum confirmation depth | **12 BSV blocks** (~2 h) | **safety** | The hard bound. Depositors and bids may commit to *more*, never less. **Fixed in code for the PoC** (D4) |
| **P2** | `C_payout` — payout confirmations | **12 BSV blocks** | **safety** | Depth a relayer must reach before it may claim a redemption was paid |
| **P3** | `D` — redemption deadline | **6 h of Solana slots** | liveness | Measured in **slots**, not wall-clock, so a cluster halt freezes the clock rather than burning the relayer who could not act |
| **P4** | `W` — challenge window | **24 h** | **safety** | Must exceed the reorg risk on the payout; a reorged payout is caught here |
| **P5** | `RECENT_REORG_WINDOW` | **12 h** | **safety** | Intended as depth-aware: a reorg of depth `R ≥ FLOOR` would pause releases, `R < FLOOR` is ordinary tip churn and is ignored. **Whether it is enforced as an on-chain gate or merely monitored and published is a known open item** — it is a safety parameter either way, and this document does not assert which until that is decided |
| **P6** | `TIP_STALENESS` | **2 h** | **safety** | Pause if the tip stops advancing. The one signal that needs a wall clock, and only for "now" |
| **P7** | `MIN_PEG_IN` | **10 BSV** | economic | Fee economics, dust and spam. Unimplemented (A9) |
| **P8** | `MAX_PEG_IN` | **10,000 BSV** | economic | Per *transaction* only. It does **not** bound a reorg — see §The limit that does not bound a reorg |
| **P9** | `MAX_PEG_OUT` | **10,000 BSV** | economic | Also bounded by live capacity: a redemption is refused unless some relayer with sufficient bond accepts it |
| **P10** | `HOT_FLOAT_CAP` | config | **safety** | The real bound on what a *successful* fraudulent mint can extract. With per-relayer deposits it is each relayer's own float, not a system hot wallet |
| — | bond `k` | `k = 1` (D5) | **safety** | **`bond_R ≥ k × owed_R`**, where `owed_R` accumulates from proofs the program verified itself. Since `bond_R` is `solBSV` the program holds, the inequality is checkable on-chain |

**Depth and maturity are different parameters**, and both are needed. **Depth** sets the cost of
attacking — a reorg must out-mine `FLOOR`. **Maturity** sets the time available to detect. A low
floor makes attacks cheap and therefore frequent, raising the number of chances for a detection
failure to slip through. `FLOOR` is a backstop beneath a market term, not a price: the depth a
deposit actually waits is a **term of the bid**, committed in the deposit's `OP_RETURN`, and the
program enforces both `confirmations ≥ committed_depth` and `committed_depth ≥ FLOOR`.

**Safety parameters may only be moved in the conservative direction** — a rule for the deferred
governance design, not a description of anything that exists (D7). Whoever can set
`FLOOR = 0` or `RECENT_REORG_WINDOW = 0` holds a mint voucher; that is categorically different
from whoever sets a fee. Loosening a safety parameter should require shipping a new program, not
flipping a flag.

**Test overrides are compile-time, not config.** "Remove the minimum during testing" must be a
`#[cfg(feature = …)]`, never a runtime value — a runtime value can leak to mainnet, a
compile-time one cannot. (G6 is the genesis equivalent.)

## The limit that does not bound a reorg

`MAX_PEG_IN` bounds one transaction. A BSV block holds thousands of transactions, so an attacker
filling a fraudulent branch with many deposits slips under any per-transaction limit. The bound
that works is an **aggregate cap per window** (`MAX_MINT_PER_WINDOW`), and it is set
conservatively as policy rather than derived, because deriving it needs a BSV price feed and a
hashpower-rental feed — exactly the oracles the design excludes.

It is a coarse backstop beneath the two bounds that need no oracle at all:

- **`FLOOR` set high**, which is what makes out-mining the honest chain expensive;
- **the hot float cap**, which limits what a *successful* attack can actually extract.

Note the honest limit on all of it: the BSV-side buffer is off-chain, so
`staked buffer ≥ maximum mintable within one reorg window` is **tracked, not verified**. Per
relayer, `owed_R` and the seizable `bond_R` are the parts the program can measure; the BSV behind
them is not.

### A caution on the stablecoin bond

An early draft specified the bond as a **stable unit (e.g. USDC)**, matched to a BSV exposure by a
price governor, and sized it against a notional hot float plus a covenant tranche
(`bond ≥ k × (hot float + releasable tranche)`). That was wrong on both counts, and it is worth
recording why rather than quietly editing it.

A stablecoin bond against a BSV liability is a **written call option on the reserve, struck at
`bond ÷ float`**. Post `$500k` against `10,000 BSV` and the relayer is short `5,000 BSV`; at `$100`
the option is in the money and absconding is the rational trade. Worse, the attacker does not need
to time the move — a large holder can help the price along and manufacture the strike. A price
governor cannot fix a written option: it is reactive, it needs an oracle, and the window between
the move and the throttle *is* the trade. Denominate the bond in `solBSV`.

The second error was the **pooled hot float and covenant tranche** the old formula was sized
against. That object no longer exists: deposits pay a relayer's own script, so there is no shared
reserve to bound, and the constraint moved to `bond_R ≥ k × owed_R` per relayer. `k = 1` is
defensible because `solBSV` and BSV are the same asset, so any deviation is an arbitrage and
closes — no price margin is needed. `k` is a **sizing multiple on `owed_R`**, not a discount for a
hoped-for detection probability: the measured liability is what sets the required collateral. The
recorded consequence is that at `k = 1` a self-dealing attack is roughly break-even,
and what makes it unprofitable is the **mining cost of the reorg**, not the bond (D5). The bond's
job is covering an honest relayer's shortfall.

The launch numbers will be conservative: low per-transaction limits, a small float cap, modest
bonds. They rise as the system earns trust.

## Changing parameters as the system grows

**There is no governance mechanism in the PoC at all** (D7). This is a decision rather than an
omission: the PoC ships without any vote, multisig or timelock, and the details are to be figured
out on review once the system is better understood and demonstrably working. **The upgrade path
is a named gap** — recorded in
[`14-decisions.md` §Deferred to final implementation](14-decisions.md#deferred-to-final-implementation)
so it cannot be quietly forgotten.

What the earlier draft described here was a timelocked team multisig with a published change log,
a 24–48 hour on-chain delay, and an emergency pause. **None of that is built, and it is not the
PoC's position.** The reasons to be careful about it survive the rewrite:

- **Safety parameters are not freely loosenable.** Whoever can vote `FLOOR` down toward zero
  holds a mint voucher, and no amount of deliberation makes that safe. The reconciling rule is
  **increase-only under governance**: a vote can make the protocol more conservative at any time,
  while making it *less* conservative means shipping a new program. That is a proposal for the
  deferred design, not a shipped power — there is nothing to vote with yet.
- **The program upgrade authority overrides every parameter** (A5). It is an unconditional mint
  voucher, it is out of scope for the PoC, and it is recorded rather than hidden. The fix is
  governance, possibly tied to staking.
- **A change must be disclosed before someone transacts.** A user should be able to see `FLOOR`,
  the caps and the float limit before they peg in. Published, the external metrics — BSV price,
  estimated reorg cost, hashrate, observed block rate — are information; consulted by the program,
  they would be oracles. Display anything; decide on nothing external.

## Deferred, and named so it is not lost

| Deferred | From | Note |
|---|---|---|
| **Governance, generally** | D7 | Absent by decision, not by accident. No vote, no multisig, no timelock exists in the PoC |
| **The `FLOOR` change mechanism** | D4 | Voting, or the stakers. `FLOOR` is fixed in code for now |
| **Bounding the unbacked peg-in** | D6 | A peg-in may proceed with **no underwriter at all**, explicitly: whoever does so accepts the risk of a system with nobody watching while liquidity is seeded. It could be bounded later — expiring it after `n + 1000` blocks, or restricting it to a designated initial LP address. Neither is needed now; both need writing down |
| **The open-staking upgrade path** | D1 | Specialists first; anyone-may-stake is a phase-2 goal the design must carry from the start |
| **An independent audit** | — | The four critical defects found so far were found by our own adversarial review |

The off-chain **reserve invariant** is monitored, not enforced (D8):
`custodied BSV ≥ outstanding solBSV` is published and shown as a ratio, and the protocol cannot
enforce it, because the reserve is off-chain BSV the program cannot read.

## Wind-down and LP exit

There is no pooled reserve, and therefore no covenant to accelerate and no tranche schedule to
open. Each relayer holds its own BSV, so "wind-down" is not one switch: a relayer stops accepting
redemptions, its `owed_R` runs off as requests settle, and its bond — `solBSV` the program holds —
is released only after its unbonding period. `solBSV` holders are unaffected throughout, because
redemption does not depend on any one relayer: any bonded relayer may pay.

**The honest caveat**, unchanged in substance: whether the last BSV is actually paid depends on
relayers honouring requests, and a relayer that simply stops is handled by returning the escrow to
the holder rather than by a covenant. That keeps the exit open but does not make it final — the
roadmap target in [Trust model](04-trust-model.md) is what would.

### Entry is fast; exit is slow

This is worth stating plainly, because it is a deliberate choice rather than an oversight. The
system is **easy to enter and slow to leave**. Peg-in is trustless once the deposit is deep enough.
Exit is bounded at three separate points: the per-transaction and capacity limits cap how much can
be requested at once, `D` and `W` cap how long a payout may take, and the unbonding period limits
how fast relayer capital can leave. So a large holder cannot pull its whole position out in one
move, and a relayer cannot recycle its bond quickly. That is the same machinery that protects
holders, and it is why relayer capital has to be genuinely long-term. The asymmetry is acceptable
for one reason: **the fast direction is the one that needs no trusted party**, and the slow
direction is the one where trust has to be substituted with collateral.

## Parameter change checklist

Any parameter change should be:

1. **Proposed publicly**, with the reasoning and the new arithmetic.
2. **Disclosed before it binds**, so a user can judge the risk they are taking before they
   transact.
3. **Published** in a changelog with the effective date.
4. **Conservative or neutral on the safety class** — an increase-only move, unless a new program
   is shipped.
5. **Never able to move funds.** The bond and the vault are program-owned; parameters are not a
   path to the reserve.

**There is no process to execute any of this yet.** The checklist is the shape the deferred
governance design has to satisfy (D7), not a description of a mechanism that exists.

---

Next: [Roadmap](07-roadmap.md)
