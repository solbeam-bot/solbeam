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

### 2. Key rotation every epoch

Directly from the source: a key is regenerated each epoch and **the assets are forwarded to it**,
because otherwise a node that leaves still knows shares of a live key — and *"after exiting the
network, Darknodes no longer have a bond that incentivises them against revealing their shares."*

**This corrects an earlier statement in this repository.** We wrote that under threshold ECDSA *"the
reserve never moves, the address never changes."* **It does move — once per epoch.** That is a
deliberate, scheduled migration rather than an ad-hoc one, and it is what bounds the damage a
departing member can do.

### 3. A second, governance-selected quorum — resolves the collective-key finding

The audit found that **collective-key bonds are a hostage, not a bond**: the same majority trusted
with the reserve controls every member's bond, so it can seize any member's bond at will, and the
bond therefore deters nothing it was meant to deter.

**The Greycore is the answer.** Two independent quorums must both sign — one selected by chance, one
selected by governance, with **disjoint membership**. Then:

- the gateway majority **cannot** move funds alone
- the Greycore **cannot** move funds alone
- **and neither can unilaterally seize a bond**

**For a five-member proof of concept, the Greycore is the founding set** — but the structure is what
matters, and it is the mechanism the earlier design lacked entirely.

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
