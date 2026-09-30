# 31. The vault — build spec

**Status: designed, not built.** This is the specification the code is written against. It
supersedes the open questions in `21-vault-structural.md` by answering them, and it records the
three decisions taken on 2026-09-30.

**The vault is the contribution.** A Solana program verifies a BSV deposit; the vault is what makes a
reorg **reversible** rather than merely detectable. RenVM does not have this, because its host chain
trusted the shard's report rather than verifying the chain.

---

## 1. The decided parameters

| | | |
|---|---|---|
| `lc.max_staleness_slots` | **54,000** (~6 hours) | How old the client's view of the chain may be before the program refuses to act. A multiple of BSV block time — ~1,500 Solana slots per block, so anything under that is meaningless |
| `v.maturity_blocks` | **0, as a STORED PARAMETER** | How deep a deposit's block must be before its staged mint releases. **Not a compile-time constant** — see §5 |
| `MIN_CONFIRMATIONS` | **12** | Unchanged. The delay between a deposit being mined and being mintable at all |

### The maturity-0 consequence, stated up front

**At maturity 0 the vault is a pass-through.** Tokens are minted into the vault and become releasable
in the same instant, and the token has **no freeze authority** — so once released, a later reorg has
nothing to reverse.

**But "the burn cannot fire" would be too strong, and an earlier revision of this document said
exactly that.** At maturity 0 the burn predicate is *satisfied immediately* rather than *unreachable*:
`burn_staged` requires the hash to differ **and** the height to have matured, and with maturity 0 the
second condition is always met. So it is a **race** — release and burn are both permitted from the
moment of staging, and in practice release wins, because the recipient wants their tokens and a reorg
is the unusual case.

**The honest statement is therefore narrower than it first appears:** maturity 0 removes the *window*
in which the reversal is comfortable, not the reversal itself. Someone who sees a reorg and calls
`burn_staged` before anyone releases **still burns the tokens**. The window exists so that the
reversal is not a race.

**What protects a deposit today is `MIN_CONFIRMATIONS = 12`** — roughly two hours, prevention rather
than reversal. And the reversal remains **available, tested, and racy** rather than disabled.

**This is a deliberate PoC setting**, not an oversight, and it is stated here, in the summary and on
the status page rather than left to be discovered.

---

## 2. Accounts

| Account | Seeds | Holds |
|---|---|---|
| **`Vault`** | `[b"vault"]` | The program-owned **token account** every mint lands in |
| **`StagedMint`** | `[b"mint", txid, vout]` | `{ recipient, amount, deposit_height, deposit_hash, maturity_at_deposit, bump }` |
| **`DepositNullifier`** | `[b"nullifier", txid, vout]` | **Built.** `{ deposit_height, bump }`. Its existence is the replay record |
| **`Config`** | `[b"config"]` | `{ maturity_blocks }` — **mutable only through the timelocked authority path** |

**`maturity_at_deposit` is load-bearing.** It records the maturity that applied when the deposit was
verified, so a later governance raise **cannot retroactively trap funds already in flight.** Without
it, a parameter that governance controls becomes a way to freeze other people's deposits.

---

## 3. Instructions

### `verify_deposit` — changed

Today it mints straight to the depositor. It must instead:

1. Verify inclusion, the script, confirmations — unchanged.
2. Create the `DepositNullifier` — unchanged (replay).
3. Create the `StagedMint`, recording **`maturity_at_deposit` read from `Config` at this moment.**
4. **Mint `solBSV` into the `Vault`**, not to the recipient.

### `release_mint(staged)` — permissionless

Releases the vaulted tokens to `staged.recipient`. Requires **all** of:

- the client is **fresh** — last accepted header within `lc.max_staleness_slots`
- `tip_height >= staged.deposit_height + staged.maturity_at_deposit`
- the client's **stored hash at `deposit_height` still equals `staged.deposit_hash`**
- the client still holds that height (it has not left the window)

**Anyone may call it.** That is what makes the exit real: the recipient does not depend on a relayer
to release their own tokens.

### `burn_staged(staged)` — permissionless

Burns the vaulted tokens when the chain has moved against the deposit. Requires:

- the client's **stored hash at `deposit_height` differs** from `staged.deposit_hash` — a reorg was
  followed
- **and** `tip_height >= deposit_height + maturity_at_deposit`

**Anyone may call it**, and it should pay a small bounty from the vault so a stranger has a reason to.
Burning is the correct answer when the BSV is gone: the tokens must not exist.

### `set_maturity(blocks)` — timelocked authority

Goes through the **existing** `propose_authority_change` / `execute_authority_change` path. **No new
authority mechanism.**

---

## 4. Why the release check is three conditions, not one

- **Staleness alone** would release against a view of the chain that has not seen the reorg.
- **Depth alone** would release even if the client's record of that height had already changed.
- **Hash equality alone** would release immediately, which is the maturity-0 behaviour and the reason
  the window exists.

**All three together are what make the claim "this deposit survived" mean something.**

---

## 5. Why maturity is a stored parameter

Not a constant, for four reasons:

1. **The PoC ships at 0**, which is the decision. The mechanism is present.
2. **The burn path is testable.** A constant at 0 can never be exercised; a parameter can be set to
   non-zero in a test, so `burn_staged` is **proven by test rather than asserted.**
3. **Governance can raise it** without a redeploy, through the path that already exists.
4. **It makes the honest sentence available:** *the vault is built; the protective window is currently
   0 and can be raised.*

---

## 6. What is still open

| | |
|---|---|
| **`FLOOR`, `D_MIN`, `C_payout`** | Not needed for the vault. They are redemption parameters |
| **The bounty for `burn_staged`** | Suggested, not sized. `fee.bounty_share` is `open` |
| **N5 — the reported spent-outpoint record** | **Decided:** the federation reports, and the program checks mints against it. **Not built.** The permissionless alternative was **considered and rejected**: making the record writable by anyone turns mint availability into an attack surface, not just mint correctness |

## 7. What the tests do NOT cover

**Recorded because the alternative is a document claiming more than it verified** — which is this
project's most repeated failure.

| | |
|---|---|
| **`StaleClient`** | **Cannot be exercised on a local validator.** The 54,000-slot bound is unreachable in a test run of minutes. The condition is implemented and **untested** |
| **`AlreadyStaged`** | Untested — a replay is refused earlier, by `AlreadyMinted`, so the second guard never fires on its own |
| **`WrongStagedMint`** | Untested |
| **`DepositHeightNotInWindow` on release and burn** | Untested |
| **The burn bounty** | Not built, and not sized. `fee.bounty_share` is `open` |
| **`burn_staged` freshness** | **No staleness check**, by design — doc 31 §3 lists two conditions for the burn and freshness is not one. Burning is the safe direction, so an old view is less dangerous here than on release, but the asymmetry is deliberate and worth knowing |

**`burn_staged` IS exercised**, at a non-zero maturity, with a followed reorg: `NotMatured` is
reachable, `DepositHashUnchanged` flips to `DepositHashChanged`, and the burn zeroes the vault with the
supply delta asserted. And a later maturity raise leaves in-flight staged items at the value they were
staged with — **proven, not merely asserted.**

## 8. The honesty requirement

**Every document and the status page must carry the maturity-0 consequence.** The vault is built; the
protective window is 0; the twelve-confirmation delay is what currently protects a deposit; and the
reversal is a mechanism that exists and is tested, not a property the running system has.
