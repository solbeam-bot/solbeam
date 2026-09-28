# SOLBEAM

**Atomic wrapper on Solana for BSV.**

SOLBEAM brings native BSV to Solana as `solBSV` — a 1:1 wrapper you can hold, trade and use, and redeem back to real BSV. No federation, no committee, no trusted custodian. Minting is verified by a BSV light client running on Solana; redemption is permissionless and bonded. In the finished design a mint lands in a **program-owned vault** first, a relayer is a role **anyone may run**, and the program consults **no oracle** — only BSV headers and Solana slots.

---

## Step 1 — Choose your terms

You hold BSV. You want it to be useful at Solana speed.

Take terms from the order book — how much liquidity, at what fee, and at what confirmation depth — or deposit with no underwriter at all and accept that risk explicitly.

## Step 2 — Send, then wait the agreed depth

Send BSV to the named deposit script, attaching an `OP_RETURN` that carries your Solana address. After **the depth the bid named** — a term of the trade, not a fixed constant — `solBSV` is minted into the program's **vault** — as designed, not to you — and released to your wallet once a maturity window passes with the deposit still canonical. If a reorg is followed in the meantime, the staged tokens are burned and you end exactly where you started.

```
   STEP 1                 STEP 2                    RESULT
 choose terms   ──────►   wait the bid's  ──────►    solBSV
 (liquidity,              depth, then the            (Solana)
  fee, depth)             maturity window
```

> **Built or designed?** The light client, the token and the mint exist and pass 17 on-chain tests. **The vault, the order book, per-relayer deposits and all of peg-out are designed and not built.** The shipped program mints straight to the depositor's token account, so the vault and maturity steps above are a specification today, not shipped behaviour.

---

## Why this exists

BSV is fast to mine but slow to *move*. Exchanges hold deposits and withdrawals for long confirmation windows because reorgs are expensive to them, and moving sizeable BSV between venues is a manual, hours-to-days process. That friction is the single biggest practical obstacle to BSV being used as money and as a trading asset.

`solBSV` removes the friction. It is a Solana SPL token, so it moves at Solana speed, trades on Raydium and Orca like any other token, and can be used in Solana DeFi. Anyone can create it (mint) and anyone can get back to BSV (redeem) without asking permission, opening an account, or waiting on an exchange.

---

## What SOLBEAM is — and isn't

| | |
|---|---|
| **Minting is trustless** | A BSV light client on Solana verifies your deposit. No attestor approves it, no oracle signs off. The proof *is* the authorisation, and the design stages the mint in a program-owned vault |
| **Redemption is permissionless and optimistic** | Anyone can become a bonded "relayer" who pays out BSV. If a payout doesn't happen in time, the holder is automatically made whole |
| **No federation, no operator** | No named signer set, no validator committee, no governance body holding funds. A relayer is a role anyone may run, not a privileged party |
| **No pooled reserve** | Deposits pay individual relayers. There is no pooled hot wallet and no covenant-locked cold reserve to trust — aggregation is what creates a single key worth stealing |
| **Market layer is external** | Liquidity comes from Raydium / Orca, P2P orderbooks and market makers. SOLBEAM is the wrapper, not an exchange — and once `solBSV` is on a market, that exit is outside the protocol's control |
| **Trading is instant** | Only the peg has latency. Traders on a pool, a P2P swap or a market maker never wait for it — see [Markets & liquidity](11-markets-and-liquidity.md) |
| **Exit is slower than entry** | Minting is trustless; as designed it stages the mint through the vault. Redemption waits on a bonded relayer and a slot-measured deadline, and relayer capital unbonds on a notice period. The fast direction is the one that needs no trusted party |
| **Bonds are in `solBSV`** | A relayer's collateral is the same asset as the exposure, so no BSV price move can shrink it relative to what it protects — and no oracle is needed to keep the two matched |
| **FOSS** | The light client, programs and relayer software are open source |

**Honest limits.** Minting is trustless. Redemption is *trust-minimised*: a bonded relayer holds a key to its own deposits, and the bond plus on-chain fraud proofs are what keep it honest. Bonding can make cheating unprofitable; it cannot make it impossible, and it does nothing against someone who takes a relayer's key and never posted a bond. There is no pooled hot wallet to drain and no privileged operator to compromise. We call that what it is, in [Trust model](04-trust-model.md#the-naked-option-attack). And a DEX or exchange exit is outside the protocol's control: if a fraudulent mint ever succeeded, the loss would land on whoever bought the unbacked token, and the protocol cannot compensate them.

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
