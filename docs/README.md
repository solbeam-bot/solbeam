# SOLBEAM

**Atomic wrapper on Solana for BSV.**

SOLBEAM brings native BSV to Solana as `solBSV` — a 1:1 wrapper you can hold, trade and use, and redeem back to real BSV. No federation, no committee, no trusted custodian. Minting is verified by a BSV light client running on Solana; redemption is permissionless and bonded.

---

## Step 1 — Accumulate

You hold BSV. You want it to be useful at Solana speed.

## Step 2 — Wait

Send it to your SOLBEAM deposit address. After **12 BSV confirmations** (about two hours), `solBSV` appears in your Solana wallet. That's it.

```
   STEP 1                 STEP 2                    RESULT
 accumulate   ──────►      wait 12 conf   ──────►    solBSV
   (BSV)                  (~2 hours)                (Solana)
```

---

## Why this exists

BSV is fast to mine but slow to *move*. Exchanges hold deposits and withdrawals for long confirmation windows because reorgs are expensive to them, and moving sizeable BSV between venues is a manual, hours-to-days process. That friction is the single biggest practical obstacle to BSV being used as money and as a trading asset.

`solBSV` removes the friction. It is a Solana SPL token, so it moves at Solana speed, trades on Raydium and Orca like any other token, and can be used in Solana DeFi. Anyone can create it (mint) and anyone can get back to BSV (redeem) without asking permission, opening an account, or waiting on an exchange.

---

## What SOLBEAM is — and isn't

| | |
|---|---|
| **Minting is trustless** | A BSV light client on Solana verifies your deposit. No attestor approves it, no oracle signs off. The proof *is* the authorisation |
| **Redemption is permissionless and optimistic** | Anyone can become a bonded "relayer" who pays out BSV. If a payout doesn't happen in time, the holder is automatically made whole |
| **No federation** | No named signer set, no validator committee, no governance body holding funds |
| **Reserve is tiered and covenant-locked** | A small hot float covered by bonds, and a large cold reserve locked by a BSV covenant that can only pay the hot wallet |
| **Market layer is external** | Liquidity comes from Raydium / Orca, P2P orderbooks and market makers. SOLBEAM is the wrapper, not an exchange |
| **Trading is instant** | Only the peg has latency. Traders on a pool, a P2P swap or a market maker never wait for it — see [Markets & liquidity](11-markets-and-liquidity.md) |
| **FOSS** | The light client, programs and relayer software are open source |

**Honest limits.** Minting is trustless. Redemption is *trust-minimised*: a bonded relayer must hold a key to the small hot float, and the bond plus on-chain fraud proofs are what keep them honest. We call that what it is. A signerless design — where the cold reserve releases funds against a zero-knowledge proof of the Solana burn, verified inside BSV Script — is the roadmap target, not the launch product.

---

## Start here

- [The problem](01-problem.md)
- [How it works](02-how-it-works.md)
- [Architecture](03-architecture.md)
- [Trust model](04-trust-model.md)
- [Relayers](05-relayers.md)
- [Parameters & governance](06-parameters.md)
- [Roadmap](07-roadmap.md)
- [FAQ](08-faq.md)
- [Glossary](09-glossary.md)
- [Brand](10-brand.md)
- [Markets & liquidity](11-markets-and-liquidity.md)
