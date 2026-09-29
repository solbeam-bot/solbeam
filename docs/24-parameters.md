# 24. Parameters and variables

**Every value the system turns on, in one place.** This is the reference file: each row has an
**ID**, a **name**, a **value**, and a **description**. It is intended to become a **JSON config**
that both the on-chain program and the node software read, so that a specification can be correct
while the numbers are still provisional.

> **Status: a stub, deliberately.** The *structure* is settled — which variables exist and what each
> controls. Most *values* are placeholders carried over from earlier decisions, marked below. The
> point of this file is that a wrong value is a one-line change rather than a scattered rewrite.

## How to read the value column

| Marker | Meaning |
|---|---|
| **mea** | **Measured.** Established from the code, the chain or the toolchain. Do not change without re-measuring |
| **dec** | **Decided.** A settled design choice |
| **ph** | **Placeholder.** A starting value that is expected to move |
| **der** | **Derived.** Computed from other parameters; changing it directly is a bug |
| **der·superseded** | **Derived, and no longer binding.** Computed from other parameters but replaced by a later decision. One marker, recorded for history |
| **open** | **Undecided.** No value yet — this is work to do |

---

## Light client

| ID | Name | Value | Description |
|---|---|---|---|
| `lc.window_hours` | Window length (hours) | **32** `dec` | How much BSV history the client keeps. Sets the deposit deadline |
| `lc.window` | Window length (records) | **192** `der` | `window_hours × 3600 / seconds_per_block`. Bounded above by the account cap |
| `lc.seconds_per_block` | Target block spacing | **600** `mea` | BSV's ten-minute target |
| `lc.record_size` | Bytes per header record | **52** `der` | `32 hash + 16 chainwork + 4 time`. Not configurable |
| `lc.lookback` | DAA lookback (records) | **147** `der` | `144 + 3` for the median-of-three. **Measured: 146 reproduces 0/324 headers, 147 reproduces 324/324** |
| `lc.daa` | Difficulty rule | **cw-144** `mea` | The rule BSV actually uses. Verified against real mainnet headers |
| `lc.daa_clamp_low` | Lower clamp | **72 × 600 s** `mea` | Multiplies `seconds_per_block`, **not** the averaging window |
| `lc.daa_clamp_high` | Upper clamp | **288 × 600 s** `mea` | As above |
| `lc.floor` | Confirmation depth | **12 blocks** `ph` | Confirmations before a deposit may be proven |
| `lc.max_fork_batch` | Headers per fork tx | **12** `mea` | Set by the 1,232-byte transaction limit |
| `lc.max_account_create` | Solana account cap | **10,240 bytes** `mea` | A Solana limit, not a choice |
| `lc.cluster_id` | Deployment identifier | **open** | Binds a deposit to one deployment (P11). Must differ per cluster |
| `lc.pow_limit_bits` | Maximum target | **open** | Regtest uses `0x207fffff`; mainnet must be set |
| `lc.max_staleness_slots` | Freshness bound | **open** | How recent the last accepted header must be before release. The successor to "tip advanced" |

## Vault

| ID | Name | Value | Description |
|---|---|---|---|
| `v.maturity_blocks` | Maturity | **144 blocks** `ph` | Staged mints wait this long before release. Must satisfy `floor + maturity ≤ window` |
| `v.reorg_margin` | Detection margin | **48 blocks** `der` | `window − maturity`. **Do not add `floor` into this subtraction.** Zero at the maximum committed depth |
| `v.escrow_close_refund` | Rent refund on close | **true** `dec` | Closing an item returns its rent to the caller |

## Federation

| ID | Name | Value | Description |
|---|---|---|---|
| `fed.bond_mint` | Mint-side bond — BSV side (**the float**) | **1,000 BSV** `dec` | The **BSV-side bond**, held **outside the reserve**. **The bond is the float** — working capital for transfers, the float that lets the federation serve redemptions — **not a capital requirement sized against the reserve**, and not a capacity ceiling. The `k × (BSV held)` coverage floor is a solvency check on mint and exit, **not** the sizing rule. **Held under the collective (threshold ECDSA) key, not the member's own**, and seized by the **members collectively** with a threshold-signed transaction; slashing pays the slashers from it. A **collective action by the majority**, not an automatic rule |
| `fed.bond_key` | Mint-side bond custody | **collective (threshold ECDSA) key** `dec` | **The design requirement the mechanism rests on.** Each member's BSV-side bond must sit under the **collective** key, **not the member's own**. If a member controls their own bond they move it the moment they are caught, or before they act, and there is nothing to slash. The same key primitive as the reserve, pointed at the bond. *Residuals:* a majority could seize an honest member's bond; nothing on BSV compels the members to sign, so the duty to slash is **social** and rests on the majority being honest, on visibility, and on the bounty |
| `fed.bond_redeem` | Redeem-side bond — `solBSV` side (**the float**) | **1,000 BSV** `dec` | The **`solBSV`-side bond**, seizable on Solana. **The bond is the float** — working capital for transfers — **not a capital requirement sized against the reserve**, and not a capacity ceiling; the `k × (solBSV held)` line is a coverage floor, not the sizing rule. **This side is enforceable.** Neither bond sits inside the reserve |
| `fed.bond_size` | Nominal bond size | **1,000 BSV** `dec` | Superseded by the two side-specific bonds above; kept as the name of the per-side default |
| `fed.total_bond` | Aggregate bond (tracked) | **superseded** | The old single running total is replaced by two side-specific aggregates. History: the mint gate used to check it |
| `fed.mint_gate` | Bonds gate minting | **true** `dec` | **F5.** A mint is refused unless the **BSV-side** bond covers the BSV held after it. **The bonds ARE the float.** This is a **solvency floor, not a capacity claim**: the bond is the float (`fed.bond_mint`) and the reserve is constrained by the **Greycore co-signature** (`fed.greycore_size` / `fed.greycore_threshold`), not by bond size. The old single check `total_bond ≥ k × (non_bonded_supply + amount)` is **superseded**: bonded `solBSV` was itself supply, so `B ≥ k × supply` demands `B ≥ B + H`; the BSV-side bond is not `solBSV` at all, which is what dissolves that |
| `fed.bond_asset` | Bond denomination | **split** `dec` | **Superseded by `fed.bond_mint` / `fed.bond_redeem`.** The mint side is BSV, the redeem side `solBSV`; neither sits inside the reserve. No price oracle is needed for either |
| `fed.collusion_mitigation` | Against a colluding threshold | **transparency** `dec` | **Stated risk, not a mechanism.** No on-chain predicate can prove who signed an off-chain threshold signature. The maximum loss is the whole **non-member supply**. Publishing the reserve and supply continuously converts a hidden theft into a visible one; **visibility is the only remaining defence** against collusion (doc 07). The unchallenged-spend case is separate and is addressed by the **federation-reported spent-outpoint record** the program checks mints against (doc 21, N5) |
| `fed.k` | Bond coverage floor | **1** `dec` | The **coverage floor**: each side must sit at `bsv_bond ≥ k × (BSV held)` and `solbsv_bond ≥ k × (solBSV held)`, checked on mint and on exit. **This is a solvency check, not a capacity ceiling.** The bond is the float (`fed.bond_mint`), and the reserve is constrained by the Greycore co-signature, not by `k`; the capacity arithmetic built on `k` and `n/t` is **withdrawn** (doc 26 §5). **Capital inefficiency accepted** — over-collateralising is a member's choice, not a requirement |
| `fed.threshold` | Gateway signing threshold | **4-of-N, `N` open** `open` | `t` of `n` for the gateway **threshold ECDSA** key. **`N` is a variable; the number is arbitrary and deferred.** The gateway key emits **one** signature and is one leg of the reserve's 2-of-2 script (`fed.script`). Changing the gateway's `t` or `n` is a **re-sharing**, not an on-chain migration. **It no longer implies a P2PKH reserve** — see `fed.script`, and audit F10 is **reversed** |
| `fed.greycore_size` | Greycore size | **open** | The second signer set on the reserve script: **trusted third parties, not node operators.** RenVM's own words are *"Darknodes that have developed reputations with the community"* — chosen by governance, with a stake in the system's safety; **people with reputations to lose, who do not run the reserve.** The Greycore **finds and admits replacement members** |
| `fed.greycore_threshold` | Greycore threshold | **open** | How many Greycore keys must sign alongside the gateway. The Greycore **co-signs every reserve spend**, which is what constrains the reserve. The reference shape is a supermajority; **4-of-5 was the earlier proposal and is not settled** |
| `fed.shards` | Shard count | **open** | One key or several groups. Affects blast radius and latency |
| `fed.unbond_slots` | Unbonding period | **open** | Must exceed the redemption deadline plus the challenge window |
| `fed.delegated_staking` | Delegated staking | **disabled** `dec` | **Phase 2.** Lets non-members delegate `solBSV` to a member and share its fee. **Activatable by governance**, not built |
| `fed.script` | Reserve deposit script | **2-of-2 `OP_CHECKMULTISIG`** `dec` | `OP_2 <gateway threshold key> <greycore key> OP_2 OP_CHECKMULTISIG`. **Both keys must sign**, so the gateway majority cannot move funds alone and the Greycore cannot move funds alone; **the Greycore polices every reserve spend**. **This reverses F10:** with a Greycore the script genuinely **is** a multisig, so `is_p2pkh` **must change** and `DepositScript::SPACE` **must grow** to ~71 bytes, against the current 38 |

## Governance

| ID | Name | Value | Description |
|---|---|---|---|
| `gov.threshold` | Pass threshold | **85%** `ph` | Share of pledged coins required |
| `gov.delay` | Delay before effect | **30 days** `ph` | Default delay. **Reducible** by governance, but never below `gov.delay_min`. Redemptions run throughout, so a hostile change that shortens the delay still has to be exited during it |
| `gov.delay_min` | Minimum delay (**floor**) | **open** (7 days proposed) | **F7.** `gov.delay` may be raised or lowered by governance. **Both readings are defensible and the choice is open:** *with a floor*, a majority cannot take the warning away — proposal 1 can only shorten the delay to the floor, so proposal 2 still has to be exited during it; *without*, the exit window is whatever the current majority allows. It is a judgement about how much the majority is trusted, **not a correctness question** |
| `gov.signal` | Signal from proposal | **true** `dec` | Live from the moment it is raised, not when it passes |
| `gov.holds_upgrade_authority` | Governance owns the upgrade key | **true** `dec` | Deliberate. Nothing is immutable — governance could rewrite even `gov.delay_min` — so the exit window, not the rule, is the protection |
| `gov.authority_threshold` | Checkpoint/pause authority | **federation threshold** `dec` | **F4.** Replaces the single deployer key. No timelock-free path to rewriting the checkpoint |
| `gov.authority_timelock` | Authority timelock | **32 slots** `ph` (**built**: `TIMELOCK_SLOTS`, a PoC value) | **F4.** Delay before a checkpoint or pause takes effect. Must be long enough to exit |
| `gov.pause_threshold` | Pause threshold | **>50%** `ph` | Lower than a governance change, because the power is bounded |
| `gov.pause_duration` | Pause auto-lift | **open** | Days before a pause lapses unless renewed |

## Fees

| ID | Name | Value | Description |
|---|---|---|---|
| `fee.mint_bp` | Peg-in fee (**gross**) | **30 bp** `dec` | **Covers all transaction fees; the remainder is member income.** Its physical location is unspecified — see N1 |
| `fee.redeem_bp` | Peg-out fee (**gross**) | **30 bp** `dec` | As above: costs first, remainder to bonded members pro rata |
| `fee.bounty_share` | Challenger bounty | **open** | Share of a slashed bond paid to whoever proves the misbehaviour |

## Peg-in

| ID | Name | Value | Description |
|---|---|---|---|
| `pi.min_peg_in` | Minimum deposit | **1 BSV** `dec` | Prices out the dust griefing |
| `pi.max_used` | Legacy replay-list cap | **200** `der·superseded` | **One marker: derived *and* superseded.** Replaced by a nullifier per deposit (decision P5). Until built it caps peg-ins at 200 per window |
| `pi.op_return_layout` | Deposit commitment | **version ‖ cluster_id ‖ program_hash ‖ flags ‖ recipient** `dec` | **Designed.** What is built checks only that the recipient's 32 bytes appear in an `OP_RETURN` |

## Peg-out

| ID | Name | Value | Description |
|---|---|---|---|
| `po.deadline` | Redemption deadline | **open** | `D`. Must satisfy `D ≥ payout_confirmations + challenge_window` |
| `po.challenge_window` | Challenge window | **open** | `W`. Confirmations a payout needs before settling |
| `po.payout_confirmations` | Payout depth | **open** | BSV confirmations before a payout is provable |

## Measured constants — do not change without re-measuring

| ID | Name | Value | Description |
|---|---|---|---|
| `m.rent_per_byte` | Solana rent | **5,080 lamports/byte** `mea` | `(bytes + 128) × 5,080`. **The 128-byte overhead is inside this rate.** 6,960 is an older toolchain's constant and overstates by ~37% |
| `m.signature_fee` | Solana base fee | **5,000 lamports** `mea` | Per signature |
| `m.token_decimals` | `solBSV` decimals | **8** `mea` | — |
| `m.blob_max` | Transaction size | **1,232 bytes** `mea` | Solana limit; drives `lc.max_fork_batch` |

---

## What is genuinely undecided

**The `open` values, load-bearing first:**

1. **`fed.threshold` — 4-of-N, with `N` a variable and the number deferred.** It is one leg of the
   reserve's 2-of-2 script, so it is the number every "no single member can move funds" claim depends
   on, and it must be fixed before launch.
2. **`fed.greycore_size` / `fed.greycore_threshold`** — the second signer set on the reserve script,
   both `open`. **The Greycore co-signature is what constrains the reserve**, so these are
   load-bearing in the same way the gateway threshold is.
3. **Leaver-shares** — not a parameter but an unresolved mechanism: a departing member retains a valid
   share, so the effective threshold degrades with churn. Key rotation or proactive re-sharing
   (doc 23, *Open — for finalisation: leaver-shares*).
4. **`po.deadline` / `po.challenge_window` / `po.payout_confirmations`** — three values that are one
   security parameter: the window in which a theft can be proven, which the earlier audits identified
   as the real one.
5. **`gov.delay_min`** — the exit floor, `open` with both readings stated in the row above. It decides
   whether a majority can shorten the warning; that is a trust judgement, not a correctness question.
6. **`gov.authority_timelock`** — load-bearing for F4 in the same way `po.deadline` is for redemptions.

The remaining placeholders — `lc.cluster_id`, `lc.pow_limit_bits`, `lc.max_staleness_slots`,
`fed.shards`, `fed.unbond_slots`, `gov.pause_duration`, `fee.bounty_share` — can be filled once the
structure is audited. **`fed.script` is settled as a shape but changed as a value:** the deposit
script is a **2-of-2 `OP_CHECKMULTISIG`**, so `is_p2pkh` must change and `DepositScript::SPACE` must
grow to ~71 bytes. **F10 is reversed, not closed as a documentation error.**

**A note on why this file exists:** several values here (`lc.window_hours`, `fed.bond_size`,
`gov.threshold`, `fee.mint_bp`) were argued about repeatedly during design. **Once they are one-line
config, arguing about the number stops blocking the specification.** That is the point.

---

## The revenue model

**The fee is gross.** A 30 bp charge on mint and on redeem covers the real transaction costs, and
**whatever is left is the income of the bonded members**, shared pro rata to stake. There is no
other revenue.

At the **1 BSV minimum** that is a useful sanity check rather than a projection:

| Per mint (1 BSV ≈ $30) | |
|---|---|
| Fee collected at 30 bp | **$0.090** |
| Solana transaction fees (mint + its share of header pushes) | ~$0.001 |
| BSV relay at 1 sat/byte | ~$0.00007 |
| First-time ATA rent | $0.115 — **refundable, a lockup not a cost** |
| **Non-refundable cost** | **~$0.001** |

**So the fee covers the non-refundable cost by roughly two orders of magnitude at the minimum
deposit**, and the margin is the members'. The ATA rent is the one figure that exceeds the fee, and
it is recoverable by closing the account — which is why it is a lockup rather than a loss, and why
it is the honest answer to "who pays for a first-time user".

**This also prices the risk.** A member's return is the fee share against two-sided 1,000 BSV
bonds — the **float**, which also stands to be lost if the member misbehaves. The market for members
is the market for that trade.

---

## The bond is the float, deliberately

**The bond is working capital, not a capital requirement.** At `k = 1` each **two-sided bond** must sit
at a coverage floor of at least what its side holds — `bsv_bond ≥ k × (BSV held)` and
`solbsv_bond ≥ k × (solBSV held)` — but that is a **solvency check on mint and exit, not a capacity
ceiling**, and nothing forces the bonds to *track* the reserve as it grows. **The bond's size is the
float** the federation needs to serve redemptions. This replaces the **superseded** single-bond
formula `aggregate_bond ≥ k × non_bonded_supply`.

**The earlier capacity claim is withdrawn.** The idea that *a bridge that caps its size at what its
members will bond cannot outrun its own collateral* is **withdrawn**: sizing the bond against the
vault produced an impossible inequality twice (`B ≥ B + H`, then the `k = 1` versus `3×`
contradiction), and the `n/t` multiple is RenVM's own bribery-cost calculation, which **does not apply
to a bond that is the float** (doc 26 §5). The `~$180k` and `$300k` capacity figures are both
**withdrawn**. **What constrains the reserve is the Greycore co-signature**, not the bond.

**Phase 2: delegated staking.** Non-members delegating `solBSV` to a member and sharing its fee,
**activatable by governance**. It would let the bond base grow without new operators, at the cost of
introducing a staking layer with its own incentive problems. Deferred deliberately: the mechanism
should not be designed until the thing it is meant to scale has been shown to work.
