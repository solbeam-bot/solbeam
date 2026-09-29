# 6. Parameters & governance

> **Most of this is a specification, not shipped behaviour.** The built set is exactly four things:
> the **light client** (cw-144 difficulty verification and Merkle inclusion), the **`solBSV`**
> **token**, the **mint**, and **fork staging with chainwork** — 20 passing on-chain tests.
> **The vault, the federation, threshold custody, governance, slashing and all of peg-out are
> designed and not built.** Nothing here is settable at runtime today: the only parameter in the
> shipped program is `MIN_CONFIRMATIONS = 12`, **fixed in code**. [`13-summary.md`](13-summary.md)
> is the authoritative model; where this document and that one disagree, that one is right.

## The parameters

The defaults below come from [`13-summary.md`](13-summary.md) and
[`23-federation.md`](23-federation.md). **All of them are parameters rather than constants** — that
is the point of the governance design — except where the note says otherwise.

| Parameter | Default | Class | What it does |
|---|---|---|---|
| **Mint fee** | **30 bp**, governed | economic | Replaces the discovered order-book fee (D2, superseded) |
| **Redeem fee** | **30 bp**, governed | economic | Same fee in the other direction |
| **Bond** | **1,000 BSV**, posted as `solBSV` | **safety** | Open-membership gate; must be seizable on Solana, which is why it is not posted in BSV |
| **Bond multiple `k`** | **1** | **safety** | `bond ≥ k × owed`. At `k = 1` the bond covers the credited liability in full and no more |
| **Governance threshold** | **85% of pledged coins** | **safety** | The proposal bar |
| **Governance delay** | **30 days** | **safety** | Signal time between passing and taking effect. **This is the exit window** |
| **Signal** | **live from the moment it is raised** | **safety** | The proposal is visible while it is only a proposal |
| **Governance scope** | **includes the upgrade authority** | **safety** | There is no immutable floor; the exit window is the floor |
| **Pause** | mints only; majority threshold; lifts after N days | **safety** | **Redemptions can never be paused.** Pausing inbound is a safety valve; pausing outbound is taking hostages |
| **`FLOOR`** | **12 BSV blocks** (~2 h) | **safety** | Minimum confirmation depth before a deposit may be minted. Settable in the design; **fixed as `MIN_CONFIRMATIONS = 12` in the built code** |
| **`MATURITY`** | **144 blocks** (~24 h) | **safety** | How long a staged mint waits before it can be released |
| **`WINDOW`** | **192 records / 32 h** | **fixed by arithmetic** | How far back the light client can prove anything at all |
| **DAA** | **cw-144** as specified | **safety** | Governable in the design, so a BSV rule change is a vote; **hard-coded in the built program (X3)** |
| **Unbonding period** | — | **safety** | Must outlast the payout deadline, or a member can take a job and leave |
| **Challenge bounty share** | — | economic | Unspecified; see §Open parameters |

**A safety parameter bounds a fraud; an economic parameter sets price and size.** The distinction is
load-bearing where it comes to governance: whoever can set `FLOOR = 0` or `WINDOW = 0` holds a mint
voucher, and whoever can pause redemptions holds the reserve hostage. That is why **redemptions are
outside governance's reach entirely** rather than merely subject to a supermajority.

### How the safety parameters actually protect

The old model's answer was "safety parameters may only be moved in the conservative direction". The
new model's answer is different, and it is the more important change in this chapter:

**There is no immutable floor. The floor is the exit window.**

A change needs **85% of pledged coins** and takes **30 days**, with the proposal signalled **live
from the moment it is raised** — and **redemptions run throughout, because they can never be
paused**. So a proposal that would harm holders does not trap them: it **empties the bridge before it
lands.** By the time it takes effect there is nothing left to take.

**The residual, stated plainly:** a holder who does not watch and does not act within 30 days is
exposed. That is a disclosure obligation, not a mechanism.

## The parameters that are arithmetic, not choices

Two numbers are worth separating from the rest, because they are not policy and cannot be governed
into being different. They are measured facts about the built client:

```
WINDOW              = 192 records  = 32 hours at 600 s/block
HEADER_RECORD_SIZE  =  52 bytes    (32 hash + 16 chainwork + 4 time)
LIGHT_CLIENT_FIXED  = 119 bytes
SPACE               = 119 + 192 × 52 = 10,103   of 10,240  ->  137 bytes margin
LOOKBACK            = 147 records  (144 + 3 for cw-144's median-of-three)
```

- **52 bytes a record, not 32.** cw-144 subtracts two cumulative chainworks and two timestamps 144
  blocks apart, so a record must carry hash, chainwork and time. A bare-hash window cannot verify
  the difficulty rule at all.
- **147 records of lookback**, so 192 is only 45 records of slack above the minimum.
- **194 is the arithmetic maximum** (10,207 bytes, 33 bytes of margin); **192 is the largest window
  with real margin.** A 48-hour window would be 288 × 52 = 14,976 bytes plus overhead, 46% over the
  cap, and `initialize` would simply revert. **The old 48-hour deposit deadline was never
  achievable.**

**Consequence, and it is a product decision rather than a detail:** the deposit lifetime is **32
hours**, down from the 48 the earlier design assumed. A deposit whose block has left the window can
never be proven again.

## Security parameters, and the one that is a market

The old design made **depth a term of the bid**, priced on an order book. **That is superseded.**
Depth is now a protocol parameter:

- **`FLOOR` = 12 blocks** is the minimum. It makes a reorg cost real mining work to undo a deposit;
  a low floor makes attacks cheap and therefore frequent, raising the number of chances for a
  detection failure to slip through.
- **`MATURITY` = 144 blocks** sets the time available to detect. A staged mint is released only once
  the tip has advanced past the deposit **and** the hash the client stores at that height still
  matches the one recorded when the deposit was proven. If it differs, the deposit was reorged and
  the staged tokens **burn**.
- **`WINDOW` = 32 hours** bounds the reorg the client can see at all. Beyond it, a reorg is not
  detectable *by definition*, because the client no longer holds the header.

The bounds that need no oracle are exactly these: a high `FLOOR` makes out-mining expensive, and the
vault means a *successful* fraud mints tokens that are **not in anyone's wallet** — there is nothing
to dump and no innocent buyer to inherit the loss.

**The honest limit:** the reserve is off-chain BSV the program cannot read, so `custodied BSV ≥
outstanding solBSV` is **tracked, not verified** (D8). What the program *can* compare is `bond ≥
k × owed`, since both are `solBSV` quantities it holds or measures.

## What the program can and cannot see

| Quantity | Where it lives | How the program treats it |
|---|---|---|
| BSV headers, chainwork, time | On Solana, in the light client | **Verified.** cw-144 replay, 324/324 real mainnet headers exact |
| Deposit inclusion | Proved against the light client | **Verified** — proof of work and Merkle branch |
| Deposit's block hash | Stored when the deposit is proven | **Compared later**, to decide release vs burn. No reporter |
| Deadline | Solana slots | **Read natively.** A cluster halt freezes the clock; it does not burn anyone |
| `owed` and the bond | On Solana, `solBSV` | **Compared on-chain**: `bond ≥ k × owed` |
| The BSV reserve itself | Off-chain, under a threshold key | **Not readable.** Monitored and published, not enforced |
| Price, hashrate, reorg cost | Off-chain | **Never consulted.** Published as information; deciding on them would make them oracles |

## What was specified before, and is now superseded

The previous version of this chapter carried a stable of IDs — `P1`–`P10`, `FLOOR` as fixed-in-code
(D4), `k = 1` (D5), "no governance at all" (D7), the reserve invariant as monitored (D8) — under a
two-gate, per-relayer, order-book model. That model is **superseded**, and the table below records
what became of each rather than rewriting it silently.

| Old ID | Old content | Now |
|---|---|---|
| **P1** | `FLOOR` = 12 BSV blocks, fixed in code for the PoC | **Kept as the default**, but it is a **governable parameter**, not fixed — except that the built code has `MIN_CONFIRMATIONS = 12` and no setter |
| **P2** | `C_payout` = 12 payout confirmations | **Superseded.** Payouts settle against the light client after a challenge window; the number is not yet specified |
| **P3** | Redemption deadline = 6 h of Solana slots | **Superseded.** The deadline is set per redemption by the holder; the default is unspecified |
| **P4** | Challenge window = 24 h | **Superseded.** The payout challenge/`MATURITY` design settles this; no default is specified |
| **P5** | `RECENT_REORG_WINDOW` = 12 h, enforced-or-monitored undecided | **Superseded.** The vault compares stored hashes; there is no separate window parameter |
| **P6** | `TIP_STALENESS` = 2 h | **Superseded** as a parameter; `set_paused` is the built authority-gated safety valve when the tip stops advancing |
| **P7** | `MIN_PEG_IN` = 10 BSV | **Superseded** — an economic parameter, no longer specified |
| **P8** | `MAX_PEG_IN` = 10,000 BSV | **Superseded.** The real bound is the bond: total value locked is capped by total bonds pledged |
| **P9** | `MAX_PEG_OUT` = 10,000 BSV | **Superseded.** A redemption is bounded by the reserve and the payout path, not a per-transaction number |
| **P10** | `HOT_FLOAT_CAP` | **Removed with the pooled float.** There is one reserve under a threshold key, and the bond is the bound |
| **—** | bond `k` = 1 (D5) | **Kept.** `bond ≥ k × owed`, `k = 1`, and it is governable |
| **D4** | `FLOOR` fixed in code | **Superseded.** The default is 12; the mechanism is governance |
| **D7** | No governance in the PoC | **Superseded.** 85% / 30 days / live signal, holding the upgrade authority |
| **D8** | Reserve invariant monitored, not enforced | **Kept**, and it is the honest residual |

**Two of the old reasons survive verbatim, because they were right:**

- **A stablecoin bond is a written call option on the reserve.** Post `$500k` against `10,000 BSV`
  and the member is short `5,000 BSV`; at `$100` the option is in the money, and absconding becomes
  the rational trade. A price governor cannot fix a written option — it is reactive, it needs an
  oracle, and the window between the move and the throttle *is* the trade. **Denominate the bond in
  `solBSV`.** That removes the position instead of hedging it, and it is also what makes `bond ≥
  k × owed` checkable on-chain with nothing external consulted.
- **A change must be disclosed before someone transacts.** A holder should be able to see the fee,
  `FLOOR`, `MATURITY` and the bond before they peg in. Published, the external metrics are
  information; consulted by the program, they would be oracles.

**One old reason does not survive:** "loosening a safety parameter should require shipping a new
program." That was written for a design with **no governance**. The new model governs the upgrade
authority itself, so the protection is not that the rules are frozen — it is that **you can leave
before they change.**

## Open parameters, named so they are not lost

| Unspecified | Note |
|---|---|
| **The threshold itself** | `t` of `n` is not stated anywhere, and it interacts with shard count and `k` |
| **Unbonding period** | Must outlast the payout deadline; no default |
| **Pause threshold and duration** | "A majority of pledged coins" and "N days" — neither is fixed |
| **Challenge bounty share** | How a seizure splits between the wronged holder and the challenger |
| **Redeem deadline default** | The holder sets it; the minimum is not stated |
| **Genesis** | Members bond `solBSV`, which does not exist until a mint happens (D3). The bootstrap has no path |
| **Shards** | One threshold key, or groups with their own |

**Three things are open in the built code, and are not governance questions:**

1. **A5 — the program upgrade authority can override every parameter.** It is an unconditional mint
   voucher. Governance holding it is the design's answer; until that exists, it is a live critical.
2. **X3 — the DAA is hard-coded.** BSV's own documentation says the rule will change, so a
   consensus change halts the bridge until a redeploy. Making it governable is the fix.
3. **The vault's design has failed two audits** and is being re-audited against this model
   ([`21-vault-structural.md`](21-vault-structural.md)).

## Change checklist

Any parameter change, once a mechanism exists, should be:

1. **Proposed publicly**, with the reasoning and the new arithmetic.
2. **Signalled live from the moment it is raised**, so the 30 days are visible time rather than a
   surprise.
3. **Effective only after the delay**, with redemptions open throughout — **they are never
   pausable**, and that is the one guarantee the design does not trade away.
4. **Published** in a changelog with the effective date.
5. **Incapable of moving funds.** The bond and the vault are program-owned; parameters are not a
   path to the reserve.

**No process to execute any of this exists yet.** The checklist is the shape the governance design
has to satisfy, not a description of a mechanism that runs.

---

Next: [Roadmap](07-roadmap.md)
