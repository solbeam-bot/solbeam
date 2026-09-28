# 4. Trust model & security

SOLBEAM is deliberately asymmetric: **trustless in, trust-minimised out.** This chapter states exactly what you are trusting, and why.

> **Built or designed?** The light client, the token and the mint exist and pass 17 on-chain
> tests. **The vault, the order book, per-relayer deposits and all of peg-out are designed and
> not built.** The shipped program mints straight to the depositor's token account, so nothing
> is staged and no relayer holds a bonded float yet. Every property below is therefore one of
> three things, and they are labelled:
>
> - **built** — the light client, the token and the mint, as they stand;
> - **designed** — the vault, maturity, per-relayer deposits, `owed_R` and `bond_R`, and all of
>   peg-out. Specified, not coded;
> - **trusted** — off-chain, and not enforceable by the program at all.

## Summary

| Property | Status |
|---|---|
| Minting authorised by proof, not by a person | **Trustless** — the program verifies the BSV headers and the Merkle branch itself |
| Backing (1 `solBSV` = 1 BSV) | **Trustless accounting** for what the program has verified. The BSV itself is held by relayers, off-chain, and is counted rather than read |
| Reserve custody at large | **No pooled reserve.** Deposits pay each relayer's own BSV script, so there is no single key worth stealing. *Designed, not built* |
| Redemption payout | **Trust-minimised** — a bonded relayer holds its own float and owes what the program has credited against it |
| The bond | `bond_R ≥ k × owed_R`, **with `k = 1`**, in seizable `solBSV`. Both sides are protocol quantities, so the inequality is checkable on-chain. *Designed, not built* |
| Reversibility | **The vault** — a program-owned account, so a staged mint can be burned or returned with no freeze authority. *Designed, not built* |
| Censorship of mints | **None** — anyone can mint, for anyone |
| Censorship of redemptions | **Bounded** — a relayer may decline, but the deadline returns the escrow and supply never changes |
| Bond denomination | `solBSV` — the same unit as the exposure, so no price move can shrink it |
| Market / price | External (Raydium/Orca) |
| Exit speed vs entry | **Deliberately slower** — minting completes after the depth the bid named, with no trusted party involved; redemption waits on a bonded relayer and a deadline measured in slots. The fast direction is the one that needs no trust |

## What is trustless, and why

**Solana can verify BSV.** BSV uses double-SHA-256 proof-of-work over an 80-byte header, and Solana exposes a native SHA-256 syscall. So a BSV light client on Solana can check, from first principles:

- the header chain links correctly,
- each header meets its difficulty target (including BSV's difficulty adjustment),
- a given transaction is included in a given block via its Merkle branch.

Minting is authorised by that proof alone. There is no attestor to bribe, no oracle to spoof, no committee to capture, and no way to censor a mint.

**What is trustless is the authorisation, not the address it credits.** The proof says *this deposit exists on BSV*; it does not say *a relayer agreed to underwrite it*. That gap is what the bond and the relayer's consent close, and it is why the rest of this chapter exists.

## Why redemption cannot be fully trustless (today)

Releasing native BSV requires a valid BSV signature. Solana programs cannot sign BSV transactions, and **BSV Script cannot verify Solana's ed25519 consensus** — BSV has only secp256k1 `OP_CHECKSIG`/`OP_CHECKMULTISIG`, and stake-weighted validator aggregation is not script-expressible.

So at the instant of redemption, *some key must exist*. This is a property of the two chains, not a shortcut in the design. SOLBEAM's response is to make that key:

- **small** — it holds only one relayer's float, never a shared reserve;
- **isolated** — deposits pay individual relayers, so a theft reaches one relayer's float and not the system's;
- **bonded in the same unit as the exposure** — the bond is `solBSV`, so no BSV price move can shrink it relative to what it protects;
- **bonded against a measured liability** — `owed_R` is accumulated from proofs the program verified itself, so the number the bond must cover is a protocol quantity rather than an attestation;
- **fraud-punishable** — a payout that is not made, or is reorged away, is provable on Solana and slashes the bond;
- **consented to** — a mint is credited to a relayer only with that relayer's signature accepting the liability, so no relayer is slashed for an attack it never agreed to underwrite.

Bonding can make theft *unprofitable*. It cannot make it *impossible*, and it does nothing at all against an attacker who never posted a bond. The rest of this chapter is about how much that actually costs.

## Who checks what

| Step | Verified by |
|---|---|
| Burn / redemption request | The Solana program — native state, nothing to prove. *Designed, not built* |
| Deposit (BSV → mint) | The Solana program, against the BSV light client |
| Payout (BSV → redeem) | The Solana program, against the BSV light client. *Designed, not built* |
| Failed redemption | Automatic — the program returns the escrow after the deadline, and slashes the bond. Supply is unchanged. *Designed, not built* |
| Unauthorised spend of a relayer's float | **Nobody can, on-chain.** `owed_R` measures what a relayer *owes*, not what it *holds*; the BSV is off-chain and the program cannot read it. The failure that is visible is a missed redemption, and that is what the bond answers |

Verification is always done by deterministic on-chain code, and it is only as complete as the paths that exist. The deposit path is built; the redemption path is not. On the built path, no committee is needed and no watcher is needed at all — the program checks every deposit proof itself.

That last point carries more weight than it first appears, and it is a **correction to the earlier version of this chapter**. This document once argued that a float with no redemption attached — a *naked spend* — had to be caught by a permissionless challenger, and that the bond was therefore sized against human vigilance. **Per-relayer deposits change the object, and with it the enforcement.** A relayer spending its own float is spending its own money; what the program cares about is that the redemptions it accepted are paid. That is a deadline, and a missed deadline reports itself. The vigilance problem is much smaller than it was — but it does not disappear, and the naked-spend reasoning still sets the shape of the residual, as §The naked-option attack sets out.

## Core invariants

1. **Backing.** Custodied BSV ≥ outstanding `solBSV` at all times. **Designed, not built**, and *monitored rather than enforced* (decision D8): the reserve is off-chain BSV the program cannot read, so the website publishes the ratio and the program does not check it. The mint path enforces its half of it today.
2. **Exposure.** `bond_R ≥ k × owed_R` — **with `k = 1`** (decision D5) — **with both sides protocol quantities in `solBSV`**: `owed_R` is what the program has credited relayer `R` from proofs it verified itself, and `bond_R` is `solBSV` the program holds and can seize. Because the bond and the exposure are the same asset, the inequality holds at every BSV price — no oracle, no governor, no reaction window. Because *both* quantities are known to the program, the inequality is **checkable on-chain**: a mint naming `R` is refused unless the check passes. *Designed, not built.*
3. **No pooled reserve.** There is no shared bridge address and no single key whose theft drains everything. Deposits pay the relayer's own script, so exposure is per-relayer and bounded by that relayer's bond. This removal is *designed, not built*: today the mint path is the only path, and there is no relayer registry.
4. **Reversibility without a freeze authority.** Every mint lands in a program-owned vault, released after a maturity window, or **burned** if a reorg is followed. Because the tokens are in the program's own account, burning and returning them is disposing of what it holds rather than confiscation. *Designed, not built.*
5. **Solvency after a failed redemption.** With `bond_R ≥ k × owed_R`, the slashed bond covers the liability: at `k = 1` the redeemer is made whole and the shortfall is bounded at zero, and the program never mints a failure path. **Solvency must therefore not depend on anyone submitting a proof** — which is what invariant 2 is for.
6. **Holder protection.** Every redemption either completes or the escrow is automatically returned after the deadline. Supply is unchanged either way. *Designed, not built.*
7. **Bonds lock.** A bond withdrawable on demand is not a bond. Release requires settling outstanding commitments and waiting out the unbonding period, which outlasts both the redemption deadline and the challenge window.

## Threats and answers

| Threat | Answer |
|---|---|
| Relayer takes the deposit and never pays | The redemption's deadline passes, the escrow is returned to the holder, and the bond is slashed. Self-reporting, no watcher required |
| Relayer spends its own float (naked spend, no redemption outstanding) | This is the relayer's own money, and no holder is out of pocket — but see §The naked-option attack for why the residual still sets a constraint. Structurally reduced by **per-relayer isolation** and by **holding no idle float** |
| A relayer's float is stolen by an outsider | The thief never posted a bond, so the slash compensates nothing directly — but the loss is confined to that one relayer's float, not a pooled reserve. Mitigated by per-relayer isolation and key hygiene |
| Nobody fulfils redemptions | Escrows are returned to holders after the deadline; open redemption is a public race, and the discovered fee attracts relayers |
| Fake deposit proof | Rejected by the light client (PoW / DAA / Merkle) |
| Mint staged, then a reorg is followed | The vault **burns** the staged tokens. The depositor's BSV is reorged away with the deposit, and they end where they started. Nobody else is affected. *Designed, not built* |
| Reorg after the vault has released | Depth, maturity and `FLOOR` are what make out-mining the honest chain cost more than the fraud is worth. Depth is a term of the bid; `FLOOR` is the backstop |
| Self-dealing at `k = 1` | An accepted risk (D5). The attacker is underwriting their own deposit, so it is roughly break-even — and what makes it unprofitable is **the mining cost of the reorg**, not the bond. It is unprofitable only to the extent detection works |
| Relayer exits to dodge a slash | The bond cannot be withdrawn instantly: unbonding requires a notice period that outlasts both the redemption deadline and the challenge window |
| The relayer's script key is compromised | The loss is that relayer's float, and its `owed_R` is still bonded and slashable. There is no system-wide key to compromise |
| BSV price rises sharply | **No longer a solvency risk.** Bond and exposure are both `solBSV`, so they move together — no top-up demand, no governor, and no window for an attacker to strike in. A sharp move only changes relayer fee revenue in dollar terms |
| Weak BSV hash rate | SPV security inherits the most-work assumption; BSV's hash rate is low relative to Bitcoin's. Mitigated by deep confirmation depths (the bid's depth and `FLOOR`), and conservative caps |
| Bridge program upgrade | Out of scope for the PoC and recorded rather than hidden: the upgrade authority can override every parameter, which is an unconditional mint voucher. The fix is governance |

## The naked-option attack

The most important attack on this design needs no redemption at all, and it is worth stating in its simplest form because it is what sets the shape of the bond.

Anyone holding a relayer's key can:

1. Spend that relayer's float to themselves.
2. Accept the slash, if it comes.

No burn, no redemption, no deadline, no victim. That position is a **naked option**: pay the bond `B`, receive the float `H`. It pays exactly when `H > B` — and it is *strictly easier* than attaching the theft to a redemption, because a redemption is what creates the deadline that slashes with certainty.

**The burn is not the attack; the burn is the liability.** The enforcement path that fires automatically is precisely the one an intelligent attacker avoids.

**The reasoning is unchanged. The object it bounds is not.** It used to be one pooled hot wallet operated by a relayer; it is now **each relayer's own float**, and the liability the bond answers is `owed_R` rather than the float itself. Two things follow, and they cut in opposite directions:

- **Better than before.** The object is *distributed*. There is no single key whose compromise is everyone's loss, and no pooled reserve contract to write, audit or trust. A relayer can only lose its own float, and its liability is a number the program derives from proofs it checked.
- **Still bounded by detection.** The program cannot see the off-chain BSV, so it cannot size the bond against a float it cannot read. What it *can* do is size the bond against `owed_R` — what it has credited — and require consent, so a relayer only owes what it agreed to underwrite. The residual attack is a relayer spending its own float and accepting that it must still pay what it owes, or not.

### The cost, and why `k` is a detection parameter

The attacker's cost is not `B`. It is:

```
expected cost  =  B × P(slashed)
```

because a naked spend is punished only if the theft strands a commitment someone can prove. Deterrence therefore requires:

```
B  >  H / P
```

This is the honest meaning of the safety factor `k`: **`k` is roughly `1/P` — a guess at detection probability wearing a number.** Sizing a bond on an assumption about human vigilance is the weakest link in the model.

**What per-relayer deposits do to `k` is replace the guess with a measurement, for the part of the exposure the program can see.** `owed_R` is derived from verified proofs, and a missed redemption reports itself at its deadline, so for the bonded liability `P` is effectively 1 and `k = 1` is defensible (decision D5). `k` has no price job left — the bond is `solBSV` either way — so its only remaining job is the part the program *cannot* see, which is the relayer's own idle float. That is the residual, and the fix is structural rather than a larger number.

### Closing it, strongest first

1. **Do not pool the reserve.** Per-relayer deposits are the primary fix: one relayer's exposure is its own `owed_R`, individually bonded and seizable, so no single theft is every holder's loss. This is what removes the object the original analysis was bounding.
2. **Hold no idle float.** If a relayer carries little beyond what it owes, a naked spend has little to take. This is operating discipline rather than a chain guarantee — the program cannot read the BSV — and it should be described as such.
3. **Bond against `owed_R`, and require consent.** The gate is checkable on-chain: a mint naming relayer `R` is refused unless `bond_R ≥ k × (owed_R + this mint)`, and it needs `R`'s signature accepting the liability. Consent is what makes slashing `R` for a shortfall defensible rather than arbitrary, and it converts reorg risk from an externality into a term `R` prices.
4. **An unbonding period.** Release of the bond must outlast the redemption deadline plus the challenge window, or a relayer can take a job, withdraw, and be gone before anyone can respond.
5. **Size `k` against the residual.** With `solBSV` denomination, `k` has no price job left. Once steps 1–4 land, `k` can stay at `1` for the bonded liability and any operational margin above it is a choice rather than a requirement.

### Why the bond is `solBSV` and not a stablecoin

A stablecoin bond against a BSV liability is not a bond. It is a **written call option on the reserve, struck at `bond ÷ float`**. Post `$500k` against a `10,000 BSV` reserve and the relayer is short `5,000 BSV`. Move BSV from `$50` to `$100` and the option is in the money: absconding becomes the *rational* trade. The attacker does not even need to time it well, because a large holder can help the price along and manufacture the strike.

A price governor cannot fix a written option. It is reactive by construction, it needs an oracle — a new trust assumption and a new manipulation surface — and there is always a window between the move and the throttle. That window *is* the trade.

Denominating the bond in `solBSV` removes the position instead of hedging it, and it costs a relayer nothing in optionality, because holding BSV is the business. This is also the answer to "just hedge the collateral with perps": you do not hedge the position, you delete it.

The per-relayer version sharpens this rather than weakening it. The bond is now compared against `owed_R`, a quantity the program measures in `solBSV`, so the inequality is not merely price-invariant in principle — it is one the program can evaluate for itself, on-chain, with nothing external consulted.

### A slashed theft is deflationary

Because the bond is `solBSV`, a slash removes supply while the reserve falls by the stolen amount. With `B ≥ H`, backing per remaining token **rises**. Honest holders are not merely protected — they end up marginally better collateralised. A stablecoin bond has the opposite property: it has to be sold at a price to make holders whole, so the system absorbs the theft *and* the market move.

**The same property bounds the damage in the two later variants, and the distinction is worth stating rather than blurring.** When the theft is caught while the mint is still staged, the vault **burns** it: supply falls, nothing was sold, and the backing behind every remaining token is strictly better. When the theft is instead a failed redemption covered by the bond, the settlement is a **transfer** — the slashed `solBSV` moves to the redeemer rather than being burned or re-minted. Supply is then unchanged rather than reduced, which is the supply-conserving outcome: the failure is not a dilution event, because no new tokens were created to cover it. Holding the bond in `solBSV` is what makes both variants work without a sale. Had the bond been a stablecoin, the program would have had to sell it at a market price, and the system would have absorbed the theft *and* the market move together.

## The roadmap to a signerless reserve

The hot key exists only because a BSV key cannot verify a Solana burn. On BSV that is *expressible*: the reserve can be locked by a covenant that releases funds only against a **zero-knowledge proof of the burn**, verified inside BSV Script. BSV is unusually suited to this — `OP_CAT` and `OP_MUL` are active, script size is effectively unlimited, and in-script Groth16/STARK verification has been demonstrated (BSVM, MIT, pre-mainnet and unaudited).

If it works, there is no hot key, no bond and no price mismatch — the reserve releases itself against a valid proof. That is the research track, and it is the reason SOLBEAM's design keeps the redemption authority behind a replaceable boundary: the token, the mint path and the light client do not change when it lands.

Per-relayer deposits are the path that does not depend on it landing. The covenant is the destination; removing the pooled reserve is what makes the journey safe without it.

## What we ask you to trust — plainly

1. **The checkpoint.** The light client starts from a block hash taken on faith. It is published, buried deep, and the only thing not proven.
2. **The BSV reserve keys.** BSV sits under keys that can spend it. There is no covenant, and BSV has no timelocks to fall back on.
3. **The program upgrade authority.** It can override every parameter, which makes it an unconditional mint voucher. Out of scope for the proof of concept, and recorded rather than hidden.
4. **That honest headers get pushed within the maturity window.** A liveness condition anyone can satisfy, with a built-in incentive: an undetected fraud eats the buffer, so the parties with the most to lose have the most reason to advance the honest chain. It is the load-bearing assumption of the whole system, and the second audit says so in as many words.
| **That the code is correct** | **Not independently audited.** Our own adversarial review found four critical defects in it; all are fixed and tested, which is not the same as correct |

Everything else — deposits, backing, minting, maturity, the bond gate and the payout proof — is enforced by code. **Except where it is not yet written:** the vault, the book, per-relayer deposits and all of peg-out are designed and not built, and this list will not be shorter than reality until they are.

---

Next: [Relayers](05-relayers.md)
