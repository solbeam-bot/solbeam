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

The multiple is **n/t**, and it falls out of the signing threshold: a briber must buy **t** of the
**n** bonds, so the bonds must be worth **n/t times** the assets for the bribe to cost more than it
wins.

> **This multiple is RenVM's, for its own parameters.** For RenVM that is 100/34 ≈ 3×. **It does not
transfer to us: our bond is the float — working capital for transfers — not a capital requirement
sized against the reserve.** See §5, where the `n/t` rule and the `~$180k` / `$300k` capacity figures
are **withdrawn**. **And the ratio is
managed by fees, not an oracle** — nodes vote on mint and burn fees to keep the bond valuable
enough. *"No explicit price oracle is needed."*

---

## What we take

### 1. Epochs, for membership — and the Greycore admits replacements

The answer to *how does a member join or leave* is **not a migration of the reserve**. It is a
**rebasing** at an epoch boundary: the bond is posted, the epoch turns, and the member is active;
deregister, wait one further epoch, and the bond is returned. **The Greycore finds and admits
replacement members** — it is the trusted, non-operational body, so admission is its job.

**And the extra epoch after deregistration is the part that matters** — it is what keeps a departing
member **slashable while their shares are still live.** It does **not** invalidate their share; that
is the leaver-shares problem (§2).

### 2. Key rotation — **not adopted, and now an open finalisation item**

RenVM regenerates every gateway key each epoch and forwards the assets to the new one, because a
departing node would otherwise still know shares of a live key. **We do not adopt it**, but the
reason is *not* that we do not need it: **a departing member keeps a valid share**, and at **4-of-N,
four leavers hold four valid shares.** The threshold degrades with churn, and rotation is one of the
two remedies.

**Rotation exists to bound what a member who has *left* can do with shares they still hold — and an
earlier revision of this document claimed, wrongly, that the threshold already handles that. It does
not.** The other remedy is **proactive re-sharing** — a protocol that invalidates old shares of the
*same* key. That is real cryptography, it requires the departing member's cooperation, and **it is
not specified anywhere.**

**Recorded as an open finalisation item, not as resolved** — doc 23, *Open — for finalisation:
leaver-shares*, carries the full statement. The costs of rotation remain real — the reserve moves
on-chain, and the deposit script would have to change with it — but they are costs, not a reason to
leave departed shares live.

**And the built program fixes `deposit_script` once in `initialize_bridge`, with no instruction to
change it.** So rotation is currently unimplementable, and adopting it would need that instruction
added and would make every deposit to the old address unprovable.

**Adopting the reference does not mean adopting all of it** — but here the reference's mechanism
points at a problem we do have, and the PoC deliberately leaves it unfinalised.

### 3. The Greycore **co-signs** the reserve — reversed from "oversight only"

The reference has the **Greycore**: a second set, chosen by community governance rather than by
chance, which **co-signs every gateway action**. In the source's own words, *"gateway shards cannot
mint or release assets without this second signature."*

**We adopt the co-signature, not just the shape.** The reserve script becomes a **2-of-2
`OP_CHECKMULTISIG`**:

```
OP_2  <gateway threshold key>  <greycore key>  OP_2  OP_CHECKMULTISIG
```

One leg is the gateway's threshold ECDSA key, which emits **one** signature however many members
signed; the other is the Greycore's. **Both must sign**, so:

- the gateway majority **cannot move funds alone** — this fixes the collective-key hostage problem
- the Greycore cannot move funds alone
- **the Greycore polices every reserve spend**, which is what the reference does

**This reverses the earlier conclusion in this document.** §3 used to say the second quorum was an
appointed oversight body that did **not** co-sign, because *"one P2PKH address has one key, and a
second signature would require a multisig script, which is the design we deliberately rejected."*
**That was the wrong way round:** it cited F10 to say the P2PKH check was correct and then used the
P2PKH shape to justify dropping the co-signature. With the Greycore adopted, **the multisig script is
the design**, and audit **F10 is reversed**: `is_p2pkh` must change and `DepositScript::SPACE` must
grow to ~71 bytes, against the current 38.

**Who the Greycore is.** RenVM's own words are *"Darknodes that have developed reputations with the
community"*, chosen by governance and with a stake in the system's safety. **Trusted third parties,
not node operators** — **people with reputations to lose, who do not run the reserve.** The Greycore
also **finds and admits replacement members.** We mirror the reference deliberately, because the
precedent is good. Its **size** and **threshold** are separately `open` (`fed.greycore_size`,
`fed.greycore_threshold`, doc 24).

**Open, for finalisation: leaver-shares.** A departing member retains a valid share, so the effective
threshold degrades with churn; at 4-of-N, four former members together hold four valid shares.
Resolving it needs **key rotation** (the reserve moves on-chain and `deposit_script` must change) or
**proactive re-sharing** (needs the departing member's cooperation). Doc 23 carries the full open
item.

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

### 5. The `n/t` capacity multiple is **demoted** — it is RenVM's calculation, not ours

**Withdrawn as a constraint on SOLBEAM.** The `n/t` multiple is RenVM's **bribery-cost calculation for
its own parameters**: a briber must buy `t` of the `n` bonds, so the bonds must be worth `n/t` times
the assets, which for RenVM's 100-node shard at a 1/3 threshold is `100/34 ≈ 3`. **It does not apply
to us, because our bond is the float** — working capital for transfers — **not a capital requirement
sized against the reserve.** There is no "assets locked" figure for it to be a multiple of.

**The arithmetic is kept as a correction rather than deleted, because it has already been wrong
twice:**

> **Correction 1.** We first sized the bond against the whole reserve, and a symmetric two-sided bond
> gave `H ≤ 0` — **zero capacity.** That was an impossible inequality, not a conservative one.
>
> **Correction 2.** We then adopted the reference's **3×** while running a `3-of-5` threshold, where
> the formula would give `5/3 ≈ 1.67×`. Quoting 3× **overstated capacity by 1.8×**, and `fed.k = 1`
> was then 1.67× short of a rule that did not apply.
>
> **Both are withdrawn.** The `~$180k` capacity figure (ten members at 1,000 BSV) and the `$300k`
> figure before it are **withdrawn**: they were the output of a sizing rule that does not hold.

**What constrains the reserve is the Greycore co-signature** (§3), not the bond. The bond's job is the
float, and the `k × (held)` line is a **coverage floor** on mint and exit, not a capacity ceiling. The
reference's fee-vs-oracle point still stands for keeping the bond's value up, but it is no longer
load-bearing for a capacity claim.

### 6. Sharding, for isolation

*"any successful attack on a gateway shard is isolated to that shard… `1/N` amount of assets."*
Assets are load-balanced so every shard holds the same. **Not needed for the current member count**,
but it is the scale path, and it is the answer to "what happens when the bridge grows".

---

## What we add that the reference does not have

**We follow RenVM for the federation layer, and add protections against reorg risk that the
reference does not have at all.** This is the project's actual contribution, and it is worth being
precise about, because the two halves are easy to conflate.

### RenVM's mint is the federation's word; ours is a verified deposit — plus a reported backing

Under the reference, a mint happens because **the shard observed a lock on the origin chain**:

> *"RenVM will never produce a minting signature unless it has witnessed a respective lock on the
> origin chain."*

**"Witnessed" means the Darknodes saw it and agreed.** Ethereum takes their signature as the proof.
So the source chain is trusted **through the federation** — which is what makes the challenge-and-prove
mechanism necessary, and why its slashing rule is phrased as *"produce an SPV proof… or every bond in
the shard is slashed."* The federation is the oracle; the bond is what keeps it honest.

**SOLBEAM verifies the deposit itself.** A Solana program **verifies the BSV chain** — proof of work
against the real difficulty rule (cw-144) and Merkle inclusion of the deposit. **But the program does
not see the UTXO set**, so it cannot tell an unspent deposit output from a spent one. The federation
therefore **reports spent deposit outpoints to Solana**, and the program checks a mint against that
record.

**This corrects the earlier blanket claim in this document.** This section used to say **"no
signature, no committee and no oracle can mint."** With a reported spent-outpoint record, the
federation **is** in the loop for backing verification, and **saying otherwise is now false.** The
honest restatement is two sentences: **"The program verifies deposits. The federation reports
backing."** And this is **not a new trust assumption:** the federation is already trusted with the
reserve, so asking it not to lie about a UTXO adds nothing — a party that can take the whole reserve
is not meaningfully constrained by an honesty request.

**So the challenge-and-prove mechanism is a backstop for us rather than the primary defence.** In the
reference it is what stops a shard minting against a lock that never happened. For us a fraudulent
mint is not *provable-after-the-fact* — it is **rejected at the instruction**, because the header
either meets cw-144 or it does not. **The one case the instruction cannot see by itself is a spent
output, and the reported record is what covers it.**

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
stores, and the vault is specified to compare its own record and burn. **That is the addition, and
it is why the light client and the vault exist.**

**And the vault is unbuilt.** There is no `release_mint` and no `burn_staged` in the program; today
`verify_deposit` mints straight to the depositor. The light client — the half that *detects* a
reorg — is built and verified. The half that *reverses* one is a specification.

### What that buys, and what it costs

**Buys:** minting whose **deposit** does not depend on the federation being honest. The federation's
trust assumption narrows to **custody of the reserve and the spent-outpoint record**, not to the truth
of every deposit's inclusion.

**Costs:** complexity the reference does not carry — a header window, cw-144, a seed at
`initialize`, a vault, a maturity period, and the 147-record bootstrap that took three audits to get
right. **The reference avoids all of it by trusting its own shards to report.** We chose not to, and we
still report the one thing Solana cannot see.

**So the honest summary:** the federation shape, the epochs, the challenge mechanism and the **Greycore
co-signature** are **taken from the reference**. Its **`n/t` capacity multiple is not** — our bond is
the float, and that arithmetic is **withdrawn** (§5). The **light client and the vault are ours**, and
they exist for exactly one reason — **to remove the reorg risk that the reference accepts and cannot
reverse** — with the spent-outpoint record supplying the one input Solana cannot see.

## What we adapt

| RenVM | SOLBEAM |
|---|---|
| Its own consensus chain (Hyperdrive) | **Not needed.** Solana is the state layer, and the program is the referee |
| **Coordination shard** — picks members, orders transactions, holds no funds | **Replaced by the Solana program**, which cannot lie about state |
| Greycore selected by community governance, and **co-signing every gateway action** | **Adopted.** The Greycore **co-signs every reserve spend** through the 2-of-2 script; it is **trusted third parties, not node operators**, and it **finds and admits replacement members**. **Not disjoint at genesis** — the founders appoint it |
| `REN` bond, REN-only, no oracle | **BSV-side and `solBSV`-side bonds** — the **float**, per the two-sided model, with a `k` coverage floor rather than a capacity rule |
| 100 Darknodes per shard | **4-of-N gateway members** (the number deferred) **plus the Greycore**, with the 2-of-2 co-signature doing the work |

**We gain one thing RenVM did not have:** we do not need a consensus protocol for the coordination
layer, because **Solana is the coordination layer and it cannot be equivocated with.**

---

## What this resolves from the audit

| Finding | Resolved by |
|---|---|
| **F5 — the mint gate is not expressible** | **Partly.** It stays a **solvency/coverage check** (`k × held`), not a per-mint script test, and **it is not built**. The `n/t` economic constraint is **withdrawn** (§5) |
| **The capacity arithmetic (`H ≤ 0`)** | **Withdrawn.** The `capacity = bonds ÷ (n/t)` derivation is RenVM's, not ours; the `~$180k` and `$300k` figures are **withdrawn** (§5). The bond is the float, and the reserve is constrained by the Greycore co-signature |
| **Collective-key bonds are a hostage** | **Resolved by the Greycore co-signature.** The gateway majority cannot move funds alone, because the reserve script is a 2-of-2 with the Greycore — see §3, which reverses the earlier "not resolved" |
| **Attribution does not work** | **Challenge-and-prove, shard-wide.** Attribution is avoided, not solved |
| **Genesis first hour** | **The first epoch.** Founders form both quorums, bond, and the first mint is a founder's ordinary peg-in |
| **Threshold is unstated** | **4-of-N, with `N` a variable and the number deferred** (doc 24); the Greycore's size and threshold are separately `open`. No longer `1/3`, and no longer claimed to be a 3×-priced rule |
| **Transparency is unverifiable** | Partly addressed: the **spent-outpoint record** is the mechanism for backing; **fees remain the lever** for the bond — but see below |

## What this does **not** resolve

**Key extraction at the gateway threshold is real, and RenVM says so.** *"a signature cannot be
produced, and the underlying ECDSA private key cannot be revealed — allowing an adversary to produce
a signature independently — unless >=1/3rd Darknodes are adversarial and coordinating."*

So **a quorum at the gateway threshold can extract the gateway key.** Slashing does not undo a stolen
key. **The Greycore co-signature is what an extracted gateway key alone cannot satisfy:** the reserve
script requires the Greycore's signature too, so key extraction without Greycore collusion does not
move the reserve. **A Greycore quorum that colludes with the gateway is the residual**, and it is the
collusion case stated below rather than a new one.

**And the reserve is still off-chain.** Publishing it is still an assertion by the party that could
steal. The reference does not solve this either; it leans on the bond being larger than the assets,
which is the economic answer rather than a verification one — and **that economic answer is weaker
now**, because the bond is the float rather than a capital requirement sized against the reserve.

---

## Accepted risks, deferred

Two things are **known to be unresolved, are recorded rather than fixed, and are deferred until
there is a team to work on them.**

### The `n/t` capacity rule is withdrawn; the bond is the float

**This is no longer an unreconciled risk; it is a settled withdrawal.** The `n/t` multiple is RenVM's
bribery-cost calculation **for its own parameters** — a 100-node shard at a 1/3 threshold,
`100/34 ≈ 3` — and **it does not apply to a bond that is the float.** We sized the bond against the
reserve and got an impossible inequality (`H ≤ 0`); we then adopted 3× and, at `3-of-5`, were running
a `5/3 ≈ 1.67×` rule while claiming 3×. **Both the arithmetic and its outputs are withdrawn:** the
`$300k` figure, the `~$180k` figure after it, and `fed.k = 1` measured against a capacity rule. **What
constrains the reserve is the Greycore co-signature** (§3), not the bond.

**The residual that remains is the denomination one, and it is stated rather than fixed.** The
`solBSV`-side bond devalues in exactly the scenario it is meant to protect against — when the reserve
is gone, `solBSV` is worth nothing, so seizing or burning it redistributes loss among holders rather
than restoring BSV. The BSV-side bond is under the same key as the reserve, so if that key moves, it
sweeps the bonds too.

**Accepted for now.** The honest statement is that **the bond is the float and it prices provable
misbehaviour; it does not restore what is lost.** Fixing this properly is a real piece of work and it
is deferred.

**And leaver-shares are open.** A departing member retains a valid share, so the effective threshold
degrades with churn; at 4-of-N, four former members together hold four valid shares. Key rotation or
proactive re-sharing — the PoC deliberately does not finalise it (doc 23).

### Collusion is unprevented

A majority of the **gateway** signers **acting together with the Greycore** can take the reserve, and
**no mechanism here prevents it.** The bond does not stop it, the co-signature does not stop a
colluding Greycore, and the slashing layer cannot fire against it. Recorded as a **stated, accepted
risk.** The Greycore raises the cost — the gateway majority alone cannot move funds — but it does not
remove the assumption; it relocates part of it into a named, reputational set.
