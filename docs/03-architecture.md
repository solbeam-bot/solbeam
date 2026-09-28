# 3. Architecture

> **Built or designed?** The light client, `solBSV` and the mint exist and pass 20 on-chain
> tests, and the header window is real. **The vault, the order book, per-relayer deposits and
> all of peg-out are designed and not built.** The shipped program mints straight to the
> depositor's token account, so every property below that depends on the vault is a
> specification rather than a property of the code. The [trust model](04-trust-model.md)
> records who bears the difference.

## Components

```
        BSV CHAIN                              SOLANA
 ┌────────────────────────┐       ┌──────────────────────────────────┐
 │  Per-relayer deposits   │       │  solBSV (SPL token, 8 dp)        │
 │  ─ a P2PKH script per   │       │                                  │
 │    relayer, no pooled   │       │  Light client                    │
 │    reserve              │       │  ─ checkpoint + 192-block window │
 │  ─ the relayer pays     │       │  ─ PoW / Merkle checks (DAA: cw-144, verified 324/324)              │
 │    redemptions from its │       │                                  │
 │    own float            │       │  Vault (program-owned account)   │
 └───────────┬─────────────┘       │  ─ every mint lands here first   │
             │                     │  ─ released after maturity, or   │
             │   pays BSV out      │    burned if a reorg is followed │
             └──────────► BSV users│                                  │
                                   │  Order book                      │
                                   │  ─ staked sell orders: liquidity,│
                                   │    fee, confirmation depth       │
                                   └───────────────┬──────────────────┘
                                                   │
                                    ┌──────────────▼──────────────┐
                                    │ Website — no consensus role │
                                    │ order entry, parameters,    │
                                    │ external metrics, status    │
                                    └─────────────────────────────┘
```

| Component | What it does |
|---|---|
| **BSV light client (Solana program)** | Holds a checkpoint plus a rolling window of BSV headers. It answers one question — *is this transaction in this block, and is that block still canonical?* — by verifying proof-of-work and Merkle inclusion. Difficulty adjustment is implemented as cw-144, the rule the SV Node's `src/pow.cpp` uses, verified against real mainnet headers at **324/324 exact**. Each stored record carries the block hash, its cumulative chainwork and its timestamp, which is what the rule needs. This is the trustless half of the system, and it is what makes minting permissionless |
| **The vault (program-owned token account)** — *designed, not built* | **Every mint lands here first, never with the depositor.** The program releases the staged `solBSV` once a maturity window passes with the deposit still canonical, or **burns** it if a reorg is followed. Because the account is program-owned, releasing or burning is disposing of what the program holds — that is what makes a transfer reversible without a freeze authority, and it removes the window in which a fraudulent mint could be sold |
| **The order book** — *designed, not built* | Underwriting, discovered rather than set by a committee. Stakers post sell orders — so much liquidity, at such a fee, at such a confirmation depth — matched by price then time, partially filled. Because depth is a term of the trade, the market prices reorg risk instead of an operator guessing at it |
| **Relayers** — *bonding and per-relayer deposits designed, not built* | A role, not a company. Anybody may run one. A relayer holds BSV, pays redemptions, and lodges a bond in `solBSV`, sized `bond_R ≥ k × owed_R`, that the program can seize. Each relayer has its own deposit script; there is no pooled reserve to hold, audit or steal |
| **The website** | Order entry, the published parameters and the external metrics. **No consensus role at all** — it can be replaced or ignored without the program noticing |

`solBSV` itself is a classic SPL token: 8 decimals, **no freeze authority**, its mint authority a program PDA, so no external key can mint.

## The header state problem

A full BSV header chain cannot live on Solana economically. There are roughly **968,000 BSV headers**; at current Solana rent that is tens of megabytes and hundreds of SOL, against a 10 MiB per-account cap. The chain therefore lives on-chain as:

- a **checkpoint** (a recent, well-buried header), plus
- a **rolling window** of **192 subsequent headers** — 32 hours at BSV's ten-minute target — held in a single account of 10,103 bytes, comfortably inside Solana's 10,240-byte account cap. A competing branch is staged in batches and committed only if **strictly heavier**; ties keep the incumbent, so an equal-length branch cannot churn the tip.

Verification itself is cheap: an 80-byte header double-SHA-256 costs **226 CU**, a 12-level Merkle branch **2,616 CU** — a **full SPV deposit proof is about 2,842 CU**, negligible against Solana's per-transaction limit. The expense is *state*, not computation.

This design is deliberately the **simplest** option: a checkpointed, optimistic header chain. Zero-knowledge proof verification (Groth16/SP1-class) is a later hardening step — it is faster to verify than to run, but the tooling is unaudited and, in the cheapest cases, restrictively licensed.

## Where the BSV sits

**There is no pooled reserve.** Aggregation is what creates a single key worth stealing, so a BSV deposit pays **an individual relayer**: each relayer has its own deposit script and its own float, and each posts a bond in `solBSV` against what it owes — `bond_R ≥ k × owed_R`, where `owed_R` is derived from proofs the program verified itself. A shortfall lands on the relayer that caused it, rather than on a shared wallet or on every holder.

The published invariant is still `custodied BSV ≥ outstanding solBSV`. **The program cannot enforce it** — the reserve is off-chain BSV it cannot read — so the website shows the ratio and the program does not check it.

See [Parameters & governance](06-parameters.md) for the sizing of `k`, and [Relayers](05-relayers.md) for the role.

## What the program consults

**No oracle.** The program reacts only to **BSV block headers** and **Solana slots**, and to nothing else. Depth and block time are read from the headers; redemption deadlines are measured in slots, so a cluster halt **freezes** the clock rather than punishing a relayer who could not act. External metrics — price, hashrate, reorg cost — are published on the website and **never consulted by the program**. Nothing the website says can change what the program accepts.

## Technology and licensing

SOLBEAM is **FOSS**, and dependencies are chosen for permissive licensing:

| Need | Choice | Licence |
|---|---|---|
| BSV headers, legacy transactions, Merkle | `btcsuite/btcd` | ISC |
| BSV signature hashing (SIGHASH_FORKID) | implemented in-house | — |
| BSV covenants (`OP_PUSH_TX` introspection) — **not used in the current design** | Rúnar, retained for the deferred signerless track | MIT |
| Solana programs | Anchor + SPL Token | Apache-2.0 |
| ZK verification (future) | `groth16-solana` | Apache-2.0 |
| Solana escrow/hashlock patterns | `kobby-pentangeli/atomic-swap` | MIT / Apache-2.0 |

Note: `scryptlib`'s *SDKs* are MIT, but the sCrypt compiler/stdlib that implements preimage introspection is **not** permissively licensed, so Rúnar is used instead. The BSV Go SDK is under the Open BSV License and is avoided for the same reason.

## What SOLBEAM does not require

- **No BSV full node.** Relay watchers need only chain data; the proof is verified on Solana, so the data source need not be trusted.
- **No Solana infrastructure.** A public or private RPC endpoint is enough.
- **No operator.** Relayering is a role anyone may run, and the website has no consensus role — no privileged party signs anything the program trusts.

---

Next: [Trust model & security](04-trust-model.md)
