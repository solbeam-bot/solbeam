# 5. Relayers

> **Built or designed?** The light client, the token, the mint and fork staging exist and pass 17
> on-chain tests. **The vault, the two gates, maturity, the order book, staking, bonds, `owed_R`,
> consent, per-relayer deposit scripts, `FLOOR` as a distinct parameter and all of peg-out are
> designed and not built.** DAA is **actively rejected in the built client** (F7), not merely
> absent. No relayer, bid or bond described below exists in code yet; the shipped program mints
> straight to the depositor's token account. Read this as a specification.

## What a relayer actually is

A relayer is **software with a BSV key and a bond lodged on Solana**. Not a person, not a
company, not a committee — it is a *role*, like "delivery driver". It has two halves of one job:
on the way in it **receives BSV**, and on the way out it **pays BSV and proves that it paid**.
It earns a fee for both.

**There is no pooled reserve.** A depositor sends BSV to **an individual relayer's own script**,
not to a bridge address — so there is no single key whose theft drains the system, and no
reserve contract to write, audit or trust. The proof of the deposit tells the program which
relayer received it, and the program **accumulates a per-relayer liability `owed_R` from the
proofs it verified itself**. That liability is a chain fact, not an attestation by a custodian.

The analogy: a driver leaves a deposit at the depot and carries one parcel. Deliver it, get
paid. Pocket it, lose the deposit and the customer is made whole. The deposit is posted in the
same currency as the parcel, so no exchange-rate move can shrink it — and it is sized against
what the driver has actually been entrusted with, which the depot can measure for itself.

## Who may run one

The role is open by design: **no committee approves a relayer, there is no whitelist, and the
bond is the only qualification.** But at launch the design has **specialists only** (D1).
Pricing reorg risk and holding BSV custody are real operational work, so the first stakers are
sophisticated parties rather than the general public; the entry bar is capital and competence,
not permission. **Anyone-may-stake is a phase-2 goal and the upgrade path is a deliverable, not
a maybe** — the design must carry it from the start rather than have it bolted on. What that
path looks like is deferred to final implementation.

The first relayer is the SOLBEAM team, and genesis is why: nothing can mint until a bond
exists, and a bond has nothing to earn from until mints happen. The genesis path — G2, the
vault-gated genesis mint (D3) — seats the first relayer and its bond, after which the ordinary
path takes over.

## The loop

### Peg-out — `solBSV` to BSV

| # | Step | Who checks it |
|---|---|---|
| 1 | A holder escrows `solBSV` **into the vault** and names a BSV destination | The program — native Solana state |
| 2 | A relayer whose bond covers the amount **accepts** the request | The program checks capacity: `bond_R ≥ k × owed_R` |
| 3 | The relayer pays BSV **from its own funds** and broadcasts | Nobody yet — it is spending its own key |
| 4 | It waits for confirmations, then obtains the header and Merkle branch for the payout | The relayer (its own risk); the data source need not be trusted |
| 5 | It submits `fulfil(request_id, proof)` | **The Solana program verifies the proof** against the light client and checks amount and destination. This closes the redemption and settles the fee |
| 6 | A challenge window passes with the payout still canonical | The program, from BSV headers. A payout later reorged away is caught here |
| 7 | No valid payout proven before the deadline — measured in slots, so a Solana halt freezes the clock | The escrow is **returned to the holder**, which already makes them whole; nothing mints, supply is unchanged, and the bond is **not** additionally transferred. The bond answers `owed_R` — theft or abandonment of what the relayer owes — not a failed redemption |

### Peg-in — BSV to `solBSV`

| # | Step | Who checks it |
|---|---|---|
| 1 | The depositor takes terms from the book and sends BSV to **that relayer's script**, with an `OP_RETURN` naming their Solana address | The program, once the proof is submitted |
| 2 | After the confirmation depth the bid named, `solBSV` is **minted into the vault** — never to the depositor | **The program verifies the proof** and credits `owed_R` to that relayer |
| 3 | Maturity passes with the deposit still canonical → the vault releases, fee to the staker. A reorg followed in the meantime → the staged tokens are **burned** | The program, from BSV headers. The depositor's BSV was reorged away with it, so they end where they started |

## Consent — what makes slashing defensible

A mint naming relayer `R` needs **`R`'s signature accepting the liability**. Without it the gate
is unfair in a way that breaks it: a fraudulent deposit could name any relayer's script, and the
program would credit `owed_R` and seize a bond for a risk that operator never agreed to and
could not have detected.

Consent converts reorg risk from an externality into a **priced term** — the relayer knows what
it is underwriting and charges for it — and it is what makes slashing defensible rather than
arbitrary. It is also what makes the capacity gate meaningful: you cannot check a relayer's
capacity if you never asked whether it was willing.

## Who checks what

Three different kinds of checking are often confused:

- **The burn needs no proof.** It is native Solana state.
- **The deposit and the payout need proofs**, because they happen on BSV. The relayer supplies
  them and **the Solana program verifies them**.
- **Do other people have to check proofs? No.** The program checks every proof
  deterministically, and `owed_R` is derived from those checks rather than attested.

The old worry — a **naked spend** of a shared float that no deadline reports — was a property of
a pooled reserve. With deposits paying individual relayers there is no pooled object to police: a
relayer that spends BSV it received is failing its own `owed_R`, which the program measures and
the bond secures. The relayer's own idle float is a different thing and is **not** covered by the
bond — it is the relayer's own money, off-chain and unreadable by the program (F4). The reasoning
is in [Trust model](04-trust-model.md#the-naked-option-attack); what changed is the *object* it
bounds, not the logic. Detection still matters systemically, because an undetected reorg eats the
staking buffer.

## What a relayer needs — and does not need

| Needs | Does not need |
|---|---|
| A BSV key and script of its own, and a way to broadcast | A BSV full node |
| A Solana RPC to watch deposits and redemptions and submit proofs | Any Solana infrastructure beyond that |
| Chain data for the Merkle proof (own SPV or public API) | To trust that source — Solana verifies the proof |
| A seizable `solBSV` bond, consent to the liability it accepts, and enough BSV to pay what it owes | A pooled reserve, a shared cosigner, or a large *unbonded* inventory |

## Economics

- **Revenue:** the fee on its bid, paid **in the asset staked** — BSV for a BSV-side staker,
  `solBSV` for a `solBSV`-side one. No cross-asset conversion, and nobody has to pay anybody out.
- **Costs:** BSV transaction fees (tiny), Solana transaction fees, and the opportunity cost of
  the bond — the dominant cost, because the bond is `solBSV` and cannot be redeemed while it is
  bonded.
- **Risk:** the bond, if it absconds with BSV it owes. That liability is what the bond secures;
  a failed redemption is handled by returning the escrow, not by a bond transfer, and the
  relayer's own float is not bonded.
- **Caps are terms, not settings:** the liquidity, fee and depth in its bid are exactly how much
  it will underwrite and on what terms.

The bond is `bond_R ≥ k × owed_R`, with **`k = 1`** (D5). At `k = 1` it covers the principal and
no more, and a self-dealing attack is roughly break-even — what makes it unprofitable is **the
mining cost of the reorg**, not the bond. The bond's job is covering an honest relayer's
shortfall. So the fee is the dial that makes the role worth running: it must clear the cost of
the locked capital, not just gas. If it is too low the bids go unfilled and redemptions fall
through to the return path — holders are still made whole, but slowly.

## Underwriting is an order book

The fee is not a percentage set by governance. Underwriting is **an order book** (see
[The book: staked bids as an order book](12-peg-mechanism.md#the-book-staked-bids-as-an-order-book)).
Stakers post bids — how much liquidity, at what fee, at what confirmation depth — and incoming
requests match them **by price, then by time**, partially filled, with unused stake returned to
the staker's own address.

Three consequences follow, and they are the reason the book replaced a fixed fee:

- **The fee is discovered rather than chosen.** There is no parameter to argue about.
- **Depth is a term of the bid**, so the market prices reorg risk instead of a committee guessing
  at it. `FLOOR` remains as a backstop rather than a price (D4).
- **The depositor sees the liquidity and the price before committing** — the disclosure the
  website was always going to have to provide.

A book with no bids is a coherent state rather than a broken one, which is also what dissolves
the bootstrap problem.

## Auto-approve — and the "sitting ducks" claim, corrected

An earlier draft argued that a bid which fills automatically is a limit order facing informed
flow, and called a book of them **"a book of sitting ducks."** *That framing was wrong, and the
settled position is auto-approve* (D2).

**The vault changes who bears the loss.** A fill is not liquid: the mint is staged, so if a miner
deposits, gets underwritten and then reorgs their own block away, a detected reorg means the
staged tokens are burned, the liability is reversed, and **the staker loses nothing**. The staker
is harmed only when detection *fails*, which is a property of the system rather than of any
individual fill. So the loss is **systemic, not per-fill** — and a toxic deposit is
indistinguishable from an honest one, so a last look has little to inspect. The levers that
matter are **maturity length**, **depth**, and **the incentive to push the honest chain**.

Auto-approve is the answer for the PoC, to be revisited against the finished system: if a
last-look window turns out to be cheap insurance, it can be added then.

## Bond custody and unbonding

The bond is `solBSV` **held by the program**, not a balance the operator can move, and it is
sized against what the program itself measured: `bond_R ≥ k × owed_R`, with `k = 1` (D5). Two
rules make it a bond rather than a promise:

- **Locked while bonded.** Bonded `solBSV` cannot be redeemed. A mint naming a relayer is
  refused unless `bond_R ≥ k × (owed_R + this mint)`, so capacity is checked on-chain at the
  moment it is used.
- **A notice period before release.** Exit is two steps: announce, then wait out the
  **unbonding period** — longer than the redemption deadline plus the challenge window — during
  which outstanding commitments must settle and liabilities must clear. Without it, a relayer
  could take a job, withdraw the bond, and be gone before it could be slashed. **A bond that can
  be withdrawn instantly is not a bond.** Delayed unstaking is also what **keeps the buffer
  present when it is needed**: an undetected fraud eats the staking buffer, and if stakers could
  leave at will the first to notice would exit before it was confirmed.

The cost is honest and accepted: **entry is fast, exit is slow.** Binding in `solBSV` also means
the collateral is the same asset as the liability, so no exchange-rate move shrinks it — and
because the amount credited is derived from verified proofs, the inequality is checkable
on-chain with no oracle. See [Parameters & governance](06-parameters.md).

## Apps: desktop and mobile

The relayer app is planned as two builds:

- **Desktop (recommended for production):** an always-on service with a KMS/HSM-backed BSV key —
  or the operator's own P2SH multisig, its own signers, its own bond — automatic fulfilment, bond
  and liability monitoring, and alerting. This is what a serious relayer should run.
- **Mobile (participation and light relaying):** a phone app for monitoring, bidding and
  fulfilling when online. Deliberately conservative: smaller caps, longer windows, and clear
  warnings — because phones suspend background work and the deadline does not care.

Both share one engine; the difference is the operating envelope and the key storage.

## Watchtowers and challengers

The role used to exist to police a shared float. **There is no shared float now**, so what
remains is the part that always mattered: **advancing the honest chain**. A reorg is visible in
the BSV headers the client already stores; detection is permissionless and cheap; and it is
**incentivised** — a fraud that goes undetected eats the staking buffer, so the parties with the
most to lose have the most reason to push the honest headers and notice an orphan. Nobody has to
be appointed as a watcher; the economics do it. **The incentive is indirect only:** there is a
**bounty for challenging a bad payout**, and **no bounty for detecting a reorg**, where detection
is the whole defence (F3). See
[The system, in summary](13-summary.md#reorg-protection).

A watchtower is still a sensible thing for a relayer to run over its own book, but it is an
optimisation rather than the enforcement mechanism, and the design does not depend on it.

## Becoming a relayer

1. Install the app (desktop or mobile).
2. Register a BSV script of your own and fund it with the BSV you intend to pay out.
3. Post a bid: how much liquidity, at what fee, at what confirmation depth.
4. Lodge a seizable `solBSV` bond of `k × owed_R` — `k = 1`, sized by what you actually take on
   rather than by what you expect to earn.
5. Consent to each liability the program credits you with, then fulfil. Earn fees in the asset
   you staked.
6. To exit: stop bidding, settle outstanding commitments, then wait out the unbonding period.

Specialists at launch. **Staking open to anyone is the phase-2 goal**, and the design carries the
upgrade path from the start (D1).

---

Next: [Parameters & governance](06-parameters.md)
