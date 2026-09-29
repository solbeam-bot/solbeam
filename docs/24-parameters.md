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
| `fed.bond_size` | Member bond | **1,000 BSV** `dec` | The price of admission. Sets the scale limit |
| `fed.total_bond` | Aggregate bond (tracked) | **derived from bonds** `dec` | A running total on-chain, since members cannot be enumerated. **This is the number the mint gate checks** |
| `fed.mint_gate` | Bond gates minting | **true** `dec` | **F5.** A mint is refused unless `total_bond ≥ k × (non_bonded_supply + amount)` after it. **The bond IS the float.** Note **non-bonded**: using total supply makes the gate unsatisfiable, since bonded `solBSV` is itself supply |
| `fed.bond_asset` | Bond denomination | **`solBSV`** `dec` | Deliberately the wrapped asset. A separate asset would need a price oracle to size the cover. **Consequence: the bond does not protect against collusion** — max loss is the full non-bonded supply |
| `fed.collusion_mitigation` | Against a colluding threshold | **none** `dec` | **Stated risk, not a mechanism.** No on-chain predicate can prove who signed an off-chain threshold signature. Accepted; revisit if one is ever needed |
| `fed.k` | Bond multiple | **1** `dec` | Aggregate bond ≥ `k ×` outstanding supply. At `k = 1` theft is unprofitable, not prevented. **Capital inefficiency accepted** — over-collateralising is a member's choice, not a requirement |
| `fed.threshold` | Signing threshold | **open** | `t` of `n`. **Stated nowhere yet** — this is a real gap |
| `fed.shards` | Shard count | **open** | One key or several groups. Affects blast radius and latency |
| `fed.unbond_slots` | Unbonding period | **open** | Must exceed the redemption deadline plus the challenge window |
| `fed.delegated_staking` | Delegated staking | **disabled** `dec` | **Phase 2.** Lets non-members delegate `solBSV` to a member and share its fee. **Activatable by governance**, not built |
| `fed.script` | Deposit script | **open** | The reserve pool address deposits pay |

## Governance

| ID | Name | Value | Description |
|---|---|---|---|
| `gov.threshold` | Pass threshold | **85%** `ph` | Share of pledged coins required |
| `gov.delay` | Delay before effect | **30 days** `ph` | **This is the floor.** Redemptions run throughout, so a hostile change empties the bridge |
| `gov.signal` | Signal from proposal | **true** `dec` | Live from the moment it is raised, not when it passes |
| `gov.holds_upgrade_authority` | Governance owns the upgrade key | **true** `dec` | Deliberate. There is no immutable floor |
| `gov.authority_threshold` | Checkpoint/pause authority | **federation threshold** `dec` | **F4.** Replaces the single deployer key. No timelock-free path to rewriting the checkpoint |
| `gov.authority_timelock` | Authority timelock | **open** | **F4.** Delay before a checkpoint or pause takes effect. Must be long enough to exit |
| `gov.delay_ratchet` | Delay is non-decreasing | **true** `dec` | **F7.** `gov.delay` may only be raised. A first proposal setting it to zero harms nobody, so nobody exits, and the second then lands instantly |
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
| `pi.max_used` | Legacy replay-list cap | **200** `der` `superseded` | Replaced by a nullifier per deposit (decision P5). Until built it caps peg-ins at 200 per window |
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

**Seven values are `open`, and two of them are load-bearing:**

1. **`fed.threshold`** — `t` of `n`. The whole security model rests on it and **it is stated nowhere.**
2. **`po.deadline` / `po.challenge_window`** — these set the window in which a theft can be proven, which is the security parameter the earlier audits identified as the real one.

The rest — `lc.cluster_id`, `fed.unbond_slots`, `fed.script`, `gov.pause_duration`, `fee.bounty_share`
— are placeholders that can be filled once the structure is audited.

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

**This also prices the risk.** A member's return is the fee share against a 1,000 BSV bond whose
entire purpose is to be lost if it misbehaves. The market for members is the market for that trade.

---

## Capital efficiency, deliberately

**Capital inefficiency is accepted, and tying up assets is the point.** At `k = 1` the aggregate bond
must be at least the outstanding supply, and a member may over-collateralise if it wishes — but
nothing forces the bond to *track* the reserve as it grows.

That is a choice, not an oversight. Making the bond scale with the reserve would mean either forcing
members to post more capital continuously, or throttling deposits to keep the ratio — both of which
cost more in complexity and liveness than they buy. **A bridge that caps its size at what its members
will bond is a bridge that cannot outrun its own collateral**, which is the property we want, and the
cost is that growth requires new members rather than larger ones.

**Phase 2: delegated staking.** Non-members delegating `solBSV` to a member and sharing its fee,
**activatable by governance**. It would let the bond base grow without new operators, at the cost of
introducing a staking layer with its own incentive problems. Deferred deliberately: the mechanism
should not be designed until the thing it is meant to scale has been shown to work.
