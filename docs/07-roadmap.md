# 7. Roadmap

The order is deliberate: **the built system now follows a real chain**, then the component the
rest of the design rests on, then the federation and the parts that need bonds and adjudication.

> **Built or designed?** The light client with cw-144, `solBSV`, the mint and fork staging are
> built and pass **34 on-chain tests**. **The vault, the bonded federation, threshold custody,
> governance, slashing and all of peg-out are designed and not built.**
> [`13-summary.md`](13-summary.md) is canonical; [`23-federation.md`](23-federation.md) is the
> federation in detail.

## Closed — fixed in code and verified

| | What closed it |
|---|---|
| **F1/F2/F3 — the light client could not follow a real chain** | Seed the difficulty bootstrap and verify the instruction path, not only the pure function: **160 real mainnet headers through `push_header`** and a real mainnet branch through `push_fork_header`. **27 tests, 0 failing** |
| **F4's initialiser vulnerability** | `initialize` and `initialize_bridge` require the program's **upgrade authority**, so the first caller of a fresh deployment no longer becomes `authority`. The authority residual (timelock + threshold) is specified in doc 24, not built |
| **F7 — the difficulty retarget** | **cw-144**, from the SV Node's `src/pow.cpp`, replayed against real mainnet headers: **324/324 predicted exactly** (`difficulty.rs`, `difficulty-vectors/`). Workstream W1.6 |
| **P2 — the fork re-anchor** | `init_staging` records `fork_parent_hash`; `commit_fork` requires the chain still to hold it at that height and fails `ForkPointMoved` otherwise. Workstream W1.7 |
| **A7 — double-minting one deposit** | Deposit identity `(txid, vout)` in the used-deposit list, and the account is pinned by its constraint, which addresses the counterfeit-list reading (X2; the audit still recommends a test) |
| **The window resize** | The window is **192 records of 52 bytes** (hash + chainwork + time), `SPACE` **10,107** of 10,240, giving a deposit lifetime of **32 hours** at 600 s/block. The old 288 × 32-byte, 48-hour layout was arithmetically impossible once cw-144 was implemented. Workstream W1.4 |

**Testnet is no longer blocked on the retarget.** It was blocked because the client rejected every
header after the checkpoint; that is fixed and verified against real headers. What remains open
from it is **X3**: the rule is hard-coded and BSV says it will change, so it must become
changeable without a redeploy — which the federation model turns into a **governance parameter**
rather than an orphaned risk.

## Open — remaining work

| Step | What ships | Why this order |
|---|---|---|
| **Transparency — reserve and supply published continuously** | The reserve balance and the `solBSV` supply published as a live, public backing ratio | **Promoted from a phase-5 monitoring task to an early deliverable.** For the two cases nothing can enforce — a colluding threshold, and an unspent-outpoint spend nobody challenges — **visibility is the only remaining defence**, and it must exist before real value does |
| ~~**F6 — the replay-list ceiling**~~ **CLOSED** | `MIN_PEG_IN` enforced, and `MAX_USED = 200` sized or replaced | A hard ceiling of **200 peg-ins per 32-hour window**, with no attacker required. An open defect in code that exists |
| **The vault** | Every mint lands in a program-owned token account; released after maturity, burned if a reorg is followed | The component the rest of the design rests on. It gives reversibility without a freeze authority and removes the window in which a fraudulent mint could be sold |
| **The federation** | Greycore-admitted membership, **two-sided bonds** (1,000 BSV per side, the float), members running software only, and the reserve held under a **2-of-2 `OP_CHECKMULTISIG`** — the gateway threshold key plus the Greycore's | Turns individually-trusted keys into a threshold over the reserve **with a Greycore co-signature**, and makes the parties with the most to lose the ones who watch. The BSV-side bond sits under the same collective key, so the members can seize it |
| **Governance** | **85% of pledged coins, 30 days, live signal**, holding the **upgrade authority**; redemptions never pausable; pause stops **mints only** | Gives the system a change mechanism that is slower than its exit: a hostile proposal empties the bridge before it lands |
| **Slashing** | **Self-proving equivocation** — members sign payout intents individually, so signing two conflicting intents is its own evidence | The proof is on-chain and needs no judgement; anything weaker cannot attribute fault from a threshold signature |
| **Peg-out** | Escrow into the vault, individually-signed payout intents, the threshold signature, settlement proved against the light client, and a permissionless cancel after the deadline | Turns a one-way wrapper into a two-way peg. Failure returns; it never mints, so supply is unchanged on every path |
| **The node and user software** | The federation member's node, and the surfaces that submit headers and resolve pending items | **No consensus role.** Last because it exposes a system rather than completing one |

**Named as open, and not dressed up** ([`13-summary.md`](13-summary.md)): the vault's design carries
unfixed audit findings and is being re-audited against this model; **sharding** the threshold key is
undecided; and the DAA is hard-coded until governance can change it.

**Genesis — decided.** Members post a **BSV-side bond at genesis**, so no `solBSV` needs to exist
first. The alternative — a **capped, explicitly-unbonded first mint** — is recorded as a documented
later option, not chosen, because it leaves the first mint backed by nothing but the members' word.

## Launch sequencing

1. **Testnet, mint only.** No longer blocked on the retarget. F6 remains; no real value.
2. **Transparency, alongside the testnet.** Publish the reserve balance and the `solBSV` supply
   continuously, so the backing ratio is public **before** any real value is at stake. An early
   deliverable, not a phase-5 monitoring task: for collusion and for an unchallenged outpoint spend,
   nothing can enforce anything, and visibility is the defence that remains.
3. **Testnet, vault.** Mints stage, mature and release. Still no real value.
4. **Testnet, federation and governance.** Bonded membership, threshold custody, governance and
   slashing exercised end to end. Still no real value.
5. **Mainnet, mint only, small caps.** Deposits proven end to end. Redemption handled manually and
   transparently while peg-out is finished.
6. **Mainnet, peg-out live, small caps.** Escrow, payout intents, the threshold signature, and the
   cancel path exercised in production.
7. **Raise caps.** The bond is the **float** and no longer a capacity ceiling — the earlier "capacity
   is capped by bonds pledged" claim is **withdrawn** — so scaling means growing the **float**, adding
   members (admitted by the Greycore) and enlarging the Greycore, and only after audits and after the
   invariants hold in production for a sustained period.

## Milestones

- The light client verifies mainnet BSV headers, **including the per-block difficulty**, against
  independent reference data. *(Built: cw-144, 324/324.)*
- A deposit is minted on mainnet with no human in the loop: staged in the vault, then released
  after maturity.
- A reorg followed inside the maturity window burns the staged tokens, and the depositor ends
  exactly where they started.
- A federation is formed from **Greycore-admitted members** at the two-sided 1,000 BSV float, and holds the
  reserve under a **2-of-2 `OP_CHECKMULTISIG`** — the gateway threshold key plus the Greycore's.
- A governance proposal passes at **85% of pledged coins** and takes effect after **30 days**,
  with redemptions live throughout.
- A redemption completes end-to-end: escrow → signed payout intents → BSV payout → proof → settlement.
- A failed redemption returns the escrow: supply unchanged, and the bond **not** additionally transferred.
- A member that signs two conflicting payout intents is slashed by anyone submitting its own two
  signatures.

---

Next: [FAQ](08-faq.md)

### Phase 2 — beyond the proof of concept

**Delegated staking.** Lets non-members delegate `solBSV` to a federation member and share its fee,
growing the bond base without new operators. **Activatable by governance.** Deliberately deferred:
a staking layer has its own incentive problems, and it should not be designed until the thing it is
meant to scale has been shown to work.
