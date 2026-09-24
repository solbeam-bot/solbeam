# 4. Trust model & security

SOLBEAM is deliberately asymmetric: **trustless in, trust-minimised out.** This chapter states exactly what you are trusting, and why.

## Summary

| Property | Status |
|---|---|
| Minting authorised by proof, not by a person | **Trustless** |
| Supply backing (1 `solBSV` = 1 BSV locked) | **Trustless accounting**, verified on-chain |
| Redemption payout | **Trust-minimised** — a bonded relayer holds the small hot float |
| Censorship of mints | **None** — anyone can mint, for anyone |
| Censorship of redemptions | **Bounded** — a relayer may decline, but the deadline refund restores the holder |
| Reserve custody at large | **Covenant-locked** — cannot be diverted, only released to the hot wallet in tranches |
| Market / price | External (Raydium/Orca) |

## What is trustless, and why

**Solana can verify BSV.** BSV uses double-SHA-256 proof-of-work over an 80-byte header, and Solana exposes a native SHA-256 syscall. So a BSV light client on Solana can check, from first principles:

- the header chain links correctly,
- each header meets its difficulty target (including BSV's difficulty adjustment),
- a given transaction is included in a given block via its Merkle branch.

Minting is authorised by that proof alone. There is no attestor to bribe, no oracle to spoof, no committee to capture, and no way to censor a mint.

## Why redemption cannot be fully trustless (today)

Releasing native BSV requires a valid BSV signature. Solana programs cannot sign BSV transactions, and **BSV Script cannot verify Solana's ed25519 consensus** — BSV has only secp256k1 `OP_CHECKSIG`/`OP_CHECKMULTISIG`, and stake-weighted validator aggregation is not script-expressible.

So at the instant of redemption, *some key must exist*. This is a property of the two chains, not a shortcut in the design. SOLBEAM's response is to make that key:

- **small** — it holds only the hot float, not the reserve;
- **bonded** — collateral posted on Solana exceeds what it can steal;
- **fraud-punishable** — unauthorised spends are provable on Solana and slash the bond.

## Who checks what

| Step | Verified by |
|---|---|
| Burn / redemption request | The Solana program — native state, nothing to prove |
| Deposit (BSV → mint) | The Solana program, against the BSV light client |
| Payout (BSV → redeem) | The Solana program, against the BSV light client |
| Unauthorised reserve spend | **Permissionless challenger** submits the evidence; the **program** verifies it |
| Failed redemption | Automatic — the **program** re-mints the holder after the deadline |

Verification is always done by deterministic on-chain code. People are needed only to *notice and report* theft — an open, rewarded bounty, not a committee.

## Core invariants

1. **Backing.** Custodied BSV ≥ outstanding `solBSV` at all times — enforced by the mint and burn paths.
2. **Exposure.** `bond value ≥ k × (hot float + releasable tranche)`, with a conservative safety factor `k` and a price governor that throttles if the ratio worsens.
3. **No diversion.** Cold reserve covenant outputs can only pay the hot wallet address.
4. **Holder protection.** Every redemption either completes or is automatically refunded after the deadline.

## Threats and answers

| Threat | Answer |
|---|---|
| Relayer steals the hot float | Bond ≥ exposure, slashed on a permissionless proof; loss bounded to the float |
| Relayer takes a job and disappears | Deadline re-mints the holder; the committed relayer's bond is slashed |
| Cold key compromised | Covenant permits payment only to the hot wallet, in tranches — the reserve cannot be diverted |
| Nobody fulfils redemptions | Holders are automatically refunded; open redemption is a public race, and fees attract relayers |
| Fake deposit proof | Rejected by the light client (PoW / DAA / Merkle) |
| Reorg after mint | 12-confirmation depth required before minting |
| Reorg after payout | 6-hour settlement window |
| BSV price rises sharply | The bond is in a stable unit while exposure is in BSV, so the **price governor throttles** (smaller float, slower tranches) and demands bond top-ups. This means **slower redemptions, still fully backed** — never unbacked tokens |
| Weak BSV hash rate | SPV security inherits the most-work assumption; BSV's hash rate is low relative to Bitcoin's. Mitigated by deep confirmations (12+), checkpoint finality for large mints, and conservative caps |
| Bridge program upgrade | Upgrade authority held by a timelocked multisig; staged caps; audits before raising them |

## The roadmap to a signerless reserve

The hot key exists only because the cold reserve cannot verify a Solana burn. On BSV that is *expressible*: the reserve can be locked by a covenant that releases funds only against a **zero-knowledge proof of the burn**, verified inside BSV Script. BSV is unusually suited to this — `OP_CAT` and `OP_MUL` are active, script size is effectively unlimited, and in-script Groth16/STARK verification has been demonstrated (BSVM, MIT, pre-mainnet and unaudited).

If it works, there is no hot key, no bond and no price mismatch — the reserve releases itself against a valid proof. That is the research track, and it is the reason SOLBEAM's design keeps the redemption authority behind a replaceable boundary: the token, the mint path and the light client do not change when it lands.

## What we ask you to trust — plainly

1. That the bridge program and light client are correctly implemented and audited (they are FOSS; verification is the community's).
2. That the bond behind the hot float is real, sized correctly, and enforced.
3. That governance parameters (caps, fees, deadlines, tranche schedules) are managed honestly and in the open.

Everything else — deposits, backing, minting, and the payout proof — is enforced by code.

---

Next: [Relayers](05-relayers.md)
