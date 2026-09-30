# 11. Peg-out — spec and default parameters

**Status: built (PoC), except step 4.** `initiate_redeem`, `cancel_redeem`, `claim_payout` and
`settle_redeem` are in the program, and the `po.*` defaults that were `open` are fixed and generated
into it from `config/params.json`. **Membership** — who is obliged to pay — is still not built, so
every redemption times out into a cancellation until a federation exists. `po.deadline` ships as a
stored, timelock-mutable `Config` field defaulting to 216,000 slots, so the deadline is policy rather
than a constant.

**Why this matters more than any other remaining gap:** the bridge is currently **one-way**. Solana
verifies BSV, so a peg-in is trustless. **A peg-out relies on a federation member paying BSV**, and
nothing in the built program does that or settles it. Until this exists there is no exit, and *"the
floor is the exit"* is an argument about a mechanism that does not exist.

---

## 1. The flow

```
1  INITIATE     the holder escrows solBSV and names a BSV address
                       ↓
2  PAYOUT       a federation member pays BSV from the reserve to that address
                       ↓
3  CLAIM        anyone proves the payment on-chain (Merkle inclusion + script + depth)
                       ↓
4  CHALLENGE    a window in which the payout can be shown to have been reorged
                       ↓
5  SETTLE       the escrowed solBSV is BURNED
                       ↓
   or, if no payout happens before the deadline:
   CANCEL       the holder takes their solBSV back, unchanged
```

**Steps 3–5 are the vault's shape applied to the other direction** — stage, wait, then resolve
against the chain's own record. The same reasoning that makes a mint reversible makes a redemption
settleable.

## 2. Why escrow and not an immediate burn

**A burn is final, and the payout might never happen.** If the holder's tokens were burned at step 1
and the member then failed to pay, there would be nothing to return — the holder would have lost
their claim on the reserve and hold nothing.

**So the tokens are escrowed, and burned only once the BSV has demonstrably left the reserve.** That
keeps the invariant `supply ≤ reserve` true at every point in between: the escrowed tokens still
exist, and the BSV has not yet left.

## 3. Why the fee works out, and where it physically lands

The holder redeems amount `A` and receives `A − fee` in BSV. The escrowed `A` is burned.

- **reserve** falls by `A − fee`
- **supply** falls by `A`
- so the **reserve-to-supply ratio improves by `fee`** — the members keep it, in BSV, inside the
  reserve they already hold

**That is the whole fee mechanism, and it needs no extra account.** The fee is not transferred to
anyone; it is simply BSV the reserve did not have to pay out.

## 4. Default parameters

**These are defaults for the PoC, marked so they can be changed in `config/params.json` without
touching code.** Where a value is a guess rather than a derivation it is marked `ph`.

| ID | Value | Status | Why |
|---|---|---|---|
| `po.payout_confirmations` | **6 BSV blocks** (~1 hour) | `ph` | How deep the payout must be before it can be claimed. Lower than the mint's 12 because the *burn* still faces a challenge window, so depth here is not the only protection |
| `po.challenge_window` | **144 BSV blocks** (~24 hours) | `ph` | The window in which a claim can be shown to have been reorged. **Matches `v.maturity_blocks`' designed value deliberately** — both are "how long before we believe the chain" |
| `po.deadline` | **216,000 Solana slots** (~24 hours) | `ph` | How long a member has to pay before the holder may cancel. In **slots**, because a deadline that cannot advance is not a deadline — and if the header feed stalls, a BSV-height deadline would never expire, freezing the holder's funds |
| `po.cancel_grace` | **0** | `dec` | Cancellation is immediate on expiry. A grace period would only delay the exit |
| `fee.redeem_bp` | **30 bp** | `dec` | The peg-out fee, matching the mint side |
| `po.d_min` | **0.01 BSV** | `ph` | Minimum redemption, so a claim's proof is worth the transaction fees to settle |
| `po.max_pending` | **64** | `ph` | Cap on concurrent pending redemptions, so the escrow cannot be used to make the program's per-instruction work unbounded |

## 5. What the claim must prove

The claim is an SPV proof, verified by the **existing** light client — no new trust and no oracle:

1. the named BSV address was **paid** in a block the client accepts
2. the amount is `≥ A − fee`
3. the payment block is at least `po.payout_confirmations` **deep**
4. and the client ratifies it after `po.challenge_window` before the burn executes

**Anyone may claim.** That matters: the holder does not depend on the member who owes them, and a
third party with the proof can settle it. The member is paid by the reserve off-chain, as they
already are.

## 6. What this does NOT solve

- **The payout is made by a member, not by the program.** The program can verify that BSV moved; it
  cannot move BSV. **So a peg-out requires a member to act, and a member who does not act delays the
  holder** — bounded by `po.deadline`, after which the holder cancels and is made whole. The
  holder is never *stuck*; they are only delayed.
- **`po.deadline` in slots means a stalled header feed does not freeze cancellation.** That is
  deliberate, and it is the one place the design prefers a Solana clock over a BSV height.
- **This is the first thing that depends on members existing.** Until there is a federation, nobody
  is obliged to pay, and every redemption would time out into cancellation. **That is a
  configuration, not a defect** — but it means peg-out cannot be demonstrated end to end until
  there is at least one member.

## 7. Build order

1. `initiate_redeem` / `cancel_redeem` — escrow and its return. **Fully specified, no dependency.**
2. `claim_payout` — the SPV proof and the depth check. **Fully specified.**
3. The challenge window and the burn. **Fully specified.**
4. **Membership** — who is obliged to pay. **Not specified, and it is the real blocker.**

**Steps 1–3 can be built and tested with the program acting as its own counterparty** — a test can
pay a synthetic BSV transaction and prove it through the light client, exactly as the burn test
already does for a reorg. **Only step 4 needs the federation.**
