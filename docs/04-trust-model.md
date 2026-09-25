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
| Bond denomination | `solBSV` — the same unit as the exposure, so no price move can shrink it |
| Market / price | External (Raydium/Orca) |
| Exit speed vs entry | **Deliberately slower** — minting completes after 12 BSV confirmations (~2 hours) with no trusted party involved; redemption waits on a bonded relayer and a 6-hour deadline. The fast direction is the one that needs no trust |

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
- **bonded in the same unit as the exposure** — the bond is `solBSV`, so no BSV price move can shrink it relative to what it protects;
- **fraud-punishable** — unauthorised spends are provable on Solana and slash the bond;
- **rate-limited** — the cold covenant releases to the hot wallet only in capped tranches, so the stealable amount cannot balloon between challenges.

Bonding can make theft *unprofitable*. It cannot make it *impossible*, and it does nothing at all against an attacker who never posted a bond. The rest of this chapter is about how much that actually costs.

## Who checks what

| Step | Verified by |
|---|---|
| Burn / redemption request | The Solana program — native state, nothing to prove |
| Deposit (BSV → mint) | The Solana program, against the BSV light client |
| Payout (BSV → redeem) | The Solana program, against the BSV light client |
| Unauthorised reserve spend | **Permissionless challenger** submits the evidence; the **program** verifies it |
| Failed redemption | Automatic — the **program** re-mints the holder after the deadline |

Verification is always done by deterministic on-chain code. People are needed only to *notice and report* theft — an open, rewarded bounty, not a committee.

That last sentence carries more weight than it first appears. A theft attached to a redemption is **self-reporting**: the deadline passes, the holder is refunded, the slash fires. A theft with **no redemption attached self-reports nothing**, because there is no deadline to miss. For that case the challenger bounty is not a nice-to-have — it is the entire enforcement mechanism. The design's answer is to make that case small and hard, rather than to depend on somebody watching.

## Core invariants

1. **Backing.** Custodied BSV ≥ outstanding `solBSV` at all times — enforced by the mint and burn paths.
2. **Exposure.** `bond ≥ k × (hot float + releasable tranche)`, **with both sides denominated in `solBSV`**. Because the bond and the exposure are the same asset, the inequality holds at every BSV price — no oracle, no governor, no reaction window.
3. **Solvency after theft.** A slash is what keeps the system whole: the reserve falls by the stolen amount and the supply falls by the slashed bond, so with `bond ≥ exposure` the backing per remaining token does not fall. Solvency must therefore **not depend on anyone submitting a proof** — which is what invariant 5 is for.
4. **No diversion.** Cold reserve covenant outputs can only pay the hot wallet address.
5. **No idle float.** The hot wallet holds nothing that is not earmarked to an outstanding redemption. This turns a detection problem into an accounting one: any theft then necessarily strands commitments, which self-report and auto-slash.
6. **Holder protection.** Every redemption either completes or is automatically refunded after the deadline.
7. **Bonds lock.** A bond withdrawable on demand is not a bond. Release requires settling outstanding commitments and waiting out the unbonding period.

## Threats and answers

| Threat | Answer |
|---|---|
| Relayer steals an **earmarked** float (theft attached to a burn) | Deadlines fail, holders are auto-refunded, the committed relayer's bond is slashed — self-reporting, no watcher required |
| Relayer steals an **idle** float (naked spend, no burn) | No deadline fires, so the bond is seized only if a challenger submits the spend. Bounded by the float cap, and closed structurally by **no idle float** plus a **veto-only cosigner** |
| Nobody fulfils redemptions | Holders are automatically refunded; open redemption is a public race, and fees attract relayers |
| Fake deposit proof | Rejected by the light client (PoW / DAA / Merkle) |
| Reorg after mint | 12-confirmation depth required before minting |
| Reorg after payout | 6-hour settlement window |
| Relayer exits to dodge a slash | The bond cannot be withdrawn instantly: unbonding requires a notice period that outlasts both the redemption deadline and the challenge window |
| Hot key stolen by an outsider | The thief never posted a bond, so slashing compensates holders rather than deterring the thief. Bounded by the float cap; mitigated by no idle float, a veto-only 2-of-2 cosigner, HSM/KMS custody and key rotation |
| Cold key compromised | Covenant permits payment only to the hot wallet, in tranches — the reserve cannot be diverted |
| BSV price rises sharply | **No longer a solvency risk.** Bond and exposure are both `solBSV`, so they move together — no top-up demand, no governor, and no window for an attacker to strike in. A sharp move only changes relayer fee revenue in dollar terms |
| Weak BSV hash rate | SPV security inherits the most-work assumption; BSV's hash rate is low relative to Bitcoin's. Mitigated by deep confirmations (12+), checkpoint finality for large mints, and conservative caps |
| Bridge program upgrade | Upgrade authority held by a timelocked multisig; staged caps; audits before raising them |

## The naked-option attack

The most important attack on this design needs no redemption at all, and it is worth stating in its simplest form because it is what sets the bond parameters.

Anyone holding the hot key can:

1. Spend the hot float to themselves.
2. Accept the bond slash, if it comes.

No burn, no redemption, no deadline, no victim. That position is a **naked option**: pay the bond `B`, receive the float `H`. It pays exactly when `H > B` — and it is *strictly easier* than attaching the theft to a redemption, because a redemption is what creates the deadline that slashes with certainty.

**The burn is not the attack; the burn is the liability.** The enforcement path that fires automatically is precisely the one an intelligent attacker avoids.

### The cost, and why `k` is a detection parameter

The attacker's cost is not `B`. It is:

```
expected cost  =  B × P(slashed)
```

because a naked spend is punished only if somebody constructs the proof. Deterrence therefore requires:

```
B  >  H / P
```

At `P = 0.5` the bond must exceed twice the float. If a challenger is unlikely to be watching, `P` collapses and the bond required explodes. This is the honest meaning of the safety factor `k ≈ 5–10` used in [Parameters](06-parameters.md): **`k` is roughly `1/P` — a guess at detection probability wearing a number.** Sizing a bond on an assumption about human vigilance is the weakest link in the model.

### Closing it, strongest first

1. **Hold no idle float.** If the hot wallet carries nothing outside outstanding commitments, a naked theft has nothing to take — and any theft that does happen strands commitments that self-report and auto-slash. The covenant cannot read Solana state, so this is enforced by the program's accounting plus tight tranche sizing and a return-to-cold rule. That makes it operating discipline rather than a hard Script guarantee, and it should be described as such.
2. **A veto-only cosigner.** Make the hot wallet a 2-of-2 bare multisig (workable on BSV without P2SH) whose second key is held by a separate party whose only power is to refuse. They cannot steal — that needs the relayer's key as well — so the trust assumption collapses from *"can take the money"* to *"can delay a redemption"*, and delay is already priced in by the deadline refund. This is the only mechanism that addresses an **outsider who steals the hot key**, since that attacker never posted a bond to be slashed.
3. **Split the float.** Independent relayers, each with its own float and its own bond, so no single key can reach the whole exposure. Pooling instead means one theft is every holder's loss.
4. **Size `k` against the residual.** With `solBSV` denomination, `k` has no price job left. While enforcement still depends on a permissionless challenger, keep `k = 5`; once steps 1–2 land, `k` can fall toward `~1.5–2` operational margin. That is where the capital efficiency is recovered.
5. **An unbonding period.** Release of the bond must outlast the redemption deadline plus the challenge window, or a thief can take a job, withdraw, and be gone before anyone can respond.

### Why the bond is `solBSV` and not a stablecoin

A stablecoin bond against a BSV liability is not a bond. It is a **written call option on the reserve, struck at `bond ÷ float`**. Post `$500k` against a `10,000 BSV` reserve and the relayer is short `5,000 BSV`. Move BSV from `$50` to `$100` and the option is in the money: absconding becomes the *rational* trade. The attacker does not even need to time it well, because a large holder can help the price along and manufacture the strike.

A price governor cannot fix a written option. It is reactive by construction, it needs an oracle — a new trust assumption and a new manipulation surface — and there is always a window between the move and the throttle. That window *is* the trade.

Denominating the bond in `solBSV` removes the position instead of hedging it, and it costs a relayer nothing in optionality, because holding BSV is the business. This is also the answer to "just hedge the collateral with perps": you do not hedge the position, you delete it.

### A slashed theft is deflationary

Because the bond is `solBSV`, a slash removes supply while the reserve falls by the stolen amount. With `B ≥ H`, backing per remaining token **rises**. Honest holders are not merely protected — they end up marginally better collateralised. A stablecoin bond has the opposite property: it has to be sold at a price to make holders whole, so the system absorbs the theft *and* the market move.

## The roadmap to a signerless reserve

The hot key exists only because the cold reserve cannot verify a Solana burn. On BSV that is *expressible*: the reserve can be locked by a covenant that releases funds only against a **zero-knowledge proof of the burn**, verified inside BSV Script. BSV is unusually suited to this — `OP_CAT` and `OP_MUL` are active, script size is effectively unlimited, and in-script Groth16/STARK verification has been demonstrated (BSVM, MIT, pre-mainnet and unaudited).

If it works, there is no hot key, no bond and no price mismatch — the reserve releases itself against a valid proof. That is the research track, and it is the reason SOLBEAM's design keeps the redemption authority behind a replaceable boundary: the token, the mint path and the light client do not change when it lands.

## What we ask you to trust — plainly

1. That the bridge program and light client are correctly implemented and audited (they are FOSS; verification is the community's).
2. That the bond behind the hot float is real, denominated in `solBSV`, sized against `hot + tranche`, and enforced by the program.
3. That the hot key itself has not been compromised, and that the residual naked-spend case is covered — by a float small enough to be irrelevant, a veto-only cosigner, or both.
4. That governance parameters (caps, fees, deadlines, tranche schedules, the unbonding period) are managed honestly and in the open.

Everything else — deposits, backing, minting, and the payout proof — is enforced by code.

---

Next: [Relayers](05-relayers.md)
