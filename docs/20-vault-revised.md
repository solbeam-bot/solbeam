# 20. The vault — revised

Supersedes [`19-vault.md`](19-vault.md), whose first draft was audited and found broken. Every
finding V1–V10 is addressed below, with the decision that settled it where one was needed.

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
