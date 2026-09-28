# 15. Second audit — who loses funds

A focused adversarial pass on the mechanism as it now stands, organised around four
questions: who loses in a **natural** reorg, who loses to a **reorg attacker**, who can
**extract funds illicitly**, and who can be **damaged without being robbed**.

Findings are marked **inherent** (a property of the design) or **unimplemented** (the design
is fine, the code does not exist yet — peg-out and the vault are proposals, not programs).

---

## A. Natural reorg — an ordinary few-block reorg, nobody attacking

| Scenario | Who loses | How much | What stops it |
|---|---|---|---|
| Deposit reorged out before anything is staged | **Nobody** | — | The UTXO is unspent. The sender still has their BSV |
| Deposit re-included at a new height, after a mint was staged and burned | **The depositor, permanently — but staging is not built, so this cannot happen in the shipped code** | **The whole deposit** | **Nothing — F1 (unimplemented)** |
| Mint already released, then a deeper reorg orphans the deposit | **The underwriter's bond first**; holders only if the bond is short, or if the mint took the D6 unbacked path | Up to `owed_R`, then pro-rata dilution | The released mint stays in `owed_R` and the **bond** is seizable, so the underwriter takes the first loss ahead of holders — reaching it still requires the reorg to be noticed. The **buffer** is off-chain over-collateralisation and is *not* slashable, so it is not a first loss-bearer |
| Peg-out payout reorged mid-challenge | **The relayer, for its own costs and the lost fee** — the holder is made whole | The relayer's costs, not the holder's principal | On failure the escrow is **returned to the holder** and supply is unchanged. The bond answers deliberate theft or abandonment, **not** a failed redemption |

### F1 — Burning a staged mint does not release its replay entry · **unimplemented · serious once built**

The deposit identity is `(txid, vout)` and **nothing removes it**. Pruning is by height only,
and a lightweight reorg does not remove a transaction — it puts it back in the mempool to be
mined again at a slightly different height.

So once staging exists, the ordinary case would be: the block is orphaned, the vault correctly
burns the staged mint, the transaction is re-mined, and the depositor **can never mint again**
because their key is still in the used list. Their BSV sits in the reserve and their tokens are
gone. It needs no attacker at all.

**This is not the shipped code's honest-user loss vector, though.** Staging and the vault do not
exist yet, so there is nothing to burn and nothing to strand; the finding is real only for the
component that will be built. In the code that runs today the honest-user loss is **F6**, the
200-entry replay ceiling. **F1 is unimplemented, not inherent**, and it is not ranked as the most
likely honest loss.

**Required rule:** burning a staged mint must remove its replay entry. S2 in the older
document claims a re-included deposit "can be re-proven", which was true before the vault
existed — when the mint happened only after finality. Staging moved the mint earlier and
invalidated that assumption.

---

## B. Reorg attacker — someone mining a branch to defraud the bridge

| Attack | Who loses | How much | What stops it |
|---|---|---|---|
| Self-reorg, **detected** | The attacker | Mining cost | Detection, then the burn |
| Self-reorg, **detection fails** | **Every holder** | The minted amount | The **bond**, if a relayer underwrote it — but a slash needs detection too, so on this path nothing is reached. The buffer is off-chain over-collateralisation and is not slashable |
| **D6 unbacked path, detection fails** | **Every holder** | The minted amount | **Nothing at all.** No underwriter means no bond and no slashable stake to absorb it |
| Reorg that orphans *someone else's* deposit | That depositor | Time, or everything if F1 bites | Nothing |
| Fill all bond capacity with reorgable deposits | **The attacker's own capacity only** | — | **A mint naming relayer R is refused without R's signature/consent**, so a third party cannot occupy an innocent relayer's capacity |

### F2 — The unbacked path has no backstop · **inherent · serious, and accepted**

D6 allows a peg-in with no underwriter. That is a deliberate risk acceptance, and the
consequence should be stated as sharply as it deserves: **the vault and detection are then the
entire defence.** If detection fails — nobody pushes the honest branch within the maturity
window — the unbacked mint releases and dilutes every holder, with no bond and no slashable stake
to absorb it. The `k = 1` bond does not help because there is no relayer in the path at all.

This is not an argument against D6. It is the argument for treating **the incentive to push
the honest chain** as a first-class deliverable rather than a nice-to-have, since on that path
it is the only thing standing.

### F3 — Detection has no reward · **inherent · serious**

Pushing headers is permissionless and cheap, and stakers lose if a fraud goes undetected — so
the incentive exists but is *indirect and diffuse*. There is a bounty for challenging a bad
payout and none for detecting a reorg, which is the case where detection is the whole defence.
Whether the indirect incentive is enough is an open empirical question, not a settled one.

---

## C. Illicit extraction

| Vector | Who loses | What stops it |
|---|---|---|
| Mint with no deposit | Holders | Forging a header now costs real work (A1 fixed) |
| Double-mint the same deposit | Holders | Identity is `(txid, vout)` (A7 fixed) — **but see F1**: the fix for double-minting is what makes the re-inclusion case unrecoverable |
| Replay a proof after a reorg | Holders | Height pruning, once the block leaves the window |
| Forge a payout proof | Holders | Not built. Intended: SPDX proof against the light client |
| Steal from the vault | Holders | The vault is program-owned, so only the program can move it — **provided the release instruction checks its destination** |
| Relayer absconds with the float | Holders, up to `owed_R` | `bond_R ≥ owed_R` in seizable `solBSV`, **provided staged mints count toward `owed_R`** |
| Staker self-deals at `k = 1` | Holders | **Only detection.** If detected, the attacker loses the mining cost and the burn reverses the mint. If not, they keep it — and nothing is slashed, because nothing is noticed |

### F4 — The bond covers only part of what a relayer holds · **inherent · serious**

`bond_R ≥ owed_R` bounds the *liability*, but a relayer also holds its own float. The two need
separating explicitly, because only the liability is bonded. And **staged mints must count
toward `owed_R`**: a depositor whose mint is still maturing has paid BSV and holds no tokens,
so if the relayer disappears at that moment the depositor has lost the full deposit and must
be covered by the bond.

### F5 — At `k = 1` and no detection, self-dealing profits · **inherent · accepted**

D5 accepts self-dealing. Worth restating precisely: the "roughly break-even" characterisation
holds **only if the shortfall is detected and the bond is slashed.** With detection failing,
the attacker keeps the minted tokens *and* the bond, because nothing was noticed to slash. So
the true statement is that **self-dealing is unprofitable only to the extent detection works**
— which makes it another instance of the same dependency, not a separate defence.

---

## D. Griefing and damage without theft

| Vector | Harm | Cost to attacker | Stopped by |
|---|---|---|---|
| Fill the replay list with dust | **All peg-ins fail** once 200 entries are used | Dust plus 200 Solana fees | **Nothing.** `MIN_PEG_IN` is unimplemented and `MAX_USED = 200 < WINDOW = 288`, so the list can be filled inside a window and refilled |
| Force a pause | **Nothing — an attacker cannot force one** | — | `set_paused` is authority-gated (A2 fixed) and `commit_fork` only emits an event, so no attacker path halts minting |
| Deny service to the advancer | **An undetected fraud releases a mint into circulation and dilutes holders** | Blocking a permissionless, unstaked role | **Nothing** for that loss: a slash needs detection too, so the bond is not reached either. Detection is the only defence against a released fraudulent mint — but it is not the whole system's single dependency (see below) |
| Occupy bond capacity | **The attacker's own capacity only** | — | **A mint naming relayer R needs R's signature/consent**, so a third party cannot occupy an innocent relayer's capacity |

### F6 — The replay list is a cheap, repeatable shutdown · **unimplemented · serious**

The list holds 200 entries for a 288-block window. Two hundred dust deposits fill it, after
which **every** peg-in fails during the rest of the window — and it can be repeated. `MIN_PEG_IN`
is specified as a parameter but is not enforced in code, so the deposits need not be
economically meaningful. For a bridge whose peg-in is the product, this is a complete
denial of service for the price of dust.

---

## What this adds up to

**Two different losses, two different defences.** An earlier draft called detection "the single
dependency". That is too strong, and it conflicts with doc 04: a redemption the program has
accepted is enforced by a deadline, not by a watcher. Separating the two makes the dependency
precise:

- **The bond answers deliberate theft or abandonment.** A relayer that takes an accepted
  redemption and then steals or abandons it is caught by the deadline: the escrow is returned to
  the holder, supply is unchanged, and **the bond is slashed as a penalty** — but *not* paid to the
  holder, who the escrow return has already made whole; paying both would compensate twice. Where
  a slashed bond goes (burned, or returned to the reserve) is a separate and unsettled question.
  That path is *self-reporting* — no watcher required. This is doc 04's argument, and it is why `k = 1` is defensible for the
  bonded liability.
- **Only detection answers a released fraudulent mint.** A reorg fraud that has already released
  a mint into circulation has no deadline and no victim to complain. If nobody pushes the honest
  chain within the maturity window, holders are diluted — and **the bond is not reached either,
  because a slash needs detection too.** The unbacked (D6) path has no bond at all.

F2, F3, F5 and the two detection rows in B and D are all the second class. So the load-bearing
statement is not that the advancer is the system's single dependency, but that **detection is
the only defence for the released-mint loss, while the redemption loss is self-reporting and
bonded.** Everything else in the design is defence in depth around the first.

Two concrete responses follow:

1. **F6 and F1 are ordinary engineering** and should be fixed in code — one is a missing
   parameter check in the shipped code, the other a missing delete in a component (staging)
   that is not built yet.
2. **F3 wants deciding, not coding**: detection is currently rewarded only indirectly. If
   detection is the only defence against a released fraudulent mint, the advancer role may
   deserve the same explicit bounty the payout challenger already has.


---

## Independent pass — what it found, and where it is wrong

A second adversarial run was made against the same code and documents. It confirmed F6, added
one finding more important than anything above, and produced **two criticals that do not
survive checking.** Both are recorded here because a wrong critical is worse than no critical:
it spends attention on a non-problem and, if believed, leads to bad design.

### F7 — The client cannot follow a difficulty retarget, so it halts permanently · **defect in built code · critical**

`push_header` requires `bits == expected_bits`, and `expected_bits` is set once at
`initialize` from the checkpoint header and never updated. `set_checkpoint` does not refresh it
either.

**This is a consequence of the A1 fix.** Before A1, `check_daa` returned `true`
unconditionally and the target was read from the submitted header, so every header passed and
the defect was invisible. Now the check actually binds — which is correct — and on **any chain
whose target changes, every header after a retarget is rejected `UnexpectedRetarget` with no
instruction able to fix it.**

On regtest this is invisible, because the target never changes. On testnet or mainnet the
bridge **stops minting at the first retarget, permanently.**

The older framing called DAA "a flag, not an omission". That is no longer accurate: the flag
is now **load-bearing for liveness**, not merely for security. Worth deciding before testnet:
implement the retarget, or permit `expected_bits` to advance at a retarget boundary.

### X1 — "The vault does not exist" · **correct, and already addressed**

The independent pass is right that the shipped program mints straight to the depositor's token
account and that no vault, maturity, release or burn exists anywhere. `13-summary.md` has been
amended to say so at the top.

**F1 is therefore classified as UNIMPLEMENTED, not inherent** — the heading, the Table A row and
the finding itself now say so. It is a genuine flaw in a component that has not been built, and
it is not ranked as "the most likely way for an honest user to lose money" while there is nothing
to lose it from. The shipped code's honest-user loss vector is **F6**, not F1.

### X2 — "Unlimited double-mint via a counterfeit replay list" · **incorrect**

The claim is that `used_deposits` has no `seeds` constraint, so an attacker can pass their own
account bearing the `UsedDeposits` discriminator, present an empty list, and mint one real
deposit repeatedly.

**The attack requires fabricating an account that the program owns and whose first eight bytes
are the discriminator. That is not possible on Solana.** Only the owning program may write an
account's data, and this program writes `UsedDeposits` data in exactly one place —
`InitializeBridge`, with fixed seeds `[b"used_deposits"]`, as an `init`. A second call fails
because the PDA already exists, and no other instruction creates one. `SystemProgram::create_account`
can set the *owner* to this program but leaves the data **zeroed**, which fails Anchor's
discriminator check; and nothing can write the discriminator afterwards, because after
assignment only the program may write.

So the constraint was missing, but the consequence was not an exploit: the account was already
bound by owner and type. **It is pinned now anyway**, because the reasoning is subtle, the
constraint costs nothing, and a future instruction creating a second `UsedDeposits` would turn
the subtlety into a real double-mint:

```rust
#[account(mut, seeds = [b"used_deposits"], bump = used_deposits.bump)]
pub used_deposits: Account<'info, UsedDeposits>,
```

**The rigorous closure is a test, not an argument.** The suite never tries a counterfeit
account — it derives the real PDA every time, so "refuses the same deposit twice" tests
nothing about binding. A test that creates a program-owned account, attempts to pass it, and
asserts rejection would settle X2 by evidence instead of by reasoning. **Recommended before
this is called closed.**

### Also claimed, and also incorrect

**"A miner can re-mine a signed deposit with a different `OP_RETURN`, redirecting the mint to
themselves."** The output script would be unchanged, so `WrongOutputScript` would not fire —
but the depositor's own signature covers the `OP_RETURN` output. Under `SIGHASH_ALL` — the
default, and what `bsvlib` builds — altering the `OP_RETURN` invalidates the input's
signature, so the transaction becomes unspendable and cannot be mined. **Not exploitable.**
It would become real if a depositor signed with `SIGHASH_NONE`, which no wallet here does.

### Confirmed, and worse than recorded

**F6 is a hard throughput ceiling, not only a griefing vector.** `MAX_USED = 200` with no
`MIN_PEG_IN` means the program can process **at most 200 peg-ins per 48-hour window** even
with no attacker at all. The griefing case is simply an attacker reaching that ceiling
deliberately. Worth separating in the write-up: one is a capacity limit, the other is abuse of
it.

**C3, a residual.** `initialize` accepts any header meeting *its own* declared `bits`, so the
checkpoint's difficulty is unchecked and self-declared. Combined with the deploy-time race
already recorded under A2, whoever calls `initialize` first chooses both the trusted root and
its difficulty. On regtest that target is trivially mineable. **The checkpoint being trusted is
inherent to the design; the race is not.**

### What the independent pass gets right that is easy to miss

**D6 is not an exception path — in the shipped code it is the only path.** There is no relayer
registry, no consent, no `owed_R`, no `bond_R` and no bond check, so *every* peg-in today is
the unbacked one. The design's layering — underwriter, bond, buffer — is entirely prospective.
That is expected at this stage, but it means **the D6 risk acceptance is currently the whole
system's risk posture**, not a seeded-book special case.

---

## X3 — BSV changes its difficulty algorithm · **protocol risk, outside our control**

**Recorded because the client now hard-codes cw-144, verified against real headers, and that rule
belongs to somebody else.**

BSV's own documentation states plainly: *"the original Bitcoin client updated the difficulty target
every 2016 blocks… however the current difficulty adjustment algorithm changes the rate every block
in an attempt to compensate for the dynamics of the multiple competing SHA256 chains that currently
exist. **The difficulty algorithm will be adjusted back to the original 2016 block adjustment rate
in the near future.**"*
([BSV Hub](https://hub.bsvblockchain.org/higher-learning/bsv-academy/bsv-theory/proof-of-work/controlling-the-block-discovery-rate.md))

If that happens, or if BSV adopts anything else, **the client rejects every header from the change
point onward and the bridge halts.** Not a theft, not an exploit — a **liveness failure** caused by
a third party.

**Why it is worth naming rather than assuming away:**

| | |
|---|---|
| **The failure is total, not partial** | The difficulty check gates every header, so nothing advances and no deposit can be proven |
| **It is already announced** | This is not a hypothetical; the Foundation has said it intends to change it |
| **Deposits in flight are at risk** | A halted client cannot release a staged mint, and the window is only ~30 hours (W1.4) |
| **It has no cryptographic defence** | No amount of design makes a hard-coded rule survive its own replacement |

**Mitigations, in order of preference:**

1. **Version the DAA rule on-chain.** Store which rule is active; accept either at a known activation
   height. A new rule becomes a parameter, not a program upgrade.
2. **Treat the difficulty rule as upgradeable**, which the program's upgrade authority (A5) already
   allows — the authority is a liability elsewhere but is the *mechanism* here. Requires monitoring
   BSV's release notes and shipping before the change activates.
3. **Monitor and warn.** A website banner is not a defence, but it converts a silent halt into a
   public one.

**This is assumed not to change in the near term, by decision.** The assumption is recorded here so
that it is an accepted risk rather than an unnoticed one — and so that whoever revisits this knows
the trigger to watch for: a BSV release that touches `CalculateNextWorkRequired`.
