# 14. Decision register

**Read this first.** [`12. Peg-in and peg-out: the mechanism`](12-peg-mechanism.md) holds the
full reasoning and the audit findings. This page is the short version: the flows as they
stand, what is settled, and every decision still open — in one place, so it can be reviewed
with fresh eyes. **Nothing is settled unless its Status says so.**

---

## The flows

**As designed** — the vault and everything downstream of it are not built; the shipped
program mints straight to the depositor. Both directions have the same shape: **enter the
program's vault, then leave it either to the counterparty or back to the sender.** No failure
path mints; every one returns.

### Peg-in — BSV → solBSV

```
1  CHOOSE   the depositor takes terms from the book
2  SEND     BSV to a relayer's script; OP_RETURN carries the Solana address
3  DEPTH    wait the depth the bid named, measured in block time
4  MINT     solBSV issued into the program's vault — not to the depositor
5  MATURE   no reorg followed -> the vault releases, fee to the stakers
            reorg followed    -> the staged tokens are burned. Nobody loses
```

### Peg-out — solBSV → BSV

```
1  ESCROW    solBSV moves into the vault; a BSV destination is named
2  ACCEPT    a relayer whose bond covers it takes the request
3  DEADLINE  measured in slots, so a Solana halt freezes the clock
4  PAY       the relayer pays BSV and proves it against the light client
5  CHALLENGE a reorged payout is caught here
6  SETTLE    burn the escrow and pay the fee
             or on failure, return the escrow. Supply never changes
```

---

## What is settled

| | |
|---|---|
| **No oracles** | The program reacts only to BSV headers and Solana slots. External metrics — price, hashrate, reorg cost — are published on the website and never consulted by the program |
| **No operators** | A relayer is a role anyone may run, not a privileged party. Minting has no trusted participant at all |
| **Reversibility without a freeze authority** | The vault is program-owned, so staged tokens can be burned or returned. No freeze authority, no Token-2022 hooks |
| **Time is chain-native** | BSV depth and block time from headers; Solana deadlines from slots |
| **Fees mature with the principal** | A fee withdrawable earlier than its mint would be an exit from maturity |
| **Same-asset yield** | BSV stakers earn BSV; `solBSV` stakers earn `solBSV` |
| **Do not pool the reserve** | Aggregation is what creates a single key worth stealing. Per-relayer deposits mean no reserve contract to write, audit or trust |
| **Confirmed by test** | 20 on-chain tests, 21/21 live SV Node checks, all Python checkers, on the local validator |

---

## Decisions

All eight are settled for the PoC. Each records what was decided, and separately what it
defers — because several of these are **decisions to defer**, which is different from leaving
a question open.

### D1 — Who may stake — **specialists first; anyone-may-stake is a phase-2 goal**

Specialists only at launch, with an open-staking phase to follow. **The upgrade path is a
deliverable, not a maybe** — the design must carry it from the start rather than have it
bolted on. What that path looks like is deferred to final implementation.

### D2 — Filling — **auto-approve, settled for the PoC**

Bids fill automatically. The reasoning is in the section below: the vault means a reorged fill
reverses the liability and the staker loses nothing, so the loss is systemic rather than
per-fill and there is little for a per-fill approval to inspect. **To be reviewed against the
finished system** — if a last look turns out to be cheap insurance, it can be added then.

### D3 — Genesis — **G2, the vault-gated genesis mint**

The genesis mint lands in the program vault and is released only once a matching BSV deposit
is verified. No unbacked window exists at any point, so there is nothing to attack and nothing
to keep quiet about.

### D4 — `FLOOR` — **12 blocks, fixed in code**

`FLOOR` is the minimum confirmation depth. Depositors and bids may commit to *more*, never
less. Twelve blocks for the PoC. The change mechanism is deferred; see the note below.

*Depth and maturity are different parameters, which is why `FLOOR` survives the book:* depth
sets **the cost of attacking** — a reorg must out-mine it — while maturity sets **the time
available to detect**. A low floor makes attacks cheap and therefore frequent, which raises
the number of chances for a detection failure to slip through.

### D5 — Bond multiple — **`k = 1`, and self-dealing stakers are accepted**

`bond_R ≥ k × owed_R` (with `k = 1` here), and a staker underwriting its own deposit is an
accepted risk rather than a prohibited one. The consequence is recorded plainly: at `k = 1` a
self-dealing attack is roughly break-even, so what makes it unprofitable is **the mining cost of
the reorg**, not the bond. The bond's job is covering an honest relayer's shortfall.

### D6 — A peg-in with no underwriter — **allowed, explicitly**

A peg-in may proceed with no underwriter at all. Whoever does so **accepts the initial risk of
a system with nobody watching while liquidity is seeded**, and may keep topping up on those
terms. This is a deliberate, stated risk acceptance rather than an oversight.

*For final implementation, not the PoC:* the note should be made that this can be bounded if
it proves necessary — for example expiring it after `n + 1000` blocks, or restricting it to a
designated initial LP address. Neither is needed now; **both need writing down so the option
is not lost.**

### D7 — Governance — **none in the PoC**

The PoC is built without any governance mechanism. Details are to be figured out on review
once the system is better understood and demonstrably working. Recorded so that "we launched
without governance" is a decision rather than an omission, and so the upgrade path stays a
named gap.

### D8 — The reserve invariant — **monitored, not enforced**

`custodied BSV ≥ outstanding solBSV` is published and monitored, and **the protocol cannot
enforce it** — the reserve is off-chain BSV the program cannot read. The website shows the
ratio; the program does not check it.

---

## Deferred to final implementation

These are named so they cannot be quietly forgotten. None blocks the PoC.

| Deferred | From | Note |
|---|---|---|
| The open-staking upgrade path | D1 | Anyone-may-stake is a phase-2 goal; the design must carry it from the start |
| Bounding the unbacked peg-in | D6 | Block-height expiry, or a designated initial LP address |
| The change mechanism for `FLOOR` | D4 | Voting, or the stakers. Fixed in code for now |
| Governance generally | D7 | Absent by decision, not by accident |
| The program upgrade authority | A5 | Can override every parameter. The fix is governance, possibly tied to staking |
| An independent audit | — | The critical defects found so far were found by our own adversarial review |

---

## D2 in full — why auto-approve

**An earlier draft of this document called a book of auto-filling bids "a book of sitting
ducks". That framing was wrong**, and the correction is worth keeping because it is the
reason the decision went the way it did.

The argument was that a miner fills a passive bid, reorgs, and the staker eats the loss. But
**the vault changes who bears it**: the mint is staged, not liquid, so if the reorg is detected
the staged tokens are burned, the liability is reversed, and **the staker loses nothing.** The
staker is only harmed when detection *fails*, which is a property of the system rather than of
any individual fill.

So the loss is **systemic, not per-fill** — and a toxic deposit is indistinguishable from an
honest one, so a last look has little to inspect. The levers that actually matter are
**maturity length**, **depth** (which keeps the attack rate down), and **the incentive to push
the honest chain**. Auto-approve is the simple answer at PoC stage, to be revisited against
the finished system.

## Not settled, and deliberately out of scope for the PoC

| | |
|---|---|
| **Upgrade authority** (A5) | It can override every parameter, which makes it an unconditional mint voucher. The fix is governance — a vote, or the stakers, possibly with additional tokens granting that right. Recorded so it is not silently forgotten |
| **Independent audit** | The critical defects found so far were found by our own adversarial review, which is not the same as an audit by someone with no stake in the answer |

## Where to read more

- [`12-peg-mechanism.md`](12-peg-mechanism.md) — full reasoning, scenarios, audit findings A1–A18
- [`04-trust-model.md`](04-trust-model.md) — what is trusted, and the roadmap to a signerless reserve
- [`05-relayers.md`](05-relayers.md) — the relayer role and bond custody
