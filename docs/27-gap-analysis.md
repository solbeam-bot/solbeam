# 27. Gap analysis — SOLBEAM against RenVM, mechanism by mechanism

**Purpose.** Doc 26 records *what we take from RenVM and why*. This document does the harder
thing: it puts our **end state, as designed**, beside **how RenVM actually works**, one mechanism at
a time, and gives each row a verdict — so a reviewer can see at a glance what we **take**, what we
**change**, what we **add**, what we **leave out**, and where we are simply **behind**.

**The RenVM column is taken from the sources, not from doc 26.** The sources are quoted and linked
throughout; every claim about what RenVM does carries its URL in [Sources](#sources).

---

## Built, and designed — the line this document never crosses

This is the single most important distinction in the document, and it is stated once here so the rest
of it does not have to hedge every sentence.

| | |
|---|---|
| **BUILT (in the code today)** | The **light client with cw-144** (27 → 34 on-chain tests; **160 real mainnet headers** through `push_header`), the **`solBSV` token**, the **mint** (`verify_deposit`), **fork staging** (`init_staging` / `push_fork_header` / `commit_fork` / `abandon_staging`), the **nullifier** (one PDA per `(txid, vout)`, with `prune_nullifier`), and the **authority timelock** (`TIMELOCK_SLOTS = 32` plus propose / execute / cancel) |
| **DESIGNED, NOT BUILT** | The **vault**, the **Greycore**, the **reserve script**, **peg-out**, and **governance** |
| **DOES NOT EXIST** | **`release_mint` and `burn_staged` are now BUILT** (this document was written the day before the vault landed) in the program. The staged-release and reorg-reversal *design* has no implementation. Do not read the design sections of docs 13, 21 or 24 as a description of running code |

Where a row below says a mechanism is designed, it means **designed, not built**, and that is a gap
RenVM does not have — RenVM shipped.

---

## The four settled decisions this document reflects

These are the current end state. Three of them **supersede doc 26**, which is called out in the
relevant rows.

1. **The bond is the FLOAT for transfers, not a vault ratio.** What constrains the reserve is the
   **Greycore co-signature**.
2. **Threshold is `4-of-N`**, with **`N` a variable**. **Leaver-share invalidation is deliberately
   unresolved**: a departing member keeps a valid share, so the effective threshold **degrades with
   churn**. Recorded as **open for finalisation**.
3. **The federation is an accepted oracle** for what Solana cannot see (spent deposit outpoints).
   **"The program verifies deposits. The federation reports backing."**
4. **The Greycore is a 2-of-2 co-signer on the reserve script:**
   `OP_2 <gateway threshold key> <greycore key> OP_2 OP_CHECKMULTISIG`. Its members are **trusted
   third parties with reputations, not node operators**.

> **Doc 26 is superseded on three points.** §3 said the second quorum "does **not** co-sign payouts";
> decision 4 makes it a co-signer on the reserve script. §5's `n/t` capacity arithmetic and the
> `3-of-5` threshold are replaced by decision 1 (bond = float, Greycore constrains the reserve) and
> decision 2 (`4-of-N`). Doc 23's "ordinary P2PKH address, no script" is superseded by the 2-of-2
> script in decision 4.

---

## How to read the verdicts

| Verdict | Meaning |
|---|---|
| **Take** | Same mechanism as RenVM, adopted for the same reason |
| **Change** | RenVM has an analogue; we adapt it, and the reason is in §*What we change* |
| **Add** | RenVM has no equivalent at all |
| **Omit** | RenVM has it; we deliberately do not |
| **Weaker** | RenVM does this better than we do |
| **Open** | Our design is **not specified** — no inference is offered |

---

## Row-by-row comparison

| Mechanism | RenVM | SOLBEAM (end state, as designed) | Verdict |
|---|---|---|---|
| **How a mint is authorised** | A gateway shard observes the lock on the origin chain, checks the UTXO, its uniqueness, and **6 confirmations**, then threshold-signs a minting signature; the **Greycore adds a second signature**. The Ethereum gateway contract checks validity and uniqueness of the signature ([Gateways](https://raw.githubusercontent.com/wiki/renproject/ren/Gateways.md), [Sharding](https://raw.githubusercontent.com/wiki/renproject/ren/Sharding.md)). Safety rests on: *"RenVM will never produce a minting signature unless it has witnessed a respective lock"* ([Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md)) | The **Solana program verifies the BSV chain itself** — cw-144 proof of work and Merkle inclusion — in `verify_deposit`. **No signature, no committee and no oracle mints.** The federation is an **accepted oracle** only for what Solana cannot see. Built (`verify_deposit`), minus the vault step | **Add** |
| **How a release is authorised** | A shard observes the **burn** on the host chain (12 confirmations), threshold-signs the BSV transaction, and the Greycore co-signs; safety property 4 is *"RenVM will never produce a releasing signature unless it has witnessed a respective burn event"* ([Gateways](https://raw.githubusercontent.com/wiki/renproject/ren/Gateways.md), [Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md)) | **Designed, not built.** Escrow `solBSV` in the vault, members sign payout intents individually on Solana, the threshold key signs the BSV payment, the payout is proved against the light client, and the escrow burns; permissionless cancel after the deadline. **Plus** the Greycore's 2-of-2 co-signature on the reserve script (decision 4). No `release` instruction exists | **Change** |
| **What the host chain trusts** | Ethereum trusts a **signature** (`rsv`) plus a uniqueness hash (`nhash`). It does **not** verify Bitcoin; the shard is the oracle and the bond is what keeps it honest ([Gateways](https://raw.githubusercontent.com/wiki/renproject/ren/Gateways.md)) | Solana trusts **its own verification** of BSV for deposits. For **backing** — spent deposit outpoints Solana cannot see — it trusts the **federation's report** (decision 3): *"The program verifies deposits. The federation reports backing."* | **Add** for deposits, **Weaker** for backing |
| **Reorg handling** | **Not addressed in the sources.** If a lock is observed and then reorged away, the source has no mechanism to notice or to reverse the mint; the bond and the challenge are the only recourse, after the fact ([Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md) contains no reorg mechanism) | **Designed.** The client keeps a header window and a stored block hash per staged mint; a followed reorg is **visible on-chain** and the staged tokens **burn**. Fork staging is **built** (`init_staging`, `push_fork_header`, `commit_fork`, `abandon_staging`); the reversal itself (`release_mint` / `burn_staged`) is **designed, not built** | **Add** |
| **Custody structure** | One **threshold ECDSA key per gateway shard**, generated by RZL/z0 sMPC, held by **100 Darknodes**, *"never seen by anyone"*; the gateway script is `ghash OP_DROP OP_DUP OP_HASH160 pub_key_hash160 OP_EQUALVERIFY OP_CHECKSIG`; keys rotated **every epoch** ([Sharding](https://raw.githubusercontent.com/wiki/renproject/ren/Sharding.md), [Gateways](https://raw.githubusercontent.com/wiki/renproject/ren/Gateways.md)) | **Designed, not built.** The reserve script is `OP_2 <gateway threshold key> <greycore key> OP_2 OP_CHECKMULTISIG` (decision 4). The gateway key is a threshold key over the federation at **`4-of-N`**, `N` variable (decision 2). The reserve never moves when membership changes — re-sharing, not migration (doc 23) | **Change** |
| **Second quorum** | The **Greycore**: a shard selected by **community governance**, of members *"that have developed reputations"*; *"gateway shards cannot mint or release assets without this second signature."* It exists to make an attacker *"attack gateway shards as if there was no Greycore, and then also attack the Greycore"* ([Sharding](https://raw.githubusercontent.com/wiki/renproject/ren/Sharding.md), [Greycore](https://raw.githubusercontent.com/wiki/renproject/ren/Greycore.md)) | The **Greycore is a 2-of-2 co-signer on the reserve script**: `OP_2 <gateway threshold key> <greycore key> OP_2 OP_CHECKMULTISIG` (decision 4). Members are **trusted third parties with reputations, not node operators**. Same *shape* as RenVM — two independent quorums, the second appointed for reputation — but enforced **in the script**, not by a second signature on a threshold key. **Designed, not built**; **supersedes doc 26 §3**, which said it does not co-sign | **Take** (shape), **Change** (mechanism) |
| **Membership admission** | Bond **100,000 REN** in the Darknode Registry, then *"wait until the beginning of the next epoch before being admitted into a shard"* ([Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md)). Selection into shards is **random and unbiased** ([Sharding](https://raw.githubusercontent.com/wiki/renproject/ren/Sharding.md)) | **Open entry** on posting both bonds (doc 23). **But**: the ceremony that admits a member into the `4-of-N` threshold set is **not specified** for the `4-of-N` end state, and `N` is a variable. `fed.threshold` is `open` (doc 24) | **Open** |
| **Membership exit** | Deregister, then *"wait until the beginning of the next epoch, and then another full epoch, before being able to withdraw their bond"* — because *"Darknode bonds that have not been withdrawn can be slashed"* ([Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md)) | Announce, wait the unbonding period, withdraw only if still covered on **both** sides (doc 23). `fed.unbond_slots` is **`open`**. **And a departing member keeps a valid share** — see the next two rows | **Open / Weaker** |
| **Key rotation** | **Every epoch**: gateway shards in `E` generate keys for `E+1` and then *"sign transactions that forwards all assets to the newly generated"* keys — *"required so that, as Darknodes exit the network, they do not continue to know shares of actively used ECDSA private keys"* ([Sharding](https://raw.githubusercontent.com/wiki/renproject/ren/Sharding.md), [Gateways](https://raw.githubusercontent.com/wiki/renproject/ren/Gateways.md)) | **Deliberately omitted.** The built program fixes `deposit_script` once in `initialize_bridge`, with **no instruction to change it** — so rotation is currently unimplementable, and adopting it would have made every deposit to the old address unprovable (doc 26 §2) | **Omit** |
| **Leaver-share invalidation** | Implied by rotation: by the next epoch a departed node holds **no shares of a live key**, so the signing set is exactly the current set ([Sharding](https://raw.githubusercontent.com/wiki/renproject/ren/Sharding.md)) | **Deliberately unresolved — "open for finalisation."** A departing member **keeps a valid share**, so the **effective threshold degrades with churn**: a set of former members can still contribute shares toward `t`. Proactive re-sharing — which would invalidate old shares of the *same* key — is **not specified anywhere** (decision 2, doc 26 §2) | **Weaker** |
| **Slashing — equivocation** | `slashDuplicatePropose` / `slashDuplicatePrevote` / `slashDuplicatePrecommit` on the DarknodeSlasher contract: *"Darknodes that propose, prevote, or precommit two different blocks in the same height and round can have their bonds slashed."* The node's **own two conflicting signatures are the entire proof** ([Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md); contract shape in doc 23) | **Same cryptographic half, on intents instead of blocks.** Members sign payout intents individually on Solana, so signing two conflicting intents produces the member's own proof; anyone submits it, anyone takes the bounty (docs 13, 23). **Designed, not built** | **Take** |
| **Slashing — bad mint/release** | **Challenge-and-prove, shard-wide.** A challenger posts a bond alleging a mint with no lock (or a release with no burn); the shard must produce an **SPV proof before the end of the next epoch**, or *"every Darknode in the challenged shard will have their bond slashed."* If the proof succeeds the challenger's bond goes to the prover ([Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md)) | **Not needed for a mint**: a fraudulent mint is **rejected at the instruction** because the header either meets cw-144 or it does not. **For a release there is no enforceable predicate** — doc 23 **deleted** the "intent matching no authorised redemption" row because a *closed* `PegOut` is indistinguishable from one that never existed (audit F8). The challenge-and-prove backstop is designed (doc 26 §4) but **not built**, and its release-side predicate is **not specified** | **Weaker** |
| **Bond purpose and size** | **100,000 REN** per Darknode; it *"sizes the system"* — the goal is `B` at least **3×** `L` in a shard. Bonds are slashed for equivocation and for a challenged bad signature. REN is used **solely** for bonding, so its value derives from fees, and node operators adjust fees to keep it high enough ([Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md), [Fees-and-Economics](https://raw.githubusercontent.com/wiki/renproject/ren/Fees-and-Economics.md)) | **The bond is the FLOAT for transfers, not a vault ratio** (decision 1). Two-sided bonds: a **BSV-side bond outside the reserve**, and a **`solBSV`-side bond seizable on Solana** (docs 13, 24). **What constrains the reserve is the Greycore co-signature**, not a bond-to-reserve ratio. Size is a placeholder (`fed.bond_mint` / `fed.bond_redeem`, 1,000 BSV) and **not specified** as a security number | **Change** |
| **Capacity constraint** | `3L < B` and `L < B`, enforced **economically** by mint, burn and continuous **fee curves** in `L/B`: as `L` approaches `B/3` the mint fee approaches 100% and the burn fee approaches 0%. Bribery is unprofitable while the shard's bonds exceed 3× the assets. **No price oracle**: Darknodes use a discounted-cash-flow valuation of their own bond ([Fees-and-Economics](https://raw.githubusercontent.com/wiki/renproject/ren/Fees-and-Economics.md), [Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md)) | **A different constraint.** The bond is the **float for transfers**; the reserve is constrained by the **Greycore co-signature** (nothing moves the reserve without it). The earlier vault-ratio arithmetic (`bsv_bond ≥ k × BSV held`, `fed.k = 1`, the `n/t` multiple) is **superseded** and no replacement numeric capacity rule is **specified**. No fee curve, no oracle | **Open / Weaker** |
| **Fee model** | Minting **0.1%**, burning **0.1%**, **continuous fee** (usually 0% in practice, ~1% p.a. in the example) charged per second by lowering the exchange rate, plus **underlying chain fees**. Curves are **governed** and adjust with `L/B` ([Fees-and-Economics](https://raw.githubusercontent.com/wiki/renproject/ren/Fees-and-Economics.md), [Gateways](https://raw.githubusercontent.com/wiki/renproject/ren/Gateways.md)) | **30 bp gross to mint and 30 bp gross to redeem**, both governed. The fee covers real transaction costs and the remainder is the bonded members' income, pro rata to stake (docs 13, 24). **No continuous fee. No order book.** One cost exceeds the fee at the minimum deposit — first-time ATA rent — and it is **refundable** | **Change** |
| **Governance** | Darknodes **vote on fees and parameters**; the Greycore is chosen **by community governance**, and inactive members can be pruned ([Fees-and-Economics](https://raw.githubusercontent.com/wiki/renproject/ren/Fees-and-Economics.md), [Greycore](https://raw.githubusercontent.com/wiki/renproject/ren/Greycore.md)) | **85% of pledged coins, 30-day delay, live signal**, holds the **upgrade authority**, redemptions **can never be paused**. The floor is the **exit window**, not an immutable rule; `gov.delay_min` is `open` (docs 13, 23). **Designed, not built** — except the **authority timelock**, which is built (`TIMELOCK_SLOTS`) | **Change** |
| **Sharding / isolation** | Darknodes are partitioned into **randomly sampled, continuously shuffling shards**; *"any successful attack on a gateway shard is isolated to that shard"* and can only remove *"`1/N` amount of assets."* Assets are **load-balanced** so every shard holds the same ([Sharding](https://raw.githubusercontent.com/wiki/renproject/ren/Sharding.md)) | **No sharding.** `fed.shards` is **`open`** (doc 24). A single reserve under a single `4-of-N` key, plus the Greycore — so the blast radius is **100% of the reserve**, not `1/N` | **Weaker** |
| **Emergency pause** | Not described as a pause in the sources. Liveliness against a Greycore attack is recovered by **governance removing unresponsive members** ([Greycore](https://raw.githubusercontent.com/wiki/renproject/ren/Greycore.md)) | **`pause_mints()` only.** Redemptions **cannot be paused, ever**; a pause **lifts automatically** unless renewed, and carries a lower threshold than a governance change because the power is bounded (docs 13, 23). Authority changes have a **32-slot timelock, built** | **Add** |
| **What the user trusts** | A **<1/3 threshold of a 100-node shard**, the **Greycore**, and the bond economics. Ethereum is a **trusted computation engine** ([Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md)) | The **Solana program verifies deposits**; the **federation reports backing** — an accepted oracle for spent outpoints (decision 3); the **Greycore co-signature** constrains the reserve (decisions 1, 4). The trust assumption is that **neither a `4-of-N` of the federation nor the Greycore colludes** | **Change** |

---

## What we take unchanged — and why the precedent is good

1. **The Greycore shape: a second, appointed quorum with a reputation stake.** RenVM's argument is
   that an attacker *"must still attack gateway shards as if there was no Greycore, and then also
   attack the Greycore itself"* ([Greycore](https://raw.githubusercontent.com/wiki/renproject/ren/Greycore.md)).
   That argument transfers to any second set, and it is the reason the Greycore is in our design at
   all. The **precedent is good** because it was deployed by a live bridge, and because the choice of
   members is deliberately **accountable** rather than anonymous.

2. **Self-proving equivocation slashing.** RenVM's slasher takes a node's own two conflicting
   signatures as the whole proof ([Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md)).
   This is the one slashing mechanism RenVM **actually shipped**, and doc 23 is explicit that we copy
   the shipped half. The **precedent is good** because it needs no attribution, no vote and no
   inference — the misbehaviour *is* the evidence.

3. **The threshold-key custody primitive.** RenVM's threshold ECDSA key is *"never seen by anyone"*
   and a signing requires a threshold of nodes ([Sharding](https://raw.githubusercontent.com/wiki/renproject/ren/Sharding.md)).
   We use the same primitive, for the same reason: **no single member can move the reserve**.

4. **The economic framing of safety.** RenVM's insight is that the bond is the real security budget
   and the fee is the lever ([Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md),
   [Fees-and-Economics](https://raw.githubusercontent.com/wiki/renproject/ren/Fees-and-Economics.md)).
   We take the framing: the bonds are the risk, the fee share is the return, and whether that trade is
   attractive is a market question.

5. **No price oracle.** RenVM explicitly declines an oracle in favour of a discounted-cash-flow
   valuation by the nodes themselves. Our two-sided bond design reaches separation **without** an
   oracle for the same reason (docs 13, 24).

---

## What we change, and why

Each of these is a real difference, with the reason rather than just the fact.

| RenVM | SOLBEAM | Why |
|---|---|---|
| **~100 Darknodes per shard, corrupted at 1/3+** | **`4-of-N`, `N` variable** | A five-or-so-member federation cannot carry a 1/3 threshold meaningfully, so we lowered `t` to near-unanimity for a small set. **The cost is in §Where we are weaker**, and the leaver problem is the sharp edge |
| **Greycore co-signs a threshold-key signature** | **Greycore co-signs the reserve script**, `OP_2 <gateway key> <greycore key> OP_2 OP_CHECKMULTISIG` | With threshold-ECDSA custody there is one key and no place to attach a second signature — so the second quorum had to move **into the script**. RenVM solves the same problem with an MPC-level second signature; we solve it with `OP_CHECKMULTISIG`. This is decision 4 and **supersedes doc 26 §3** |
| **`3L < B` capacity, enforced by fee curves** | **The bond is the float for transfers; the Greycore co-signature constrains the reserve** | The vault-ratio version did not reconcile: a symmetric bond gave `H ≤ 0` (zero capacity), and the adopted 3× was RenVM's own `n/t` for 100 nodes at 1/3 — it did not transfer. Rather than force a ratio, capacity is bounded by the **second quorum's willingness to co-sign**. Decision 1 |
| **The shard witnesses and reports the lock; Ethereum trusts the signature** | **The program verifies the BSV chain; the federation reports backing only where Solana cannot see** | A Solana program can check proof of work and Merkle inclusion itself, so it does — removing the need to trust a committee for deposits. But Solana cannot see BSV's **UTXO set**, so spent-outpoint knowledge remains an oracle. Decision 3 |
| **Key rotation every epoch** | **No rotation** | Rotation exists to stop a **departed** member holding live shares. We removed it for a concrete reason: the built `initialize_bridge` fixes `deposit_script` once, so the reserve address cannot change and old-address deposits would become unprovable. **This is an omission, not a free win** — see below |
| **Sharding for isolation (`1/N` loss)** | **One reserve, no shards** | Five members do not shard. Sharding is the **scale path**, not the proof of concept. The cost is that an attack is not contained |
| **Node voting on fee curves; continuous fee** | **85% supermajority, 30-day delay, live signal, redemptions never pausable** | A small set needs a slow, visible supermajority rather than a continuous fee market. The protection is the **exit window** during the delay, not immutability |
| **Challenge-and-prove is the primary defence against a fraudulent mint** | **A fraudulent mint is rejected at the instruction; challenge-and-prove is a backstop** | Under our model a mint cannot happen without a verified header, so the after-the-fact challenge is no longer load-bearing for mints. It remains designed for the release side, where **no enforceable predicate is specified** |
| **REN-only bond, no oracle** | **Two-sided bonds in the asset each side holds** | Each side must be seizable where the liability sits; a `solBSV`-side bond is seizable on Solana, a BSV-side bond is native BSV. No external price is consulted |

---

## What we add

**This is the contribution, and it should be stated as such.**

> **RenVM trusts its shards to *report* a lock. We *verify* it.**

RenVM's safety property is that it *"will never produce a minting signature unless it has witnessed a
respective lock"* ([Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md)).
**"Witnessed" means the Darknodes saw it and agreed** — and Ethereum takes their signature as the
proof. The source chain is trusted **through the federation**.

SOLBEAM inverts that. Concretely, we add:

1. **A light client on Solana.** It verifies BSV **proof of work against the real difficulty rule
   (cw-144)** and **Merkle inclusion** of the deposit. No signature, no committee and no oracle can
   mint. **Built**, including 160 real mainnet headers through the `push_header` instruction path.
2. **The vault.** Every mint lands in a **program-owned token account**, not the depositor's wallet.
   A staged token is not in anyone's wallet, so there is nothing to dump and no innocent buyer to
   inherit the loss. **Designed, not built.**
3. **Reorg reversal.** The program stores the block hash a deposit was proven against and compares
   its own headers. If the hash at that height **differs**, the deposit was reorged and the staged
   tokens **burn**. A reorg is a fact about headers the program already keeps, not a report from
   anyone. **Fork staging built; the reversal designed, not built** — there is no `release_mint` and
   no `burn_staged` in the code.
4. **The nullifier**, one PDA per `(txid, vout)`, replacing a bounded used-deposit list so replay is
   per-account rather than a capacity ceiling. **Built.**
5. **The 147-record seed and header window**, so a reorg is **visible on-chain** at all. **Built.**

**The reference does not address the reorg case.** If a lock is observed and then reorged away, RenVM
has no mechanism to notice and none to reverse the mint; the bond and the challenge are the only
recourse, **after** the fact and only if someone notices. **Ours notices on-chain and reverses it.**
That is why the light client and the vault exist.

**What it buys:** minting that does not depend on the federation being honest. The federation's trust
assumption narrows to **custody of the reserve and backing Solana cannot observe** — not to the truth
of every deposit.

**What it costs:** a header window, cw-144, a seed at `initialize`, a vault, a maturity period, and the
147-record bootstrap. RenVM avoids all of it by trusting its own shards to report. We chose not to.

---

## What we deliberately leave out

Two mechanisms RenVM has, and we do not. **The cost of each is stated, not softened.**

### 1. Key rotation — omitted

RenVM regenerates every gateway key each epoch and forwards all assets to the new key, *"required so
that, as Darknodes exit the network, they do not continue to know shares of actively used ECDSA
private keys"* ([Sharding](https://raw.githubusercontent.com/wiki/renproject/ren/Sharding.md),
[Gateways](https://raw.githubusercontent.com/wiki/renproject/ren/Gateways.md)).

**Honest cost.** Removing rotation removes a real protection. A member who **leaves, is removed, or
is compromised** keeps shares of a **live** key indefinitely. Rotation would have bounded that to one
epoch. We give that bound up, and the only replacement — proactive re-sharing — is **not specified
anywhere**. There is also a hard implementation fact: the built program fixes `deposit_script` once in
`initialize_bridge`, so rotation is **currently unimplementable** and adopting it would have made
every deposit to the old address unprovable.

### 2. Leaver-share invalidation — omitted, and recorded as open for finalisation

RenVM does not need a separate mechanism, because rotation **is** the invalidation: after the epoch
boundary, a departed node holds no shares of an active key.

**Honest cost.** A departing member **keeps a valid share**. The **effective threshold degrades with
churn**: former members can still contribute shares toward `t`, so a set that was `4-of-N` at
genesis may be signable by fewer current members after departures. **This is deliberately
unresolved** (decision 2) and is the sharpest open problem in the federation design. It is listed
again below, because it belongs in the weaker-than-RenVM list and not only in an omissions list.

---

## Where we are weaker than RenVM

**This is the most useful section for a reviewer, so it is not softened.**

1. **RenVM ran a BFT consensus with 100-node shards; we run `4-of-N` with `N` a variable.**
   RenVM's safety holds while **less than 1/3** of a 100-node shard is adversarial — on the order of
   **34 coordinating nodes** ([Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md)).
   Our threshold is **4 members**, and `N` is not even fixed. A **four-person collusion** is a far
   smaller, far more findable, far more bribeable set than 34 anonymous Darknodes. Near-unanimity of
   a small set is **not** equivalent to a supermajority of a large one.

2. **RenVM sharded for isolation; we do not.** RenVM's shards mean *"any successful attack on a
   gateway shard is isolated to that shard"* and can only remove *"`1/N` amount of assets"*
   ([Sharding](https://raw.githubusercontent.com/wiki/renproject/ren/Sharding.md)). With one reserve
   and no shards, our **blast radius is 100% of the reserve**. `fed.shards` is `open`, which means
   this is not a deliberate scope decision so much as an unbuilt one.

3. **RenVM's capacity and bond economics are specified; ours are a float.** RenVM states `3L < B` and
   `L < B`, gives **governed fee curves** that enforce them, and explains how nodes value their bonds
   without an oracle ([Fees-and-Economics](https://raw.githubusercontent.com/wiki/renproject/ren/Fees-and-Economics.md)).
   Our bond is *"the float for transfers"*, the vault-ratio arithmetic (`fed.k = 1`, the `n/t`
   multiple) is **superseded**, and **no replacement numeric capacity rule is specified.** A reviewer
   cannot compute our capacity from our documents, and that is a genuine gap.

4. **The leaver-share problem is unaddressed.** RenVM guarantees that a departed member holds **no
   shares of a live key** (rotation). We guarantee the opposite by omission: a departing member keeps
   a valid share, the effective threshold **degrades with churn**, and invalidation is **deliberately
   unresolved**. This is the single clearest case where RenVM is stronger and we know it.

5. **RenVM's slashing is enforced on-chain by contracts; half of ours is a social action.** RenVM's
   bonds live in the Darknode Registry and the Slasher, so *"an adversary cannot escape the slashing
   conditions"* ([Safety](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md)).
   Our BSV-side bond sits under the **collective key**, and seizing it requires the members to
   *choose* to sign — the duty is **social, not on-chain**, and a majority could seize an honest
   member's bond (docs 13, 23).

6. **Our challenge-and-prove has no enforceable predicate on the release side.** RenVM's challenge
   has a bonded accuser, an SPV-proof deadline and a shard-wide penalty. We **deleted** the
   "intent matching no authorised redemption" row because the predicate is undecidable (audit F8), so
   for releases we have a designed mechanism whose triggering condition is **not specified**.

7. **RenVM shipped; our second half is designed.** The vault, the Greycore, the reserve script,
   peg-out and governance are **designed, not built**, and the vault's design has already failed two
   audits. RenVM ran in production. Whatever our design's merits, **the implementation gap is real**,
   and `release_mint` / `burn_staged` now exist and are permissionless — with the shipped maturity at 0, so the protective window is off by parameter.

---

## Sources

RenVM is the authority. These are the pages fetched and read for this document:

- [Sharding](https://raw.githubusercontent.com/wiki/renproject/ren/Sharding.md) — gateway / coordination / Greycore shards, 100 nodes, epochs, rotation, DKG, random selection, load balancing
- [Safety and Liveliness](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md) — assumptions, safety properties, `1/3` thresholds, bonds and slashing, challenge-and-prove, the `3L < B` and `L < B` constraints, Sybil/bribery analysis
- [Gateways](https://raw.githubusercontent.com/wiki/renproject/ren/Gateways.md) — lock/mint and burn/release flows, confirmation depths, gateway script template, epoch rotation, fee curves
- [Greycore](https://raw.githubusercontent.com/wiki/renproject/ren/Greycore.md) — the Greycore's two purposes, selection, safety and liveliness arguments
- [Fees and Economics](https://raw.githubusercontent.com/wiki/renproject/ren/Fees-and-Economics.md) — mint/burn/continuous/underlying fees, bonding, economic-security constraints, no price oracle

**Fetched, and 404 — they do not exist at these paths:**
`Darknode-Registry.md`, `Darknode-Slasher.md`, `Epochs.md`. Where this document states what those
pages describe (the 100,000 REN bond, the deregistration delay, the duplicate-propose slashing
functions), the statement is taken from
[Safety and Liveliness](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md),
which repeats it. The **DarknodeSlasher function names and the "placeholder" quotation** are cited in
doc 23 from
[`renproject.github.io/ren-client-docs`](https://renproject.github.io/ren-client-docs/contracts/darknode-sol/DarknodeSlasher)
and were **not re-fetched here**; they are used only as doc 23 uses them.

---

## Open items this document does not close

1. **Leaver-share invalidation** — deliberately unresolved; no mechanism specified (decision 2).
2. **`fed.threshold` and `N`** — `4-of-N` is settled as a *shape*, but `N` and the admission ceremony
   for the threshold set are **not specified**.
3. **Capacity** — the bond is the float and the Greycore constrains the reserve, but **no numeric
   capacity rule is specified** to replace the superseded vault-ratio arithmetic.
4. **The release-side challenge predicate** — undecidable as written; the release path has no
   enforceable slashing condition (audit F8).
5. **`fed.shards`** — unsharded today; no stated threshold at which sharding becomes required.
6. **Built / designed** — the vault, the Greycore, the reserve script, peg-out and governance are
   **built**, and the burn path is exercised by test; the shipped maturity is 0, which removes the protective window.
