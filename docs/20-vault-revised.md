# 20. The vault — revised

> **Historical audit.** Findings are as recorded; where one has since been closed, the item says so
> inline.

Supersedes [`19-vault.md`](19-vault.md), whose first draft was audited and found broken. Every
finding V1–V10 is addressed below, with the decision that settled it where one was needed.

> ⚠️ **SUPERSEDED BY [`21-vault-structural.md`](21-vault-structural.md). SECOND AUDIT: NOT SOUND TO BUILD.** This revision was audited and the fixes do not hold.
> Four of the claimed corrections are inert or reintroduce the defect. See §Second audit at the end.
> **Model note, later still:** the `bond_R >= k * owed_R` formulas below are **superseded** by **two-sided bonds, neither inside the reserve** (D14), now stated as **the float**; and the reserve is a **2-of-2 `OP_CHECKMULTISIG`** — the gateway's threshold ECDSA key plus the **Greycore**'s — so the earlier "threshold key, not a threshold script" note is itself **superseded by the reversal of F10** (D15). The text is preserved as history.

**Status: designed, not built.** The shipped program still mints straight to the depositor.

---

## What changed, and why

| | Finding | Correction |
|---|---|---|
| **V1** | The nullifier *was* the `PendingMint`, and release closed it — so one deposit minted repeatedly | **Two accounts.** The nullifier is separate and outlives the item |
| **V2** | `cancel_redeem` raced `settle_redeem`; `D = 6h < W = 24h` made it certain | A **claimed** state blocks cancel, and deadlines nest: `D ≥ W + C_payout` |
| **V3** | The peg-in fee was never deducted, so every release over-drew the commingled vault | Mint the **net** amount; store net |
| **V4** | `settle_redeem` bound neither amount nor destination — pay 1 sat, settle a 1,000-token redemption | Verify value **and** script; store `fee` |
| **V5** | Release did not require the chain to have moved, so a frozen window released everything unchecked | **Release requires the tip to advance.** *Decision D-a* |
| **V6** | `MATURITY` was a Solana slot while canonicality is a BSV height | Maturity is a **BSV height**; slots only for the redemption deadline |
| **V7** | `PendingMint` had no relayer, so `owed_R` was unattributable | A **relayer field**, a per-relayer counter, and **consent** at peg-in. *Decision D-b* |
| **V8** | No slash instruction existed at all | `slash`, for **real loss only** — absconding with received BSV. *Decision D-c* |
| **V9** | The separator's encoding was undefined; a program id is identical across clusters | **Length-prefixed exact parse**, and it carries a **cluster id** |
| **V10** | Rent, custody, griefing, `index_of` | Addressed individually below |

---

## Accounts

| Account | Seeds | Holds | Closed by |
|---|---|---|---|
| **Vault** | `[b"vault"]` | A token account, authority = a program PDA. All staged mints and escrowed redemptions, commingled | — |
| **Bond custody** | `[b"bond", relayer]` | `solBSV`, **separate from the vault** | unbond only |
| **`Nullifier`** | `[b"used", txid, vout]` | Nothing but its own existence. **The replay record** | `burn_staged`, or a permissionless prune once the height leaves the window |
| **`PendingMint`** | `[b"mint", txid, vout]` | `{ recipient, relayer, net_amount, deposit_hash, deposit_height, release_height, cluster_id }` | `release_mint` |
| **`PendingRedeem`** | `[b"redeem", holder, nonce]` | `{ holder, amount, fee, bsv_destination, deadline_slot, relayer, claimed }` | `settle_redeem` or `cancel_redeem` |
| **`Relayer`** | `[b"relayer", key]` | `{ script, owed, bump }` — the per-relayer **counter**, so `owed_R` is summable without enumeration | — |

**V1 is the structural correction.** The nullifier and the pending item are different accounts with
different lifetimes: the item exists while tokens are staged and closes on release; the nullifier
exists **while the deposit's height is still inside the window**, which is exactly as long as a
replay could be verified at all.

**Bond custody is separate** (decision M2). If bonds shared the vault, the vault would *be* the
pooled reserve we removed, and "balance = sum of items" would be false from deployment.

---

## Peg-in

```
verify_deposit(claim, relayer_consent)
    • parses the OP_RETURN EXACTLY: version || cluster_id || program_hash || recipient
      — a substring match is unsafe, because a payload committing to two separators
        would mint on both deployments (V9)
    • requires the relayer's SIGNATURE accepting the liability          (V7, D-b)
    • requires bond_R >= k * (owed_R + net_amount)                      (V7)
    • creates Nullifier  [b"used", txid, vout]
    • mints NET amount into the VAULT — not to the depositor            (V3)
    • creates PendingMint, incrementing Relayer.owed

release_mint(deposit)          permissionless
    • requires the tip has ADVANCED past the deposit                   (V5, D-a)
    • requires deposit_height still in the window AND hash matches
    • vault -> recipient; closes PendingMint

burn_staged(deposit)           permissionless
    • requires deposit_height in the window AND hash DIFFERS
    • requires that new hash to have MATURITY_BLOCKS behind it, so a transient
      fork cannot be used to burn a legitimate mint                    (V10/M3)
    • burns net_amount from the vault; closes PendingMint AND Nullifier
```

### The release predicate, precisely — *decision D-a*

```
release  when:  tip_height >= deposit_height + MATURITY_BLOCKS
          and:  deposit_height >= window_start
          and:  window[deposit_height] == deposit_hash

release  also when:  deposit_height < window_start        (survived the whole window)
          BUT ONLY IF the tip has advanced past it        (V5)

refuse   when:  deposit_height > tip_height               (M4: "too new", not "too old")
```

**The first line is the correction.** Previously, a stalled advancer froze the window and every
staged mint released with a *vacuous* hash check — comparing the window to itself. Now release
requires the chain to have **moved**, which proves someone is actually pushing. If nobody is,
nothing releases: minting stalls, but **nothing is falsely released.** For a safety parameter that
is the correct failure.

And `confirmations + MATURITY_BLOCKS ≤ WINDOW` is now checkable at `verify_deposit`, so a deposit
staged so late that it can never be reorged is refused up front rather than stranding.

### Maturity is a BSV height, not a slot — *V6*

Storing `release_slot` compared a Solana slot count against a BSV block window, which is
meaningless. Maturity is now `MATURITY_BLOCKS` of BSV. Slots are used **only** for the redemption
deadline, where the property wanted is that a Solana halt freezes the clock.

---

## Peg-out

```
request_redeem(amount, fee, bsv_destination)
    • holder -> vault; creates PendingRedeem with deadline_slot  = now + D

accept_redeem(redeem, relayer)
    • relayer consents; requires bond_R >= k * (owed_R + amount) afterwards

settle_redeem(redeem, proof)
    • requires NOT already cancelled
    • verifies payout_value  == amount - fee                          (V4)
    • verifies payout_script == bsv_destination                       (V4)
    • marks claimed, starts the challenge window W
    • after W: burns the escrow from the vault, pays the fee, closes the item

cancel_redeem(redeem)          permissionless
    • requires past deadline_slot
    • requires NOT claimed          <- the correction                 (V2)
    • vault -> holder; closes the item
```

**V2 was the ordering contradiction.** `D = 6h` is shorter than `W = 24h`, so a relayer could never
both wait out the challenge and settle before cancel became callable — the holder would be paid
twice and the relayer would lose the payout. Now **`D ≥ W + C_payout`**, and submitting a proof
sets `claimed`, which blocks cancel outright. A relayer that has paid but not yet proved is
protected by the deadline having enough room, not by the holder's restraint.

---

## Slashing — *decision D-c: real loss only*

```
slash(relayer, proof_of_spend)
    • proves the relayer spent BSV it held against outstanding obligations
    • seizes bond_R; the proceeds go to the reserve, not to a holder
      (the holders were made whole by the escrow return)
```

**No slashing for non-delivery.** A failed redemption returns the escrow and leaves the holder
whole, so there is no loss to punish — and punishing it would be punishing an inconvenience. The
bond answers **absconding with received BSV**, which is the case that actually leaves
`custodied BSV < outstanding solBSV`. That is the only thing `slash` is for.

---

## Rent — *V10/M1*

Every item is one PDA, about 0.002 SOL, refunded when it closes:

| Account | Closed by | Rent to |
|---|---|---|
| `PendingMint` | `release_mint` / `burn_staged` | the caller |
| `PendingRedeem` | `settle_redeem` / `cancel_redeem` | the caller |
| `Nullifier` | `burn_staged`, or a **permissionless prune** once the height leaves the window | the caller |

**The prune is the answer to M1.** Without it, a nullifier is unclaimable rent forever — nothing
can enumerate it to close it. With it, anyone holding the `(txid, vout)` can close it after the
height drops out of the window, at which point it can never be needed again. The depositor has the
receipt; everyone else has the incentive.

---

## What this design still does not solve

1. **Detection is a liveness question, not a cryptographic one.** V5 removes the *vacuous* release,
   but the honest branch still has to reach the client for a fraud to be *burned* rather than
   released. Nobody is paid to stage it (P7).
2. **`MATURITY_BLOCKS` is unset.** It must satisfy `confirmations + MATURITY ≤ WINDOW`, and it trades
   user latency against detection time.
3. **Bond custody needs its own rent and unbond schedule** — named here, not designed.
4. **The unbacked path (P8)** still needs to be an explicit mode, and it is not marked.
5. **Nothing verifies the vault's balance equals the sum of its items.** Each instruction moves
   exactly what its record says, but the invariant wants a test that opens and closes many items
   and asserts the balance returns to zero.

---

## Closing the remaining gaps — proposed

Five items were left open. Four of them turn out not to need new machinery.

### R1 — Detection needs no bounty, because V5 already motivates it

The worry was that nobody is paid to advance the chain or to stage a fork. **V5 removes the need
for a bounty**, because it made release *depend* on the tip advancing:

> If the tip stalls, **nothing releases** — including the depositor's own mint.

So anyone waiting on a mint has a direct reason to push headers. And a bot that tries to push and
gets `BrokenLinkage` **knows a reorg has happened** — the same bot stages and commits the fork.

**No new mechanism. No bounty. No operator.** The advancer is a permissionless bot that anyone may
run, and it cannot lie: the program verifies every header it submits. The website will run one as a
convenience, exactly as it runs the front end — a service, not a trust party.

*Worth noting for later:* if advancing ever turns out to lag in practice, a bounty is the obvious
addition. It is not needed to make the design work.

### R2 — `MATURITY_BLOCKS = 144` (about 24 hours)

The constraint is `confirmations + MATURITY ≤ WINDOW`, with `FLOOR = 12` and `WINDOW = 288`, so the
ceiling is 276. **144 leaves a wide margin** (12 + 144 = 156 of 288) and a total user wait of about
26 hours. *(The window has since been fixed at **192 records / 32 h** by W1.4 — cw-144 forces
chainwork and time into each record — so this arithmetic is the superseded revision, kept as
written.)* It is a round number, comfortably inside the window, and it is a parameter — raise it for
more detection time, lower it for faster mints.

### R3 — Bond custody: the ordinary shape, nothing clever

```
Bond PDA      [b"bond", relayer]        holds solBSV, its own rent paid by the relayer
stake(amount)                           increase the bond
announce_unbond()                       starts UNBOND_SLOTS
withdraw_bond()   after UNBOND_SLOTS    requires bond_R >= k * owed_R still holds,
                                        so a relayer with outstanding liability cannot leave
```

**`UNBOND_SLOTS` must exceed the redemption deadline plus the challenge window**, or a relayer could
take a job and withdraw before anyone could act. Standard, and the same rule docs 05 already states.

### R4 — The unbacked path: one flag, in the deposit

A depositor who wants no underwriter says so **in the `OP_RETURN`**, and `PendingMint` records it.

```
OP_RETURN = version || cluster_id || program_hash || flags || recipient
flags bit 0: UNBACKED — no relayer consent required, no bond behind it
```

Why a flag rather than a global switch: it makes the risk **the depositor's explicit choice**, it is
**visible on-chain** in the mint record, and it needs no governance to turn on. The unbacked path is
then exactly what it was always meant to be — the initial seeding — rather than a default nobody
chose.

### R5 — The vault invariant is a test, not a mechanism

Solana cannot enumerate PDAs, so nothing can check "balance equals the sum of items" on-chain, and
nothing needs to: **every instruction moves exactly the amount its own record says**, and every unit
in the vault arrived through one of those paths.

What is required is the test: **open and close many items of both kinds, interleaved, and assert the
vault balance returns to exactly zero.** That is the only place the invariant can be checked, and it
should exist before the vault is used.

---

## The resulting system, in one paragraph

A depositor names terms, sends BSV to a relayer's own script, and after the agreed depth `solBSV` is
minted **into a program-owned vault** rather than to them. It is released once the chain has
advanced `MATURITY_BLOCKS` beyond their deposit **and** the program's own record of that block still
matches; if it does not, the staged tokens are burned and the depositor keeps the BSV that the reorg
returned to them. Redemptions escrow into the same vault, a bonded relayer pays BSV and proves it,
and failure returns the escrow without minting. **Nothing is trusted, nobody's permission is
required, and the only thing anybody must do is keep the Solana copy of the BSV chain current —
which they are motivated to do because nothing releases until they do.**

---

## Second audit — the revision does not hold

The revision was audited adversarially and **four of its claimed fixes are wrong.** Recorded in
full, because the pattern matters: each fix addressed the *stated* defect without addressing the
*mechanism* behind it.

### W1 — The prune is a replay oracle · **critical, inherent**

V1 said the nullifier must be separate and pruned once its height leaves the window. But I specified
that it holds *"nothing but its own existence"* — so `prune` must take the height **as an argument**,
and the program has no stored field to check it against. It can only verify
`height < window_start`, never that the height belongs to *this* nullifier.

So: deposit, release, call `prune(nullifier, window_start - 1)` with a fabricated height, and the
nullifier is gone while the deposit is still in the window. **Re-mint. Repeat every ~24 hours.**
Cost about $0.001 per cycle; unbacked supply accumulates.

**The fix is one field: store `deposit_height` in the nullifier and check it.** With that, the prune
is sound — and the re-inclusion path is *not* independently exploitable, because `window_start` is
monotonic, so re-including a deposit at a later in-window height would require orphaning its original
block, which is a reorg deeper than the window itself.

### W2 — V5 is inert in the branch it was written for · **critical, inherent**

V5's whole point was to stop a stalled advancer causing a vacuous release. **It does not.**

In branch 3 — the height has left the window — there is no hash to check and cannot be, because the
record is gone. And by then `tip >= window_start + 287 >= deposit_height + 288` *(the arithmetic of
this revision's 288-record window; the built window is 192 — W1.4)*, so *"the tip has
advanced past it"* is **automatically true**. V5 adds nothing there. The in-window branch already had
the hash check.

**The actual defect was never "did the tip advance" but "is the client's view current".** A stalled
advancer *freezes* the window, so the height stays in it, and the hash check compares the window to
itself.

**The right fix is staleness, not advancement:** record the Solana slot of the last accepted header
and require it to be recent before releasing. Then a stalled advancer blocks release outright — the
safe failure.

### W3 — `slash` has no on-chain predicate · **critical**

"Proves the relayer spent BSV it held against outstanding obligations" is not program-checkable. A
theft-spend cannot be distinguished from a legitimate payout, "outstanding" is undefined, and the
proceeds are said to go to an off-chain reserve. **As written the bond still does nothing** — the
same defect V8 was supposed to fix.

Making it checkable means storing, per relayer, the deposit outpoints it is accountable for, and
letting a challenger submit a spending transaction that is *not* a registered payout. That is the
naked-spend challenger docs 04 already describes — real work, not a sentence.

### W4 — R1's no-bounty argument is unsound · **critical**

V5 makes release depend on **a** tip advancing, not *the honest* tip. The attacker is the miner and
is the most motivated pusher. A bot that receives `BrokenLinkage` cannot "stage and commit the fork"
unless its branch is longer than the attacker's — precisely the hashpower it lacks.

And **`commit_fork` used to compare height, not chainwork** — fixed in W1.7, so a lower-work branch
no longer wins (though the comparison itself is only exercised on a constant-difficulty chain). On the unbacked path the only party with a pending mint is the attacker, so the
incentive is inverted. Adding C3 and F7, "the program verifies every header" is also not currently
true.

### W5 — Payout proofs are replayable · **high**

Binding value and script is necessary but **not sufficient**. One BSV payout of `amount − fee` to a
given destination settles **every** `PendingRedeem` with the same amount and destination. Two
redemptions to the same exchange deposit address are ordinary traffic — one payment settles both,
the second holder's escrow burns, and no BSV is sent.

**Fix: the payout transaction must carry the redemption's identifier** in an `OP_RETURN`, checked by
`settle_redeem`. The fee sink is also still unnamed.

### W6 — `claimed` is a one-way latch with no resolver · **high**

Submit a proof, `claimed` is set, cancel is blocked — and **nothing clears it.** If the payout is
later reorged away, `settle` can no longer verify, `cancel` is refused, and no listed instruction
resolves the item: **the escrow is frozen forever.** A relayer can do this deliberately.

### W7 — Consent destroys the permissionless remedy · **high**

Requiring the relayer's signature at `verify_deposit` means a depositor **cannot mint without the
relayer** — which destroys P3's remedy, that minting is permissionless and is itself the enforcement.
The deposit then sits in a key-controlled script with no timelock, so a relayer that declines costs
the depositor the whole deposit: exactly the loss doc 18 decided was solved.

**The reconciliation:** consent should be the relayer **registering a script**, not signing each
deposit. A registered script *is* standing consent to be liable for deposits that pay it. That keeps
the bond meaningful, keeps slashing defensible, and leaves minting permissionless.

Also: **nothing decrements `owed`**, so it is monotone and `withdraw_bond` becomes unsatisfiable.

### Also

| | |
|---|---|
| **W8** | The vault-invariant claim is false while the fee sink and the shared payout are undefined |
| **W9** | The `UNBACKED` flag labels an unbacked mint without bounding it — the loss still lands on holders who never saw the flag |
| **W10** | The instruction set does not compose: `claimed` has no resolver, settlement is described as two-phase but specified as one, the `OP_RETURN` layout contradicts R4, `D ≥ W + C_payout` mixes Solana slots with BSV blocks, `nonce` is in the seeds but not the record, `owed` has no decrement, and `verify_deposit`'s two inits are not stated to be atomic |
| **W11** | The closing paragraph is false: the checkpoint *is* trusted, the upgrade authority can override every parameter, a relayer signature was required, and F7 means the client halts at the first retarget so its view cannot be kept current on testnet. *(F7 has since been fixed — W1.6, cw-144 verified 324/324; X3, the hard-coded rule, remains.)* |

### What the revision did genuinely fix

The separate nullifier closes the original V1 *until the prune is called*; minting the net amount is
correct; `D ≥ W + C_payout` names the right ordering constraint; value-and-script binding is a real
improvement; and the length-prefixed separator is right. **None of it survives the holes above.**

### Minimum changes before it is buildable

1. Store `deposit_height` in the nullifier; check it in `prune`.
2. Replace "tip advanced" with **staleness** — the last accepted header's slot must be recent.
3. Give the reorged-payout state an **unclaim/refund** path.
4. Add a **payout identifier binding** to `settle_redeem`.
5. Make `slash`'s predicate explicit, or drop the claim that the bond is enforced.
6. Make consent the **script registration**, and define `owed`'s decrement.
