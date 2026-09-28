# 7. Roadmap

The order is deliberate: **make the built system survive a real chain first**, then build the
component the rest of the design rests on, then the parts that need bonds and adjudication.

> **Built or designed?** The light client, `solBSV` and the mint are built and pass 20 on-chain
> tests. **The vault, the order book, per-relayer deposits and all of peg-out are designed and
> not built.** The step that comes first — F6, the replay-list ceiling — is an open defect in the
> code that does exist; every step from *the vault* onward is a specification.

| Step | What ships | Why this order |
|---|---|---|
| ~~**F7 — the difficulty retarget**~~ | ~~`expected_bits` advances at a boundary~~ | **Closed.** cw-144 is implemented and verified against real mainnet headers, **324/324 exact**, so the client follows a real chain. **Still open:** the rule is hard-coded and BSV plans to change it, so the algorithm needs to be swappable without a redeploy |
| **F6 — the replay-list ceiling** | `MIN_PEG_IN` enforced, and `MAX_USED = 200` sized or replaced | A hard ceiling of **200 peg-ins per 32-hour window**, with no attacker required |
| **The vault** | Every mint lands in a program-owned token account; release after maturity, burn if a reorg is followed | The component the rest of the design rests on. It gives reversibility without a freeze authority and removes the window in which a fraudulent mint could be sold |
| **Per-relayer deposits and `owed_R`** | Each relayer's own deposit script; `bond_R ≥ k × owed_R` from proofs the program verified; relayer consent | Turns a fraud from something holders absorb into something the relayer is charged for, and removes the pooled reserve |
| **Peg-out** | Escrow into the vault, deadlines in Solana slots, the payout proof against the light client, challenge, settlement and refunds | Turns a one-way wrapper into a two-way peg. Supply is unchanged on every path: no failure mints |
| **The website** | Order entry, published parameters, the external metrics and live status | **No consensus role.** Last because it exposes a system rather than completing one |

## Launch sequencing

1. **Testnet, mint only — after F6.** The replay ceiling has to be fixed
   before a real chain will run the built program for long. (The retarget, F7, is already closed:
   cw-144, verified 324/324. What remains open from it is X3, that the rule is hard-coded and BSV
   may change it.) No real value.
2. **Testnet, vault and book.** Mints stage, mature and release; per-relayer deposits and `owed_R`
   carry the liability. Still no real value.
3. **Mainnet, mint only, small caps.** Deposits proven end to end. Redemption handled manually and
   transparently while peg-out is finished.
4. **Mainnet, peg-out live, small caps.** Escrow, deadlines, payout proofs and the refund path
   exercised in production.
5. **Website, then raise caps.** Order entry published once the flows it exposes exist; caps raised
   only after audits and after the invariants hold in production for a sustained period.

## Milestones
- The light client verifies mainnet BSV headers, **including the difficulty retarget**, against
  independent reference data.
- A deposit is minted on mainnet with no human in the loop: staged in the vault, then released
  after maturity.
- A reorg followed inside the maturity window burns the staged tokens, and the depositor ends
  exactly where they started.
- A redemption completes end-to-end: escrow → BSV payout → proof → settlement.
- A failed redemption returns the escrow: supply unchanged, the bond **not** additionally transferred.
- A permissionless challenger successfully slashes a deliberately misbehaving test relayer.
- The reserve invariant is published continuously and matches the on-chain supply.

---

Next: [FAQ](08-faq.md)
