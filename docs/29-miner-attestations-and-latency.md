# 29. Miner-attested hashrate, and the latency question

**Status: not a dependency.** This records a proposal raised in the BSV community on
2026-09-29, assesses honestly whether it helps SOLBEAM, and separates **what we could build today**
from **what we must not bet on**. It also addresses the expectation that **minting should be fast**,
which is a real problem with our current design regardless of what happens to the proposal.

---

## 1. The proposal, as raised

Miner-attested hashrate, roughly:

- A miner signs an attestation that a transaction **will be mined and not double-spent**
  (*"I commit my hashpower"*).
- If **80% of hashrate** has attested, the transaction is treated as **finalised before inclusion**.
- Node software would **reject blocks** containing a competing or double-spending transaction.
- It requires **coordination between the miner set** — a leader, or an endpoint that does tie-breaking
  and hands out block templates.
- The stated motive: **long block times let a miner reorg what suits them**, and miners could exclude
  transactions — which would disrupt a federation like ours.

**The open questions raised alongside it** were the substantive ones: *Teranode has no mempool, so how
are conflicts between miners ordered and resolved?* If a subtree is invalidated by a double-spend,
does the search space become worse than quadratic? Who runs the tie-breaking API, and how does 80% of
hashrate come to agree on it?

---

## 2. Does it help the reorg problem? Partly — and not the part we have

**What it would genuinely fix:** a deposit could be treated as final **before** it is mined, so
minting could be near-instant **and** the reorg risk would collapse, because breaking an attestation
means attacking a hashrate supermajority rather than just finding one block.

**But the reorg risk we actually have is a different one.** Our problem is *a confirmed deposit
reorged out after the mint*. Attestation makes a transaction final earlier; it does not make a chain
final. **A minority reorg below the attestation threshold remains possible**, and our vault still has
to reverse it. So the attestation **shrinks the window we must defend against; it does not remove
the need to defend it.**

**Which means the vault is still required, and still the contribution.** Attestations would let us
shorten the maturity period; they would not let us delete the burn path.

### And it is a new trust assumption, not the removal of one

Worth stating plainly, because "miner attested" sounds trustless and is not:

| | |
|---|---|
| **It is a supermajority vote** | 80% of hashrate can finalise **anything**, including a transaction that should not be finalised |
| **It can censor** | The same 80% can simply **refuse to attest**, which is the exact failure mode the proposal exists to prevent |
| **It reintroduces a leader** | The tie-breaking endpoint is a **centralisation point** — whoever runs it sees every transaction first, and can order or delay |
| **It needs consensus change** | "Reject blocks containing a competing transaction" is a **hard fork** of BSV, with everything that implies |
| **Hashrate is not identity** | A miner can rotate coinbase keys; any weighting scheme has to be robust to that |

**So it is a federation — a federation of miners, weighted by hashrate, with a leader.** Better than
a small federation in that its members are expensive to acquire; worse in that it is **unaccountable
and can censor**.

---

## 3. Could we verify it? Yes — and the mechanism is interesting

**This is the part worth recording, because it is a real design if the proposal ever lands.**

A Solana program could verify a miner attestation **without a new oracle**, because **a miner's weight
is observable from the chain itself:**

```
weight(miner)  =  that miner's blocks in a recent window  /  all blocks in the window
```

**You cannot fake hashrate weight, because to have weight you must have found blocks.** So:

1. **The light client already stores headers**, and the coinbase Merkle root is in every header.
2. A **Merkle proof of the coinbase transaction** would reveal the miner's receiving key.
3. Weight is then **derived from the stored window** — no trusted registry, no report.
4. An attestation is **one signature** over `(txid)`, counted against that derived weight.

**So a hashrate supermajority could be verified on-chain against headers we already hold.** That is a
genuinely trustless way to consume the attestation — the weights are not asserted, they are computed.

**It is not something we can build today**, because the attestations do not exist to verify. But it is
the design we would use, and it would not require changing the shape of the light client — only
adding coinbase inclusion proofs and a weight accumulator.

---

## 4. The latency problem, which is real and ours

**The community expectation is that minting is fast — effectively instant.** Our design is not:

| | |
|---|---|
| `MIN_CONFIRMATIONS` | **12** — with 10-minute blocks, **~2 hours** before a deposit is mintable |
| Vault maturity | **on top of that** — `MATURITY` is not yet settled |

**So a deposit is hours from usable, against an expectation of seconds.** That is a product problem,
not a technical one, and it does not fix itself.

**The honest framing of the trade-off:**

> **How fast a mint can be released is exactly how much reorg risk is being accepted.** The vault
> exists so that the two can be **decoupled** — mint quickly, let maturity be the safety dial, rather
> than gating the user's first sight of their tokens on twelve confirmations.

**Decision: `MIN_CONFIRMATIONS` stays at 12 for the PoC.** We are not shortening it, and we are not
adopting an attested fast path. What is recorded instead is **the gap between what we do and what the
market expects**, so that it is a known limitation rather than an unnoticed one.

**And the decoupling is still worth building**, because it is available now and does not depend on
anyone else changing anything: mint into the vault so the user **sees** `solBSV` immediately, and
**unlock** it at maturity. Fast to appear, safe to spend — with the confirmations still at 12 behind
it.

**The upgrade path, if the community ever ships attestation:**

| | |
|---|---|
| **Now** | 12 confirmations, then maturity. Safe, slow |
| **Available now** | Decouple: visible immediately, unlocked at maturity. Same safety, better UX |
| **If attestation ships** | Mint on attestation, verified on-chain against derived hashrate weight (§3). **Fastest — and a parameter change rather than a redesign, because the vault already exists as the safety dial** |

**We cannot change node software, and we are not proposing to.** This is recorded so that **if the
change happens, we can use it** — and so that nobody assumes it has.
---

## 5. What this changes, and what it does not

**Changes:** the latency problem is now recorded as a **product requirement with a proposed
mechanism (b)**, rather than an unexamined consequence of `MIN_CONFIRMATIONS = 12`.

**Changes:** §3 records a **trustless verification path** for miner attestations, so that if the
proposal lands we can consume it without an oracle.

**Does not change:** the vault is still required. **The reorg reversal remains the contribution.**
Attestation moves the boundary; it does not remove the problem, and any design that assumes it will
ship is betting the project on a BSV consensus change we do not control.

**Does not change:** we do not depend on it. **Nothing in this design may assume miner attestation
exists**, and no document should describe minting as instant until it does.
