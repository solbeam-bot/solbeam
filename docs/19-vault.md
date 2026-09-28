# 19. The vault — instruction set

The vault is the component the rest of the design rests on. Every mint lands in it, and every
redemption is escrowed in it. It is **one program-owned token account**, plus a small PDA per
pending item.

**Status: designed, not built.** The shipped program mints straight to the depositor.

---

## Why a vault at all

Three things follow from the tokens sitting in a program-owned account rather than a user's:

1. **Reversibility without a freeze authority.** The program can burn or return tokens in its
   *own* account. That is not confiscation — it is disposing of what it holds. No freeze
   authority, no Token-2022 hooks, plain SPL.
2. **A fraudulent mint cannot be sold.** A staged token is not in circulation, so there is nothing
   to dump on a market and no innocent buyer to inherit the loss.
3. **Supply is conserved.** A failed redemption returns the escrow rather than re-minting it.

---

## Accounts

| Account | Seeds | Holds |
|---|---|---|
| **Vault** | `[b"vault"]` | A token account, authority = a program PDA. Holds **all** staged mints and escrowed redemptions, commingled |
| **PendingMint** | `[b"mint", txid, vout]` | `{ recipient, amount, deposit_hash, deposit_height, release_slot, bump }` |
| **PendingRedeem** | `[b"redeem", holder, nonce]` | `{ holder, amount, bsv_destination, deadline_slot, relayer, bump }` |

**One vault, a PDA per item.** Not a list — a list would recreate the 200-per-window ceiling that
decision P5 exists to remove. Solana cannot enumerate PDAs, but it never needs to: every operation
names the item it is acting on, so the account is derived rather than searched for.

Tokens are fungible, so the vault's balance is the *sum* of pending items. The program never needs
to verify that sum — each instruction moves exactly the amount its own record says, and every unit
in the vault arrived through one of the paths below.

---

## Peg-in

```
verify_deposit(claim)     verifies the BSV deposit, then:
                          • mints `amount` INTO THE VAULT, not to the depositor
                          • creates PendingMint{ recipient, amount, deposit_hash,
                                                deposit_height, release_slot }

release_mint(deposit)     permissionless, after release_slot:
                          • requires the deposit still canonical  (see below)
                          • vault -> recipient
                          • closes PendingMint, rent back to the caller

burn_staged(deposit)      permissionless, if the deposit was reorged:
                          • requires the deposit NO LONGER canonical
                          • burns `amount` from the vault
                          • closes PendingMint
```

### How the program decides "still canonical", trustlessly

`PendingMint` records **the block hash the deposit was proven against** — not just its height. Then:

| | Condition | Result |
|---|---|---|
| Deposit's height **still in the window**, stored hash **matches** | canonical | `release_mint` succeeds |
| Deposit's height **still in the window**, stored hash **differs** | reorged | `burn_staged` succeeds |
| Deposit's height **has left the window** | survived 48 hours | `release_mint` succeeds; burning is no longer possible |

**No oracle, no reporter, no discretion.** The program knows because it stored the hash and it can
see its own window. A reorg is a fact about the headers, not a claim by anyone.

**This is why replay is keyed on `(txid, vout)` and not height.** A reorg re-includes a transaction
at a different height; keying on identity means the same deposit is recognised wherever it lands.

### The constraint this creates

**`MATURITY` must be shorter than the window (48 h).** If a staged mint outlives its window it can
still be released — the third row above — so this is safe, but it means the *reorg* branch closes
after 48 hours. A reorg deeper than the window cannot be detected by definition, which is the same
boundary the whole client already has.

---

## Peg-out

```
request_redeem(amount, bsv_destination)
                          • holder -> vault
                          • creates PendingRedeem{ ..., deadline_slot }

accept_redeem(redeem, relayer)
                          • a relayer consents, taking the liability
                          • requires bond_R >= k * owed_R AFTER this redemption
                          • starts the settlement clock

settle_redeem(redeem, proof)
                          • the payout is proven against the light client
                          • burns the escrowed amount from the vault
                          • pays the fee to the relayer
                          • closes PendingRedeem

cancel_redeem(redeem)
                          • permissionless, after deadline_slot, if unsettled
                          • vault -> holder
                          • closes PendingRedeem
```

**Failure returns; it never mints.** `cancel_redeem` sends the escrow back, so supply is unchanged
and the holder is whole without anyone's cooperation. This is why the bond is *not* needed here —
the escrow return already covers it, and paying both would compensate twice.

**Consent is what makes the liability real.** `accept_redeem` is the relayer's signature accepting
`owed_R`; without it a third party could occupy an innocent relayer's capacity.

**Deadlines are in slots**, so a Solana halt freezes the clock rather than burning the relayer.

---

## What is permissionless, and why

`release_mint`, `burn_staged` and `cancel_redeem` are **callable by anyone**. Each names an item
whose authority is already on-chain, and each does one thing the program fully verifies. Making
them permissionless means **no party's cooperation is ever required to resolve a pending item** —
the holder can always act, and anyone can act on their behalf. The caller gets the closed account's
rent back, which is enough incentive for a chore worth ~0.001 SOL.

## Rent

Every pending item is one PDA — about 0.002 SOL, **refunded when it closes**. Nothing here is a
list, so there is no ceiling and no pruning.

## Open items this design exposes

1. **`MATURITY` length.** Must be under 48 hours. Unset. It trades user latency against the window
   of detection, and it is the parameter P3's deadline depends on.
2. **The unbacked path (P8).** D6 allows a peg-in with no underwriter. It should be an explicit,
   visible mode rather than the default, and this design does not yet mark which deposits used it.
3. **`owed_R` must include staged mints (P10).** A depositor whose mint is still in the vault has
   paid BSV and holds no tokens, so its `PendingMint` must count toward the relayer's liability or
   that window is unbonded.
4. **The vault is a single point of accounting.** Every instruction moves exactly what its record
   says, but nothing independently verifies that the vault's balance equals the sum of pending
   items. Worth a test that opens and closes many items and asserts the balance returns to zero.
5. **Who pays the vault account's own rent**, and what happens to it at shutdown.
