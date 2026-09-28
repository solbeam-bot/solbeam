# 18. Before we code — the critical list

Consolidated from three adversarial audits (findings A1–A18, F1–F7, C1–C4, plus the
documentation verification). Organised by **when it has to be dealt with**, not by severity,
because that is the useful question now.

---

## 1. Live in shipped code — fix or decide before writing anything new

### P1 — The checkpoint race · **critical, unfixed**
`initialize` accepts any 80-byte header that meets **its own declared `bits`**, and it is
unauthenticated. Whoever calls it first chooses both the trusted root **and the difficulty for
the client's entire life.** On a fresh deploy the transaction is front-runnable.

The checkpoint being trusted is inherent to the design. **The race is not.** Options: deploy and
initialize atomically; gate `initialize` on a known key; or accept it and document that the
deploy transaction must be private. *Decision needed.*

### P2 — `commit_fork` does not re-anchor the staged branch · **critical, unfixed**
Linkage is validated when each branch header is *pushed*. At *commit* it only checks that the
fork height is still in the window, then splices `headers[..=fork_idx] ++ staging.hashes`
**without re-checking that `headers[fork_idx].hash` is still the block the branch links to.**

If another fork commits in between, the stored window is spliced from two different chains.
`verify_deposit` then proves against a record whose linkage is broken — **the only invariant the
light client has.** This is a potential mint-forgery vector, not merely untidy. Fix: store the
expected parent hash in the staging account at `init_staging` and re-verify it at commit.

### P3 — A deposit has a hard ~48-hour life, and then it is unspendable · **critical, NOT previously recorded**
`verify_deposit` requires the deposit's height to be **inside the window** (`index_of(height)`).
The window holds 288 headers and `window_start` advances with every header pushed.

So a deposit must be minted within roughly 48 hours of its block. After that the proof can never
be verified again — **and the BSV is already with the relayer.** An honest depositor whose mint is
delayed — because the advancer stalled, a gate closed, or they simply waited — loses the deposit
permanently, with no on-chain refund path (A15).

This is the sharpest gap found, and it is a **timing trap on honest users rather than an attack.**
Options: size the window against `depth + maturity` with margin and state the deadline in the UI;
allow a historic header to be supplied with a chain of headers; or add the refund path. *Decision
needed, and it interacts directly with the maturity length.*

### P4 — `FLOOR` and the retarget · **F7, blocks testnet**
`push_header` requires `bits == expected_bits`, set once at `initialize` and never refreshed.
**The client halts permanently at the first difficulty retarget.** Invisible on regtest; fatal on
testnet or mainnet. Needs a stored difficulty-period anchor — BSV retargets every 2016 blocks and
we store 288.

### P5 — The replay list is a cheap shutdown and a hard ceiling · **F6, unfixed**
`MAX_USED = 200` with `MIN_PEG_IN` unimplemented. Two hundred dust deposits block every peg-in for
the rest of the window, repeatably. Independently, it caps the protocol at **200 peg-ins per
48 hours** with no attacker at all.

---

## 2. Live the moment the vault is built — design them in, not after

### P6 — The vault must release the replay entry when it burns · **F1**
Pruning is by height, and a natural reorg does not remove a transaction — it returns it to the
mempool to be mined again. So the ordinary case is: block orphaned, vault correctly burns the
staged mint, transaction re-mined, **and the depositor can never mint again.** Their BSV is in the
reserve and their tokens are gone. This is the most likely honest-user loss in the *designed*
system and it needs no attacker.

### P7 — Detection has no reward · **F3**
Pushing headers is permissionless, unpaid and unstaked. Staging a competing branch — the actual
detection act — has no bounty, while payout challenging does. On the unbacked path (D6) there is
no staker whose buffer is at risk either, so **there is no incentive at all.** If detection is the
backstop, it needs paying for. *Decision needed before the vault ships.*

### P8 — The unbacked path has no backstop · **F2, accepted by decision**
D6 allows a peg-in with no underwriter. Then the vault and detection are the *entire* defence: no
bond, no buffer, nothing to slash. That is a deliberate risk acceptance, but it should be
implemented as an explicit, visible mode rather than as the default.

### P9 — Self-dealing is profitable when detection fails · **F5, `k = 1`**
At `k = 1` a staker underwriting its own fraudulent deposit is roughly break-even **only if the
shortfall is detected and the bond is slashed.** With detection failing, nothing is noticed to
slash and the attacker keeps the mint *and* the bond.

### P10 — The bond must cover staged mints · **F4**
`owed_R` must include mints still maturing in the vault. A depositor whose mint is maturing has
paid BSV and holds no tokens, so omitting them leaves exactly that window unbonded. Defined in
doc 12; **must be implemented that way.**

---

## 3. Known and accepted — do not re-litigate

| | |
|---|---|
| **The program upgrade authority** (A5) | Can mint arbitrarily by replacing the program. Out of scope for the PoC; the fix is governance, possibly tied to staking |
| **The reserve is keys, not a covenant** (A6) | No covenant, and no timelocks on BSV to fall back on. Per-relayer deposits are the mitigation that does not require the covenant track |
| **The buffer is off-chain** (A4) | Tracked, not verified. Its integrity rests on the custodian |
| **Refunds need a key** (A15) | No on-chain return-to-sender path exists |
| **Detection depends on somebody pushing headers** | A liveness condition anyone can satisfy — but see P7, since nobody is paid to |

---

## 4. Audit gaps — not yet examined at all

These have had **no adversarial attention** and are worth a pass before mainnet, if not before
code:

1. **The order book's economics.** No analysis of whether a staker can be systematically picked
   off, griefed out of capacity, or manipulated by a large depositor. The auto-approve decision
   rests on the vault reversing a reorged fill — untested reasoning.
2. **The vault's instruction set.** No design yet for release, burn, escrow and return — so no
   audit of their authorisation, recipients or edge cases.
3. **Cross-chain replay.** Whether a BSV transaction used on one Solana deployment can be replayed
   against another, and whether a chain-id or programme-id is bound into the deposit.
4. **The `used_deposits` counterfactual.** X2 argued the missing `seeds` constraint was not
   exploitable. It is now pinned, but **no test creates a counterfeit account and asserts
   rejection** — the closure is by argument, not by evidence.
5. **`verify_deposit`'s parsing.** No adversarial pass on malformed transactions, unusual output
   counts, or `OP_PUSHDATA` handling in the `OP_RETURN`.

---

## The three decisions this document needs

1. **P1** — how is the first `initialize` protected?
2. **P3** — how long may a deposit wait before it is unmintable, and what happens then?
3. **P7** — is detection paid for, or is it assumed to happen?

Everything else here is either fixed, scheduled, or explicitly accepted.
