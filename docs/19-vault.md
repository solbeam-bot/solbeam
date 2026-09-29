# 19. The vault — instruction set

The vault is the component the rest of the design rests on. Every mint lands in it, and every
redemption is escrowed in it. It is **one program-owned token account**, plus a small PDA per
pending item.

**Status: designed, not built.** The shipped program mints straight to the depositor.

---

> ⚠️ **SUPERSEDED BY [`21-vault-structural.md`](21-vault-structural.md).** This first draft was audited and found broken; the corrected design is in doc 21 (doc 20 was the intermediate revision, itself superseded).
> **Model note, later still:** the per-relayer bond formulas below (`bond_R >= k * owed_R`) are **superseded** by **two-sided bonds, neither inside the reserve** (D14), now stated as **the float**; and the reserve is a **2-of-2 `OP_CHECKMULTISIG`** — the gateway's threshold ECDSA key plus the **Greycore**'s — so the earlier "threshold key, not a threshold script" note is itself **superseded by the reversal of F10** (D15). The text is preserved as history.
> An adversarial review of the first draft found four critical defects, three of them inherent to
> the design as described rather than merely unspecified. They are listed in §Audit findings at the
> end and the design needs a revision pass before any code is written.

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

---

## Audit findings — the first draft is broken

An adversarial review found these. Severity by expected loss. **None were acknowledged by the
design**, and the first four are critical.

### V1 — The nullifier *is* the pending item, and release closes it · **critical, inherent**

**My error, and it defeats replay protection entirely.** Decision P5 made the nullifier a
per-deposit PDA. This design gave that same PDA a second job — the `PendingMint` record — and then
`release_mint` **closes it**. Close the item, and the replay entry is gone.

So: deposit, wait for maturity, release, PDA closed, **submit the identical claim again**, and it
verifies afresh because there is nothing left to say it was used. Repeat until the height leaves
the window — perhaps thirty to forty cycles for about $0.0004 each.

**Fix:** the nullifier must be a **separate account**, closed only by `burn_staged` or by a
permissionless prune once the height leaves the window. The pending item and the replay record
cannot be the same account. This also resolves the rent question in M1: the nullifier is what
outlives the item.

### V2 — `cancel_redeem` races `settle_redeem` · **critical, inherent**

A relayer broadcasts BSV but has not yet proved it, so the item is "unsettled" — and
`cancel_redeem` is legal the moment the deadline passes. The holder gets the escrow back; the
relayer's BSV is gone and its proof now has nothing to settle against.

**And the design's own deadlines make this certain rather than unlucky:** `D = 6 h` is *shorter*
than the challenge window `W = 24 h`, which doc 12 places *before* settlement. A relayer can never
both wait out the challenge and settle before cancel becomes callable.

**Fix:** a **paid/claimed state that blocks cancel** once a payout proof is submitted, and deadlines
that nest — `D ≥ W + C_payout`.

### V3 — The peg-in fee is never deducted · **critical, cross-document**

Doc 12 says the peg-in fee is taken in BSV, so a 100 BSV deposit mints 99.9. This design mints the
**full amount** into the vault and records `amount` on the item. Every release is then short by the
fee — and because the vault is commingled, it pays the shortfall out of other people's staged mints
and escrowed redemptions, silently and without reverting.

**Fix:** mint the **net** amount and store that; define the fee flow once, outside the vault.

### V4 — `settle_redeem` binds neither amount nor destination · **critical, unspecified**

Nothing compares the proof's output value or script against `PendingRedeem.amount` or
`bsv_destination`, and there is no `fee` field. So a relayer can accept a 1,000 `solBSV` redemption,
pay **1 satoshi**, prove it, settle, and keep the rest. There is no recourse — the escrow is burned
and the liability discharged.

**Fix:** verify `payout_value == amount − fee` and `payout_script == bsv_destination`, and store
`fee`.

### Also found

| | | |
|---|---|---|
| **V5** | The third canonical row releases an item whose height left the window — so the real detection budget is **`MATURITY`, not 48 hours**, and a Solana halt *switches every staged mint to unchecked release* rather than protecting anyone | serious |
| **V6** | `MATURITY` is stored as a Solana **slot** while canonicality is a BSV **height**. Comparing a slot count to a block count is meaningless; the predicate must be `confirmations + MATURITY_blocks ≤ WINDOW`, with slots used only for the halt property | serious |
| **V7** | `PendingMint` has **no relayer field**, so `owed_R` cannot be attributed, incremented or decremented — the unbonded window P10 names. And `verify_deposit` takes no consent, which doc 12 argues is needed or a fraudulent mint slashes an innocent relayer | serious |
| **V8** | **No slash instruction exists anywhere.** Four documents say the bond answers theft or abandonment; this instruction set implements the no-enforcement version | serious |
| **V9** | The domain separator's **encoding is undefined**. A substring test would let a payload commit to two separators at once and mint on both — the exact double-backing the fix exists to prevent. A program id is also identical across devnet and mainnet, so a cluster id is needed too | serious |
| **V10** | Rent is unclaimable (M1), bond custody is undefined and may share the vault (M2), permissionless resolution lets a third party burn a legitimate staged mint during any transient fork (M3), and `index_of` conflates "too old" with "too new" (M4) | minor |

### And the summary claims do not hold as written

**"Everything passes through the vault"** — true only for a subset. Bonds, the peg-in fee and the
settlement fee leave by paths the design never defines.

**"Nobody's cooperation is ever required"** — true only of *resolving an already-created item*. A
relayer's signature is required for consent, payment needs a relayer to broadcast BSV and prove it,
and detection needs someone unpaid to stage the honest fork.
