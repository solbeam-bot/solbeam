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

## Decisions

### D1 — Who may stake — **settled: specialists first, with an upgrade path**

Specialists only at launch. **Anyone may stake is a phase-2 goal**, so the design must carry
an upgrade path from the start rather than bolting one on. That path is not designed yet and
is recorded as a deliverable, not a maybe.

### D3 — Genesis — **settled: G2, the vault-gated genesis mint**

The genesis mint lands in the program vault and is released only once a matching BSV deposit
has been verified. No unbacked window exists at any point, so there is nothing to attack and
nothing to keep quiet about.

### D4 — `FLOOR` — **settled for the PoC: 12 blocks, fixed**

**The question was whether `FLOOR` is the depth a deposit waits. It is — the minimum
confirmation depth.** Depositors and bids may commit to *more*; never less. Twelve blocks is
the PoC value, fixed in code, with a change mechanism named as a gap and deliberately not
built yet.

**Does `FLOOR` still make sense once bids name their own depth? Yes, and it is worth being
precise about why, because it is a different parameter from maturity:**

| | Controls | Effect |
|---|---|---|
| **Depth** (bids, floored) | How deep a deposit must be before minting | **The cost of attacking.** A reorg must out-mine the depth |
| **Maturity** | How long a staged mint waits before release | **The time available to detect.** A longer window is more chance for the honest chain to be pushed |

They are not substitutes. Tuning depth changes how *expensive* an attack is; tuning maturity
changes how *likely* it is to be caught. A low floor makes attacks cheap and therefore
frequent, which raises the number of chances for a detection failure to slip through — so
the floor is what keeps the attack *rate* down, not what makes any single attack safe.

### D5 — Bond multiple — **settled: `k = 1`**

`bond ≥ owed`. Note the consequence honestly: at `k = 1` a **self-dealing** staker who
underwrites its own fraudulent deposit is roughly break-even, so what makes that attack
unprofitable is the mining cost of the reorg, not the bond. The bond's job at `k = 1` is
covering an honest relayer's shortfall, not punishing a determined one.

### D6 — The unbacked path — **clarified**

The genesis case is not a risk: before any BSV is in the reserve there is nothing to steal,
so an attacker gains nothing by minting early. That reasoning is sound.

**But D6 was asking about steady state, not genesis** — whether a peg-in may proceed with *no
staker willing to underwrite it* once the system is live. That is a different question and is
still open. The two cases need separating because the genesis answer ("nothing to steal yet")
does not carry over to a funded reserve.

### D7 — Votable or increase-only — **settled: it stays a gap**

Launch without a governance mechanism at all, provided an upgrade path exists, undefined for
now. Either direction is acceptable when it is built. Recorded so that "we launched without
governance" is a decision rather than an oversight.

### D8 — The reserve invariant — **settled: monitored, not enforced**

`custodied BSV ≥ outstanding solBSV` is **published and monitored, and the protocol cannot
fix it** — the reserve is off-chain BSV the program cannot read. The website shows the ratio;
the program does not check it. Stated plainly rather than left as an implied guarantee.

---

## D2 — Can a staker's bid fill automatically? — needs review

**This was summarised as "a bid that always fills is a bid of sitting ducks". Working it
through properly, that framing was wrong, and the corrected version is below.**

### Why "sitting ducks" overstated it

The original argument was that a miner fills a passive bid, reorgs, and the staker eats the
loss. But **the vault changes who bears it**: the attacker's mint is staged, not liquid. If
the reorg is detected, the staged tokens are burned, the liability is reversed, and **the
staker loses nothing at all.** The staker is only harmed when detection *fails* — and that is
not a property of any individual fill.

So the loss is **systemic, not per-fill.** It depends on whether somebody pushes the honest
chain during the maturity window, which a staker cannot assess by looking at a deposit. There
is little for a last look to inspect, because a toxic deposit is indistinguishable from an
honest one.

### What that implies

| Option | Assessment |
|---|---|
| **Auto-approve** ✅ | The per-fill check was never the protection. Defensible once depth and maturity carry the risk, and it keeps the book simple and fast |
| Last look | Still some value — it lets a staker decline when the *system* looks unhealthy, not when a deposit looks toxic — but the staleness gate below already covers that case |
| Explicit approval | Strongest on paper, slowest in practice, and it adds a censorship point for a risk it cannot actually judge |

### The lever that actually matters

If the risk is detection failure, then the parameters that matter are:

1. **Maturity length** — how long the attacker's tokens stay burnable;
2. **The incentive to push the honest chain** — permissionless and cheap, and stakers have the
   most to lose, but nothing yet rewards it;
3. **Depth**, which keeps the attack rate down (see D4).

**D2 therefore reduces to a smaller question: is auto-approve acceptable given that the
protection is depth plus maturity plus an incentivised advancer?** The alternative is to treat
the last look as cheap insurance. Either is defensible; the "sitting duck" argument for
forcing it does not survive contact with the vault.

## Not settled, and deliberately out of scope for the PoC

| | |
|---|---|
| **Upgrade authority** (A5) | It can override every parameter, which makes it an unconditional mint voucher. The fix is governance — a vote, or the stakers, possibly with additional tokens granting that right. Recorded so it is not silently forgotten |
| **Independent audit** | The four critical defects found so far were found by our own adversarial review, which is not the same as an audit by someone with no stake in the answer |

## Where to read more

- [`12-peg-mechanism.md`](12-peg-mechanism.md) — full reasoning, scenarios, audit findings A1–A19
- [`04-trust-model.md`](04-trust-model.md) — what is trusted, and the roadmap to a signerless reserve
- [`05-relayers.md`](05-relayers.md) — the relayer role and bond custody
