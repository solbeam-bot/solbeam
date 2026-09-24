# 7. Roadmap

The order is deliberate: **ship the half that is trustless first**, then the half that needs bonds and adjudication.

| Phase | What ships | Why this order |
|---|---|---|
| **P0 — Design & vectors** | Chain constants frozen; header-state cost model (checkpoint / rolling window / sharding / rent); golden test vectors for headers, difficulty adjustment, Merkle proofs and BSV signature hashing; mint confirmation depth fixed at 12 | Compute is cheap (~2,842 CU per SPV proof); the open question is *state*, so it is modelled before code |
| **P1 — Mint only** | BSV light client on Solana; `solBSV` token; `mint`. Devnet → public testnet. Permissionless: anyone can mint | This half is **trustless** and delivers most of the value on its own. It can launch before redemption exists |
| **P2 — Redeem** | `burn`, redemption requests, `fulfil`, `challenge`, `slash`; relayer bonds; 6-hour deadlines; automatic refunds | Turns a one-way wrapper into a two-way peg |
| **P3 — Apps & hardening** | Desktop relayer app; mobile app; watchtower tooling; audits of the light client and bridge program; proof-of-reserves; capped mainnet | Opens the relayer role to the public and scales usage |
| **P4 — Raise limits & research** | Conservative parameter raises as bonds grow; begin the signerless research track (BSV covenant verifying a zero-knowledge proof of the Solana burn) | Removes the hot key, the bond and the price mismatch if successful |

## Launch sequencing

1. **Testnet, mint only.** Prove the light client against real BSV blocks. No real value.
2. **Mainnet, mint only, small caps.** Deposits proven end-to-end; redemption handled manually and transparently while P2 is finished.
3. **Mainnet, redeem live, small caps.** Bonds posted, deadlines active, refund path exercised in production.
4. **Raise caps.** Only after audits and after the invariants hold in production for a sustained period.

## Milestones
- Light client verifies mainnet BSV headers and DAA against independent reference data.
- A deposit is minted on mainnet with no human in the loop.
- A redemption completes end-to-end: burn → BSV payout → proof → settlement.
- The refund path fires in production (deliberately triggered test).
- A permissionless challenger successfully slashes a deliberately misbehaving test relayer.
- The reserve invariant is published continuously and matches the on-chain supply.

---

Next: [FAQ](08-faq.md)
