# 02. How it works

**Three parts, and the division of trust between them is the design.**

```
  1  A LIGHT CLIENT on Solana   verifies BSV proof of work (cw-144) and Merkle
                                inclusion. It holds no funds

  2  A VAULT                    every mint lands here first, not with the depositor.
                                It leaves when the program is satisfied, and it can
                                be burned if it isn't

  3  A FEDERATION               bonded members who run nodes, relay headers, hold the
                                BSV reserve under a 2-of-2 script with the Greycore,
                                sign payouts and challenge theft
```

**The deposit is verified and reversal is trustless** — the program checks BSV proof of work itself
and compares its own stored header hash — while the **reserve is trusted and bounded**, held under a
**2-of-2 `OP_CHECKMULTISIG`** (the gateway threshold ECDSA key plus the **Greycore**'s) that neither
the gateway majority nor the Greycore can move alone. The one assumption is that the gateway threshold
**together with the Greycore** does not collude; **continuous publication of the reserve and supply**
is the mitigation that makes a theft visible. [05. Trust model](05-trust-model.md) is the honest
account of it.

> **Built or designed?** Built: the **light client** with cw-144 and Merkle inclusion, the **`solBSV`
> token**, the **mint** (`verify_deposit`), **fork staging**, the **nullifier**, the **timelocked
> authority**, and the **vault** (`release_mint`, `burn_staged`, `set_maturity`) — **21 instructions,
> 64 passing / 0 failing**. **Designed and not built: the federation, the Greycore, peg-out and
> governance.** The vault's protective window ships at 0, so the reversal is available and racy
> rather than automatic (§4). See [08. Status and roadmap](08-status-and-roadmap.md) for the full
> built/designed line.

---

## 1. Why the header chain, and not just Merkle proofs

**The question every reader asks, and the answer is a reorg.**

A deposit claim carries a Merkle proof and the 80-byte header. So why relay every header rather than
just the proofs? Because a Merkle proof establishes two different things, only one of which it can:

| What must be true | How |
|---|---|
| The transaction is in **this block** | **Merkle proof** — cheap, supplied by the claimant |
| **This block is on the chain** | The header chain — or somebody's word for it |

A proof folds a transaction to a Merkle root. A fake block has a perfectly valid root, so the proof
alone proves inclusion in *a* block, never in *the* chain. Checking that needs the chain.

**A tip does not help.** "Post the proof and the latest header" does not close the gap: a bare tip is
a hash, and to know the claimant's block is on the chain ending at that tip you need **every header
between them** — checked for linkage *and* proof of work, which is cw-144.

**The reorg is the decisive case.** Suppose a deposit is proven against block H, minted into the
vault — and the client then never sees the chain again, only proofs. **Then H is reorged out by a
heavier branch.** Nothing in the situation is detectably wrong: the Merkle proof was true when made
and is still verifiable. It is simply no longer *about the chain*. No proof can say *"the block I
proved three days ago is no longer canonical"*, because **a proof is about the past, and a reorg is a
change in the present.**

So without the chain: `burn_staged` is impossible; a deposit becomes final on first proof — the exact
failure the vault exists to prevent; and a depositor reorged out of their own deposit keeps the tokens
*and* gets the BSV back.

**The header chain is not there for proof of work. It is there so that a reorg is visible.**

```
header chain on Solana  →  a reorg is visible, so the vault can burn
Merkle proof per claim  →  inclusion, cheap, supplied by the claimant
cw-144 on-chain         →  the chain cannot be faked with easy blocks
147-ancestor seed       →  the client can start at all
```

### The header state problem

A full BSV header chain cannot live on Solana economically: roughly **968,000 headers**, tens of
megabytes and hundreds of SOL, against a **10 MiB** per-account cap. The chain therefore lives
on-chain as a **checkpoint** (a recent, well-buried header) plus a **rolling window** of **192
subsequent headers** — **32 hours** at BSV's ten-minute target — in a single account of **10,107
bytes**, inside Solana's **10,240-byte** account cap.

The window is fixed by arithmetic rather than taste:

```
LIGHT_CLIENT_FIXED = 123 bytes
HEADER_RECORD_SIZE =  52 bytes   (32 hash + 16 chainwork + 4 time)
SPACE              = 123 + 192 × 52 = 10,107   of 10,240  ->  133 bytes margin
LOOKBACK           = 147 records (144 + 3 for cw-144's median-of-three)
```

- **52 bytes a record, not 32.** cw-144 subtracts two cumulative chainworks and two timestamps 144
  blocks apart, so a record must carry hash, chainwork and time. A bare-hash window cannot verify the
  difficulty rule at all.
- **194 is the arithmetic maximum** (10,211 bytes, 29 bytes margin); **192 is the largest window with
  real margin**.
- A 48-hour window would be 288 × 52 = 14,976 bytes plus overhead, **46% over the cap**, and
  `initialize` would simply revert. **The old 48-hour deposit deadline was never achievable.**
- 192 leaves only **45 records of slack** above cw-144's 147-record lookback.

**Consequence, and it is a product decision rather than a detail:** the deposit lifetime is **32
hours**. A deposit whose block has left the window can never be proven again.

A competing branch is staged in batches and committed only if **strictly heavier** in accumulated
chainwork; ties keep the incumbent, so an equal-length branch cannot churn the tip. `commit_fork`
records and re-checks the branch's `fork_parent_hash` (audit P2/A11) and compares chainwork, not
height.

Verification itself is cheap: an 80-byte header double-SHA-256 costs **226 CU**, a 12-level Merkle
branch **2,616 CU** — a full SPV deposit proof is about **2,842 CU**, negligible against Solana's
per-transaction limit. The expense is *state*, not computation. The design is deliberately the
simplest option — a checkpointed, optimistic header chain; zero-knowledge proof verification is a
later hardening step, since the tooling is unaudited and sometimes restrictively licensed.

### The difficulty rule

`cw-144` from the SV Node's `src/pow.cpp` is implemented in `difficulty.rs` and verified against
**324/324 real mainnet headers, exact** — no tolerance and no fitting. The **instruction path**, not
only the pure function, is exercised by **160 real mainnet headers driven through `push_header`** and
a real mainnet branch through `push_fork_header`.

**The residual (X3):** the rule is hard-coded, and BSV's own documentation says it will revert to
2016-block retargeting at some point. A consensus change would halt the bridge until governance acts
— a liveness failure, not a theft. The federation model makes the DAA a **governance parameter**,
which gives the risk an owner; it does not make the rule parameterisable in code today.

### Where SOLBEAM does not need infrastructure

- **No BSV full node.** A header source needs only chain data; the proof is verified on Solana, so the
  data source need not be trusted.
- **No Solana infrastructure.** A public or private RPC endpoint is enough.
- **No single operator.** Membership is open at two-sided 1,000 BSV bonds, and the website has **no
  consensus role**: nothing it says can change what the program accepts.

### Technology and licensing

| Need | Choice | Licence |
|---|---|---|
| BSV headers, legacy transactions, Merkle | `btcsuite/btcd` | ISC |
| BSV signature hashing (SIGHASH_FORKID) | implemented in-house | — |
| BSV covenants (`OP_PUSH_TX` introspection) — **not used in the current design** | Rúnar, retained for the deferred signerless track | MIT |
| Solana programs | Anchor + SPL Token | Apache-2.0 |
| ZK verification (future) | `groth16-solana` | Apache-2.0 |
| Solana escrow/hashlock patterns | `kobby-pentangeli/atomic-swap` | MIT / Apache-2.0 |

`scryptlib`'s *SDKs* are MIT, but the sCrypt compiler/stdlib that implements preimage introspection
is **not** permissively licensed, so Rúnar is used instead. The BSV Go SDK is under the Open BSV
License and is avoided for the same reason.

---

## 2. The two directions

```
   PEG IN — BSV to solBSV                  PEG OUT — solBSV to BSV

   1  send BSV + OP_RETURN                 1  escrow solBSV + a BSV destination
   2  wait FLOOR = 12 confirmations        2  members sign payout INTENTS
   3  mint into the VAULT, not to you         individually, on Solana
   4  MATURE (designed: 144 blocks)        3  the THRESHOLD KEY signs the BSV
   5  RELEASE to you                          payment; the GREYCORE co-signs
      reorg first: the staged              4  SETTLE: the payout is proved
      tokens BURN. Your BSV went               against the light client; the
      with the reorg, so you end               escrow burns
      where you started.                       or
                                             4' CANCEL: after the deadline the
                                                escrow returns to you
```

Both directions have the same shape: **enter the vault, then leave it either to the counterparty or
back to the sender.** A failure is a return, never a new mint. Supply is unchanged on every failure
path.

---

## 3. Peg-in — BSV → `solBSV`

```
  YOU                    BSV CHAIN                    SOLANA
   │                         │                           │
   │  1. send BSV + OP_RETURN│                           │
   │     naming your Solana  │                           │
   │     address             │                           │
   ├────────────────────────►│                           │
   │                         │  2. FLOOR = 12 blocks     │
   │                         │     of confirmation       │
   │                         │                           │
   │                         │  3. light client verifies │
   │                         │     header + cw-144 PoW   │
   │                         │     + Merkle inclusion    │
   │                         ├──────────────────────────►│
   │                         │                           │  4. mint into the VAULT
   │                         │                           │     (not to you), with the
   │                         │                           │     deposit's block hash
   │                         │                           │     recorded
   │                         │                           │
   │                         │                           │  5. MATURE (designed 144)
   │                         │                           │  6. RELEASE — permissionless
   │◄────────────────────────────────────────────────────┤     hash still matches:
   │                         │                           │       vault → you
   │                         │                           │     hash differs (reorg):
   │                         │                           │       staged tokens BURN
```

1. **You send BSV** to the federation's deposit script, attaching an `OP_RETURN`. The design carries
   `version ‖ cluster_id ‖ program_hash ‖ flags ‖ recipient` so a payload is bound to this program and
   this cluster; **the built program checks only that the recipient's 32 bytes appear in an
   `OP_RETURN`** in the same transaction, which is what makes a deposit non-transferable between
   claims. The longer form is the cross-deployment replay fix (P11) and is **not built**.
2. **You wait `FLOOR` — 12 BSV blocks (~2 hours)**, measured in *block time from the BSV headers*,
   never against a wall clock. **12 is a floor, not a price.**
3. **The light client proves it.** The program verifies the 80-byte header's hash against its stored
   window, the proof of work under cw-144, the linkage, and the Merkle branch showing the transaction
   in that block. **No signature, committee or oracle is involved: the proof is the authorisation**,
   and anyone may submit it for anyone. `MIN_CONFIRMATIONS = 12` is the code's fixed form of `FLOOR`.
4. **`solBSV` is minted into the vault, not to you.** The record (`StagedMint`) stores the block hash
   the deposit was proven against, its height, and **`maturity_at_deposit`**.
5. **Maturity** — designed 144 blocks, **shipped 0** (§4).
6. **`release_mint` and `burn_staged` are permissionless** — anyone may resolve a pending item and
   reclaim its rent, so **no party's cooperation is ever required.**

**Step 4 is what makes a fraudulent mint unsellable.** A staged token is not in anyone's wallet, so
there is nothing to dump and no innocent buyer to inherit the loss.

### The honest cliff

The block must still be inside the client's window when the proof is submitted, and the window is
**32 hours**. A deposit nobody proves within that time can never be proven, and the BSV is with the
federation. That is a real failure mode, disclosed rather than dressed up; the wallet-side app is what
mitigates it by minting automatically.

---

## 4. The vault — the built reversal, and the window that ships off

**The vault is the contribution.** RenVM's host chain trusted the shard's report; ours compares the
program's own stored headers, so a deposit that is reorged out can be **reversed**, not merely
detected — and because the tokens are in an account the program owns, this needs **no freeze
authority** and no Token-2022 hooks.

**Accounts (built):**

| Account | Seeds | Holds |
|---|---|---|
| **`Vault`** | `[b"vault"]` | The program-owned token account every mint lands in |
| **`StagedMint`** | `[b"mint", txid, vout]` | `{ recipient, amount, deposit_height, deposit_hash, maturity_at_deposit, bump }` |
| **`DepositNullifier`** | `[b"nullifier", txid, vout]` | `{ deposit_height, bump }`. Its existence is the replay record |
| **`Config`** | `[b"config"]` | `{ maturity_blocks }` — mutable only through the timelocked authority path |

**`maturity_at_deposit` is load-bearing.** It records the maturity that applied when the deposit was
verified, so a later authority raise **cannot retroactively trap funds already in flight.**

**`release_mint` requires all of:**

- the client is **fresh** — last accepted header within `Config`'s staleness bound (`StaleClient`);
- `tip_height >= deposit_height + maturity_at_deposit`;
- the client's **stored hash at `deposit_height` still equals the recorded one**;
- the client still holds that height.

**`burn_staged` requires:**

- the client's **stored hash at that height differs** — a reorg was followed; **and**
- `tip_height >= deposit_height + maturity_at_deposit`.

There is deliberately **no staleness check on the burn**: burning is the safe direction, and an old
view is less dangerous there than on release.

**Why three conditions on release, not one.** Staleness alone would release against a view that has
not seen the reorg; depth alone would release even if the record of that height had already changed;
hash equality alone would release immediately — the maturity-0 behaviour, and the reason the window
exists. All three together are what make "this deposit survived" mean something.

### The maturity-0 consequence, stated up front

**At maturity 0 the vault is a pass-through.** Tokens are minted into the vault and become releasable
in the same instant, and the token has no freeze authority — so once released, a later reorg has
nothing to reverse.

**But "the burn cannot fire" would be too strong.** At maturity 0 the burn predicate is *satisfied
instantly* rather than *unreachable*: `burn_staged` requires the hash to differ **and** the height to
have matured, and at 0 the second condition is always met. So it is a **race** — release and burn are
both permitted from the moment of staging, and in practice **release wins**, because the recipient
wants their tokens and a reorg is the unusual case.

**The honest statement is narrower than it first appears:** maturity 0 removes the *window* in which
the reversal is comfortable, not the reversal itself. Someone who sees a reorg and calls `burn_staged`
before anyone releases **still burns the tokens.**

**What protects a deposit today is `MIN_CONFIRMATIONS = 12`** — roughly two hours, **prevention, not
reversal**. The reversal remains **available, tested, and racy** rather than disabled. **Do not write
"reorg-reversible" as a property of the running system.**

Maturity is a **stored parameter**, not a constant, for four reasons: the PoC ships at 0 by decision;
the burn path is testable (a constant at 0 could never be exercised); governance can raise it through
the path that already exists; and it makes the honest sentence available — *the vault is built; the
protective window is currently 0 and can be raised.*

**The detection budget, if maturity is raised to 144.** With `WINDOW = 192` and `FLOOR = 12`, the
margin between "releasable" (`MATURITY`) and "no longer checkable" (`WINDOW`) is `WINDOW − MATURITY`
= **48 blocks (~8 hours)**. A deposit committing to a deeper `FLOOR` in its `OP_RETURN` has less; at
the maximum committed depth the margin is zero, because the clocks coincide. This is a parameter
decision with a security consequence, not a mechanism.

### What the vault tests do NOT cover

Recorded because the alternative is a document claiming more than it verified:

| | |
|---|---|
| **`StaleClient`** | **Cannot be exercised on a local validator.** The ~54,000-slot bound is unreachable in a test run of minutes. Implemented and **untested** |
| **`AlreadyStaged`** | Untested — a replay is refused earlier, by `AlreadyMinted`, so the second guard never fires on its own |
| **`WrongStagedMint`** | Untested |
| **`DepositHeightNotInWindow` on release and burn** | Untested |
| **The burn bounty** | Not built, and not sized. `fee.bounty_share` is `open` |

**`burn_staged` IS exercised**, at a non-zero maturity, with a followed reorg: `NotMatured` is
reachable, `DepositHashUnchanged` flips to `DepositHashChanged`, and the burn zeroes the vault with
the supply delta asserted. A later maturity raise leaves in-flight staged items at the value they were
staged with — proven, not merely asserted.

### The nullifier, and the ceiling it removed

Replay is a **nullifier PDA per `(txid, vout)`**, not a fixed list. The built program used to cap a
used-deposit list at `MAX_USED = 200` per window, which bounded peg-ins to **200 per 32 hours with no
attacker at all**. The nullifier removes the list, the ceiling and the pruning-by-capacity at once.
`prune_nullifier` closes a nullifier only once its stored `deposit_height < window_start`.

**What is still not built:** the aggregate per-window mint cap (`MAX_MINT_PER_WINDOW`), wanted as a
coarse backstop set by policy rather than derived.

---

## 5. Peg-out — `solBSV` → BSV (designed, not built)

```
  YOU                    SOLANA                       BSV CHAIN
   │                         │                           │
   │  1. escrow solBSV into  │                           │
   │     the VAULT, naming a │                           │
   │     BSV destination and │                           │
   │     a deadline          │                           │
   ├────────────────────────►│                           │
   │                         │  2. members sign payout   │
   │                         │     INTENTS individually, │
   │                         │     on Solana. Each is    │
   │                         │     attributed on record  │
   │                         │                           │
   │                         │  3. once enough intents   │
   │                         ├──────────────────────────►│
   │                         │     exist, the THRESHOLD  │
   │                         │     KEY signs and the     │
   │                         │     GREYCORE co-signs     │
   │                         │                           │
   │                         │  4. the payout is PROVED  │
   │                         │     against the light     │
   │                         │     client: the escrow    │
   │                         │     burns                 │
   │                         │                           │
   │  5. or CANCEL — permissionless after the deadline:   │
   │     the escrow returns to you                   ◄────┤
```

1. **You escrow `solBSV` into the vault** and name the BSV destination and a deadline. The escrow and
   destination are native Solana state, so nothing needs proving.
2. **Members sign payout intents individually**, on Solana. This is the attribution mechanism: a
   member who signs two conflicting intents has produced **their own proof of guilt** — see
   [Slashing](05-trust-model.md#slashing--self-proving-misbehaviour).
3. **Once enough attributed intents exist, the gateway threshold ECDSA key signs and the Greycore
   co-signs the BSV payment.** The reserve script is a **2-of-2 `OP_CHECKMULTISIG`**, both required.
4. **Settlement is proved, not asserted.** The payout transaction is proved against the light client
   — inclusion and amount — and the escrow **burns**. Unlike a report from a signer, this is something
   the program can check. The payout must carry the redemption's identifier, or one payment would
   settle every redemption with the same amount and destination.
5. **Failure returns; it never mints.** After the deadline, `cancel` is **permissionless** and the
   escrow returns to you. Supply is unchanged and you are whole without asking anyone.

**Nothing here is pausable.** There is no instruction that stops a redemption; the deadline and the
cancel path are the only time limits, and they run in the holder's favour.

**The refusal hole (N2).** Every escalation in the design assumes a signed artifact; **refusal to
sign is not an artifact.** A member — or enough members to fall below the threshold — can decline to
attest, and the holder's only remedies are to wait out the deadline and take `solBSV` back. That is a
denial of the peg-out service, not theft, but a persistent stall makes the exit the *token* rather
than BSV. The obvious candidate — a timeout-and-rotate rule — is **not in the design**.

**Genesis.** Members post a **BSV-side bond at genesis**, so no `solBSV` needs to exist first (D16).
A capped, explicitly-unbonded first mint is a documented later option, not chosen, because it leaves
the first mint backed by nothing but the members' word.

---

## 6. Reorg detection, and where the loss lands

### Reorg detection

**Rule 1 — depth, not wall-clock.** The trigger compares two depths:

| Observed reorg depth `R` | Response |
|---|---|
| `R < FLOOR` (e.g. a 2-block tip reorg) | **Nothing.** Confirmations already cover it |
| `R ≥ FLOOR` | The reorg response applies: re-examine mints credited within the last `R − FLOOR` blocks, and stop crediting new ones |

A uniform 12-hour freeze on *any* reorg would stall the bridge through ordinary tip churn. The
program decides release from its own headers; the node software watches and acts.

**Rule 2 — read the time from the headers.** The 80-byte header carries a 4-byte little-endian Unix
timestamp at offset 68, and every `HeaderRecord` stores `time`, so the client derives chain-relative
time from the chain itself:

| Signal | Definition | Reads as |
|---|---|---|
| **Regression** | new header `time` well below tip `time` | A fork, not an extension. Sharpest signal available |
| **Catch-up** | blocks arriving fast in wall-clock while their own `time`s span hours | Advancer was down, **or** a withheld branch is being released — indistinguishable, so pause |
| **Staleness** | `now − tip.time` > bound | Advancer down, or a deep reorg in progress |

**The honest limit:** a privately mined branch released later has *normal* block-time spacing, so
spacing alone will not catch it. What catches it is the mismatch between **arrival rate and block-time
spacing**.

### The invariant, stated the right way round

What is at risk is **unbacked `solBSV`** — more `solBSV` outstanding than BSV custodied:

> `custodied BSV ≥ outstanding solBSV`

**The invariant is monitored, not enforced (D8).** It is published and shown as a ratio, and the
protocol cannot enforce it, because the reserve is off-chain BSV the program cannot read. The reverse
(more BSV than supply) is the *safe* direction, merely inefficient.

### A fraudulent mint has more than one exit

The attacker's difficulty is not obtaining `solBSV` — it is turning it into something else. Three
routes, and the protocol controls exactly one:

| Exit | Who absorbs the loss | Gateable? |
|---|---|---|
| **Peg-out** via the protocol | The federation's reserve, bounded by the bonds behind it | Partly — the intent set refuses an unauthorised payout, and the reserve is finite |
| **DEX** (Raydium / Orca) | The **LPs** in the pool, who bought `solBSV` that is not backed | **No.** A permissionless AMM cannot be frozen |
| **CEX** | The exchange, and its users if it cannot cover | **No.** Off-chain entirely |

Gating peg-out closes one door and leaves two open. It is still worth doing — it removes the deepest,
fastest exit — but it is **not** what makes the peg safe.

**What actually protects:** `FLOOR`, which makes out-mining the honest chain cost more than the fraud
is worth; the vault and maturity, which make a detected fraud reversible; the 2-of-2 script, which
stops one quorum moving the reserve; and the bonds. The honest consequence: **if depth is too low,
the loss lands on DEX LPs and exchanges, and the protocol cannot compensate them.**

### How the bonds bound the hole

| | Custody | Supply | Peg holds? |
|---|---|---|---|
| Normal | `D` | `D` | Yes — 1:1 |
| With bonded stake `S` | `D + S` | `D` | Yes — over-collateralised by `S` |
| Fraudulent mint of `M` | `D + S` | `D + M` | Yes, **iff `S ≥ M`** |
| Attacker then exits via peg-out | `D + S − M` | `D` | Yes, iff `S ≥ M` — **the federation is down `M`; nobody else is** |

So the market never sees the shortfall. Three tiers: **detected in time** → staged tokens burned, no
loss; **detection fails, bonds cover it** → the federation loses, the market does not; **bonds short**
→ holders absorb a discount, the one case that cannot be repaired.

**The `S ≥ M` sizing rule is not a capacity rule and is not in force.** The bond is the **float**, not
capital sized against the reserve; the `~$180k` / `$300k` capacity figures are **withdrawn** (the
`n/t` multiple was RenVM's calculation for a 100-node shard). **What constrains the reserve is the
Greycore co-signature.** **No numeric capacity rule has replaced the superseded arithmetic** — that
is a genuine gap, and it is recorded as one in [03. The federation](03-the-federation.md).

### What the bond answers — and what it does not

The bonds are the float, with a coverage floor: `bsv_bond ≥ k × (BSV held)` and `solbsv_bond ≥ k ×
(solBSV held)` (`k = 1`). They answer a member's **provable misbehaviour** — self-proving equivocation
on a payout intent.

They do **not** answer "an intent matching no authorised redemption": that predicate is undecidable —
a *closed* `PegOut` is indistinguishable from one that never existed — and checking it would
false-positive against an honest member who attested before a cancel. They are **not** reorg
insurance, and they do **not** top up a failed redemption: the returned escrow already makes the
holder whole, so paying the bond too would compensate twice.

**Return-to-sender is a different mechanism, and it is not built.** RTS is for a *real* deposit the
bridge declines to mint: the funds go back to the address that funded the deposit transaction,
derived from that transaction's own inputs — never a relayed or user-supplied destination. In a
fraudulent mint there is no aggrieved sender; the victims are whoever ends up holding the unbacked
tokens.

---

## 7. Scenarios

| # | Scenario | Handling | Who loses |
|---|---|---|---|
| S1 | Clean peg-in | Deposit proven at `FLOOR`; mint staged in the vault; released after maturity; fee 30 bp | Nobody |
| S2 | Reorg **below** the deposit block, while the mint is staged | The staged tokens **burn**. Re-inclusion does **not** restore the mint by itself: the deposit identity `(txid, vout)` stays in the replay record, so it cannot be minted again until the burn also releases it. Then the re-mined deposit mints normally | Nobody — the depositor keeps the BSV the reorg returned |
| S3 | Reorg **after** a mint, depth ≥ `FLOOR` | If the mint is **still staged**, the program burns it. Only a mint that has already **released** cannot be reversed — no freeze authority, by design. New mints pause; depth and the bonds carry the rest | The federation's bonds, if depth was too low and release had happened |
| S4 | Clean peg-out | Members sign intents; the gateway threshold key signs and the Greycore co-signs; the payout is proved; the escrow burns; the fee is paid | Nobody |
| S5 | Payout reorged after it was proved | The proof no longer matches the window; settlement does not complete and the **escrow returns** after the deadline. Supply is unchanged. The federation has lost the BSV it paid — the bond is not reorg insurance | The federation (its reserve) |
| S6 | No threshold signature before the deadline | The deadline expires → the **escrow returns**, permissionlessly. Nothing mints | Nobody loses tokens; the members who failed to sign are at fault |
| S7 | **The attack** — fraudulent mint then peg-out | The vault stages the mint; a reorg of depth ≥ `FLOOR` burns it; the intent set refuses an unauthorised payout; the bonds bound what a release can extract | The bonds, if detection fails and the mint released; otherwise nobody |
| S8 | Deposit valid but the tip is stale | **Delayed, not refunded.** Funds stay in the federation's script; the mint proceeds once the tip advances | Nobody |

**If a refund is ever needed, the rule is absolute: return to sender.** The destination is the address
that funded the deposit transaction, derived from that transaction's own inputs. Anything else turns
"refund" into a way to redirect someone else's coins. This is **not built**, and the UX must show it
before it can happen: a greyed-out box reading "if refunded, it goes back to the sender address",
populated from the deposit, visible while the deposit is pending.

---

## 8. What the program consults — no oracles, by construction

The mechanism needs none. Every row below would have needed an oracle, and the right column is what
replaced it:

| Would have needed | Replaced by |
|---|---|
| A hashpower or price feed to compute a "safe" confirmation depth | **`FLOOR`, 12 blocks**, a governed parameter |
| A feed to price reorg risk | The **governed fee**; metrics are published and never consulted |
| A price feed to value the bond | `1 BSV = 1 solBSV` by construction. A deviation is an arbitrage, not an input |
| An external source for *has BSV reorged, and how deep?* | **The BSV headers themselves.** Chain data, not external data |
| Wall-clock to expire a redemption deadline | **Solana slots.** A halt freezes the clock instead of stranding the holder |
| A price oracle to size the mint cap | A conservative policy value, published, with the metrics beside it |

Two rules cover every row:

- **Chain-native.** Anything the program must *react* to is read from a chain — reorg depth and block
  time from BSV headers, deadlines from Solana slots.
- **Published, never consulted.** Anything that would need external data is displayed and shapes what
  people choose; it never changes what the program *does*.

**The one residual judgement is `FLOOR` itself** — a policy value, not a measurement. The design has
no price oracle; it does have parameters somebody has to choose, and *that* is the thing worth
arguing about.

### What the program can and cannot see

| Quantity | Where it lives | How the program treats it |
|---|---|---|
| BSV headers, chainwork, time | On Solana, in the light client | **Verified.** cw-144 replay, 324/324 real mainnet headers exact |
| Deposit inclusion | Proved against the light client | **Verified** — proof of work and Merkle branch |
| Deposit's block hash | Stored when the deposit is proven | **Compared later**, to decide release vs burn. No reporter |
| Deadline | Solana slots | **Read natively.** A cluster halt freezes the clock |
| Bonds | One on Solana (`solBSV`), one on BSV (native, outside the reserve) | **Compared on-chain per side.** The **mint-side bond is seized by the members collectively** under the collective key, not by the program |
| The BSV reserve itself | Off-chain, under a threshold key | **Not readable.** Monitored and published, not enforced |
| Spent deposit outpoints | Off-chain BSV | **Reported by the federation**, and the program checks mints against that record. The one accepted oracle |
| Price, hashrate, reorg cost | Off-chain | **Never consulted.** Published as information; deciding on them would make them oracles |

---

## 9. What this chapter replaced

Earlier drafts of this material described **per-relayer deposit scripts, an order book of staked
bids, a fee discovered on it, and "no governance in the PoC"**. All four are **superseded**: deposits
now pay one federation deposit script, the order book is removed, the fee is a governed **30 bp**, and
governance exists at 85% / 30 days. The full record of those reversals, with reasons, is in
[07. Decisions](07-decisions.md) and [10. Audit history](10-audit-history.md).

**Why the book was removed:** it solved fee discovery and capacity allocation, and a governed fee
solves both more simply — while deleting the one subsystem that never received an adversarial review.
One piece of its reasoning survives and is worth keeping: **the vault, not per-fill approval, is what
reverses a reorged fill.** A fill was never liquid, so a detected reorg reversed the liability. That
argument is still right; what changed is that there is no per-fill decision to make at all.

**The sound parts of the old reasoning are kept:** depth and maturity do different jobs (§4), the
loss-landing analysis (§6), and the fact that a stablecoin bond against a BSV liability is a written
call option on the reserve — which is why each side's bond is denominated in the asset that side
holds.
