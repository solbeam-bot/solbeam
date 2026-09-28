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
| Deposit re-included at a new height, after a mint was staged and burned | **The depositor, permanently** | **The whole deposit** | **Nothing — F1** |
| Mint already released, then a deeper reorg orphans the deposit | **Every `solBSV` holder** | Pro-rata dilution | The buffer, if one exists. Otherwise nothing |
| Peg-out payout reorged mid-challenge | **The relayer** | The payout | The challenge window; the bond covers the user |

### F1 — Burning a staged mint does not release its replay entry · **inherent · serious**

The deposit identity is `(txid, vout)` and **nothing removes it**. Pruning is by height only,
and a lightweight reorg does not remove a transaction — it puts it back in the mempool to be
mined again at a slightly different height.

So the ordinary case is: the block is orphaned, the vault correctly burns the staged mint, the
transaction is re-mined, and the depositor **can never mint again** because their key is still
in the used list. Their BSV sits in the reserve and their tokens are gone. This is the most
likely way for an honest user to lose money, and it needs no attacker at all.

**Required rule:** burning a staged mint must remove its replay entry. S2 in the older
document claims a re-included deposit "can be re-proven", which was true before the vault
existed — when the mint happened only after finality. Staging moved the mint earlier and
invalidated that assumption.

---

## B. Reorg attacker — someone mining a branch to defraud the bridge

| Attack | Who loses | How much | What stops it |
|---|---|---|---|
| Self-reorg, **detected** | The attacker | Mining cost | Detection, then the burn |
| Self-reorg, **detection fails** | **Every holder** | The minted amount | The buffer — if a staker underwrote it |
| **D6 unbacked path, detection fails** | **Every holder** | The minted amount | **Nothing at all.** No underwriter means no bond, so there is no buffer to absorb it |
| Reorg that orphans *someone else's* deposit | That depositor | Time, or everything if F1 bites | Nothing |
| Fill all bond capacity with reorgable deposits | Honest depositors | Blocked, not robbed | Nothing |

### F2 — The unbacked path has no backstop · **inherent · serious, and accepted**

D6 allows a peg-in with no underwriter. That is a deliberate risk acceptance, and the
consequence should be stated as sharply as it deserves: **the vault and detection are then the
entire defence.** If detection fails — nobody pushes the honest branch within the maturity
window — the unbacked mint releases and dilutes every holder, with no bond and no buffer to
absorb it. The `k = 1` bond does not help because there is no relayer in the path at all.

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
| Force a pause | Minting halts | Mining a heavier branch (A1 fixed) | Nothing, though it is now expensive |
| Deny service to the advancer | **Detection fails**, so the buffer is exposed | Blocking a permissionless, unstaked role | **Nothing.** This is the liveness assumption, and it is the load-bearing one |
| Occupy bond capacity | Honest deposits blocked | Mining | Nothing |

### F6 — The replay list is a cheap, repeatable shutdown · **unimplemented · serious**

The list holds 200 entries for a 288-block window. Two hundred dust deposits fill it, after
which **every** peg-in fails during the rest of the window — and it can be repeated. `MIN_PEG_IN`
is specified as a parameter but is not enforced in code, so the deposits need not be
economically meaningful. For a bridge whose peg-in is the product, this is a complete
denial of service for the price of dust.

---

## What this adds up to

**The single dependency is detection.** Five separate findings — F2, F3, F5, and the two
detection rows in B and D — all reduce to the same sentence: *if nobody pushes the honest
chain within the maturity window, holders lose and nothing else intervenes.* Everything
else in the design is defence in depth around that one assumption.

That is not a flaw. It is a consequence of having removed the operator, and it is worth being
explicit that **the liveness of the advancer is the load-bearing assumption of the whole
system**, not a background detail. Two concrete responses follow:

1. **F1 and F6 are ordinary engineering** and should be fixed in code — one is a missing
   delete, the other a missing parameter check.
2. **F3 wants deciding, not coding**: detection is currently rewarded only indirectly. If the
   advancer role is the load-bearing one, it may deserve the same explicit bounty the payout
   challenger already has.
