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
3. **A bonded federation** holding the reserve under a **2-of-2 script with the Greycore**, relaying headers, signing
   payouts and challenging theft. **Greycore-admitted membership, 1,000 BSV float**; members run software, not
   judgement.

## What is trustless, and what is not

| | |
|---|---|
| **Minting** | **The deposit is verified; the backing is reported.** The program verifies BSV proof of work and Merkle inclusion directly. **The program verifies deposits; the federation reports backing** — it reports spent deposit outpoints because Solana cannot read the BSV UTXO set; not a new trust assumption. **The program's upgrade authority is the other exception** — it can re-anchor the checkpoint, so in production it must be threshold-held and timelocked, and the 30-day exit is the real guarantee |
| **Reversal** | **Trustless.** The program compares its own stored header hash against the one a deposit was proven with. A reorg is a fact about headers, not a report from anyone. **The vault is built, but its protective window ships at 0**, so the reversal is a race and release normally wins; what protects a deposit today is `MIN_CONFIRMATIONS = 12` — prevention, not reversal |
| **The reserve** | **Trusted, and bounded.** The BSV is under a **2-of-2 `OP_CHECKMULTISIG`** — the gateway's threshold ECDSA key plus the **Greycore**'s, both required — so no gateway majority and no Greycore can move it alone. What protects you is a **two-sided bond under the collective key** — the program seizes the `solBSV` side automatically; the members seize the BSV side by signing, which is a **collective action by the majority and a social duty, not an on-chain guarantee** |

**The one trust assumption: the gateway threshold and the Greycore do not collude.** It is not
eliminated; it is bounded — by the **Greycore co-signature** (the gateway majority cannot move funds
alone), by an exit window that never closes, and by proofs anyone can submit. It does **not** make
collusion unprofitable: a colluding gateway-plus-Greycore can take the reserve, and the maximum loss
is the entire non-member supply.

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

**Built and tested:** the light client with cw-144, the `solBSV` token, the mint, fork staging
with chainwork, the **nullifier**, the **timelocked authority**, and **the vault** (`release_mint`,
`burn_staged`, `set_maturity`) — **17 instructions, 45 passing / 0 failing**, **51/51** synthetic
Phase 1A checks, **21/21** against a live SV Node. The window is **192 records of 52 bytes**, `SPACE`
**10,107** of 10,240, a **32-hour** deposit lifetime. F7 (the retarget), P2 (the fork re-anchor), A7
(double-mint) and the window resize are all fixed in code.

**Designed, not built:** the **federation** (threshold custody, the Greycore, governance, slashing)
and **all of peg-out**. Those are a specification, not a property of the code. The vault's
**protective window is currently 0**, so the reversal is a race rather than a comfortable window.

**Removed:** the order book and discovered fees, per-relayer independent keys, and the
"no governance" posture.

**Still open, and not dressed up:** leaver-shares are unfinalised (a departing member keeps a valid
share, so the effective threshold degrades with churn); sharding the threshold key is undecided, so
the blast radius is 100% of the reserve; the DAA is hard-coded, so a BSV consensus change would halt
the bridge until governance acts; there is no numeric capacity rule (the bond is the float); and the
vault's protective window ships at **0**. The replay list (`MAX_USED = 200`) is **gone** — replay is a
nullifier PDA per deposit, so the 200-peg-in ceiling no longer exists. **Genesis is decided:** members
post a BSV-side bond, so no `solBSV` needs to exist first.

**Nothing here is audited. Do not put money in it.**

| | |
|---|---|
| Website | <https://solbeam.me> |
| Status, built vs designed | [`docs/08-status-and-roadmap.md`](docs/08-status-and-roadmap.md) — read this first |
| The federation in detail | [`docs/03-the-federation.md`](docs/03-the-federation.md) |
| Documentation | [`docs/`](docs/README.md) — the GitBook |
| Proof of concept | [`poc/`](poc/README.md) — test code, fixtures, scripts and the phased plan |
| Licence | MIT |

---

## Repository layout

| Path | What it is |
|---|---|
| `docs/` | The project documentation: what it is, how it works, the federation, the flow, the trust model, parameters, decisions, status and roadmap, costs, and the audit history |
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
