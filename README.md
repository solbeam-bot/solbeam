# SOLBEAM

**Atomic wrapper on Solana for BSV.**

`solBSV` is a 1:1 wrapper for native BSV on Solana: minting is verified by a BSV light client running on Solana, and redemption is permissionless and bonded.

| | |
|---|---|
| Website | <https://solbeam.me> |
| Documentation | [`docs/`](docs/README.md) — the GitBook; read this first |
| Proof of concept | [`poc/`](poc/README.md) — throwaway test code, fixtures, scripts and the phased plan |
| Licence | MIT |

---

## Repository layout

| Path | What it is |
|---|---|
| `docs/` | The project documentation. Trust model, relayers, parameters, roadmap, FAQ |
| `website/` | The static site at `solbeam.me`. Deployed by a Cloudflare Worker named `solbeam-main`, configured by `wrangler.jsonc` at the repo root — see [`website/README.md`](website/README.md) |
| `poc/` | The proof of concept: Python checkers, fixtures, scripts and the Phase 0–5 plan. Deliberately disposable — the production stack is a separate decision, made on the evidence this produces |
| `GITHUB_SETUP.md` | How this account, its keys and its deploy key are set up |

---

## The claim, and its limits

**Trustless in, trust-minimised out.**

A BSV deposit is minted against proof of work and proof of inclusion. There is no attestor to bribe, no oracle to spoof, and no committee to capture.

Redemption needs a BSV signature, and BSV Script cannot verify Solana's consensus — so a key must exist somewhere. SOLBEAM makes that key **small**, **collateralised in `solBSV`** (the same asset as the exposure, so no price move can shrink it relative to what it protects) and **punishable**.

Bonding can make cheating unprofitable. It cannot make it impossible, and it does nothing against someone who steals the hot key and never posted a bond. That is stated plainly, with its mitigations, in [`docs/04-trust-model.md`](docs/04-trust-model.md#the-naked-option-attack).

---

## Status

**Early.** The documentation is written. The proof of concept has Phase 1A passing — peg-in on the BSV side, offline, with a portable mint instruction, 136 checks — and needs an x86_64 host before the Solana and SV Node work can start. See [`poc/TEST_PLAN.md`](poc/TEST_PLAN.md) for exactly what is proven and what is not.

Nothing here is audited. Do not put money in it.

---

## Development

```bash
bash poc/checks/run_all.sh      # the BSV checker suite — needs only Python 3
```

The checkers run anywhere Python runs. Two validate against live chain data over the network; the rest are offline.

---

## Licence

MIT — see [`LICENSE`](LICENSE). Everything here is intended to be usable, forkable and reviewable by anyone.

The token ticker is **`solBSV`** (display name SOLBEAM).
