# SOLBEAM

**A wrapped-BSV token on Solana.** SOLBEAM issues `solBSV` — an SPL token, 8 decimals, no freeze
authority — backed 1:1 by BSV held by a bonded federation, following the architecture of **RenVM**
and adding **reorg protection the reference does not have**. Peg only: there is no order book, no
leverage and no exchange mechanism. `1 solBSV` is always a claim on `1 BSV`. MIT, proof of concept.

**The honest one-liner.** A Solana program **verifies** BSV deposits itself — proof of work under
BSV's real difficulty rule (`cw-144`) and Merkle inclusion — where RenVM trusts its shards to
*report* a lock. The tokens a deposit creates land in a program-owned **vault**, not the depositor's
wallet, which is what makes a followed reorg **reversible**. The federation is an **accepted oracle**
for the one thing Solana cannot see (which deposit outpoints are spent): *the program verifies
deposits; the federation reports backing.* The reserve is trusted, and bounded by a 2-of-2 script,
two-sided bonds and an exit that can never be paused.

**→ If you are reviewing, start at [08. Status and roadmap](08-status-and-roadmap.md).** It is the
built-versus-designed line, what we would attack, and the roadmap.

---

## The document set

| | |
|---|---|
| [01. What it is](01-what-it-is.md) | The problem, the pitch, the brand, the vocabulary and the common questions |
| [02. How it works](02-how-it-works.md) | Architecture, the light client, the vault, peg-in and peg-out |
| [03. The federation](03-the-federation.md) | Membership, the reserve script, governance, slashing — and the RenVM comparison |
| [04. The flow](04-the-flow.md) | Every actor's journey, with the failure cases beside the step that can fail |
| [05. Trust model](05-trust-model.md) | What is verified, what is trusted, what is not protected — and the audit findings |
| [06. Parameters](06-parameters.md) | Every value the system turns on, with its status |
| [07. Decisions](07-decisions.md) | What is settled, what is deferred, and every reversal with its reason |
| [08. Status and roadmap](08-status-and-roadmap.md) | **The reviewer entry point.** Built vs designed, what we would attack, what ships when |
| [09. Costs](09-costs.md) | What it costs to run, and who pays |
| [10. Audit history](10-audit-history.md) | How the design got here, and the honest record of what was wrong |
| [11. Peg-out — spec and default parameters](11-peg-out-spec.md) | The redemption half: the flow, the `po.*` defaults, and what it does not solve |
| [12. Federation operations](12-federation-operations.md) | What a member does, what a member is exposed to, and how little of it exists — the operational companion to 03 |

---

## Built, designed, trusted

The distinction the rest of this set is careful about:

- **Built** — the light client with `cw-144`, the `solBSV` token, the mint (`verify_deposit`), fork
  staging, the nullifier, the timelocked authority, and **the vault** (`release_mint`,
  `burn_staged`, `set_maturity`). **27 instructions, 77 passing / 0 failing.**
- **Designed, not built** — the **federation**, the **Greycore**, **peg-out** ([spec](11-peg-out-spec.md), with defaults now fixed), and **governance**.
  The reserve script's *shape* is accepted by the code; the threshold key and second quorum that
  would fill it are not.
- **Trusted** — the checkpoint, the reserve, and the federation's spent-outpoint report; all
  off-chain, none enforceable by the program.

**The vault is built, and its protective window is 144 BSV blocks (~24 hours)**, by stored
parameter. It shipped at 0, where the burn predicate was satisfied *instantly* rather than
unreachable, so release and burn were a **race** and release normally won; 144 blocks is what gives a
followed reorg a window in which the staged mint can be burned. **`MIN_CONFIRMATIONS = 12` (~2 hours)
is still the prevention; the window is the reversal.** See
[08. Status and roadmap](08-status-and-roadmap.md) §2.

A full account of what is built, what is not, and what the tests do not cover is in
[08](08-status-and-roadmap.md); the running history of the claims this project got wrong is in
[10. Audit history](10-audit-history.md).
