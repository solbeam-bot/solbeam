# 21. The vault — structural redesign

Two reviews of the previous vault failed on the same four classes of defect. This is a redesign
against those classes rather than a third patch. It supersedes [`20-vault-revised.md`](20-vault-revised.md)
and [`19-vault.md`](19-vault.md).

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

1. **`commit_fork` compares height, not chainwork** (`lib.rs:401`), so a longer lower-work branch
   wins. This is a light-client defect, not a vault one, and it undermines V5's replacement too.
2. **F7 — the client halts at the first retarget.** Until fixed, none of this runs on testnet.
3. **`MAX_STALENESS_SLOTS`, `MATURITY_BLOCKS`, `D`, `W` are unset.** `D >= C_payout + W` and
   `confirmations + MATURITY <= WINDOW` constrain them; the values are a separate decision.
4. **`owed_R` needs a decrement on settlement** (specified above) **and a rule for whether released
   mints leave it standing** — the liability persists after release, since the holder may still
   redeem.
5. **The `slash` predicate and the reserve account it pays into** need review.
6. **Nothing has verified that `verify_deposit`'s account creations are atomic.** Two inits in one
   instruction; if the second fails the first should roll back, and that should be a test.
