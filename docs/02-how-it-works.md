# 2. How it works

> **Built or specified?** The light client, the `solBSV` token and the mint exist today and pass
> 17 on-chain tests. **The vault, the order book, per-relayer deposits and all of peg-out are
> designed, not built.** The program that ships mints straight to the depositor, so every step
> below that depends on the vault is a specification rather than a description of running code.
> [`13-summary.md`](13-summary.md) is the authoritative account; the reasoning is in
> [`12-peg-mechanism.md`](12-peg-mechanism.md).

## The two directions

```
   PEG IN — BSV to solBSV                  PEG OUT — solBSV to BSV

   1  send BSV + OP_RETURN                 1  escrow solBSV + a BSV address
   2  wait the agreed depth                2  a relayer pays BSV and proves it
   3  mint into the VAULT                  3  a challenge window follows
   4  MATURE: release to you               4  success: burn the escrow, the
      reorg first: burn the staged           relayer keeps the fee
      tokens. Your BSV went with           failure: return the escrow.
      the reorg, so you end where          Supply never changes.
      you started.
```

Both directions have the same shape: **enter the vault, then leave it either to the
counterparty or back to the sender.** A failure is a return, never a new mint.

That is the whole of the user experience. Everything below is what happens underneath.

---

## What it costs to run

The light client is the only part of SOLBEAM that has to be kept alive continuously, so it is worth being precise about which of its costs actually recur.

### Storage is a one-time deposit, not a burn

Solana charges rent-exemption up front, at roughly **5,080 lamports per byte**, and **the entire amount is returned when the account is closed**. It is a deposit, not a fee.

| | Size | One-time | at $77/SOL |
|---|---|---|---|
| Light client, 48-hour window | 9,286 B | 0.048 SOL | **$3.68** |
| *(Solana's per-account creation ceiling)* | *10,240 B* | *0.053 SOL* | *$4.06* |

A 24-hour window would cost half that, at 4,678 B. The 48-hour window is the one built, because the same money buys twice the reorg horizon — see below.

**This does not grow with the BSV chain, and that is the entire point of the rolling window.** If the client stored history instead, the same numbers would be:

| | Size | One-time | at $77/SOL |
|---|---|---|---|
| Every BSV header, 80 B each | 77 MB | 394 SOL | **$30,305** |
| Every BSV block hash, 32 B each | 31 MB | 157 SOL | **$12,122** |

…and it would keep climbing forever. The window turns a cost that grows without bound into a fixed deposit of a few dollars — roughly a **3,000×** reduction, and permanently capped.

### The recurring cost is transaction fees

Every BSV block needs one header pushed, so the client costs **144 transactions a day — 52,560 a year.** Each is a single-signature transaction whose base fee is 5,000 lamports. The measured compute is about 4,571 CU, so priority fees stack on top:

| Priority fee (µlamports/CU) | Fee per header | Per year | At $77/SOL |
|---|---|---|---|
| 0 (base fee only) | 5,000 lamports | 0.263 SOL | **$20** |
| 100,000 | 5,457 | 0.287 SOL | $22 |
| 1,000,000 | 9,571 | 0.503 SOL | $39 |
| 10,000,000 | 50,710 | 2.665 SOL | $205 |
| 100,000,000 | 462,100 | 24.29 SOL | $1,870 |

**Base fees are trivial; priority fees are the real variable.** Under congestion this is the number that matters, and it is beyond the protocol's control.

Two mitigations exist. Headers can be **batched**, since about 13 fit in one transaction: that cuts base fees to **$1.56/year**, at the cost of the client lagging about two hours behind the chain — acceptable, because a mint already waits out the confirmation depth the depositor chose, and then the maturity window on top. Batching does not reduce priority fees, which dominate.

### Being parsimonious: what the window stores, and why it is now one word long

An earlier layout kept a 32-byte block hash **and** a 32-byte Merkle root per header. The hash is what authenticates the header — but **the Merkle root is a field inside the header**, bytes 36 to 68 of the same 80 bytes the hash was computed from. Storing it separately was redundant: the hash already commits to it.

The window now stores a **bare block hash**. A deposit claim carries the raw 80-byte header; the program checks `hash(supplied) == stored_hash[height]`, then reads the Merkle root straight out of it. Same security, half the storage — and the check is not optional, because without it a claimant could substitute a header of its own choosing and prove anything. There is a test for exactly that.

| | Per header | Window | Account |
|---|---|---|---|
| Before | 64 B (hash + root) | 144 (24 h) | 9,286 B |
| Now | **32 B (hash only)** | **288 (48 h)** | 9,286 B |

**The same deposit now buys twice the reorg horizon** — two days instead of one, leaving a full day of margin over the "BSV is broken if it reorgs for a day" line.

A further step is possible — store only every Nth hash and have the claimant supply the handful of headers that bridge the gap — but it runs into the same 1232-byte transaction limit that constrains everything else here, and it buys storage the protocol does not currently need. Worth knowing about; not worth doing yet.

---

## Peg-in — BSV → `solBSV` (trustless, permissionless)

```
  YOU                    BSV CHAIN                    SOLANA
   │                         │                           │
   │  1. take terms from the book                        │
   │     (liquidity, fee, depth)                         │
   │  2. send BSV + OP_RETURN│                           │
   │     (your Solana addr)  │                           │
   ├────────────────────────►│                           │
   │                         │  3. wait the agreed depth │
   │                         │                           │
   │                         │  4. light client verifies │
   │                         │     header + PoW + Merkle │
   │                         ├──────────────────────────►│
   │                         │                           │  5. mint into the VAULT
   │                         │                           │     (not to you)
   │                         │                           │
   │                         │                           │  6. still canonical after
   │                         │                           │     maturity?
   │◄────────────────────────────────────────────────────┤     yes: released to you
   │                         │                           │     no:  staged tokens burned
```

1. **You take terms from the book.** An order book of underwriting lists what stakers will serve: how much liquidity, at what fee, waiting how many confirmations. You pick a bid. **Depth is a term of the trade, not a fixed number** — accept a longer wait and you should get a better rate, because that wait is less risk for whoever fronts the mint.
2. **You send BSV** to that relayer's own script, attaching an `OP_RETURN` that carries your Solana address and the depth you agreed. There is **no shared bridge address**: each relayer receives its own deposits, so there is no single key whose theft drains the system. (BSV's data-carrier limit is effectively unlimited, so this is a normal transaction.)
3. **The agreed depth passes**, measured in block time from the BSV headers — never against a wall clock.
4. **The light client proves it.** A BSV light client running on Solana verifies the 80-byte header, its proof-of-work against the difficulty target, the header chain linkage, and the Merkle branch showing your transaction is in that block.
5. **`solBSV` is minted into the vault, not to you.** The tokens exist, but they are not yet yours to spend: they sit in a token account the program owns.
6. **The vault releases after maturity.** Once a maturity window passes with your deposit's block still canonical, the tokens are released to the address you named. If a reorg is followed first, the staged tokens are **burned** instead — and your BSV went back with the reorg, so you end exactly where you started.

No one approves this. There is no oracle, no attestor, no committee vote. **The proof is the authorisation.** Anyone can do it, for anyone, at any time.

---

## Peg-out — `solBSV` → BSV (permissionless, optimistic)

```
  YOU                    SOLANA                       BSV CHAIN
   │                         │                           │
   │  1. escrow solBSV       │                           │
   │     into the VAULT      │                           │
   │     + BSV destination   │                           │
   ├────────────────────────►│                           │
   │                         │  2. a relayer whose bond  │
   │                         │     covers it accepts     │
   │                         │                           │
   │                         │  3. the relayer pays BSV  │
   │                         ├──────────────────────────►│
   │                         │     and proves the payout │
   │                         │     against the light     │
   │                         │     client                │
   │                         │                           │
   │                         │  4. a challenge window    │
   │                         │     follows; a reorged    │
   │                         │     payout is caught here │
   │                         │                           │
   │  5. success: escrow     │                           │
   │     burned, relayer     │                           │
   │     keeps the fee       │                           │
   │     failure: escrow ◄───┤                           │
   │     returned to you     │                           │
```

1. **You escrow `solBSV` into the vault** and name the BSV address you want paid. The escrow and the destination are recorded in the bridge program — native Solana state, so nothing needs proving.
2. **A relayer whose bond covers the amount accepts the request.** Any relayer may; this is underwriting rather than permission, and it is why the bond is denominated in `solBSV`.
3. **The relayer pays you BSV and proves it.** It has a deadline measured in Solana slots — so a cluster halt freezes the clock rather than burning a relayer that could not act. It then submits the BSV transaction with an SPV proof; the program verifies the payout against the light client and checks the amount and destination match your request.
4. **A challenge window follows**, during which a payout that a later reorg removes can be caught.
5. **Settlement.** On success the escrowed `solBSV` is **burned** and the relayer keeps the fee. On failure the escrow is **returned to you**. Supply is unchanged either way: no failure path mints.

---

## What you experience

| | |
|---|---|
| **Peg-in latency** | The agreed depth — 12 BSV blocks minimum, about two hours — plus the maturity window |
| **Peg-out latency** | The relayer's deadline, in slots, plus the challenge window; usually much less |
| **Fees** | Discovered on the order book, published, and paid to whoever underwrites the trade |
| **Who approves you** | Peg-in: nobody. Peg-out: any relayer may take the request — underwriting, not permission |
| **What you need** | A BSV wallet and a Solana wallet |

## Why the wait exists

The waits are not arbitrary, and they now come from the market rather than a constant:

- **On the way in**, depth is what makes a reorg expensive — an attacker has to out-mine it — while maturity is what makes a reorg visible. The tokens are staged in the vault long enough for honest headers to be pushed and an orphan noticed. Detect it and the staged tokens are burned; the deposit itself already went back with the reorg.
- **On the way out**, the challenge window gives the network time to see a payout and for any challenge to be raised before settlement is final.

`FLOOR` — twelve blocks — remains as a backstop under the book, so no trade can commit to a depth so shallow that the attack is cheap. Everything is measured on a chain: depth and block time from BSV headers, deadlines from Solana slots.

If you want speed without a wrapper, the market layer handles it: `solBSV` trades on Raydium/Orca, so you can buy and sell at Solana speed while peg-in and peg-out handle the edges.

---

Next: [Architecture](03-architecture.md)
