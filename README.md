# SOLBEAM

**A wrapped-BSV token on Solana, pegged one-for-one.**

One `solBSV` is always backed by one BSV in the reserve. **Peg only:** no exchange mechanism, no
order book, no leverage. Branding **SOLBEAM**, ticker `solBSV`, 8 decimals, MIT.

**The system has three parts:**

1. **A light client on Solana** that verifies BSV itself. It holds a checkpoint and a rolling
   window of **192** BSV headers, verifies proof of work against BSV's real difficulty rule
   (**cw-144**, verified against **324/324** real mainnet headers) and verifies Merkle inclusion of
   a transaction in a block.
2. **A vault.** Every mint lands in a program-owned token account rather than the depositor's. It
   leaves when the program is satisfied, and it can be burned if it isn't.
3. **A bonded federation** holding the reserve under a **threshold key**, relaying headers, signing
   payouts and challenging theft. **Open membership, 1,000 BSV bond**; members run software, not
   judgement.

## What is trustless, and what is not

| | |
|---|---|
| **Minting** | **Trustless, given the deployed program.** The program verifies BSV proof of work and Merkle inclusion directly: no member's signature, no committee vote and no oracle mints anything. **The program's upgrade authority is the one exception** — it can re-anchor the checkpoint, so in production it must be threshold-held and timelocked, and the 30-day exit is the real guarantee |
| **Reversal** | **Trustless.** The program compares its own stored header hash against the one a deposit was proven with. A reorg is a fact about headers, not a report from anyone |
| **The reserve** | **Trusted, and bounded.** The BSV is under a threshold key, so no single member can move it. What protects you is a **bond anyone can seize by proving misbehaviour on-chain** |

**The one trust assumption: a threshold of federation members do not collude.** It is not
eliminated; it is bounded — by a bond that must cover the non-bonded supply, by an exit window
that never closes, and by proofs anyone can submit. It does **not** make collusion unprofitable:
the bond is `solBSV`, so a colluding threshold recovers its own bond and keeps the honest members'
bonds and the non-member supply with it.

## Governance, and the floor that is an exit

| | Default |
|---|---|
| **Who may propose** | Any member |
| **To pass** | **85% of pledged coins** |
| **Delay** | **30 days**, signalled live from the moment a proposal is raised |
| **Includes** | **The upgrade authority** |
| **Cannot touch** | **Redemptions. They are never pausable.** A pause stops **mints only** |

**There is no immutable floor, deliberately.** A hostile change needs 85% *and* 30 days, and
redemptions run throughout — so a proposal that would harm holders empties the bridge before it
lands. **The floor is the exit window, not a constitution.** The residual, stated plainly: a holder
who does not watch and does not act within 30 days is exposed.

The fee is a **governed 30 bp**, not a discovered one. Slashing is **self-proving equivocation**:
members sign payout intents individually, so a member who signs two conflicting intents has
produced its own evidence, and anyone may submit it.

## Where it stands

**Built and tested:** the light client with cw-144, the `solBSV` token, the mint, and fork staging
with chainwork — **24 on-chain tests**, **51/51** synthetic Phase 1A checks, **21/21** against a
live SV Node. The window is **192 records of 52 bytes**, `SPACE` **10,107** of 10,240, a
**32-hour** deposit lifetime. F7 (the retarget), P2 (the fork re-anchor), A7 (double-mint) and the
window resize are all fixed in code.

**Designed, not built:** the **vault**, the **federation** (threshold custody, governance,
slashing) and **all of peg-out**. Those are a specification, not a property of the code; the
shipped program mints straight to the depositor's token account.

**Removed:** the order book and discovered fees, per-relayer independent keys, and the
"no governance" posture.

**Still open, and not dressed up:** the vault's design carries unfixed audit findings and is being
re-audited against this model; the genesis bootstrap has no path (members bond `solBSV`, which
does not exist until a mint happens); sharding the threshold key is undecided; the DAA is
hard-coded; and F6, the replay-list ceiling, is a hard **200 peg-ins per 32-hour window**.

**Nothing here is audited. Do not put money in it.**

| | |
|---|---|
| Website | <https://solbeam.me> |
| The model, in one document | [`docs/13-summary.md`](docs/13-summary.md) — read this first |
| The federation in detail | [`docs/23-federation.md`](docs/23-federation.md) |
| Documentation | [`docs/`](docs/README.md) — the GitBook |
| Proof of concept | [`poc/`](poc/README.md) — test code, fixtures, scripts and the phased plan |
| Licence | MIT |

---

## Repository layout

| Path | What it is |
|---|---|
| `docs/` | The project documentation. The model, the federation, trust model, parameters, roadmap, FAQ |
| `website/` | The static site at `solbeam.me`. Deployed by a Cloudflare Worker named `solbeam-main`, configured by `wrangler.jsonc` at the repo root — see [`website/README.md`](website/README.md) |
| `poc/` | The proof of concept: Python checkers, fixtures, scripts and the phased plan. Deliberately disposable — the production stack is a separate decision, made on the evidence this produces |
| `workstreams/` | Measurement workstreams. [`W1`](workstreams/W1-light-client-verification.md) closed the difficulty rule and the window resize |
| `GITHUB_SETUP.md` | How this account, its keys and its deploy key are set up |

---

## Development

```bash
bash poc/checks/run_all.sh      # the BSV checker suite — needs only Python 3
```

The checkers run anywhere Python runs. Two validate against live chain data over the network; the
rest are offline.

---

## Licence

MIT — see [`LICENSE`](LICENSE). Everything here is intended to be usable, forkable and reviewable by anyone.

The token ticker is **`solBSV`** (display name SOLBEAM).
