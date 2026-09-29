# 2. How it works

> **Built or designed?** The built set is exactly four things: the **light client** (with cw-144
> difficulty verification and Merkle inclusion), the **`solBSV` token**, the **mint**, and **fork
> staging with chainwork**. That is **27 passing on-chain tests**, **51/51** synthetic BSV peg-in
> checks (Phase 1A) and **324/324** real mainnet headers replayed exactly.
>
> **The vault, the federation, threshold custody, governance, slashing and all of peg-out are
> designed, not built.** Where this document describes them it is describing a specification, and
> it says so. The shipped program mints straight to the depositor's token account.
> [`13-summary.md`](13-summary.md) is the authoritative model; where this document and that one
> disagree, that one is right.

## Why the header chain, and not just Merkle proofs

**The question every reader asks, and the answer is a reorg.**

A deposit claim carries a Merkle proof and the 80-byte header. So why relay every header rather
than just the proofs? Because a Merkle proof establishes two different things only one of which it
can:

| What must be true | How |
|---|---|
| The transaction is in **this block** | **Merkle proof** — cheap, supplied by the claimant |
| **This block is on the chain** | The header chain — or somebody's word for it |

A proof folds a transaction to a Merkle root. A fake block has a perfectly valid root, so the proof
alone proves inclusion in *a* block, never in *the* chain. Checking that needs the chain.

### A tip does not help

"Post the proof and the latest header" sounds like it closes the gap, and it does not. A bare tip is
a hash; to know the claimant's block is on the chain ending at that tip you need **every header
between them**. And once a claimant supplies that path, the program must check it — linkage *and*
proof of work, which is cw-144. So the question returns to where it started.

### The reorg is the decisive case

Suppose a deposit is proven against block H, minted into the vault — and the client then never sees
the chain again, only proofs. **Then H is reorged out by a heavier branch.**

**Nothing in the situation is detectably wrong.** The Merkle proof was true when made and is still
verifiable. It is simply no longer *about the chain*. And no proof can say *"the block I proved
three days ago is no longer canonical"* — because **a proof is about the past, and a reorg is a
change in the present.**

So without the chain:

- **`burn_staged` is impossible.** It works by comparing the stored hash at a height against what the
  client now holds. With no chain there is nothing to compare against
- **A deposit becomes final on first proof** — which is the exact failure the vault exists to prevent
- **A depositor reorged out of their own deposit keeps the tokens** *and* gets the BSV back

**The header chain is not there for proof of work. It is there so that a reorg is visible.** That is
the role no proof can play, and it is why the chain is relayed rather than the proofs alone.

### Each part does a job the others cannot

```
header chain on Solana  →  a reorg is visible, so the vault can burn
Merkle proof per claim  →  inclusion, cheap, supplied by the claimant
cw-144 on-chain         →  the chain cannot be faked with easy blocks
147-ancestor seed       →  the client can start at all
```

## Three parts

```
  1  A LIGHT CLIENT on Solana     verifies BSV proof of work (cw-144) and
                                  Merkle inclusion. It holds no funds

  2  A VAULT                      every mint lands here first, not with the
                                  depositor. It leaves when the program is
                                  satisfied, and it can be burned if it isn't

  3  A FEDERATION                 bonded members who run nodes, relay headers,
                                  hold the BSV reserve under a THRESHOLD key,
                                  sign payouts and challenge theft
```

The division of trust is the design: **minting and reversal are trustless** — the program checks
BSV proof of work itself and compares its own stored header hash — while the **reserve is trusted
and bounded**, held under a threshold key that no single member can move, and protected by a bond
anyone can seize by proving misbehaviour on-chain. The one assumption is that a threshold of
members do not collude. [`04-trust-model.md`](04-trust-model.md) is the honest account of it.

---

## The two directions

```
   PEG IN — BSV to solBSV                  PEG OUT — solBSV to BSV

   1  send BSV + OP_RETURN                 1  escrow solBSV + a BSV destination
   2  wait FLOOR = 12 confirmations        2  members sign payout INTENTS
   3  mint into the VAULT, not to you         individually, on Solana
   4  MATURE: 144 blocks                   3  the THRESHOLD KEY signs the BSV
   5  RELEASE to you                          payment once enough intents exist
      reorg first: the staged              4  SETTLE: the payout is proved
      tokens BURN. Your BSV went               against the light client; the
      with the reorg, so you end               escrow burns
      where you started.                       or
                                            4' CANCEL: after the deadline the
                                               escrow returns to you
```

Both directions have the same shape: **enter the vault, then leave it either to the counterparty
or back to the sender.** A failure is a return, never a new mint. Supply is unchanged on every
failure path.

---

## What it costs to run

The light client is the only part of SOLBEAM that has to be kept alive continuously, so it is worth
being precise about which of its costs actually recur. The figures below are measured constants
rather than guesses; [`17-costs.md`](17-costs.md) carries the working.

### Storage is a one-time deposit, not a burn

Solana charges rent-exemption up front, at roughly **5,080 lamports per byte**, and **the entire
amount is returned when the account is closed**. It is a deposit, not a fee.

| | Size | One-time | at $77/SOL |
|---|---|---|---|
| Light client, 192 records / 32 hours | **10,107 B** | 0.052 SOL | **$4.00** |
| *(Solana's per-account creation ceiling)* | *10,240 B* | *0.053 SOL* | *$4.06* |

The arithmetic is worth stating because it is what fixes the window:

```
LIGHT_CLIENT_FIXED = 123 bytes
HEADER_RECORD_SIZE =  52 bytes   (32 hash + 16 chainwork + 4 time)
SPACE              = 123 + 192 × 52 = 10,107   of 10,240  ->  133 bytes margin
```

**The 32-hour window is what fits, not what was chosen.** cw-144 needs per-block cumulative
chainwork and time, which is 52 bytes a record rather than the 32 a bare hash took; 194 records is
the arithmetic maximum and 192 leaves real margin. A 24-hour window (144 records) would fit
comfortably but would shorten the reorg horizon, and cw-144's own lookback already consumes **147
records**, so 192 is only 45 records of slack above the minimum.

**This does not grow with the BSV chain, and that is the entire point of the rolling window.** If
the client stored history instead, the same numbers would be roughly **77 MB** for every 80-byte
header (about **394 SOL**, or **$30,305** at $77/SOL) — and it would keep climbing forever. The
window turns a cost that grows without bound into a fixed deposit of a few dollars: a
**~3,000×** reduction, permanently capped.

### The recurring cost is transaction fees

Every BSV block needs one header pushed, so the client costs **144 transactions a day — 52,560 a
year.** Each is a single-signature transaction whose base fee is 5,000 lamports. The measured
compute is about 4,571 CU, so priority fees stack on top:

| Priority fee (µlamports/CU) | Fee per header | Per year | At $77/SOL |
|---|---|---|---|
| 0 (base fee only) | 5,000 lamports | 0.263 SOL | **$20** |
| 100,000 | 5,457 | 0.287 SOL | $22 |
| 1,000,000 | 9,571 | 0.503 SOL | $39 |
| 10,000,000 | 50,710 | 2.665 SOL | $205 |
| 100,000,000 | 462,100 | 24.29 SOL | $1,870 |

**Base fees are trivial; priority fees are the real variable.** Under congestion this is the number
that matters, and it is beyond the protocol's control.

Two mitigations exist. Headers can be **batched**, since a 1232-byte transaction fits about **12** of
them (the built constant `MAX_FORK_BATCH = 12` is that ceiling): that cuts base fees by an order of
magnitude, at the cost of the client
lagging about two hours behind the chain — acceptable, because a mint already waits out `FLOOR`
(12 blocks, ~2 hours) and then `MATURITY` (144 blocks, ~24 hours) on top. Batching does not reduce
priority fees, which dominate.

### Being parsimonious: what the window stores, and why

An earlier layout kept a 32-byte block hash **and** a 32-byte Merkle root per header. The hash is
what authenticates the header — but **the Merkle root is a field inside the header**, bytes 36 to
68 of the same 80 bytes the hash was computed from. Storing it separately was redundant: the hash
already commits to it.

The window stores **no Merkle root and no `bits`**. A deposit claim carries the raw 80-byte header;
the program checks `hash(supplied) == stored_hash[height]`, then reads the Merkle root straight out
of it. That check is not optional, because without it a claimant could substitute a header of its
own choosing and prove anything — there is a test for exactly that. `bits` is deliberately not
stored either: it is a re-encoding of the target the client recomputes anyway, so a stored copy
could only agree with itself or hide a wrong target.

| | Per header | Window | Account |
|---|---|---|---|
| Earlier layout, before cw-144 | 32 B (hash only) | 288 (48 h) | 9,322 B |
| **Now** — what the difficulty rule needs | **52 B (hash + chainwork + time)** | **192 (32 h)** | **10,107 B** |

**The window had to shrink from 288 records to 192, and the reason is not storage economy.** The
extra 20 bytes per record are what cw-144 consumes, and at 52 bytes 288 records would be 14,976
bytes — 46% over the 10,240-byte cap, so `initialize` would simply revert. **The window is 32 hours,
not 48**, and the lost sixteen hours are the difficulty rule's cost.

A further step is possible — store only every Nth hash and have the claimant supply the handful of
headers that bridge the gap — but it runs into the same 1232-byte transaction limit that constrains
everything else here, and it buys storage the protocol does not currently need. Worth knowing
about; not worth doing yet.

---

## Peg-in — BSV → `solBSV` (trustless)

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
   │                         │                           │  5. MATURITY: 144 blocks
   │                         │                           │  6. RELEASE — permissionless
   │◄────────────────────────────────────────────────────┤     hash still matches:
   │                         │                           │       vault → you
   │                         │                           │     hash differs (reorg):
   │                         │                           │       staged tokens BURN
```

1. **You send BSV** to the federation's deposit script, attaching an `OP_RETURN` that carries your
   Solana address. The design carries `version ‖ cluster_id ‖ program_hash ‖ flags ‖ recipient` so
   a payload is bound to this program and this cluster; **the built program checks only that the
   recipient's 32 bytes appear in an `OP_RETURN` in the same transaction**, which is what makes a
   deposit non-transferable between claims.
2. **You wait `FLOOR` — 12 BSV blocks**, about two hours, measured in *block time from the BSV
   headers*, never against a wall clock. **12 is a floor, not a price**: the deposit waits at least
   this long, and the design permits longer.
3. **The light client proves it.** The program verifies the 80-byte header's hash against its own
   stored window, the proof of work under **cw-144**, the header chain linkage, and the Merkle
   branch showing your transaction is in that block. No signature, committee or oracle is involved:
   **the proof is the authorisation**, and anyone may submit it for anyone.
4. **`solBSV` is minted into the vault, not to you.** The tokens exist, but they are not yet yours
   to spend: they sit in an account the program owns, and the record stores **the block hash the
   deposit was proven against**. That stored hash is what makes the next step decidable without
   asking anybody.
5. **Maturity: 144 blocks.** The staged mint waits.
6. **Release is permissionless.** Anyone may call it, and it requires two things of the program's
   own state: the tip has **advanced past** the deposit, and the hash the client now stores **at
   that height still matches** the recorded one. If it matches, the vault pays you. If it differs,
   the deposit was reorged: the staged tokens **burn**, and your BSV went back with the reorg, so
   you end exactly where you started. `release_mint` and `burn_staged` are both permissionless, so
   **no party's cooperation is ever required**, and a resolver reclaims the item's rent.

**Step 4 is what makes a fraudulent mint unsellable.** A staged token is not in anyone's wallet, so
there is nothing to dump and no innocent buyer to inherit the loss.

### The honest cliff

The block must still be inside the client's window when the proof is submitted, and the window is
**32 hours**. A deposit nobody proves within that time can never be proven, and the BSV is with the
federation. That is a real failure mode, disclosed rather than dressed up; the wallet-side app is
what mitigates it by minting automatically.

---

## Peg-out — `solBSV` → BSV (threshold-signed, verifiable)

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
   │                         │     KEY signs the payment │
   │                         │                           │
   │                         │  4. the payout is PROVED  │
   │                         │     against the light     │
   │                         │     client: the escrow    │
   │                         │     burns                 │
   │                         │                           │
   │  5. or CANCEL — permissionless after the deadline:   │
   │     the escrow returns to you                   ◄────┤
```

1. **You escrow `solBSV` into the vault** and name the BSV destination and a deadline. The escrow
   and the destination are native Solana state, so nothing needs proving.
2. **Members sign payout intents individually**, on Solana. This is the attribution mechanism:
   because each member signs separately, a member who signs two conflicting intents has produced
   **their own proof of guilt** — see [Slashing](04-trust-model.md#slashing--self-proving-misbehaviour).
3. **Once enough attributed intents exist, the threshold key signs the BSV payment.** The reserve is
   under a threshold key, so this needs a quorum and **no single member can move it**.
4. **Settlement is proved, not asserted.** The payout transaction is proved against the light
   client — inclusion and amount — and the escrow **burns**. Unlike a report from a signer, this is
   something the program can check.
5. **Failure returns; it never mints.** After the deadline, `cancel` is **permissionless** and the
   escrow returns to you. Supply is unchanged and you are whole without asking anyone.

**Steps 2–5 are designed, not built.** Peg-out is the half of the system that no code yet
implements; only the mint direction runs.

---

## What you experience

| | |
|---|---|
| **Peg-in latency** | `FLOOR` — 12 BSV blocks, about two hours — plus `MATURITY` at 144 blocks, about a day |
| **Peg-out latency** | The federation's signing and the payout, plus the proof; usually much less, but it is a design target rather than a measurement |
| **Fees** | **30 bp, governed** — on both directions. There is no order book and no discovered fee |
| **Who approves you** | Peg-in: nobody. Peg-out: a quorum of the federation signs, and each signature is on record |
| **What you need** | A BSV wallet and a Solana wallet |

## Why the waits exist

- **Depth (`FLOOR`) sets the cost of attacking.** A reorg must out-mine 12 honest blocks to undo a
  deposit, which is what makes the fraud expensive rather than free.
- **Maturity (`MATURITY`) sets the time available to detect.** The tokens sit staged long enough for
  honest headers to be pushed and an orphan noticed. Detect it and the staged tokens burn; the
  deposit itself already went back with the reorg.
- **The vault makes a detected fraud a non-event.** Because the tokens were never in a wallet,
  burning them removes supply that was never sold.

Everything is measured on a chain: depth and block time from BSV headers, deadlines from Solana
slots. **A Solana cluster halt freezes a deadline rather than punishing anyone who could not act.**

## What this document's predecessor said

This chapter previously described **per-relayer deposit scripts, an order book of staked bids, and
a fee discovered on it**. All three are **superseded**: deposits now pay one federation deposit
script, the order book is removed, and the fee is a governed **30 bp**. The reasoning is in
[`23-federation.md`](23-federation.md) §What this replaces. The old model's "depth is a term of the
bid" survives only as `FLOOR` plus the fact that the design permits deeper waits.

If you want speed without a wrapper, the market layer handles it: `solBSV` trades on
Raydium/Orca, so you can buy and sell at Solana speed while peg-in and peg-out handle the edges.

---

Next: [Architecture](03-architecture.md)
