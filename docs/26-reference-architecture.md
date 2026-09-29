# 26. The reference architecture — what we take from RenVM, and why

SOLBEAM follows the architecture of **RenVM** (which issued wrapped ZEC, and is the model
referred to throughout as "ZEC-style"). This document records what that architecture actually is,
**what we take from it, what we adapt, and what we cannot take** — and which of the outstanding
audit findings each part resolves.

Read from the source rather than recalled:
[RenVM Sharding](https://raw.githubusercontent.com/wiki/renproject/ren/Sharding.md) and
[Safety and Liveliness](https://raw.githubusercontent.com/wiki/renproject/ren/Safety-and-Liveliness.md).

**This supersedes several things written earlier in this repository, each marked below.**

---

## The reference, in one page

**Three kinds of shard:**

| | |
|---|---|
| **Gateway shards** | Randomly selected, 100 Darknodes each. Generate, hold and rotate a **threshold ECDSA key** used to custody assets. A shard is *corrupted* at **1/3+** adversarial |
| **Coordination shard** | 100 nodes. Decides which nodes are in which shard, and orders transactions. **Holds no funds** |
| **Greycore** | Selected **by community governance**, not randomisation — "Darknodes that have developed reputations with the community". **Acts as a secondary signature for every gateway shard** |

**The double signature is the Greycore.** In the source's own words:

> *"gateway shards **cannot mint or release assets without this second signature**."*

**Two independent quorums must both agree**, and the second one is chosen by governance rather than
by chance.

**Epochs.** A discrete interval (24 hours) at which everything rebases:

- pending registrations **activate**, pending deregistrations **deactivate**
- existing nodes are **shuffled** into new shards
- **all gateway keys are regenerated** — and *"the gateway shards in E sign transactions that
  **forwards all assets** to the newly generated" keys*

**Membership.**

| | |
|---|---|
| **Join** | Bond 100,000 REN, then **wait until the next epoch** before admission |
| **Leave** | Deregister, then wait **until the next epoch *and* one full epoch** before withdrawing |
| **Why** | *"Darknode bonds that have not been withdrawn can be slashed"* — **you remain slashable until your bond is out** |

**Slashing — two mechanisms, and only one of them is about attribution.**

1. **Equivocation.** Proposing, prevoting or precommitting two different blocks at the same height
   and round. Both conflicting signatures are submitted to the slasher contract. Self-proving.
2. **Challenge-and-prove, shard-wide.** A shard that signs a mint **without witnessing a lock**, or a
   release **without witnessing a burn**, can be challenged. **The challenger posts a bond.** The
   shard must produce an **SPV proof** before the **end of the next epoch** — or *"**every Darknode
   in the challenged shard** will have their bond slashed."* If the proof succeeds, the challenger's
   bond goes to the prover.

**Capacity is an economic constraint, not a script:**

> *"a bribery attack is not profitable as long as the sum value of all REN bonds of all Darknodes in
> the shard is **greater than 3x the value of origin assets locked in the shard**."*

The **3×** falls out of the **1/3** threshold: a briber must buy 1/3 of the bonds, so those bonds
must be worth three times the assets for the bribe to cost more than it wins. **And the ratio is
managed by fees, not an oracle** — nodes vote on mint and burn fees to keep the bond valuable
enough. *"No explicit price oracle is needed."*

---

## What we take

### 1. Epochs, for membership — resolves "who may join, and how"

The answer to *how does a relayer join or leave* is **not a migration of the reserve**. It is a
**rebasing**: register, wait for the epoch boundary, and you are in. Deregister, wait one further
epoch, and your bond is returned.

**And the extra epoch after deregistration is the part that matters** — it is what keeps a departing
member **slashable while their shares are still live.**

### 2. ~~Key rotation every epoch~~ — **not adopted**

RenVM regenerates every gateway key each epoch and forwards the assets to the new one, because a
departing node would otherwise still know shares of a live key. **We do not need this.**

**Rotation exists to bound what a member who has *left* can do with shares they still hold.** But in
our design a member who has left **cannot move the reserve** — the threshold sees to that — and the
rotation's real cost is high: the entire reserve moves on-chain every epoch, and the deposit script
the program verifies against would have to change with it.

**And the built program fixes `deposit_script` once in `initialize_bridge`, with no instruction to
change it.** So rotation is not merely unnecessary — it is currently unimplementable, and adopting it
would have made every deposit to the old address unprovable.

**Adopting the reference does not mean adopting all of it.** This is the clearest case: a mechanism
that solves a problem we do not have, at a cost we would have paid.

### 3. A second quorum — appointed, for change management and unwinding

The reference has the **Greycore**: a second set, chosen by community governance rather than by
chance, which **co-signs every gateway action**. We take the *shape* — a second set with a
supermajority threshold — but **not the per-payout co-signature**, because that does not compose with
threshold-ECDSA custody: one P2PKH address has one key, and a second signature would require a
multisig script, which is the design we deliberately rejected.

**So the second quorum is an appointed oversight body, not a co-signer:**

| | |
|---|---|
| **Size** | **5**, appointed at setup by the founders |
| **Who** | **Honest public actors** — reputational stake, not an anonymous set |
| **Threshold** | **4-of-5**, so a change needs near-unanimity and one dissenter can block |
| **Role** | **Managing change, and unwinding if it becomes necessary** |
| **Not a role** | It does **not** co-sign payouts, and it does not hold the reserve |

**What it is for.** Managing parameter changes, and — if the thing has to be wound up — having a
quorum with the standing to do it. It is the body that decides *what the rules are*, not one that
executes them.

**What it does not fix, stated plainly.** It is **not** a second signing quorum and therefore does
**not** stop a majority of the reserve signers seizing a bond. The earlier claim that a second quorum
resolves the collective-key problem **is withdrawn** — at `n = 5`, two disjoint 3-quorums need six
people, so "a second quorum of the same five" is the same people twice.

### 4. Challenge-and-prove slashing — resolves the attribution problem

The audit found that per-member attribution **does not work**: an off-chain threshold signature
reveals nothing about who signed, so "all signers are provably guilty" was false, and the
individual-attestation layer ("layer 2") caught only harmless minority equivocation.

**RenVM does not solve attribution. It avoids it.** Slashing is **shard-wide**: the whole quorum's
bonds are at stake unless the underlying event is proven.

**And this maps onto what we already have**, because proving a BSV event is exactly what the light
client does:

```
challenge:   an SPV proof that a mint happened with no corresponding lock
             (or a release with no corresponding burn)
response:    the challenged quorum must produce the SPV proof of the lock
             before the deadline
outcome:     proof produced -> challenger loses their bond to the prover
             no proof       -> EVERY bond in the quorum is slashed
```

**A bonded accuser, a deadline, and a collective penalty.** No attribution, no per-member proofs, and
no reliance on a culprit volunteering evidence — which the audit correctly identified as the fatal
flaw in layer 2.

### 5. `bonds ≥ 3 × assets` — resolves the capacity arithmetic

Our capacity claim was wrong: we sized the bond against the whole reserve, and with a symmetric
two-sided bond that gave `H ≤ 0`, i.e. **zero capacity.**

**The reference gives the rule.** Capacity is **bonds ÷ 3**, for a 1/3 threshold, because a briber
must buy a third of the bonds and must still lose money.

**And the ratio is held by fees, not an oracle** — which is the piece we were missing when we worried
that a `solBSV`-denominated bond devalues in exactly the scenario it protects against.

### 6. Sharding, for isolation

*"any successful attack on a gateway shard is isolated to that shard… `1/N` amount of assets."*
Assets are load-balanced so every shard holds the same. **Not needed for five members**, but it is
the scale path, and it is the answer to "what happens when the bridge grows".

---

## What we add that the reference does not have

**We follow RenVM for the federation layer, and add protections against reorg risk that the
reference does not have at all.** This is the project's actual contribution, and it is worth being
precise about, because the two halves are easy to conflate.

### RenVM's mint is the federation's word; ours is a proof

Under the reference, a mint happens because **the shard observed a lock on the origin chain**:

> *"RenVM will never produce a minting signature unless it has witnessed a respective lock on the
> origin chain."*

**"Witnessed" means the Darknodes saw it and agreed.** Ethereum takes their signature as the proof.
So the source chain is trusted **through the federation** — which is what makes the challenge-and-prove
mechanism necessary, and why its slashing rule is phrased as *"produce an SPV proof… or every bond in
the shard is slashed."* The federation is the oracle; the bond is what keeps it honest.

**SOLBEAM inverts this.** A Solana program **verifies the BSV chain itself** — proof of work against
the real difficulty rule (cw-144) and Merkle inclusion of the deposit. **No signature, no committee
and no oracle can mint.** The federation cannot lie about a deposit, because it is not asked.

**So the challenge-and-prove mechanism is a backstop for us rather than the primary defence.** In the
reference it is what stops a shard minting against a lock that never happened. For us a fraudulent
mint is not *provable-after-the-fact* — it is **rejected at the instruction**, because the header
either meets cw-144 or it does not.

### The reorg protections, specifically

| Protection | What it does | Does the reference have it? |
|---|---|---|
| **Light client on Solana** | Verifies BSV proof of work and inclusion directly | **No.** The shard observes and reports |
| **The vault** | Every mint lands in a program-owned account, not the depositor's | **No.** Mints go straight to the recipient |
| **Staged release, burned on a followed reorg** | A deposit reorged out after minting is **burned**, not left in circulation | **No.** There is nothing to reverse; the mint stands |
| **cw-144 on-chain** | BSV changes difficulty every block; the client follows the real rule | **No.** Difficulty is the source chain's business |
| **The 147-record seed + header window** | A reorg is *visible* — the client can see a stored hash change | **No.** No header history is kept on the host chain |

**The reorg case is the one the reference simply does not address.** If a lock is observed and then
the block is reorged away, the reference has no mechanism to notice, and no mechanism to reverse the
mint — the bond and the challenge are the only recourse, **after** the fact and only if someone
notices.

**Ours notices on-chain and reverses it.** A reorg is a fact about the headers the program already
stores; `burn_staged` compares its own record and burns. **That is the addition, and it is why the
light client and the vault exist.**

### What that buys, and what it costs

**Buys:** minting that does not depend on the federation being honest. The federation's trust
assumption narrows to **custody of the reserve**, not to the truth of every deposit.

**Costs:** complexity the reference does not carry — a header window, cw-144, a seed at
`initialize`, a vault, a maturity period, and the 147-record bootstrap that took three audits to get
right. **The reference avoids all of it by trusting its own shards to report.** We chose not to.

**So the honest summary:** the federation, the epochs, the second quorum, the challenge mechanism
and the `3×` capacity rule are **taken from the reference**. The **light client and the vault are
ours**, and they exist for exactly one reason — **to remove the reorg risk that the reference accepts
and cannot reverse.**

## What we adapt

| RenVM | SOLBEAM |
|---|---|
| Its own consensus chain (Hyperdrive) | **Not needed.** Solana is the state layer, and the program is the referee |
| **Coordination shard** — picks members, orders transactions, holds no funds | **Replaced by the Solana program**, which cannot lie about state |
| Greycore selected by community governance | **The founding set**, for a PoC — with the same *disjoint second quorum* structure |
| `REN` bond, REN-only, no oracle | **BSV-side and `solBSV`-side bonds**, per the two-sided model |
| 100 Darknodes per shard | **5 members**, with the threshold and the 3× rule doing the work |

**We gain one thing RenVM did not have:** we do not need a consensus protocol for the coordination
layer, because **Solana is the coordination layer and it cannot be equivocated with.**

---

## What this resolves from the audit

| Finding | Resolved by |
|---|---|
| **F5 — the mint gate is not expressible** | The gate becomes the **3× economic constraint**, checked where it can be (the `solBSV` side) and **declared and visible** on the BSV side. It is a solvency rule, not a per-mint script test |
| **The capacity arithmetic (`H ≤ 0`)** | **`capacity = bonds ÷ 3`**, derived from the 1/3 threshold rather than asserted |
| **Collective-key bonds are a hostage** | **The second quorum.** Neither set can move funds or seize a bond alone |
| **Attribution does not work** | **Challenge-and-prove, shard-wide.** Attribution is avoided, not solved |
| **Genesis first hour** | **The first epoch.** Founders form both quorums, bond, and the first mint is a founder's ordinary peg-in |
| **Threshold is unstated** | **1/3**, taken from the reference, with the key-extraction consequence it openly acknowledges |
| **Transparency is unverifiable** | Partly addressed: **fees are the lever**, and the ratio is a governed parameter — but see below |

## What this does **not** resolve

**Key extraction at the threshold is real, and RenVM says so.** *"a signature cannot be produced,
and the underlying ECDSA private key cannot be revealed — allowing an adversary to produce a
signature independently — unless >=1/3rd Darknodes are adversarial and coordinating."*

So **a 1/3 quorum can extract the key and then sign anonymously forever.** Slashing does not undo a
stolen key. **The reference accepts this, and so must we** — it is priced by the 3× rule, not
prevented.

**And the reserve is still off-chain.** Publishing it is still an assertion by the party that could
steal. The reference does not solve this either; it leans on the bond being larger than the assets,
which is the economic answer rather than a verification one.

---

## Accepted risks, deferred

Two things are **known to be unresolved, are recorded rather than fixed, and are deferred until
there is a team to work on them.**

### The `3×` rule and the two-sided bond do not reconcile

The reference requires **`bonds ≥ 3 × assets`**, derived from a **1/3** threshold: a briber must buy
a third of the bonds, so those bonds must be worth three times what they would win. Our parameters
carry **`fed.k = 1`**, and at `k = 1` a briber needs roughly `0.4 × assets` — so **the design is
about 3× under-collateralised against the rule it cites.** Either `k` becomes 3, or the capacity
figure is 3× too large. Neither is chosen.

**And the `solBSV`-side bond devalues in exactly the scenario it is meant to protect against** —
when the reserve is gone, `solBSV` is worth nothing, so seizing or burning it redistributes loss
among holders rather than restoring BSV. The BSV-side bond is under the same key as the reserve, so
if that key moves, it sweeps the bonds too.

**Accepted for now.** The honest statement is that **the bond deters and prices entry, and does not
restore what is lost.** Fixing this properly is a real piece of work and it is deferred.

### Collusion is unprevented

A majority of the reserve signers can take the reserve, and **no mechanism here prevents it.** The
bond does not stop it, the second quorum does not stop it, and the slashing layer cannot fire
against it. Recorded as a **stated, accepted risk**.
