# 3. Architecture

## Components

```
        BSV CHAIN                              SOLANA
 ┌───────────────────────┐        ┌──────────────────────────────────┐
 │  Cold reserve         │        │  solBSV (SPL token, 8 dp)        │
 │  ─ covenant-locked    │        │                                  │
 │  ─ can ONLY pay the   │        │  Bridge program                  │
 │    hot wallet address │        │  ─ mint (verifies BSV SPV proof) │
 │  ─ staggered tranches │        │  ─ burn + redemption requests     │
 │                       │        │  ─ fulfil / challenge / slash    │
 │  Hot wallet           │        │  ─ bonds, caps, pause            │
 │  ─ small float        │        │                                  │
 │  ─ relayer-held key   │        │  BSV light client                │
 └───────────┬───────────┘        │  ─ checkpoint + rolling window    │
             │                    │  ─ PoW / DAA / Merkle checks      │
             │   relayer pays     └───────────────┬──────────────────┘
             └──────────► BSV users               │
                                     ┌────────────▼─────────────┐
                                     │ Raydium / Orca pools     │
                                     │ (external market layer)  │
                                     └──────────────────────────┘
```

| Component | What it does |
|---|---|
| **BSV light client (Solana program)** | Holds a checkpoint plus a rolling window of BSV headers and chainwork. Verifies proof-of-work and difficulty adjustment, and checks Merkle inclusion of BSV transactions. This is the trustless half of the system |
| **`solBSV` (SPL token)** | A classic SPL token, 8 decimals, no freeze authority. Its mint authority is the bridge program's PDA — no external key can mint |
| **Bridge program** | `mint`, `burn`, `fulfil`, `challenge`, `slash`; bond accounting; supply caps; pause. Adjudicates everything |
| **Hot wallet** | A small BSV float. The only funds a relayer can spend freely. Bond-covered |
| **Cold reserve** | A BSV covenant (bare script) that can only pay the hot wallet address, released in staggered tranches. A stolen cold key cannot divert the reserve anywhere else |
| **Relayers** | Permissionless, automated, bonded agents that pay redemptions and prove the payout |
| **Challengers / watchtowers** | Permissionless watchers that submit proofs of unauthorised reserve spends and collect a share of the slashed bond |
| **Raydium / Orca** | The market layer. Not built by SOLBEAM |

## The header state problem

A full BSV header chain cannot live on Solana economically. There are roughly **968,000 BSV headers**; at current Solana rent that is tens of megabytes and hundreds of SOL, against a 10 MiB per-account cap. The chain therefore lives on-chain as:

- a **checkpoint** (a recent, well-buried header), plus
- a **rolling window** of subsequent headers and their chainwork, sharded across accounts, advanced **optimistically** with a challenge period.

Verification itself is cheap: an 80-byte header double-SHA-256 costs **226 CU**, a 12-level Merkle branch **2,616 CU** — a **full SPV deposit proof is about 2,842 CU**, negligible against Solana's per-transaction limit. The expense is *state*, not computation.

This design is deliberately the **simplest** option: a checkpointed, optimistic header chain. Zero-knowledge proof verification (Groth16/SP1-class) is a later hardening step — it is faster to verify than to run, but the tooling is unaudited and, in the cheapest cases, restrictively licensed.

## The tiered reserve

The reserve is split so that a key compromise cannot take everything:

| Tier | Holding (illustrative) | Rule | Who can spend |
|---|---|---|---|
| **Hot wallet** | small float | none beyond software policy | relayer key — **bond-covered** |
| **Cold reserve** | the bulk | covenant: **only** pay the hot wallet address, in scheduled tranches | tranche path only |
| **Total** | hot + cold | — | — |

Because the cold covenant cannot send funds anywhere but the hot wallet, a stolen cold key cannot steal the reserve — it can only nudge the next tranche toward a small, monitored, bonded wallet. That is what lets total deposits comfortably exceed the bond.

See [Parameters & governance](06-parameters.md) for the sizing invariant.

## Technology and licensing

SOLBEAM is **FOSS**, and dependencies are chosen for permissive licensing:

| Need | Choice | Licence |
|---|---|---|
| BSV headers, legacy transactions, Merkle | `btcsuite/btcd` | ISC |
| BSV signature hashing (SIGHASH_FORKID) | implemented in-house | — |
| BSV covenants (`OP_PUSH_TX` introspection) | Rúnar | MIT |
| Solana programs | Anchor + SPL Token | Apache-2.0 |
| ZK verification (future) | `groth16-solana` | Apache-2.0 |
| Solana escrow/hashlock patterns | `kobby-pentangeli/atomic-swap` | MIT / Apache-2.0 |

Note: `scryptlib`'s *SDKs* are MIT, but the sCrypt compiler/stdlib that implements preimage introspection is **not** permissively licensed, so Rúnar is used instead. The BSV Go SDK is under the Open BSV License and is avoided for the same reason.

## What SOLBEAM does not require

- **No BSV full node.** Relay watchers need only chain data; the proof is verified on Solana, so the data source need not be trusted.
- **No Solana infrastructure.** A public or private RPC endpoint is enough.
- **No bridge-owned chain, token, or validator set.**

---

Next: [Trust model & security](04-trust-model.md)
