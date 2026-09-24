# 2. How it works

## The two steps

```
   STEP 1                 STEP 2                     RESULT
 accumulate   ──────►      wait 12 conf   ──────►     solBSV
   (BSV)                  (~2 hours)                 (Solana)
```

That is the whole user experience. Everything below is what happens underneath.

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
