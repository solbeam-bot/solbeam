# 21. The vault — structural redesign

> **Historical audit.** Findings are as recorded; where one has since been closed, the item says so
> inline.

Two reviews of the previous vault failed on the same four classes of defect. This is a redesign
against those classes rather than a third patch. It supersedes [`20-vault-revised.md`](20-vault-revised.md)
and [`19-vault.md`](19-vault.md).

> 🛑 **THIRD AUDIT: NOT SOUND TO BUILD, AND THE CHAINWORK PREMISE IS FACTUALLY WRONG.**
> BSV does not retarget every 2016 blocks — its difficulty adjusts **every block**. The foundation
> this document was written on does not exist. See §Third audit.

**Status: designed, not built.**

---

## The four classes, and the rule that answers each

The previous designs kept failing the same four ways. Rather than fix the instances, each class gets
a structural rule.

| Class | Why it kept happening | **The rule** |
|---|---|---|
| **1. Identity confusion** | One account held two jobs, so closing it for one destroyed the other (V1) | **One account, one job.** No account is reused |
| **2. Expiry** | A record that is pruned must not trust a caller to say when — the nullifier stored nothing, so `prune` took a height as an argument (W1) | **A record that can be pruned must contain the condition for its own pruning** |
| **3. Liveness as cryptography** | "Has this been reorged?" is really "has anyone told the client?" (V5, W2, W4) | **The program checks its own freshness, and the design says plainly that detection requires someone to push** |
| **4. Uncheckable invariants** | A commingled vault's "balance = sum of items" cannot be verified on Solana (V3, R5, W8) | **No shared value account exists.** Every token account belongs to exactly one item |

**Class 4 is the biggest change: there is no vault account.** "The vault" becomes a *rule* — a set of
program-owned escrow accounts, one per item — not a pooled address holding everyone's tokens.

---

## Accounts — every one has a single job

| Account | Seeds | Its one job | Closed by |
|---|---|---|---|
| **`PegIn`** | `[b"in", txid, vout]` | Metadata for one staged mint; owns its escrow | `release_mint`, `burn_staged` |
| **`PegInEscrow`** | ATA of the `PegIn` PDA | Holds **that one item's** tokens | with its `PegIn` |
| **`Marker`** | `[b"used", txid, vout]` | Replay record. Stores **`deposit_height`** — the condition for its own pruning | permisionless prune, once `deposit_height < window_start` |
| **`PegOut`** | `[b"out", holder, nonce]` | Metadata for one redemption; owns its escrow | `settle_redeem`, `cancel_redeem` |
| **`PegOutEscrow`** | ATA of the `PegOut` PDA | Holds **that one item's** tokens | with its `PegOut` |
| **`Relayer`** | `[b"relayer", key]` | `{ script, owed, bond_account }` — the counter that makes `owed_R` readable without enumeration | — |
| **`Bond`** | ATA of the `Relayer` PDA | That relayer's staked `solBSV` | `withdraw_bond` |

**Why this answers class 1:** `PegIn` does not double as the replay marker. Closing a resolved item
cannot destroy the replay record, because they are different accounts with different lifetimes.
**Why it answers class 2:** `Marker` stores `deposit_height`, so `prune` checks a stored field rather
than trusting an argument — the W1 hole closes, and the re-inclusion path was never independently
exploitable because `window_start` is monotonic.
**Why it answers class 4:** each escrow holds one item's tokens. There is no sum to falsify — a
transfer either finds the tokens in that item's own account or it fails.

---

## Peg-in

```
register_relayer(script, bond)   the relayer's STANDING CONSENT, given once, not per deposit

verify_deposit(claim)            permissionless
    • parses the OP_RETURN exactly: version || cluster_id || program_hash || recipient
    • requires the deposit's script to be a REGISTERED relayer script
    • mints NET amount into a NEW PegInEscrow owned by a NEW PegIn            (one item, one account)
    • creates Marker{ deposit_height }                                        (its own prune condition)
    • Relayer.owed += net

release_mint(item)               permissionless
    • client FRESH: last accepted header within MAX_STALENESS slots
    • tip_height >= deposit_height + MATURITY_BLOCKS
    • deposit_height still in the window AND hash matches
    • escrow -> recipient; closes PegIn and its escrow

burn_staged(item)                permissionless
    • client fresh
    • deposit_height in the window AND hash DIFFERS
    • the new hash has MATURITY_BLOCKS behind it, so a transient fork cannot burn a good mint
    • burns the escrow; closes PegIn, its escrow, AND the Marker

prune(marker)                    permissionless
    • requires marker.deposit_height < window_start      <- a STORED field, never an argument
```

### The release predicate, and what changed — class 3

Previous designs checked whether the **tip had advanced**. That was inert: once a height leaves the
window the tip is tautologically past it, and a *stalled* advancer freezes the window so the height
never leaves at all, making the hash check compare the window to itself.

**The check is now freshness, not advancement:**

```
release  when:  now_slot - last_push_slot <= MAX_STALENESS_SLOTS      <- the real fix
          and:  tip_height >= deposit_height + MATURITY_BLOCKS
          and:  deposit_height >= window_start  ->  hash must match
          or:   deposit_height <  window_start  ->  no hash exists; freshness is the only guard
refuse   when:  deposit_height > tip_height        ("too new", not "too old")
```

**A stalled advancer now blocks release outright.** Nothing releases while the client's view is
stale — minting stalls, but nothing is falsely released. And the honest statement that goes with it:
**beyond the window, canonicity is not checked and cannot be.** The window size *is* the security
parameter, and `MATURITY_BLOCKS` is the real detection budget.

---

## Peg-out

```
request_redeem(amount, fee, bsv_destination, deadline_slot)
    • holder -> a NEW PegOutEscrow, owned by a NEW PegOut         (one item, one account)

accept_redeem(item, relayer)     the relayer's consent; requires bond covers owed + amount;
                                 Relayer.owed += amount

settle_redeem(item, proof)       requires slot <= deadline_slot
    • payout_value  == amount - fee
    • payout_script == bsv_destination
    • the payout's OP_RETURN carries THIS item's id            <- binds one payment to one item
    • payout depth >= W                                        <- the challenge window
    • burns the escrow; fee -> relayer; closes PegOut; owed -= amount

cancel_redeem(item)              requires slot > deadline_slot
    • escrow -> holder; closes PegOut
```

**Two fixes are structural here.**

**No `claimed` latch** — that was a one-way state with no resolver, so a reorged payout froze the
escrow forever (W6). Instead the two instructions are **mutually exclusive by time**:
`settle` requires `slot <= deadline_slot`, `cancel` requires `slot > deadline_slot`. There is no
intermediate state to get stuck in, because there is no intermediate state at all.

**The payout carries the item's id** (W5). Binding value and script was not enough: one payment of
`amount − fee` to a given destination would have settled *every* redemption with that amount and
destination — and two redemptions to the same exchange address are ordinary traffic.

The relayer's obligation is therefore **pay and prove within `D`**, with `D >= C_payout + W`.

---

## Slash — the predicate, stated

W3: the bond did nothing, because "proves the relayer spent BSV it owed" is not program-checkable.

**The predicate, made checkable by what `Marker` now stores:**

```
Marker{ deposit_height, relayer, outpoint, amount }        recorded at verify_deposit

slash(marker, spending_tx)       permissionless, by anyone
    • the spending tx consumes marker.outpoint
    • its outputs match NO authorized payout for that relayer
      (checked against the item ids the program has settled)
    • burns bond_R; amount -> the holders' reserve account
```

This is the naked-spend challenge docs 04 already describes. It is now *stated*, which it was not
before — but it is a substantial instruction and **it needs its own review before it is built.**

---

## Two things removed rather than fixed

**The `UNBACKED` flag is gone.** W9 was right that one bit labels a risk without bounding it — the
loss lands on holders who never saw the flag. **Every deposit must now pay a registered relayer's
script.** Genesis seeding uses the ordinary bonded path: the team registers a script and stakes like
anyone else. That removes the entire class rather than annotating it.

**No unbacked path, so no "no stakers" case** — which also removes the incentive gap R1 failed to
close.

---

## Still required, and not this document's to fix

1. ~~`commit_fork` compares height, not chainwork~~ — **fixed below.**
2. **F7 — the client halts at the first retarget.** Until fixed, none of this runs on testnet.
3. **`MAX_STALENESS_SLOTS`, `MATURITY_BLOCKS`, `D`, `W` are unset.** `D >= C_payout + W` and
   `confirmations + MATURITY <= WINDOW` constrain them; the values are a separate decision.
4. **`owed_R` needs a decrement on settlement** (specified above) **and a rule for whether released
   mints leave it standing** — the liability persists after release, since the holder may still
   redeem.
5. **The `slash` predicate and the reserve account it pays into** need review.
6. **Nothing has verified that `verify_deposit`'s account creations are atomic.** Two inits in one
   instruction; if the second fails the first should roll back, and that should be a test.

---

## Chainwork — required, and nearly free

**Was a defect; now fixed.** `commit_fork` compared **height**, so a longer but lower-work branch
would have replaced a heavier one: while the difficulty is constant those are the same thing, which is
why it never bit on regtest, but the moment DAA is enabled they diverge and the reorg rule becomes
"whoever mines the most blocks wins" regardless of the work in them. Since W1.6/W1.7 every record
carries its own cumulative chainwork — derived from its own `bits` — and the commit compares that. The
comparison is still asserted on a constant-difficulty chain (the fixture), so **the rule is exercised
but a varying-difficulty branch choice is not**: the chainwork arithmetic is verified against mainnet
headers in `difficulty-vectors/`, and the comparison against them is not.

### The rule

```
chainwork(H)  =  Σ  work(bits_at(h))   for h <= H
work(b)       =  2^256 / (target(b) + 1)
```

**`commit_fork` and any tip replacement must require strictly greater `chainwork`, never height.**

### Why this needs no extra per-header storage

The naive fix is to store each header's work in the window — 16 bytes per record, taking
288 × 48 = 13,824 bytes, well past the 10,240-byte account cap.

**But retargets are every 2016 blocks and the window holds 288.** So **at most one difficulty
boundary can ever sit inside the window.** Two stored `bits` values and the height they changed at
are therefore enough to derive any in-window header's difficulty:

```
LightClient { ..., expected_bits, prev_bits, retarget_height, chainwork: u128, last_push_slot }

bits_at(h) =  if h >= retarget_height  { expected_bits }  else  { prev_bits }
```

- **`push_header`** validates `bits == bits_at(height)`, adds `work` to `chainwork`, and records
  `last_push_slot` — which is also the freshness check the release predicate needs.
- **`commit_fork`** derives the fork point's work by subtracting the in-window work, adds the staged
  branch's, and requires the total to be **strictly greater** than the current tip's.

**Cost: 16 bytes for the cumulative value, 4 + 4 for the two difficulties, 4 for the retarget height,
8 for the last-push slot — about 36 bytes.** The window stays at 288 and `LightClient::SPACE` goes
from 9,322 to roughly 9,358, far inside the cap.

This also gives DAA a home: implementing the retarget now means **computing** `expected_bits` at a
boundary rather than accepting a declared one (F7), and the anchor it needs — the previous period's
start height and time — is the same kind of stored scalar.

### One caveat, stated rather than hidden

`work(b) = 2^256 / (target + 1)` fits a `u128` for BSV's real difficulty range (per-block work is
roughly `2^69`, and a full chain about `2^89`). It would not fit for an absurdly high difficulty, and
the arithmetic should **saturate** rather than wrap. Worth a test at the boundary.

---

## Third audit — not sound, and one error is factual

### The foundational error: BSV's difficulty changes **every block**

This document asserted: *"retargets are every 2016 blocks and the window holds 288, so at most one
difficulty boundary can ever sit inside the window."* **That is false, and I stated it as fact
without checking.**

BSV does not use Bitcoin's 2016-block interval. Its difficulty adjustment is a moving-average
algorithm applied **per block** — per BSV's own documentation, *"the current difficulty adjustment
algorithm changes the rate every block"* ([BSV wiki](https://wiki.bitcoinsv.io/index.php/Target),
[BSV Hub](https://hub.bsvblockchain.org/higher-learning/bsv-academy/bsv-theory/proof-of-work/controlling-the-block-discovery-rate.md)).
ASERT derives the target from an **anchor block** and the incoming block's own timestamp.

**Everything built on that premise falls:**

| | |
|---|---|
| **Two `bits` values derive in-window difficulty** | False. Up to 288 distinct values can sit in a 288-header window |
| **~36 bytes, window stays 288** | False. `hash + bits` is 36 B × 288 = 10,368, plus 106 B overhead = **10,474 > the 10,240 cap** |
| **`push_header` works on a real chain** | **It does now.** The finding below was correct when written; W1.6 replaced the fixed target with cw-144, verified against real mainnet headers at 324/324 exact. *Original finding:* it requires `bits == expected_bits`, so it rejects every header after a per-block adjustment — F7 is not "halts at the next retarget", it is "halts almost immediately on any real chain" |
| **Chainwork can be computed from two scalars** | Cannot. `bits_at(height)` is wrong for nearly every header |

**This also means the shipped light client has never been tested against a real difficulty.** Regtest
uses the maximum target and never adjusts, so the constant-difficulty assumption has held for the
entire PoC — and would have failed on contact with testnet.

**What the fix now looks like, and why it is research rather than design:** if BSV uses **ASERT**,
then the expected target is computable from **one stored anchor** (height, time, bits) plus the
incoming header's own timestamp — which the header already carries, so **no per-header storage is
needed at all**, and the window can stay 288. If it is the older 144-block moving average, the client
needs 144 timestamps and the window must shrink to about 253 records at 40 bytes each.

**Which one it is has to be established from BSV's specification and checked against real headers.
It cannot be assumed.** This is the correction that matters most in the whole document set.

### The other findings, briefly

| | | |
|---|---|---|
| **T1** | `slash` is **forgeable** — nothing requires the spending transaction to be in a block (no header or Merkle proof), so anyone can craft bytes consuming a public outpoint and **burn any relayer's entire bond**. The predicate is also a universal quantifier over a set Solana cannot enumerate | critical |
| **T2** | **The redesign violates its own class-4 rule.** `slash` pays into *"the holders' reserve account"* — one program-owned account funded by many bonds, unverifiable, exactly the pooled account the redesign claims to have removed. It is also self-contradictory: burning the bond leaves nothing to transfer | critical |
| **T3** | **The peg-in fee has no home.** The `OP_RETURN` layout has no amount or fee field, so "mints NET amount" is undefined — and if the relayer declares the fee, the program trusts the party with the incentive to overstate it | critical |
| **T4** | **Recorded P2 is still unanswered.** `commit_fork` still splices without re-checking the fork point, and this document's "still required" list omits it entirely — the one defect doc 18 called *"the one genuine forgery vector"* | critical |
| **T5** | **`owed_R` double-counts.** Incrementing at both mint and accept, with only settlement decrementing, means a mint redeemed through the same relayer goes 100 → 200 → 100 and the mint line is never discharged. Monotone growth makes `bond_R ≥ k · owed_R` unsatisfiable, so the bond locks | high |
| **T6** | **The settle/cancel race is still live** — `deadline_slot` is caller-supplied with no `>= now + D` requirement, so a holder can choose a past deadline, keep the BSV the relayer paid, and reclaim the escrow | high |
| **T7** | **Post-settle reorg leaves the holder with nothing.** The escrow is burned and the item closed, so neither instruction can resolve it — contradicted by docs 13/14 | high |
| **T8** | **Freshness is satisfied by the attacker** — `last_push_slot` advances on *any* accepted header, so it measures update recency, not honesty. And a mint staged on a branch that later loses becomes releasable once its height leaves the window: **the honest chain advancing destroys the only evidence** | critical |
| **T9** | `Marker` is again three things at once (replay record, slash evidence, closeable object) with contradictory field lists — **class 1 and class 2 were obeyed per instance, not per class** | high |
| **T10** | Chainwork arithmetic underspecified: 256-bit division, the baseline at `initialize` (cumulative from genesis — not derivable from a checkpoint header, and never listed as a trusted scalar), and `commit_fork` never specified to *write* the new state | high |
| **T11** | Rent now ~$0.11 extra per item, and no `stake`/`announce_unbond`/`withdraw_bond` instruction exists though the table says `Bond` is closed by one | medium |
| **T12** | **Silent decision reversals:** requiring a registered relayer script reverses D6/P8, and "genesis is the ordinary bonded path" reverses D3/G2, with no change recorded in doc 14. Doc 13:39 and doc 03:53 claim *"strictly-heavier commit — yes"*; the commit now compares **chainwork**, not height (W1.7), and the suite is 20 tests rather than 17 | medium |

### Verdict

**Not sound to build.** Three designs, three audits, and the holes have moved rather than closed: the
pooled reserve violates the redesign's own rule, `slash` is forgeable, `owed_R` does not balance, the
settle/cancel race is live, and the chainwork rule rests on a false statement about the chain.
