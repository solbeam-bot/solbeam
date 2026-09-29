# 7. Roadmap

The order is deliberate: **the built system now follows a real chain**, then the component the
rest of the design rests on, then the federation and the parts that need bonds and adjudication.

> **Built or designed?** The light client with cw-144, `solBSV`, the mint and fork staging are
> built and pass **20 on-chain tests**. **The vault, the bonded federation, threshold custody,
> governance, slashing and all of peg-out are designed and not built.**
> [`13-summary.md`](13-summary.md) is canonical; [`23-federation.md`](23-federation.md) is the
> federation in detail.

## Closed — fixed in code and verified

| | What closed it |
|---|---|
| **F7 — the difficulty retarget** | **cw-144**, from the SV Node's `src/pow.cpp`, replayed against real mainnet headers: **324/324 predicted exactly** (`difficulty.rs`, `difficulty-vectors/`). Workstream W1.6 |
| **P2 — the fork re-anchor** | `init_staging` records `fork_parent_hash`; `commit_fork` requires the chain still to hold it at that height and fails `ForkPointMoved` otherwise. Workstream W1.7 |
| **A7 — double-minting one deposit** | Deposit identity `(txid, vout)` in the used-deposit list, and the account is pinned by its constraint, which addresses the counterfeit-list reading (X2; the audit still recommends a test) |
| **The window resize** | The window is **192 records of 52 bytes** (hash + chainwork + time), `SPACE` **10,103** of 10,240, giving a deposit lifetime of **32 hours** at 600 s/block. The old 288 × 32-byte, 48-hour layout was arithmetically impossible once cw-144 was implemented. Workstream W1.4 |

**Testnet is no longer blocked on the retarget.** It was blocked because the client rejected every
header after the checkpoint; that is fixed and verified against real headers. What remains open
from it is **X3**: the rule is hard-coded and BSV says it will change, so it must become
changeable without a redeploy — which the federation model turns into a **governance parameter**
rather than an orphaned risk.

## Open — remaining work

| Step | What ships | Why this order |
|---|---|---|
| **F6 — the replay-list ceiling** | `MIN_PEG_IN` enforced, and `MAX_USED = 200` sized or replaced | A hard ceiling of **200 peg-ins per 32-hour window**, with no attacker required. An open defect in code that exists |
| **The vault** | Every mint lands in a program-owned token account; released after maturity, burned if a reorg is followed | The component the rest of the design rests on. It gives reversibility without a freeze authority and removes the window in which a fraudulent mint could be sold |
| **The federation** | Open membership, a **1,000 BSV bond**, members running software only, and the reserve held under a **threshold key** | Turns individually-trusted keys into a threshold over the reserve, and makes the parties with the most to lose the ones who watch |
| **Governance** | **85% of pledged coins, 30 days, live signal**, holding the **upgrade authority**; redemptions never pausable; pause stops **mints only** | Gives the system a change mechanism that is slower than its exit: a hostile proposal empties the bridge before it lands |
| **Slashing** | **Self-proving equivocation** — members sign payout intents individually, so signing two conflicting intents is its own evidence | The proof is on-chain and needs no judgement; anything weaker cannot attribute fault from a threshold signature |
| **Peg-out** | Escrow into the vault, individually-signed payout intents, the threshold signature, settlement proved against the light client, and a permissionless cancel after the deadline | Turns a one-way wrapper into a two-way peg. Failure returns; it never mints, so supply is unchanged on every path |
| **The node and user software** | The federation member's node, and the surfaces that submit headers and resolve pending items | **No consensus role.** Last because it exposes a system rather than completing one |

**Named as open, and not dressed up** ([`13-summary.md`](13-summary.md)): the vault's design carries
unfixed audit findings and is being re-audited against this model; the **genesis bootstrap** has no
path (members bond `solBSV`, which does not exist until a mint happens); **sharding** the threshold
key is undecided; and the DAA is hard-coded until governance can change it.

## Launch sequencing

1. **Testnet, mint only.** No longer blocked on the retarget. F6 remains; no real value.
2. **Testnet, vault.** Mints stage, mature and release. Still no real value.
3. **Testnet, federation and governance.** Bonded membership, threshold custody, governance and
   slashing exercised end to end. Still no real value.
4. **Mainnet, mint only, small caps.** Deposits proven end to end. Redemption handled manually and
   transparently while peg-out is finished.
5. **Mainnet, peg-out live, small caps.** Escrow, payout intents, the threshold signature, and the
   cancel path exercised in production.
6. **Raise caps.** Capacity is capped by bonds pledged, so raising it means more members or larger
   bonds — and only after audits and after the invariants hold in production for a sustained period.

## Milestones

- The light client verifies mainnet BSV headers, **including the per-block difficulty**, against
  independent reference data. *(Built: cw-144, 324/324.)*
- A deposit is minted on mainnet with no human in the loop: staged in the vault, then released
  after maturity.
- A reorg followed inside the maturity window burns the staged tokens, and the depositor ends
  exactly where they started.
- A federation is formed from **open membership** at the 1,000 BSV bond, and holds the reserve
  under a threshold key.
- A governance proposal passes at **85% of pledged coins** and takes effect after **30 days**,
  with redemptions live throughout.
- A redemption completes end-to-end: escrow → signed payout intents → BSV payout → proof → settlement.
- A failed redemption returns the escrow: supply unchanged, and the bond **not** additionally transferred.
- A member that signs two conflicting payout intents is slashed by anyone submitting its own two
  signatures.

---

Next: [FAQ](08-faq.md)
