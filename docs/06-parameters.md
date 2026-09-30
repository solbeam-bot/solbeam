# 06. Parameters

**Every value the system turns on, in one place.** Each row has an **ID**, a **name**, a **value** and
a **description**, with a status marker. The intent is that this becomes a config that both the
on-chain program and the node software read, so a specification can be correct while the numbers are
still provisional.

**The sheet is now driven by [`config/params.json`](../config/params.json); `docs/parameters.csv` is generated from it by `config/gen.py`.**
> **Almost all of this is a specification, not shipped behaviour.** The built set is 17 instructions
> and 45 passing / 0 failing. The only parameters actually settable in the built program are
> **`maturity_blocks`** (a stored `Config` field, shipped at **0**) and the authority timelock
> (`TIMELOCK_SLOTS = 32`, a constant). **`FLOOR` is not a parameter in code** — it is the constant
> `MIN_CONFIRMATIONS = 12`. Nothing else here is settable at runtime.

## How to read the value column

| Marker | Meaning |
|---|---|
| **mea** | **Measured.** From the code, the chain or the toolchain. Do not change without re-measuring |
| **dec** | **Decided.** A settled design choice |
| **ph** | **Placeholder.** A starting value expected to move |
| **der** | **Derived.** Computed from other parameters; changing it directly is a bug |
| **built** | Present in the program today |
| **der·superseded** | Derived, and no longer binding; recorded for history |
| **open** | **Undecided.** No value yet — this is work to do |

---

## The sheet — where every value actually lives

**This is the distinction that matters, and the one we have repeatedly blurred:** *"the parameters
document says X"* and *"the program can be set to X"* are different claims.

| Class | Meaning | Count |
|---|---|---|
| **`built-mutable`** | In the program, in an account, changeable at runtime through the timelocked authority | **1** |
| **`built-shape`** | The shape is enforced by the built program; the keys behind it are not | **1** |
| **`built-frozen`** | In the program as a constant. Changing it needs a **redeploy** | **15** |
| **`measured`** | A fact about BSV, Solana or the toolchain, not a choice | — |
| **`derived`** | Computed from others; changing it directly is a bug | — |
| **`designed`** | Decided, and **not in the program** | — |
| **`open`** | **No value yet.** This is work to do, not a decision deferred | — |
| **`superseded`** | Recorded for history only | — |

> ### The single most important row
>
> **`v.maturity_blocks` is the only parameter the built program can change at runtime.** It ships at
> **0**, which removes the protective window: the vault is a pass-through, the burn predicate is
> satisfied instantly rather than being unreachable, and release and burn are a **race** that release
> normally wins. **What protects a deposit today is `MIN_CONFIRMATIONS = 12` — prevention, not
> reversal.**
>
> Everything else marked `designed` above is a specification, and **17 instructions exist**, not 56
> parameters.

**A machine-readable copy is at [`parameters.csv`](parameters.csv)**, regenerated from this table.

---

## Light client

| ID | Name | Value | Description |
|---|---|---|---|
| `lc.window_hours` | Window length (hours) | **32** `dec` | How much BSV history the client keeps. Sets the deposit deadline |
| `lc.window` | Window length (records) | **192** `der` | `window_hours × 3600 / seconds_per_block`. Bounded above by the account cap |
| `lc.seconds_per_block` | Target block spacing | **600** `mea` | BSV's ten-minute target |
| `lc.record_size` | Bytes per header record | **52** `der` | `32 hash + 16 chainwork + 4 time`. Not configurable |
| `lc.lookback` | DAA lookback (records) | **147** `der` | `144 + 3` for the median-of-three. **Measured: 146 reproduces 0/324 headers, 147 reproduces 324/324** |
| `lc.daa` | Difficulty rule | **cw-144** `mea` | The rule BSV actually uses. Verified against real mainnet headers. **Hard-coded (X3)** |
| `lc.daa_clamp_low` | Lower clamp | **72 × 600 s** `mea` | Multiplies `seconds_per_block`, **not** the averaging window |
| `lc.daa_clamp_high` | Upper clamp | **288 × 600 s** `mea` | As above |
| `lc.floor` | Confirmation depth | **12 blocks** `built` | `MIN_CONFIRMATIONS`, **fixed in code**, not a runtime parameter |
| `lc.max_fork_batch` | Headers per fork tx | **12** `mea` | Set by the 1,232-byte transaction limit |
| `lc.max_account_create` | Solana account cap | **10,240 bytes** `mea` | A Solana limit, not a choice |
| `lc.cluster_id` | Deployment identifier | **open** | Binds a deposit to one deployment (P11). Must differ per cluster. **Not built** — the code checks only that the recipient's 32 bytes appear in the `OP_RETURN` |
| `lc.pow_limit_bits` | Maximum target | **`0x1d00ffff` / `0x207fffff`** `built` | In code as `MAINNET_POW_LIMIT_BITS`, with `no_retargeting` derived from the regtest value |
| `lc.max_staleness_slots` | Freshness bound | **54,000 slots (~6 h)** `built` | How recent the last accepted header must be before release. A multiple of BSV block time — ~1,500 Solana slots per block. **Implemented (`StaleClient`), untested on a local validator** |

## Vault

| ID | Name | Value | Description |
|---|---|---|---|
| `v.maturity_blocks` | Maturity | **0** `built` | Staged mints wait this long before release. **A stored parameter, not a constant**, so governance can raise it without a redeploy. At 0 the vault is a pass-through and release/burn are a **race**. The designed value is **144 blocks** |
| `v.reorg_margin` | Detection margin | **48 blocks** at the *designed* values `der` | `window − maturity`. **Do not add `floor` into this subtraction.** Zero at the maximum committed depth |
| `v.escrow_close_refund` | Rent refund on close | **true** `dec` | Closing an item returns its rent to the caller |

## Federation

| ID | Name | Value | Description |
|---|---|---|---|
| `fed.bond_mint` | Mint-side bond — BSV side (**the float**) | **1,000 BSV** `dec` | Held **outside the reserve**. **The bond is the float** — working capital for transfers — **not a capital requirement sized against the reserve**, and not a capacity ceiling. The `k × (BSV held)` line is a solvency check on mint and exit. **Held under the collective (threshold ECDSA) key, not the member's own**, and seized by the **members collectively**; slashing pays the slashers from it. A collective action by the majority, not an automatic rule |
| `fed.bond_key` | Mint-side bond custody | **collective (threshold ECDSA) key** `dec` | **The design requirement the mechanism rests on.** If a member controls their own bond they move it the moment they are caught. *Residuals:* a majority could seize an honest member's bond; nothing on BSV compels signing, so the duty to slash is **social** |
| `fed.bond_redeem` | Redeem-side bond — `solBSV` side (**the float**) | **1,000 BSV** `dec` | Seizable on Solana. **This side is enforceable.** Neither bond sits inside the reserve |
| `fed.bond_size` | Nominal bond size | **1,000 BSV** `dec` | Superseded by the two side-specific bonds; kept as the per-side default name |
| `fed.total_bond` | Aggregate bond (tracked) | **superseded** | The old single running total is replaced by two side-specific aggregates |
| `fed.mint_gate` | Bonds gate minting | **true** `dec` | **F5.** A mint is refused unless the BSV-side bond covers the BSV held after it. A **solvency floor, not a capacity claim.** The old `total_bond ≥ k × (non_bonded_supply + amount)` is **superseded** |
| `fed.bond_asset` | Bond denomination | **split** `dec` | Mint side BSV, redeem side `solBSV`; neither inside the reserve. No price oracle is needed for either |
| `fed.collusion_mitigation` | Against a colluding threshold | **transparency** `dec` | **Stated risk, not a mechanism.** No on-chain predicate proves who signed an off-chain threshold signature. The maximum loss is the whole **non-member supply**. Publishing the reserve and supply converts a hidden theft into a visible one; the unchallenged-spend case is separately addressed by the **reported spent-outpoint record** |
| `fed.k` | Bond coverage floor | **1** `dec` | `bsv_bond ≥ k × (BSV held)` and `solbsv_bond ≥ k × (solBSV held)`, checked on mint and exit. **A solvency check, not a capacity ceiling.** The capacity arithmetic built on `k` and `n/t` is **withdrawn**. Capital inefficiency accepted |
| `fed.threshold` | Gateway signing threshold | **4-of-N, `N` open** | `t` of `n` for the gateway **threshold ECDSA** key. **`N` is a variable; the number is arbitrary and deferred.** The key emits **one** signature and is one leg of the reserve's 2-of-2 script. Changing `t` or `n` is a **re-sharing**, not a migration |
| `fed.greycore_size` | Greycore size | **open** | The second signer set on the reserve script: **trusted third parties, not node operators** — people with reputations to lose, who do not run the reserve. The Greycore **finds and admits replacement members** |
| `fed.greycore_threshold` | Greycore threshold | **open** | How many Greycore keys must sign alongside the gateway. The Greycore **co-signs every reserve spend**, which is what constrains the reserve. 4-of-5 was an earlier proposal and is not settled |
| `fed.shards` | Shard count | **open** | One key or several groups. Affects blast radius and latency. **There is no sharding today: blast radius is 100%** |
| `fed.unbond_slots` | Unbonding period | **open** | Must exceed the redemption deadline plus the challenge window |
| `fed.delegated_staking` | Delegated staking | **disabled** `dec` | **Phase 2.** Lets non-members delegate `solBSV` to a member and share its fee. Activatable by governance, not built |
| `fed.script` | Reserve deposit script | **2-of-2 `OP_CHECKMULTISIG`** `dec`, **shape accepted in code** `built` | `OP_2 <gateway threshold key> <greycore key> OP_2 OP_CHECKMULTISIG`. Both keys must sign; the Greycore polices every reserve spend. The committed code accepts this shape: `is_reserve_multisig`, `MAX_SCRIPT_LEN = 71`, `DepositScript::SPACE = 84`. **The keys/quorum behind it are not built** |
| `fed.bond_enforced_on_chain` | BSV-side bond seizure | **collective action** `dec` | The Solana program cannot seize the BSV-side bond; the members sign a threshold transaction. Stated as a residual, not hidden |
| `fed.spent_report` | Spent-outpoint record (N5) | **one PDA per `(txid, vout)`, written by the program authority** `built` | `report_spent` records a deposit outpoint the reserve has spent; `verify_deposit` refuses a deposit whose outpoint is in the record. The signer is the **upgrade authority** — a **PoC stand-in** for the federation, which does not exist yet. Permanent and rent-exempt with **no prune**: closing a record would re-open the mint of an already-spent deposit |

## Governance

| ID | Name | Value | Description |
|---|---|---|---|
| `gov.threshold` | Pass threshold | **85%** `ph` | Share of pledged coins required |
| `gov.delay` | Delay before effect | **30 days** `ph` | **Reducible** by governance, but never below `gov.delay_min`. Redemptions run throughout |
| `gov.delay_min` | Minimum delay (**floor**) | **open** (7 days proposed) | **F7.** Both readings are defensible and the choice is open: *with a floor*, a majority cannot take the warning away — proposal 1 can only shorten the delay to the floor, so proposal 2 still has to be exited during it; *without*, the exit window is whatever the current majority allows. A judgement about trust, **not a correctness question** |
| `gov.signal` | Signal from proposal | **true** `dec` | Live from the moment it is raised |
| `gov.holds_upgrade_authority` | Governance owns the upgrade key | **true** `dec` | Deliberate. Nothing is immutable, so the exit window, not the rule, is the protection |
| `gov.authority_threshold` | Checkpoint/pause authority | **federation threshold** `designed` — in code it is the **upgrade authority**, one key, timelocked | **F4.** Replaces the single deployer key. No timelock-free path to rewriting the checkpoint. **Not built** |
| `gov.authority_timelock` | Authority timelock | **32 slots** `built` | `TIMELOCK_SLOTS`, a PoC value. Applied to `propose_authority_change` / `execute_authority_change` / `cancel_authority_change`. Must be long enough to exit |
| `gov.pause_threshold` | Pause threshold | **>50%** `ph` | Lower than a governance change, because the power is bounded |
| `gov.pause_duration` | Pause auto-lift | **open** | Days before a pause lapses unless renewed |

## Fees

| ID | Name | Value | Description |
|---|---|---|---|
| `fee.mint_bp` | Peg-in fee (**gross**) | **30 bp** `dec` | Covers all transaction fees; the remainder is member income. **Its physical location is unspecified — N1** |
| `fee.redeem_bp` | Peg-out fee (**gross**) | **30 bp** `dec` | As above |
| `fee.bounty_share` | Challenger bounty | **open** | Share of a slashed bond paid to whoever proves the misbehaviour. The `burn_staged` bounty is suggested, not sized, and **not built** |

## Peg-in

| ID | Name | Value | Description |
|---|---|---|---|
| `pi.min_peg_in` | Minimum deposit | **1 BSV** `dec` | Prices out dust griefing. Decided; **enforcement is not in code** |
| `pi.max_used` | Legacy replay-list cap | **200** `der·superseded` | Replaced by a **nullifier PDA per deposit**, which is **built**. There is no list and no ceiling |
| `pi.op_return_layout` | Deposit commitment | **version ‖ cluster_id ‖ program_hash ‖ flags ‖ recipient** `dec` | **Designed.** What is built checks only that the recipient's 32 bytes appear in an `OP_RETURN` |

## Peg-out

| ID | Name | Value | Description |
|---|---|---|---|
| `po.deadline` | Redemption deadline | **open** | `D`. Must satisfy `D ≥ payout_confirmations + challenge_window` |
| `po.challenge_window` | Challenge window | **open** | `W`. Confirmations a payout needs before settling |
| `po.payout_confirmations` | Payout depth | **open** | BSV confirmations before a payout is provable. `C_payout` |

## Measured constants — do not change without re-measuring

| ID | Name | Value | Description |
|---|---|---|---|
| `m.rent_per_byte` | Solana rent | **5,080 lamports/byte** `mea` | `(bytes + 128) × 5,080`. **The 128-byte overhead is inside this rate.** 6,960 is an older toolchain's constant and overstates by ~37%. Measured: `solana rent 0` = 650,240 lamports; `solana rent 165` = 1,488,440 |
| `m.signature_fee` | Solana base fee | **5,000 lamports** `mea` | Per signature |
| `m.token_decimals` | `solBSV` decimals | **8** `mea` | — |
| `m.blob_max` | Transaction size | **1,232 bytes** `mea` | Solana limit; drives `lc.max_fork_batch` |

---

## The parameters that are arithmetic, not choices

Two numbers cannot be governed into being different. They are measured facts about the built client:

```
WINDOW              = 192 records  = 32 hours at 600 s/block
HEADER_RECORD_SIZE  =  52 bytes    (32 hash + 16 chainwork + 4 time)
LIGHT_CLIENT_FIXED  = 123 bytes
SPACE               = 123 + 192 × 52 = 10,107   of 10,240  ->  133 bytes margin
LOOKBACK            = 147 records  (144 + 3 for cw-144's median-of-three)
```

- **52 bytes a record, not 32.** cw-144 subtracts two cumulative chainworks and two timestamps 144
  blocks apart, so a record must carry hash, chainwork and time. A bare-hash window cannot verify the
  difficulty rule at all.
- **147 records of lookback**, so 192 is only 45 records of slack above the minimum.
- **194 is the arithmetic maximum** (10,211 bytes, 29 bytes margin); **192 is the largest window with
  real margin.** A 48-hour window would be 288 × 52 = 14,976 bytes plus overhead, 46% over the cap,
  and `initialize` would simply revert. **The old 48-hour deposit deadline was never achievable.**

**Consequence:** the deposit lifetime is **32 hours**. A deposit whose block has left the window can
never be proven again.

## How the safety parameters actually protect

The old model's answer was "safety parameters may only be moved in the conservative direction". The
current answer is different:

**There is no immutable floor. The floor is the exit window.**

A change needs **85% of pledged coins** and takes **30 days**, signalled **live from the moment it is
raised** — and **redemptions run throughout, because they can never be paused.** So a proposal that
would harm holders does not trap them: it **empties the bridge before it lands.**

**A safety parameter bounds a fraud; an economic parameter sets price and size.** The distinction is
load-bearing: whoever can set `FLOOR = 0` or `WINDOW = 0` holds a mint voucher, and whoever can pause
redemptions holds the reserve hostage. That is why **redemptions are outside governance's reach
entirely** rather than merely subject to a supermajority.

**The residual, stated plainly:** a holder who does not watch and does not act within the delay is
exposed. That is a disclosure obligation, not a mechanism.

**Test overrides are compile-time, not config.** "Remove the minimum during testing" must be a
`#[cfg(feature = …)]`, never a runtime value — a runtime value can leak to mainnet, a compile-time one
cannot.

## Security parameters, and the one that is a market

Depth is a protocol parameter, not a market term (the order book is removed):

- **`FLOOR` = 12 blocks** is the minimum. It makes a reorg cost real mining work; a low floor makes
  attacks cheap and frequent, raising the number of chances for a detection failure to slip through.
- **Maturity = 144 blocks designed** sets the time available to detect. A staged mint is released only
  once the tip has advanced past the deposit **and** the hash the client stores at that height still
  matches. If it differs, the deposit was reorged and the staged tokens **burn**. **Shipped at 0.**
- **`WINDOW` = 32 hours** bounds the reorg the client can see at all. Beyond it, a reorg is not
  detectable *by definition*.

**The honest limit:** the reserve is off-chain BSV the program cannot read, so `custodied BSV ≥
outstanding solBSV` is **tracked, not verified**. What the program *can* compare is each bond against
what its side holds, since those are quantities it holds or measures, and neither bond sits inside the
reserve.

**One calibration warning, load-bearing:** BSV's hashpower is not Bitcoin's. Six confirmations do not
carry the same meaning here, so `FLOOR` must be calibrated to **BSV**, not inherited. And depth must
move with the value it secures — a depth adequate for 10 BSV is not adequate for 10,000. Governance is
what lets the value move without shipping a program.

## What was specified before, and is now superseded

An earlier chapter carried a stable of IDs under a two-gate, per-relayer, order-book model. The table
records what became of each rather than rewriting it silently.

| Old ID | Old content | Now |
|---|---|---|
| **P1** | `FLOOR` = 12 BSV blocks, fixed in code for the PoC | **Kept as the default**, but a **governable parameter** in the design — except the built code still has `MIN_CONFIRMATIONS = 12` and no setter |
| **P2** | `C_payout` = 12 payout confirmations | **Superseded.** Payouts settle against the light client after a challenge window; the number is not specified |
| **P3** | Redemption deadline = 6 h of Solana slots | **Superseded.** The deadline is set per redemption; the default and minimum are unspecified |
| **P4** | Challenge window = 24 h | **Superseded.** No default specified |
| **P5** | `RECENT_REORG_WINDOW` = 12 h | **Superseded.** The vault compares stored hashes; there is no separate window parameter |
| **P6** | `TIP_STALENESS` = 2 h | **Superseded** as a parameter by `lc.max_staleness_slots`; the pause is the built authority-gated safety valve |
| **P7** | `MIN_PEG_IN` = 10 BSV | **Superseded** — now 1 BSV, decided, not enforced |
| **P8** | `MAX_PEG_IN` = 10,000 BSV | **Superseded.** The earlier "total value locked is capped by total bonds pledged" is **withdrawn** |
| **P9** | `MAX_PEG_OUT` = 10,000 BSV | **Superseded.** A redemption is bounded by the reserve and the payout path |
| **P10** | `HOT_FLOAT_CAP` | **Removed** with the pooled float. There is one reserve, and the two-sided bonds are the bound |
| **D4** | `FLOOR` fixed in code | **Superseded.** The default is 12; the mechanism is governance |
| **D7** | No governance in the PoC | **Superseded.** 85% / 30 days / live signal, holding the upgrade authority |
| **D8** | Reserve invariant monitored, not enforced | **Kept**, and it is the honest residual |

**Two of the old reasons survive verbatim, because they were right:**

- **A stablecoin bond is a written call option on the reserve** (see
  [05. Trust model](05-trust-model.md#why-the-bonds-are-denominated-in-what-they-protect-not-a-stablecoin)).
  Denominate each bond in the asset its side holds: that removes the position instead of hedging it.
- **A change must be disclosed before someone transacts.** A holder should be able to see the fee,
  `FLOOR`, maturity and the bond before they peg in.

**One old reason does not survive:** "loosening a safety parameter should require shipping a new
program." That was written for a design with no governance. The protection is not that the rules are
frozen — it is that you can leave before they change.

## Open parameters, named so they are not lost

**Load-bearing first:**

1. **`fed.threshold` — 4-of-N, with `N` deferred.** It is one leg of the reserve's 2-of-2 script, so
   every "no single member can move funds" claim depends on it. It must be fixed before launch.
2. **`fed.greycore_size` / `fed.greycore_threshold`** — the second signer set, both `open`. The
   Greycore co-signature is what constrains the reserve, so these are load-bearing in the same way.
3. **Leaver-shares** — not a parameter but an unresolved mechanism; the effective threshold degrades
   with churn.
4. **`po.deadline` / `po.challenge_window` / `po.payout_confirmations`** — three values that are one
   security parameter: the window in which a theft can be proven.
5. **`gov.delay_min`** — the exit floor; it decides whether a majority can shorten the warning.
6. **`gov.authority_timelock`** — load-bearing for F4 in the same way `po.deadline` is for redemptions.

The remaining placeholders — `lc.cluster_id`, `lc.pow_limit_bits`, `fed.shards`, `fed.unbond_slots`,
`gov.pause_duration`, `fee.bounty_share` — can be filled once the structure is audited.

**Three things are open in the built code, and are not governance questions:**

1. **The program upgrade authority can override every parameter.** It is the design's answer to put it
   under governance; until that exists, it is a live critical.
2. **X3 — the DAA is hard-coded.** A BSV consensus change halts the bridge until a redeploy.
3. **The vault's protective window ships at 0**, so the reversal is available and racy rather than
   automatic.

## Change checklist

Any parameter change, once a mechanism exists, should be:

1. **Proposed publicly**, with the reasoning and the new arithmetic.
2. **Signalled live from the moment it is raised**, so the 30 days are visible time rather than a
   surprise.
3. **Effective only after the delay**, with redemptions open throughout — **they are never pausable**,
   and that is the one guarantee the design does not trade away.
4. **Published** in a changelog with the effective date.
5. **Incapable of moving funds.** The bond and the vault are program-owned; parameters are not a path
   to the reserve.

**No process to execute any of this exists yet.** The checklist is the shape the governance design has
to satisfy, not a description of a mechanism that runs.

---

## The revenue model

**The fee is gross, and it is the only revenue.** A 30 bp charge on mint and on redeem covers the real
transaction costs, and **whatever is left is the income of the bonded members**, shared pro rata to
stake.

At the **1 BSV minimum** this is a useful sanity check rather than a projection:

| Per mint (1 BSV ≈ $30) | |
|---|---|
| Fee collected at 30 bp | **$0.090** |
| Solana transaction fees (mint + its share of header pushes) | ~$0.001 |
| BSV relay at 1 sat/byte | ~$0.00007 |
| First-time ATA rent | $0.115 — **refundable, a lockup not a cost** |
| **Non-refundable cost** | **~$0.001** |

**The fee covers the non-refundable cost by roughly two orders of magnitude at the minimum deposit**,
and the margin is the members'. The ATA rent is the one figure that exceeds the fee, and it is
recoverable by closing the account.

**This also prices the risk.** A member's return is the fee share against two-sided 1,000 BSV bonds —
the float, which also stands to be lost if the member misbehaves. The market for members is the market
for that trade.

## The bond is the float, deliberately

**The bond is working capital, not a capital requirement.** At `k = 1` each two-sided bond must sit at
a coverage floor of at least what its side holds, but that is a **solvency check, not a capacity
ceiling**, and nothing forces the bonds to track the reserve as it grows. This replaces the
**superseded** single-bond formula `aggregate_bond ≥ k × non_bonded_supply`.

**The earlier capacity claim is withdrawn.** Sizing the bond against the vault produced an impossible
inequality twice (`B ≥ B + H`, then the `k = 1` versus `3×` contradiction), and the `n/t` multiple is
RenVM's own bribery-cost calculation, which does not apply to a bond that is the float. The `~$180k`
and `$300k` capacity figures are both **withdrawn**. **What constrains the reserve is the Greycore
co-signature, not the bond.**

**Phase 2: delegated staking.** Non-members delegating `solBSV` to a member and sharing its fee,
activatable by governance. It would let the bond base grow without new operators, at the cost of
introducing a staking layer with its own incentive problems. Deliberately deferred: the mechanism
should not be designed until the thing it is meant to scale has been shown to work.
