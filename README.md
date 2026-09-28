# SOLBEAM

**Atomic wrapper on Solana for BSV.**

`solBSV` is a 1:1 wrapper for native BSV on Solana: minting is verified by a BSV light client running on Solana, and redemption is permissionless and bonded. In the finished design every mint lands in a **program-owned vault** first and is released only after maturity. A relayer is a role **anyone may run** — there is no privileged operator — and the program consults **no oracle**: it reacts only to BSV headers and Solana slots.

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

A BSV deposit is proved against proof of work and proof of inclusion, and the design places the mint in a program-owned vault rather than with the depositor. There is no attestor to bribe, no oracle to spoof, and no committee to capture — the program reacts only to BSV headers and Solana slots, and external metrics are published on the website and never consulted by it.

Redemption needs a BSV signature, and BSV Script cannot verify Solana's consensus — so a key must exist somewhere. There is no pooled hot wallet: each relayer holds its own deposits and posts a bond in `solBSV` (the same asset as the exposure, so no price move can shrink it relative to what it protects). A relayer is a role anyone may run, so the design has **no privileged operator**, and the bond makes cheating **punishable**.

Bonding can make cheating unprofitable. It cannot make it impossible, and it does nothing against someone who takes a relayer's key and never posted a bond. That is stated plainly, with its mitigations, in [`docs/04-trust-model.md`](docs/04-trust-model.md#the-naked-option-attack).

**One limit worth naming here.** Trading is outside the protocol's control: if a fraudulent mint ever succeeded, the loss would land on whoever bought the unbacked token, and the protocol cannot compensate them — which is why the design stages every mint in the vault instead of releasing it at once.

---

## Status — built vs designed

**Early, and the split matters.**

**Built and tested:** the light client, the `solBSV` token and the mint — **20 on-chain tests**, plus 21/21 checks against a live SV Node and the full Python checker suite. The shipped program mints straight to the depositor's token account.

**Designed, not built:** the vault and its two gates, the order book, staking and bonds, per-relayer deposits, and **all of peg-out**. Those are a specification at this point, not a property of the code. See [`poc/TEST_PLAN.md`](poc/TEST_PLAN.md) §0 for the honest baseline and [`docs/14-decisions.md`](docs/14-decisions.md) for the settled decisions.

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
