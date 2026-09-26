# Phase 5 — public monitoring on solbeam.me

**Status: plan only.** Nothing here is implemented, and none of it needs to be until Phases 1–3 produce data worth showing.

The idea: the site stops being a brochure and becomes an instrument. Instead of *asserting* the trust model, it publishes the numbers a user needs in order to decide **how much to have in flight right now** — and publishes them so anyone can recompute them.

---

## 1. The one rule

**Every number is a derivation, not a claim. Show the inputs and the method, or don't show the number.**

If a reader cannot recompute a figure from public data, it does not belong on the page. This is the same rule as the documentation's copy discipline: never imply more safety than the trust model supports.

---

## 2. Metric 1 — BSV hash rate → the cost to rewrite the chain

The question a user actually has: *if I deposit 5 BSV and it gets minted, what would it cost someone to undo that?*

**Inputs**

| Input | Source |
|---|---|
| Network hash rate | BSV node `getnetworkhashps`, or derived from header timestamps and difficulty |
| Block interval | ~600 s (BSV target) |
| SHA-256 hashpower rental price | a public hashpower marketplace, recorded with its timestamp and venue |

**The model, stated plainly.** Rewriting `N` blocks requires an attacker to out-produce the honest chain over those `N` blocks. The defensible headline is the cost of renting the network's hash rate for the time those blocks take:

```
attack_cost(N)  ≈  hashrate × rental_price_per_hash_second × N × T
```

That is a **lower bound**: at twice the network's rate the time halves but the rate doubles, so the product is `2 × N × T × R`. Publishing the minimum and labelling it as a minimum is the honest choice. Show the curve for `N = 1, 6, 12, 100`, not one number.

**The caveat that must appear next to it, prominently.** BSV mines **SHA-256** — the same algorithm as BTC and BCH. So the hash power an attacker can rent is **not bounded by BSV's own hash rate**; the relevant market is global SHA-256 rental, and it is deep. A chain with a unique algorithm has a higher floor on attack cost than one that shares the most liquid mining market in existence.

This is uncomfortable and it is exactly why it belongs on the page. It is an argument for deeper confirmation requirements than a naive "our hash rate is fine" comparison would suggest — and a user deciding position size deserves to know it.

**What this is not:** a security proof. An attacker with owned hardware pays less than the rental price. Hash rate is itself an estimate.

---

## 3. Metric 2 — value at risk, and the ratio that actually decides anything

Raw attack cost is a curiosity. The decision-relevant figure is:

```
safety_multiple(N)  =  attack_cost(N)  /  value_reorgable(N)
```

where `value_reorgable(N)` is what a reorg of depth `N` could reverse:

- **Mints:** the BSV value of deposits confirmed but not yet beyond the reorg horizon — plus, worse, the value of any `solBSV` **already issued** from a deposit young enough to be reversed. That second term is the one that matters: it is the maximum an attacker can extract for the price of a reorg.
- **Redeems:** the value of payouts broadcast inside the settlement window.

**Then do the arithmetic for the user.** Publish:

```
suggested_max_in_flight  =  attack_cost(N) / SAFETY_FACTOR
```

with `SAFETY_FACTOR` stated (a hundred is a reasonable start, and it should be a published parameter, not a secret). If a user's deposit is larger than that, the page says so: *too large for current chain conditions — split it, or wait for more confirmations.* That converts an abstract security parameter into a per-user instruction, which is the whole point of the feature.

---

## 4. Metric 3 — reserve and supply, as a check rather than a claim

| Figure | Source |
|---|---|
| `solBSV` supply | Solana `getTokenSupply` |
| Reserve at large | the covenant-locked cold address balance, read from BSV |
| Hot float | the relayer hot wallets |
| **The invariant** | `reserve ≥ supply`, shown as surplus or deficit |

**Publish the addresses.** A proof-of-reserves that says "here are the addresses, go and look" is stronger than a signed attestation, because it is independently checkable and cannot be quietly restated. Show the series over time, not a snapshot, so a deficit is visible *while it develops* rather than only in the post-mortem.

Display supply in both tokens and BSV, and show the peg-out queue next to it, because supply alone does not tell a user whether they can actually get out today.

---

## 5. Metric 4 — activity and health

- **Flow:** cumulative mints (count and BSV volume), cumulative redemptions, net supply, refunds, slashes.
- **Latency:** the distribution of deposit→mint and redemption→payout times. This is the honest answer to "how long does it take", and it will be uglier than a single advertised number — which is the point.
- **Bond coverage:** aggregate bond against `k × (hot float + releasable tranche)`, the number of active relayers, and the unbonding queue. Publishing this makes the trust model **live** rather than described; a reader can watch the safety factor instead of taking it on faith.
- **Fees** actually paid, not advertised.

---

## 6. Metric 5 — candidates worth building next

1. **An adaptive confirmation recommendation** — "12 today; 48 right now, because the safety multiple is low" — derived from §3 rather than fixed.
2. **A peg-out time estimator** from the current float and tranche schedule.
3. **A relayer table** — uptime, missed deadlines, capital locked. Clearly labelled as a **market signal for choosing** a relayer, never as enforcement. Reputation does not slash anyone.
4. **Alerts**: an RSS or webhook feed for invariant breaches, slashes, and parameter changes.
5. **A parameter-change log**, wired to the governance timelock, so the history of every cap and fee is public.

---

## 7. Architecture — deliberately boring

```
BSV node ─┐
Solana RPC ─┼─► indexer (Python, every 60 s) ─► stats.json ─► R2 / Worker KV
bridge accounts ─┘                                                    │
                                                                      ▼
                                                    solbeam.me reads and renders
```

- **Python indexer** (decision 3), one file output. No database, no server, no always-on process required: a scheduled job.
- **Cloudflare Worker Cron + KV, or R2** — the site is already a Worker (`solbeam-main`), so this adds a binding rather than new infrastructure.
- **The page fetches `stats.json` client-side and renders it as progressive enhancement.** If the fetch fails, the existing page is unchanged — the marketing site must never depend on the monitoring working.
- History is appended as JSONL if series are wanted; that is the only place a store becomes necessary.

---

## 8. Honesty rules, to be visible on the page itself

1. Attack cost is a **lower bound** derived from a rental price, not a guarantee.
2. Hash rate is an **estimate**; the estimator and its window are named.
3. The safety multiple is a **heuristic**, not a security proof.
4. The reorg model assumes **rational, profit-seeking** attackers, not a motivated adversary who will spend more than they gain.
5. **No per-user data, ever.** No deposit addresses, no wallet links, nothing that de-anonymises a holder. Aggregate only.
6. Nothing on the page may imply the peg is safer than [`docs/04-trust-model.md`](../docs/04-trust-model.md) says it is. Minting is trustless; redemption is trust-minimised.

---

## 9. Suggested sub-phases

| | Content | Why this order |
|---|---|---|
| **5a** | `stats.json` plumbing, reserve/supply, **the invariant** | Highest value and lowest risk: one call to Solana, one balance read, one comparison |
| **5b** | Hash rate, attack cost, `value_reorgable`, the safety multiple | Needs 5a's data and a rental-price source. The genuinely novel part |
| **5c** | Flow, latency, bond coverage, relayer table | Needs Phases 1–3 running to have anything to report |
| **5d** | History, alerts, parameter log | Only useful once there is a history worth keeping |

**5a is worth doing early even if nothing else happens**: publishing the reserve address and the supply, and letting anyone check `reserve ≥ supply`, is a stronger statement about this project than any page of copy.

---

Next: [Test plan](TEST_PLAN.md) · [Adversary playbook](ADVERSARY_PLAYBOOK.md) · [Trust model](../docs/04-trust-model.md)
