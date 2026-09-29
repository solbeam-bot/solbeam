# 1. The problem

> **Built or specified?** The light client with cw-144, the `solBSV` token, the mint and fork
> staging exist today and pass **20 on-chain tests**. **The vault, the bonded federation, threshold
> custody, governance, slashing and all of peg-out are designed, not built.**
> [`13-summary.md`](13-summary.md) is the authoritative account.

## BSV is fast to mine, slow to move

A BSV block arrives roughly every ten minutes. That is not the bottleneck. The bottleneck is
**confirmation policy**: because a chain can reorganise, every custodian (exchanges especially)
chooses how many confirmations a block requires before treating it as final, and how long a
withdrawal must sit before it is broadcast. For BSV, those windows are long.

The result is friction at exactly the wrong moment:

- **Moving sizeable BSV between venues** takes hours to days, and often a support ticket.
- **Trading BSV** means parking it on an exchange, with all the custody and counterparty risk that implies.
- **New users** — particularly in places where obtaining BSV is genuinely difficult (eg USA) — face a slow, gated on-ramp.
- **Arbitrage** between venues is slow, so price dislocations persist longer than they should.

The asset itself is fine. The problem is plumbing.

## What a wrapper fixes

A wrapped representation on a fast chain solves the movement problem without touching BSV itself:

1. **Speed.** Once `solBSV` exists, it moves at Solana speed — seconds, not hours.
2. **Permissions.** Minting and redeeming do not require an exchange account, a jurisdiction, or an approval.
3. **Composability.** `solBSV` is an ordinary SPL token, so it can be used across Solana DeFi.
4. **An exit that cannot be closed.** Redemptions can never be paused, so the way back to BSV stays open even while governance is changing the rules.

## Why not just use an existing bridge?

Existing bridges for Bitcoin-family assets on Solana are **federated or custodial**: guardian sets,
notary committees, or a custodian holding the coins. They work, but they ask you to trust a group
of operators, and their track record is mixed.

SOLBEAM's answer is a **bonded federation** — and that is a deliberate change from earlier drafts
of this document, which described an operator-less design. The honest division is:

- **Minting is trustless.** A Solana program verifies BSV proof of work (under BSV's real rule,
  **cw-144**) and Merkle inclusion itself. No signature, no committee and no oracle can mint anything.
- **Reversal is trustless.** The program compares its own stored header hash against the one a
  deposit was proven with. A reorg is a fact about headers, not a report from anyone.
- **The reserve is trusted, and bounded.** The BSV sits under a **threshold key** held by the
  federation, so no single member can move it. What protects a holder is a **bond anyone can seize
  by proving misbehaviour on-chain** — not the absence of trust.

That leaves **one trust assumption: a threshold of federation members do not collude.** It is not
eliminated. It is bounded — by bonds that exceed what the members could take, by a 30-day governed
exit window during which redemptions never pause, and by proofs anyone can submit.

The scale limit follows from the same fact, and it is stated rather than implied: with `k = 1`,
value locked is capped by the bonds pledged. Ten members at the canonical **1,000 BSV** bond is
roughly **$300k** of capacity. That is a proof of concept, and it says so.

---

Next: [How it works](02-how-it-works.md)
