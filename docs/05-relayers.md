# 5. Relayers

## What a relayer actually is

A relayer is **software with a BSV key and a bond lodged on Solana**. Not a person, not a company, not a committee — it is a *role*, like "delivery driver". Anyone can run one. The first will be run by the SOLBEAM team; the role then opens to the public through the desktop and mobile apps.

The job: when someone burns `solBSV` and asks for real BSV, a relayer **pays them from the hot wallet and then proves to Solana that it paid**. It earns a fee for that. To be allowed to hold the hot key at all, it lodges a **bond** — collateral the Solana program can seize.

The analogy: a driver leaves a cash deposit at the depot. Deliver the parcel, get paid. Pocket the parcel, lose the deposit and the customer is refunded. **The deposit must be bigger than the parcel**, which is why no relayer is ever trusted with more than it has at risk — and why anyone can compete for the work.

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

**Do other people have to check proofs? No.** The program checks every payout proof deterministically. Other people matter only for catching **theft of the hot float**: anyone may submit the offending BSV transaction with a proof, the program verifies it against outstanding redemptions, and an unmatched spend slashes the bond. That bounty is permissionless and rewarded — an open market for vigilance, not a committee.

## What a relayer needs — and does not need

| Needs | Does not need |
|---|---|
| A BSV key (hot wallet) and a way to broadcast | A BSV full node |
| A Solana RPC to watch burns and submit `fulfil` | Any Solana infrastructure |
| Chain data for the Merkle proof (own SPV or public API) | To trust that source — Solana verifies the proof |
| A bond (in a stable unit) and a little gas float | BSV inventory of its own |

## Economics

- **Revenue:** a percentage fee on each fulfilled redemption, plus a share of slashed bonds when acting as a challenger.
- **Costs:** BSV transaction fees (tiny), Solana transaction fees, and the opportunity cost of the bond.
- **Risk:** the bond, if it cheats or fails to deliver after accepting a job.
- **Caps:** each relayer publishes its own limits — maximum concurrent redemption size, total float, and hours of operation.

The fee is the dial that makes the role worth running. If it is too low, nobody fulfils and redemptions fall through to the automatic refund path — holders are still made whole, but slowly. Governance sets a fee that keeps the role attractive.

## Two fulfilment models

| | Open race | Accepted job |
|---|---|---|
| Who fulfils | Anyone, first to prove wins | A relayer that claimed the request |
| If nobody fulfils | Holder is auto-refunded | Holder is auto-refunded **and** the bond is slashed |
| Bond covers | Theft of the float | Theft **and** liveness failure |
| UX | Simple, no coordination | Stronger delivery commitment |

SOLBEAM supports both; the commitment model can be enabled per-redemption or globally by governance.

## Apps: desktop and mobile

The public relayer app is planned as two builds:

- **Desktop (recommended for production):** an always-on service with a KMS/HSM-backed hot key, automatic fulfilment, float and bond monitoring, and alerting. This is what a serious relayer should run.
- **Mobile (participation and light relaying):** a phone app for monitoring, claiming, and fulfilling when online. Deliberately conservative: smaller caps, longer acceptance windows, and clear warnings — because phones suspend background work and the 6-hour deadline does not care.

Both apps share one engine; the difference is the operating envelope and the key storage.

## Watchtowers and challengers

A separate role — possibly the same operator — that watches for unauthorised reserve spends and submits proofs, earning a share of the slashed bond. It needs only gas money. The first will be run by the SOLBEAM team; anyone can run one thereafter.

## Becoming a relayer

1. Install the app (desktop or mobile).
2. Create or import a BSV hot wallet; fund it with a small gas float.
3. Post a bond on Solana.
4. Publish your limits (max size, float, schedule).
5. Fulfil redemptions. Earn fees.

The role is open. There is no application to a committee, no whitelist, and no permission required.

---

Next: [Parameters & governance](06-parameters.md)
