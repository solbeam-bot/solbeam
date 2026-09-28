# 4. Trust model & security

SOLBEAM is deliberately asymmetric: **trustless in, trust-minimised out.** This chapter states exactly what you are trusting, and why.

> **Built or designed?** The light client, the token, the mint and fork staging exist and pass 17
> on-chain tests. **The vault, the two gates, maturity, the order book, staking, bonds, `owed_R`,
> consent, per-relayer deposit scripts, `FLOOR` as a distinct parameter and all of peg-out are
> designed and not built.** DAA is worse than absent — it is **actively rejected in the built
> client** (F7), not merely unimplemented. The shipped program mints straight to the depositor's
> token account, so nothing is staged and no relayer holds a bond yet. Every property below is
> therefore one of three things, and they are labelled:
>
> - **built** — the light client, the token, the mint and fork staging, as they stand;
> - **designed** — the vault, the two gates, maturity, the order book, staking, bonds, `owed_R`,
>   consent, per-relayer deposit scripts, `FLOOR` as a distinct parameter (the shipped mint uses a
>   fixed `MIN_CONFIRMATIONS = 12`), and all of peg-out. Specified, not coded;
> - **trusted** — off-chain, and not enforceable by the program at all.

## Summary

| Property | Status |
|---|---|
| Minting authorised by proof, not by a person | **Trustless** — the program verifies the BSV headers and the Merkle branch itself |
| Backing (1 `solBSV` = 1 BSV) | **Trustless accounting** for what the program has verified. The BSV itself is held by relayers, off-chain, and is counted rather than read |
| Reserve custody at large | **No pooled reserve.** Deposits pay each relayer's own BSV script, so there is no single key worth stealing. *Designed, not built* |
| Redemption payout | **Trust-minimised** — a bonded relayer holds its own float and owes what the program has credited against it |
| The bond | `bond_R ≥ k × owed_R`, **with `k = 1`**, in seizable `solBSV`. It answers a relayer's deliberate theft or abandonment of what it owes (`owed_R`, including staged mints still in the vault) — **not a reorg, not a failed redemption, and not the relayer's own float**. Both sides are protocol quantities, so the inequality is checkable on-chain. *Designed, not built* |
| Reversibility | **The vault** — a program-owned account, so a staged mint can be burned or returned with no freeze authority. *Designed, not built* |
| Censorship of mints | **None** — anyone can mint, for anyone |
| Censorship of redemptions | **Bounded** — a relayer may decline, but the deadline returns the escrow and supply never changes |
| Bond denomination | `solBSV` — the same unit as the exposure, so no price move can shrink it |
| Market / price | External (Raydium/Orca) |
| Exit speed vs entry | **Deliberately slower** — minting completes after the depth the bid named, with no trusted party involved; redemption waits on a bonded relayer and a deadline measured in slots. The fast direction is the one that needs no trust |

## What is trustless, and why

**Solana can verify BSV.** BSV uses double-SHA-256 proof-of-work over an 80-byte header, and Solana exposes a native SHA-256 syscall. So a BSV light client on Solana can check, from first principles:

- the header chain links correctly,
- each header meets its difficulty target — **with a caveat: DAA is actively rejected in the built client, not merely absent.** `check_daa` is a stub with no caller and `push_header` requires `bits` to equal the value fixed at initialization, so the client **halts permanently at the first retarget** (F7). Following BSV's difficulty adjustment is designed, not built;
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
- **fraud-punishable** — a relayer that absconds with what it owes is provable on Solana and its bond is seizable; a failed redemption is **not** a bond transfer, because returning the escrow already makes the holder whole;
- **consented to** — a mint is credited to a relayer only with that relayer's signature accepting the liability, so no relayer is slashed for an attack it never agreed to underwrite.

Bonding can make theft *unprofitable*. It cannot make it *impossible*, and it does nothing at all against an attacker who never posted a bond. The rest of this chapter is about how much that actually costs.

## Who checks what

| Step | Verified by |
|---|---|
| Burn / redemption request | The Solana program — native state, nothing to prove. *Designed, not built* |
| Deposit (BSV → mint) | The Solana program, against the BSV light client |
| Payout (BSV → redeem) | The Solana program, against the BSV light client. *Designed, not built* |
| Failed redemption | Automatic — the program returns the escrow to the holder after the deadline. Supply is unchanged and **the bond is not additionally transferred**, because the returned escrow already makes the holder whole. *Designed, not built* |
| Unauthorised spend of a relayer's float | **Nobody can, on-chain.** `owed_R` measures what a relayer *owes*, not what it *holds*; the BSV is off-chain and the program cannot read it. A missed redemption is answered by returning the escrow, **not by the bond**; the bond answers `owed_R`, the liability the relayer has been credited and not discharged, and the float itself is not covered by it (F4) |

Verification is always done by deterministic on-chain code, and it is only as complete as the paths that exist. The deposit path is built; the redemption path is not. On the built path, no committee is needed and no watcher is needed at all — the program checks every deposit proof itself.

That last point carries more weight than it first appears, and it is a **correction to the earlier version of this chapter**. This document once argued that a float with no redemption attached — a *naked spend* — had to be caught by a permissionless challenger, and that the bond was therefore sized against human vigilance. **Per-relayer deposits change the object, and with it the enforcement.** A relayer spending its own float is spending its own money; what the program cares about is that the redemptions it accepted are paid. That is a deadline, and a missed deadline reports itself. The vigilance problem is much smaller than it was — but it does not disappear, and the naked-spend reasoning still sets the shape of the residual, as §The naked-option attack sets out.

## Core invariants

1. **Backing.** Custodied BSV ≥ outstanding `solBSV` at all times. **Designed, not built**, and *monitored rather than enforced* (decision D8): the reserve is off-chain BSV the program cannot read, so the website publishes the ratio and the program does not check it. The mint path enforces its half of it today.
2. **Exposure.** `bond_R ≥ k × owed_R` — **with `k = 1`** (decision D5) — **with both sides protocol quantities in `solBSV`**: `owed_R` is what the program has credited relayer `R` from proofs it verified itself, and `bond_R` is `solBSV` the program holds and can seize. Because the bond and the exposure are the same asset, the inequality holds at every BSV price — no oracle, no governor, no reaction window. Because *both* quantities are known to the program, the inequality is **checkable on-chain**: a mint naming `R` is refused unless the check passes. *Designed, not built.*
3. **No pooled reserve.** There is no shared bridge address and no single key whose theft drains everything. Deposits pay the relayer's own script, so exposure is per-relayer and bounded by that relayer's bond. This removal is *designed, not built*: today the mint path is the only path, and there is no relayer registry.
4. **Reversibility without a freeze authority.** Every mint lands in a program-owned vault, released after a maturity window, or **burned** if a reorg is followed. Because the tokens are in the program's own account, burning and returning them is disposing of what it holds rather than confiscation. *Designed, not built.*
5. **Solvency after a failed redemption.** The escrow is **returned to the holder** and supply is unchanged, so the redeemer is made whole **without touching the bond** — the bond is not additionally transferred, because the returned escrow already does the job. The bond's own job is `owed_R`: the liability the program has credited and the relayer has not discharged, **including staged mints still in the vault**. **Solvency must therefore not depend on anyone submitting a proof** — which is what invariant 2 is for.
6. **Holder protection.** Every redemption either completes or the escrow is automatically returned after the deadline. Supply is unchanged either way. *Designed, not built.*
7. **Bonds lock.** A bond withdrawable on demand is not a bond. Release requires settling outstanding commitments and waiting out the unbonding period, which outlasts both the redemption deadline and the challenge window.

## Threats and answers

| Threat | Answer |
|---|---|
| Relayer takes the deposit and never pays | The redemption's deadline passes and the escrow is **returned to the holder**, which makes them whole; supply is unchanged and the bond is **not additionally transferred**, because the returned escrow already does the job. The bond answers the relayer's `owed_R` — theft or abandonment of what it owes. Self-reporting, no watcher required |
| Relayer spends its own float (naked spend, no redemption outstanding) | This is the relayer's own money, and no holder is out of pocket — but see §The naked-option attack for why the residual still sets a constraint. **The bond does not cover it**; structurally reduced by **per-relayer isolation** and by **holding no idle float** |
| A relayer's float is stolen by an outsider | The thief never posted a bond, so the slash compensates nothing directly — but the loss is confined to that one relayer's float, not a pooled reserve. Mitigated by per-relayer isolation and key hygiene |
| Nobody fulfils redemptions | Escrows are returned to holders after the deadline; open redemption is a public race, and the discovered fee attracts relayers |
| Fake deposit proof | Rejected by the light client (proof of work and Merkle inclusion). The difficulty-retarget check is a stub, so a retargeted chain **halts the client** rather than being followed — DAA is actively rejected, not merely absent (F7) |
| Mint staged, then a reorg is followed | The vault **burns** the staged tokens. The depositor's BSV is reorged away with the deposit, and they end where they started. Nobody else is affected. *Designed, not built* |
| Reorg after the vault has released | Depth, maturity and `FLOOR` are what make out-mining the honest chain cost more than the fraud is worth. Depth is a term of the bid; `FLOOR` is the backstop |
| Self-dealing at `k = 1` | An accepted risk (D5). The attacker is underwriting their own deposit, so it is roughly break-even — and what makes it unprofitable is **the mining cost of the reorg**, not the bond. It is unprofitable only to the extent detection works |
| Relayer exits to dodge a slash | The bond cannot be withdrawn instantly: unbonding requires a notice period that outlasts both the redemption deadline and the challenge window |
| The relayer's script key is compromised | The loss is that relayer's float, and its `owed_R` is still bonded and slashable. There is no system-wide key to compromise |
| BSV price rises sharply | **No longer a solvency risk.** Bond and exposure are both `solBSV`, so they move together — no top-up demand, no governor, and no window for an attacker to strike in. A sharp move only changes relayer fee revenue in dollar terms |
| Weak BSV hash rate | SPV security inherits the most-work assumption; BSV's hash rate is low relative to Bitcoin's. Mitigated by deep confirmation depths (the bid's depth and `FLOOR`), and conservative caps |
| Bridge program upgrade | Out of scope for the PoC and recorded rather than hidden: the upgrade authority can override every parameter, which is an unconditional mint voucher. The fix is governance |

## The naked-option attack

The residual this design cannot close with collateral needs no redemption at all, and it is worth stating in its simplest form because it is what the float cap is for.

Anyone holding a relayer's key can spend that relayer's BSV float. No burn, no redemption, no deadline, no victim. But this is **not a claim on the bond**: the float is the relayer's own money and the bond does not cover it (F4). What the bond covers is `owed_R`, and a relayer that has spent BSV it owes has failed that liability. A relayer that spends its own idle float, owing nothing, has taken nothing the program can reach — which is why the float should be small, and why the bond is sized against `owed_R` rather than against a quantity the program cannot read.

**The burn is not the attack; the burn is the liability.** The enforcement path that fires automatically is precisely the one an intelligent attacker avoids.

**The reasoning is unchanged. The object it bounds is not.** It used to be one pooled hot wallet operated by a relayer; it is now **each relayer's own float**, and the liability the bond answers is `owed_R` rather than the float itself. Two things follow:

- **Better than before.** The object is *distributed*. There is no single key whose compromise is everyone's loss, and no pooled reserve contract to write, audit or trust. A relayer can only lose its own float, and its liability is a number the program derives from proofs it checked.
- **Still not readable on-chain.** The program cannot see the off-chain BSV, so it cannot size the bond against a float it cannot read. What it *can* do is size the bond against `owed_R` — what it has credited — and require consent, so a relayer only owes what it agreed to underwrite. The residual is a relayer spending its own float; that is operating discipline and a float cap, not a bond claim.

### What `k` is for

**`k` is a sizing multiple on `owed_R`, nothing more.** The constraint is `bond_R ≥ k × owed_R`, and because `owed_R` is derived from proofs the program verified itself, both sides are quantities it can compare on-chain. **`k = 1`** (D5) means the bond covers the credited liability in full. There is no detection probability in the formula and no discount for one.

It is not a price hedge. The bond is denominated in `solBSV`, the same asset as the exposure, so the price position is *removed* rather than hedged — there is no price job for `k` to do. Raising `k` would buy margin against the liability `owed_R` already measures, not against a guess about who is watching.

The part the program *cannot* measure — the relayer's own idle float — is not something any value of `k` reaches, because the float is not owed. That residual is bounded structurally: per-relayer isolation, a float cap, and holding no idle float.

### Closing it, strongest first

1. **Do not pool the reserve.** Per-relayer deposits are the primary fix: one relayer's exposure is its own `owed_R`, individually bonded and seizable, so no single theft is every holder's loss. This is what removes the object the original analysis was bounding.
2. **Hold no idle float.** If a relayer carries little beyond what it owes, a naked spend has little to take. This is operating discipline rather than a chain guarantee — the program cannot read the BSV — and it should be described as such.
3. **Bond against `owed_R`, and require consent.** The gate is checkable on-chain: a mint naming relayer `R` is refused unless `bond_R ≥ k × (owed_R + this mint)`, and it needs `R`'s signature accepting the liability. Consent is what makes slashing `R` for a shortfall defensible rather than arbitrary, and it converts reorg risk from an externality into a term `R` prices.
4. **An unbonding period.** Release of the bond must outlast the redemption deadline plus the challenge window, or a relayer can take a job, withdraw, and be gone before anyone can respond.
5. **Keep `k` sized against `owed_R`.** `k = 1` already covers the bonded liability, and the float is not owed, so it is not a `k` question at all. Any operational margin a relayer keeps above the bond is its own business decision, not a protocol requirement.

### Why the bond is `solBSV` and not a stablecoin

A stablecoin bond against a BSV liability is not a bond. It is a **written call option on the reserve, struck at the ratio of the stablecoin bond to the BSV exposure it must cover**. Post `$500k` against a `10,000 BSV` reserve and the relayer is short `5,000 BSV`. Move BSV from `$50` to `$100` and the option is in the money: absconding becomes the *rational* trade. The attacker does not even need to time it well, because a large holder can help the price along and manufacture the strike.

A price governor cannot fix a written option. It is reactive by construction, it needs an oracle — a new trust assumption and a new manipulation surface — and there is always a window between the move and the throttle. That window *is* the trade.

Denominating the bond in `solBSV` removes the position instead of hedging it, and it costs a relayer nothing in optionality, because holding BSV is the business. This is also the answer to "just hedge the collateral with perps": you do not hedge the position, you delete it.

The per-relayer version sharpens this rather than weakening it. The bond is now compared against `owed_R`, a quantity the program measures in `solBSV`, so the inequality is not merely price-invariant in principle — it is one the program can evaluate for itself, on-chain, with nothing external consulted.

### A slashed theft is deflationary

Because the bond is `solBSV`, a slash removes supply while the reserve falls by the stolen amount. Where the bond covers the liability (`bond_R ≥ owed_R`), backing per remaining token **rises**. Honest holders are not merely protected — they end up marginally better collateralised. A stablecoin bond has the opposite property: it has to be sold at a price to make holders whole, so the system absorbs the theft *and* the market move.

**The same property bounds the damage in the two cases the bond actually answers, and the distinction is worth stating rather than blurring.** When the theft is caught while the mint is still staged, the vault **burns** it: supply falls, nothing was sold, and the backing behind every remaining token is strictly better. When the theft is instead a relayer absconding with BSV it owes, the seizable `solBSV` bond answers `owed_R` and can be burned, so supply falls against a reserve that also fell. A **failed redemption is not one of these cases**: the escrow is returned to the holder and supply is unchanged, and **the bond is not additionally transferred**, because the returned escrow already makes the holder whole. Holding the bond in `solBSV` is what makes both seizure paths work without a sale. Had the bond been a stablecoin, the program would have had to sell it at a market price, and the system would have absorbed the theft *and* the market move together.

## The roadmap to a signerless reserve

The hot key exists only because a BSV key cannot verify a Solana burn. On BSV that is *expressible*: the reserve can be locked by a covenant that releases funds only against a **zero-knowledge proof of the burn**, verified inside BSV Script. BSV is unusually suited to this — `OP_CAT` and `OP_MUL` are active, script size is effectively unlimited, and in-script Groth16/STARK verification has been demonstrated (BSVM, MIT, pre-mainnet and unaudited).

If it works, there is no hot key, no bond and no price mismatch — the reserve releases itself against a valid proof. That is the research track, and it is the reason SOLBEAM's design keeps the redemption authority behind a replaceable boundary: the token, the mint path and the light client do not change when it lands.

Per-relayer deposits are the path that does not depend on it landing. The covenant is the destination; removing the pooled reserve is what makes the journey safe without it.

## What we ask you to trust — plainly

1. **The checkpoint.** The light client starts from a block hash taken on faith. It is published, buried deep, and the only thing not proven.
2. **The BSV reserve keys.** BSV sits under keys that can spend it. There is no covenant, and BSV has no timelocks to fall back on.
3. **The program upgrade authority.** It can override every parameter, which makes it an unconditional mint voucher. Out of scope for the proof of concept, and recorded rather than hidden.
4. **That honest headers get pushed within the maturity window.** This is the one path where detection is the whole defence. A **released fraudulent mint** has no deadline and no bond to reach — the fraud's depositor is the attacker — so if nobody advances the honest chain and notices the orphan within maturity, holders are diluted. Pushing headers is permissionless, cheap and incentivised: an undetected fraud eats the buffer, so the parties with the most to lose have the most reason to do it. A relayer's **deliberate theft or abandonment** is the other case and does not rest on detection — that loss is self-reporting at the redemption deadline, where the escrow is returned and the bond answers `owed_R` (see [the second audit](15-audit-2.md)).
5. **That the code is correct.** Not independently audited. Our own adversarial review found Three critical defects fixed (a vacuous proof-of-work check, an unauthenticated checkpoint path, an unconstrained mint) and one serious one (a replay key that double-minted after a reorg); three criticals remain open (A4, A5, A6). Three of the five fixes have tests.

Everything else — deposits, backing, minting, maturity, the bond gate and the payout proof — is enforced by code. **Except where it is not yet written:** the vault, the book, per-relayer deposits and all of peg-out are designed and not built, and this list will not be shorter than reality until they are.

---

Next: [Relayers](05-relayers.md)
