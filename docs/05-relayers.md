# 5. Relayers

## What a relayer actually is

A relayer is **software with a BSV key and a bond lodged on Solana**. Not a person, not a company, not a committee — it is a *role*, like "delivery driver". Anyone can run one. The first will be run by the SOLBEAM team; the role then opens to the public through the desktop and mobile apps.

The job: when someone burns `solBSV` and asks for real BSV, a relayer **pays them from the hot wallet and then proves to Solana that it paid**. It earns a fee for that. To be allowed to hold the hot key at all, it lodges a **bond in `solBSV`** — collateral the Solana program can seize, locked for as long as it holds the role.

The analogy: a driver leaves a cash deposit at the depot. Deliver the parcel, get paid. Pocket the parcel, lose the deposit and the customer is refunded. **The deposit must be bigger than the parcel**, and it is posted in the same currency as the parcel, so no exchange-rate move can shrink it. That is why no relayer is ever trusted with more than it has at risk — and why anyone can compete for the work.

## The loop

| # | Step | Who checks it |
|---|---|---|
| 1 | Someone burns `solBSV` and names a BSV address; the program records a redemption with a 6-hour deadline | The program — this is native Solana state |
| 2 | A relayer reads the request (open race) or calls `accept` to claim it (commitment model) | The program |
| 3 | The relayer signs a BSV transaction from the **hot wallet** to the named address and broadcasts it | Nobody yet — it is spending its own key |
| 4 | It waits for confirmations, then obtains the header and Merkle branch for the payout | The relayer (its own risk); the data source need not be trusted |
| 5 | It submits `fulfil(request_id, proof)` | **The Solana program verifies the proof** and checks amount and destination. This closes the redemption and settles the relayer |
| 6 | No valid payout proven before the deadline | The program **re-mints the holder** and, in the commitment model, **slashes the bond** |

## Who checks what

Two different kinds of checking are often confused:

- **The burn needs no proof.** It is native Solana state.
- **The payout needs a proof**, because it happens on BSV. The relayer supplies it and **the Solana program verifies it**.
- **Fraud proofs are a separate, adversarial path.** They are not part of paying anyone; they exist to punish a relayer who moved reserve funds improperly.

**Do other people have to check proofs? No.** The program checks every payout proof deterministically. People matter only for catching **theft of the hot float**: anyone may submit the offending BSV transaction with a proof, the program verifies it against outstanding redemptions, and an unmatched spend slashes the bond. That bounty is permissionless and rewarded — an open market for vigilance, not a committee.

The distinction that matters: a theft **attached to a redemption** reports itself when the deadline passes. A **naked spend** — float taken with no redemption outstanding — reports nothing, because there is no deadline to miss. So the challenger is optional in the first case and is the *entire* enforcement mechanism in the second. See [Trust model](04-trust-model.md#the-naked-option-attack).

## What a relayer needs — and does not need

| Needs | Does not need |
|---|---|
| A BSV key (hot wallet) and a way to broadcast | A BSV full node |
| A Solana RPC to watch burns and submit `fulfil` | Any Solana infrastructure |
| Chain data for the Merkle proof (own SPV or public API) | To trust that source — Solana verifies the proof |
| A `solBSV` bond, locked on Solana, plus a small BSV gas float in the hot wallet | A large *unbonded* BSV inventory of its own |

## Economics

- **Revenue:** a percentage fee on each fulfilled redemption, plus a share of slashed bonds when acting as a challenger.
- **Costs:** BSV transaction fees (tiny), Solana transaction fees, and the opportunity cost of the bond — which is the dominant cost, because the bond is `solBSV` and cannot be redeemed while it is bonded.
- **Risk:** the bond, if it cheats or fails to deliver after accepting a job.
- **Caps:** each relayer publishes its own limits — maximum concurrent redemption size, total float, and hours of operation.

The fee is the dial that makes the role worth running. At the recommended `k = 5` a relayer locks five times the float it serves, so the fee has to clear the cost of that locked capital, not just gas. If the fee is too low, nobody fulfils and redemptions fall through to the automatic refund path — holders are still made whole, but slowly. Governance sets a fee that keeps the role attractive.

## Two fulfilment models

| | Open race | Accepted job |
|---|---|---|
| Who fulfils | Anyone, first to prove wins | A relayer that claimed the request |
| If nobody fulfils | Holder is auto-refunded | Holder is auto-refunded **and** the bond is slashed |
| Bond covers | Theft of the float | Theft **and** liveness failure |
| UX | Simple, no coordination | Stronger delivery commitment |

SOLBEAM supports both; the commitment model can be enabled per-redemption or globally by governance.

### What neither model covers

Both models handle theft that is attached to a redemption, because the deadline does the reporting. Neither handles a **naked spend**, since there is no deadline to miss. That case is bounded by the float cap and enforced only by the challenger bounty — which is exactly why the float should never be idle and why the hot wallet should require a veto-only cosigner.

## Bond custody and unbonding

The bond is `solBSV` held by the program, not a balance the operator can move. Two rules make it a bond rather than a promise:

- **Locked while bonded.** Bonded `solBSV` cannot be redeemed. A relayer's capital is genuinely at risk for as long as it holds the hot key — which is why the role demands real capital and why, at `k = 5`, a relayer ties up several times the float it serves.
- **A notice period before release.** Exit is two steps: announce, then wait out the **unbonding period** — longer than the redemption deadline plus the challenge window — during which outstanding commitments must settle and challenges may still land. Without it, a relayer could take a job, withdraw the bond, and be gone before anyone could slash. **A bond that can be withdrawn instantly is not a bond.**

The cost is honest and accepted: **entry is fast, exit is slow.** Minting is trustless and completes in about two hours; redemption and bond release are rate-limited by design. See [Parameters & governance](06-parameters.md).

## Apps: desktop and mobile

The public relayer app is planned as two builds:

- **Desktop (recommended for production):** an always-on service with a KMS/HSM-backed hot key (ideally a 2-of-2 veto-only cosigner), automatic fulfilment, float and bond monitoring, and alerting. This is what a serious relayer should run.
- **Mobile (participation and light relaying):** a phone app for monitoring, claiming, and fulfilling when online. Deliberately conservative: smaller caps, longer acceptance windows, and clear warnings — because phones suspend background work and the 6-hour deadline does not care.

Both apps share one engine; the difference is the operating envelope and the key storage.

## Watchtowers and challengers

A separate role — possibly the same operator — that watches for unauthorised reserve spends and submits proofs, earning a share of the slashed bond. It needs only gas money. The first will be run by the SOLBEAM team; anyone can run one thereafter.

This role is **load-bearing, not decorative**. When a theft is attached to a redemption the deadline does the work and the challenger is optional; for a naked spend the challenger *is* the enforcement. Two consequences follow: a watcher should run from day one, and **the bounty must not be removable by governance** — a bounty that can be voted away is a bounty an attacker can simply wait out. The design does not depend on the watcher being effective (the float cap bounds the loss and the bond covers it), but the watcher is what turns "covered" into "punished".

## Becoming a relayer

1. Install the app (desktop or mobile).
2. Create or import a BSV hot wallet; fund it with a small gas float.
3. Lock a bond in `solBSV` on Solana — sized at `k × (float + tranche)`, not by what you expect to earn.
4. Publish your limits (max size, float, schedule).
5. Fulfil redemptions. Earn fees.
6. To exit: stop accepting, settle outstanding commitments, then wait out the unbonding period.

The role is open. There is no application to a committee, no whitelist, and no permission required.

---

Next: [Parameters & governance](06-parameters.md)
