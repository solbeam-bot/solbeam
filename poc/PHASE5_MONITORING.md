# Phase 5 — public monitoring on solbeam.me

**Status: plan only.** Nothing here is implemented, and none of it needs to be until Phases 1–3 produce data worth showing.

The idea: the site stops being a brochure and becomes an instrument. Instead of *asserting* the trust model, it publishes the numbers a user needs in order to **judge the chain's security for themselves** — and publishes them so anyone can recompute them.

---

## 1. The one rule

**Every number is a derivation, not a claim. Show the inputs and the method, or don't show the number.**

If a reader cannot recompute a figure from public data, it does not belong on the page. This is the same rule as the documentation's copy discipline: never imply more safety than the trust model supports.

---

## 2. Metric 1 — the absolute cost to attack BSV

The question: *what would it cost to rewrite the BSV chain?* Published as an **absolute number**.

### Why not a ratio

An earlier draft of this plan proposed `safety_multiple = attack_cost / value_reorgable`. That was wrong, and it is worth recording why.

A ratio quietly assumes a **rational, profit-maximising attacker** who strikes only when the arithmetic favours them. That assumption fails often enough to be dangerous:

- An attacker may be **short** `solBSV`, BSV, or something correlated. The payoff then comes from the position, not from the peg — the attack only has to *work*, not to pay for itself.
- An attacker may want to **destroy confidence** in the peg, which can be worth more to a competitor than any amount of stolen BSV.
- Some attackers are simply not optimising, and the ones who are not are exactly the ones a ratio cannot model.

So the honest product is: **publish the absolute cost, publish how much hash power is actually available to rent, and let the reader judge.** Do not do the risk arithmetic *for* them with a single number that implies an actor who may not exist.

### Inputs

| Input | Source | Reading at 2026-09-26 |
|---|---|---|
| BSV network hash rate `H` | BSV node `getnetworkhashps`, or `difficulty × 2³² / T` | **≈ 200 EH/s** (difficulty 27,874,022,525) |
| **Total SHA-256 available for rent** `A` | hashpower marketplaces, summed — each with its own timestamp | **≈ 85 EH/s** — NiceHash ≈ 28 EH/s + MiningRigRentals ≈ 56 EH/s |
| Rental price `R` | the same venues | **≈ 0.50 BTC per EH per day** (NiceHash ask) |
| Block interval `T` | BSV target | 600 s |

The market figures come from live public APIs and are **snapshots**, not constants — they moved within the hour this was written. The indexer must record the **venue, the timestamp and the figure**, and the page must show all three. A hashpower availability number without a timestamp is meaningless.

### The feasibility gate — put this first

```
attack is possible by rental   ⟺   A  >  H
```

**At the readings above, it is not.** ~85 EH/s is available to rent; BSV is running at ~200 EH/s. You cannot rent a majority of BSV's hash rate at *any* price, because that much hash power is not for sale.

That is a far more useful statement than any cost figure, and it belongs at the top of the page. It also explains why this metric is worth building at all: **it is a live, falsifiable measurement of a security property, not an assumption.** It can stop being true — the rental market can grow, an attacker can accumulate orders slowly and quietly, or a large incumbent can redirect hash power it already owns.

### The absolute cost, when it is feasible

```
cost(N)  =  X × R × ( N × T × H / (X − H) )         for X > H
```

where `X` is the hash rate the attacker commands. Making `X` explicit is the point — it exposes how little the margin costs:

| Attacker's rate | Time to catch up `N` blocks | Cost |
|---|---|---|
| `X = 2H` (2× BSV's rate) | ≈ `N × T` | **2 × N × T × H × R** |
| `X = 4H` | ≈ `N × T / 3` | ≈ 1.33 × the floor |
| `X → ∞` | → 0 | → **N × T × H × R** — the floor |

Publish the curve for `N = 1, 6, 12, 100`, in BTC and in USD with the BTC price as a stated input, labelled a **lower bound**. An attacker using hardware it already owns does not pay the rental price at all — it pays an opportunity cost.

### The realistic vector is redirecting, not renting

The rental market is the wrong model for the most plausible attacker. SHA-256 hardware is fungible across BTC, BCH and BSV, so a large incumbent can **point existing hash power at BSV** and forgo what it would have earned elsewhere. Its cost is **forgone mining revenue**, anchored to prevailing SHA-256 revenue per hash — typically *higher* than the rental rate, because the rental rate clears below the mining margin.

This is why `A` and `H` are the news and the price is secondary: **the binding constraint on attacking BSV is not the price of hash power, it is whether enough of it can be brought to bear at once.**

### Why publishing this is uncomfortable, and still right

BSV shares SHA-256 with BTC and BCH, so its security is priced in the most liquid mining market in existence. A chain with a unique algorithm has a structurally higher floor. That is a real, permanent feature of BSV — not a temporary condition — and a holder deciding how much to keep in flight deserves to see it rather than infer it.

The useful response is not a bigger ratio. It is **observable numbers plus a depth parameter that can be raised when they move**: if the feasibility gate closes, the confirmation requirement should rise with it. That is a governance decision made in public, on a published measurement — see §6.

### What this is not

Not a security proof. `H` is an estimate derived from difficulty; `A` is *advertised* availability, which fluctuates and which an attacker can game by accumulating orders quietly; and an attacker who already owns hash power pays less than any published figure. Every number carries its source and its timestamp.

---

## 3. Metric 2 — value at risk, published *alongside*, never as a quotient

Still worth showing, because it is the next thing a reader wants — but as **separate absolute figures**, not as a ratio:

| Figure | What it is |
|---|---|
| `value_reorgable(N)` | What a reorg of depth `N` could reverse. For **mints**: deposits confirmed but not yet beyond the reorg horizon, plus — the term that actually matters — the `solBSV` **already issued** from a deposit young enough to be reversed. That is the most an attacker can extract for the price of one reorg. For **redeems**: payouts broadcast inside the settlement window |
| `value_in_flight` | Deposits awaiting depth, redemptions awaiting payout |
| `exposed_supply` | Total `solBSV` whose backing sits on a chain that could be reorged at the current depth |

Show them next to the absolute attack cost and let the reader form their own view. Explicitly **do not** render a derived "safety multiple": it invites the rational-actor assumption §2 explains is unsafe, and it hides the two numbers that actually matter.

**The one thing we can honestly state for the reader**, next to the figures:

> The cost of a reorg does not scale with the size of your deposit. It costs the same to reverse one deposit or a thousand — which is precisely why an attacker's motive may have nothing to do with the amount at stake.

That is the insight a ratio destroys, and it is the reason the absolute numbers are the product.

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

1. **An adaptive confirmation recommendation** — "12 today; 48 right now, because the rentable-hashpower gate closed" — derived from §2's feasibility measurement rather than fixed. This is the honest use of the numbers: they change a **parameter**, in public, instead of producing a comfort score.
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

1. Attack cost is a **lower bound** derived from a rental price. An attacker using hardware it already owns pays an opportunity cost instead — every figure carries its source and its timestamp.
2. Hash rate is an **estimate**, derived from difficulty; the estimator and its window are named.
3. **Rentable hashpower is advertised availability, not a ceiling.** It fluctuates, and it can be accumulated quietly over time. The feasibility gate is a live measurement, not a proof.
4. **Do not model the attacker as rational.** The page shows absolute numbers and computes no "safety" score, because an attacker may be short the asset, may want to destroy confidence, or may not be optimising at all.
5. **No per-user data, ever.** No deposit addresses, no wallet links, nothing that de-anonymises a holder. Aggregate only.
6. Nothing on the page may imply the peg is safer than [`docs/04-trust-model.md`](../docs/04-trust-model.md) says it is. Minting is trustless; redemption is trust-minimised.

---

## 9. Suggested sub-phases

| | Content | Why this order |
|---|---|---|
| **5a** | `stats.json` plumbing, reserve/supply, **the invariant** | Highest value and lowest risk: one call to Solana, one balance read, one comparison |
| **5b** | BSV hash rate, **total SHA-256 available for rent**, the feasibility gate, the absolute attack-cost curve, and `value_reorgable` alongside it | Needs 5a's data and the marketplace APIs. The genuinely novel part — and the one that can change a confirmation parameter |
| **5c** | Flow, latency, bond coverage, relayer table | Needs Phases 1–3 running to have anything to report |
| **5d** | History, alerts, parameter log | Only useful once there is a history worth keeping |

**5a is worth doing early even if nothing else happens**: publishing the reserve address and the supply, and letting anyone check `reserve ≥ supply`, is a stronger statement about this project than any page of copy.

---

Next: [Test plan](TEST_PLAN.md) · [Adversary playbook](ADVERSARY_PLAYBOOK.md) · [Trust model](../docs/04-trust-model.md)
