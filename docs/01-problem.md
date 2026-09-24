# 1. The problem

## BSV is fast to mine, slow to move

A BSV block arrives roughly every ten minutes. That is not the bottleneck. The bottleneck is **confirmation policy**: because a chain can reorganise, every custodian (exchanges especially) chooses how many confirmations a block requires before they treat it as final, and how long a withdrawal must sit before they broadcast it. For BSV, those windows are long.

The result is friction at exactly the wrong moment:

- **Moving sizeable BSV between venues** takes hours to days, and often a support ticket.
- **Trading BSV** means parking it on an exchange, with all the custody and counterparty risk that implies.
- **New users** — particularly in places where obtaining BSV is genuinely difficult (eg USA) — face a slow, gated on-ramp.
- **Arbitrage** between venues is slow, so price dislocations persist longer than they should, market makers hate this.

Meanwhile the asset itself is fine. The problem is plumbing.

## What a wrapper fixes

A wrapped representation on a fast chain solves the movement problem without touching BSV itself:

1. **Speed.** Once `solBSV` exists, it moves at Solana speed — seconds, not hours.
2. **Permissions.** Minting and redeeming do not require an exchange account, a jurisdiction, or an approval, meaning its easier .
3. **Composability.** `solBSV` is an ordinary SPL token, so it trades on Raydium and Orca and can be used across Solana DeFi.
4. **Arbitrage.** Anyone can mint when `solBSV` trades above BSV and redeem when it trades below. That pressure is what keeps the wrapper honest and the price tight.

## Why not just use an existing bridge?

Existing bridges for Bitcoin-family assets on Solana are **federated or custodial**: guardian sets, notary committees, or a custodian holding the coins. They work, but they ask you to trust a group of operators, and their track record is mixed. The assets they wrap also carry small supply — the market has not rewarded the model.

SOLBEAM takes a different starting point: **make the peg in trustless, and enforce honesty on the peg out.**

- **Peg-in is trustless** because Solana can verify BSV directly: BSV uses double-SHA-256 over an 80-byte header, and Solana has a native SHA-256 syscall. A light client can check proof-of-work, the header chain and transaction inclusion without trusting anyone.
- **Peg-out cannot be trustless today** for a reason that is mathematical rather than architectural: releasing native BSV requires a BSV signature, and BSV Script cannot verify Solana's ed25519 consensus. So a key must exist somewhere. SOLBEAM makes that key **small, bonded, and fraud-punishable** — and treats eliminating it as the long-term research goal.

That asymmetry — trustless in, trust-minimised out.

---

Next: [How it works](02-how-it-works.md)
