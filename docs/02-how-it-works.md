# 2. How it works

## The two steps

```
   STEP 1                 STEP 2                     RESULT
 accumulate   ──────►      wait 12 conf   ──────►     solBSV
   (BSV)                  (~2 hours)                 (Solana)
```

That is the whole user experience. Everything below is what happens underneath.

---

## What it costs to run

The light client is the only part of SOLBEAM that has to be kept alive continuously, so it is worth being precise about which of its costs actually recur.

### Storage is a one-time deposit, not a burn

Solana charges rent-exemption up front, at roughly **5,080 lamports per byte**, and **the entire amount is returned when the account is closed**. It is a deposit, not a fee.

| | Size | One-time | at $77/SOL |
|---|---|---|---|
| Light client, 24-hour window (today) | 9,286 B | 0.048 SOL | **$3.68** |
| Light client, 24-hour window, hash-only | 4,678 B | 0.024 SOL | **$1.88** |
| *(Solana's per-account creation ceiling)* | *10,240 B* | *0.053 SOL* | *$4.06* |

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

Two mitigations exist. Headers can be **batched**, since about 13 fit in one transaction: that cuts base fees to **$1.56/year**, at the cost of the client lagging about two hours behind the chain — acceptable, because minting already waits twelve confirmations. Batching does not reduce priority fees, which dominate.

### Being parsimonious: we are storing one thing we do not need

The header record currently holds a 32-byte block hash **and** a 32-byte Merkle root. The hash is what authenticates the header. But **the Merkle root is a field inside the header** — bytes 36 to 68 of the same 80 bytes the hash was computed from.

So the Merkle root is already covered by the hash, and storing it separately is redundant. A deposit claim can simply carry the raw 80-byte header; the program checks `hash(supplied) == stored_hash[height]` and reads the Merkle root out of it. That is the same security for half the storage:

| | Per header | Window | Fits in one account |
|---|---|---|---|
| Today | 64 B | 144 (24 h) | 158 headers |
| Hash only | 32 B | 144 (24 h) | 317 headers |
| Hash only, same budget | 32 B | **288 (48 h)** | — |

Same deposit, **twice the reorg horizon**: two days instead of one, which leaves a full day of margin over the "BSV is broken if it reorgs for a day" line. This is the recommended direction.

A further step is possible — store only every Nth hash and have the claimant supply the handful of headers that bridge the gap — but it is bounded by the same 1232-byte transaction limit that constrains everything else here, and it buys storage the protocol does not currently need. Worth knowing about; not worth doing yet.

---

## Mint — BSV → solBSV (trustless, permissionless)

```
  YOU                    BSV CHAIN                    SOLANA
   │                         │                           │
   │  1. send BSV + OP_RETURN│                           │
   │     (your Solana addr)  │                           │
   ├────────────────────────►│                           │
   │                         │  2. 12 confirmations      │
   │                         │                           │
   │                         │  3. light client verifies │
   │                         │     header + PoW + Merkle │
   │                         ├──────────────────────────►│
   │                         │                           │  4. mint solBSV
   │◄────────────────────────────────────────────────────┤     to you
```

1. **You send BSV** to the reserve address, attaching an `OP_RETURN` that carries your Solana address. (BSV's data-carrier limit is effectively unlimited, so this is a normal transaction.)
2. **Twelve confirmations** pass — about two hours on BSV.
3. **The light client proves it.** A BSV light client running on Solana verifies the 80-byte header, its proof-of-work against the difficulty target, the header chain linkage, and the Merkle branch showing your transaction is in that block.
4. **`solBSV` is minted** to the address you specified.

No one approves this. There is no oracle, no attestor, no committee vote. **The proof is the authorisation.** Anyone can do it, for anyone, at any time.

---

## Redeem — solBSV → BSV (permissionless, optimistic)

```
  YOU                    SOLANA                       BSV CHAIN
   │                         │                           │
   │  1. burn solBSV         │                           │
   │     + BSV destination   │                           │
   ├────────────────────────►│                           │
   │                         │  2. redemption request    │
   │                         │     recorded, 6h deadline │
   │                         │                           │
   │                         │  3. a relayer pays you BSV│
   │                         │◄──────────────────────────┤
   │                         │                           │
   │                         │  4. relayer submits SPV   │
   │                         │     proof of the payout   │
   │◄────────────────────────┤     → program verifies    │
   │  5. tokens burned,      │                           │
   │     BSV received        │                           │
   │                         │                           │
   │  6. no payout in 6h?    │                           │
   │     solBSV re-minted ◄──┤  relayer bond slashed     │
```

1. **You burn `solBSV`** and name the BSV address you want paid. The burn and the destination are recorded in the bridge program — this is native Solana state, so nothing needs proving.
2. **A redemption request** is created with a **6-hour deadline**.
3. **A relayer pays you BSV** from its hot wallet.
4. **The relayer proves the payout.** It submits the BSV transaction with an SPV proof; the Solana program verifies it against the light client and checks the amount and destination match your request.
5. **Your redemption closes.** The relayer keeps the fee.
6. **If no valid payout arrives by the deadline**, the program **re-mints your `solBSV` automatically** and the bond is slashed. **You cannot lose.**

---

## What you experience

| | |
|---|---|
| **Mint latency** | ~2 hours (12 BSV confirmations) |
| **Redeem latency** | Up to 6 hours (settlement window), usually much less |
| **Fees** | A percentage of the redeemed amount, set by governance and published |
| **Who approves you** | Nobody |
| **What you need** | A BSV wallet and a Solana wallet |

## Why the wait exists

Twelve confirmations and a six-hour redemption window are not arbitrary. Both exist to make **reorgs** a non-issue:

- **On the way in**, the wait means a BSV reorg cannot un-mint you.
- **On the way out**, the window gives the network time to see a payout and for any challenge to be raised before settlement is final.

If you want speed without a wrapper, the market layer handles it: `solBSV` trades on Raydium/Orca, so you can buy and sell at Solana speed while mint and redeem handle the edges.

---

Next: [Architecture](03-architecture.md)
