# 13. Decision register

**Read this first.** [`12. Peg-in and peg-out: the mechanism`](12-peg-mechanism.md) holds the
full reasoning and the audit findings. This page is the short version: the flows as they
stand, what is settled, and every decision still open — in one place, so it can be reviewed
with fresh eyes. **Nothing is settled unless its Status says so.**

---

## The flows

Both directions have the same shape: **enter the program's vault, then leave it either to
the counterparty or back to the sender.** No failure path mints; every one returns.

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
| **Confirmed by test** | 17 on-chain tests, 21/21 live SV Node checks, all Python checkers, on the local validator |

---

## Decisions open

### D1 — Who may stake: specialists, or delegated retail?

| Option | Pros | Cons |
|---|---|---|
| Specialists only, minimum stake either side | Sophisticated parties who can price reorg and custody risk | Thin liquidity at launch; excludes everyone else |
| **Delegated staking** — retail pledges to a specialist operator | Deep liquidity; a real product for exchanges and miners, with rewards for retail | Retail cannot assess operator risk, so slashing lands on people who could not evaluate it |

Not exclusive: specialists first, delegation once the mechanics are proven. **Status: open.**

### D2 — Can a staker's bid fill automatically?

**No, as it stands.** A bid that always fills is farmed by a miner who deposits and then
reorgs their own block away — a book of auto-filling bids is a book of sitting ducks.

| Option | Consequence |
|---|---|
| **Last-look window** | Firm, but refusable briefly. Standard practice for this adverse selection |
| Explicit approval per request | Safer, but adds latency and a censorship point |
| Auto-approve, risk priced in | Does not survive a mining attacker |

**Status: open. The one most likely to change the shape of the book.**

### D3 — How does the system start? There is a circularity.

```
a seizable bond must be solBSV
solBSV exists only if someone minted it
a mint is only safe if a bond already exists
```

| # | Option | Assumption it carries |
|---|---|---|
| G1 | **Self-underwritten genesis** — the team is the first relayer and mints against its own BSV | The same one `initialize` already makes. No new trust |
| G2 | **Vault-gated genesis mint** — released only once a matching BSV deposit verifies | None. No unbacked window exists at any point |
| G3 | Genesis bond in SOL, migrated to `solBSV` later | A price mismatch, tolerable only briefly |
| G4 | Capped unbacked genesis — the first `X` BSV needs no underwriting | The cap is below what anyone would attack |
| G5 | Slot-expiring authority — a named key seeds until a slot, then is dead | A short privileged window |
| G6 | Compile-time test mint (`#[cfg(feature = "poc")]`) | Test only. Not a runtime flag, which could leak |

**Recommended: G2, optionally with G1.** **Status: open.**

### D4 — Is `FLOOR` still needed?

Once depth is a term of each bid, `FLOOR` is a backstop rather than a price. Keeping it low
enough never to bind, high enough to catch a bid nobody should accept, is the current
recommendation. Dropping it is defensible if the book is the only way in. **Status: open.**

### D5 — The bond multiple `k`

`bond ≥ k × owed`. `k = 1` covers principal; more covers the case where the bond is worth
less exactly when called upon. **Status: open.**

### D6 — Does the unbacked path exist at all?

An unseeded book is safe against accident but **not** against attack: the attacker in a
self-reorg *is* the depositor, so a detection failure dilutes every holder. Either exclude
the path until the book is seeded, or accept the tail explicitly. **Status: open.**

### D7 — Votable, or increase-only?

The one place the two positions in the documentation genuinely differ. Whoever can vote
`FLOOR` toward zero holds a mint voucher, so the proposal is **increase-only**: a vote may
make the system more conservative, and making it less conservative means shipping a new
program. **Status: open.**

### D8 — The reserve invariant

`custodied BSV ≥ outstanding solBSV`. The protocol cannot check it — the reserve is
off-chain — so is it a target with an explicit failure mode, or does it not need stating at
all? **Status: open.**

---

## Not settled, and deliberately out of scope for the PoC

| | |
|---|---|
| **Upgrade authority** (A5) | It can override every parameter, which makes it an unconditional mint voucher. The fix is governance — a vote, or the stakers, possibly with additional tokens granting that right. Recorded so it is not silently forgotten |
| **Independent audit** | The four critical defects found so far were found by our own adversarial review, which is not the same as an audit by someone with no stake in the answer |

## Where to read more

- [`12-peg-mechanism.md`](12-peg-mechanism.md) — full reasoning, scenarios, audit findings A1–A19
- [`04-trust-model.md`](04-trust-model.md) — what is trusted, and the roadmap to a signerless reserve
- [`05-relayers.md`](05-relayers.md) — the relayer role and bond custody
