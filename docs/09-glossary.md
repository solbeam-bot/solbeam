# 9. Glossary

Plain-English definitions of the terms used in this documentation.

> **Specification, not shipped behaviour.** The light client, the token, the mint and fork
> staging are **built and tested**. **The vault, the two gates, maturity, the order book, staking,
> bonds, `owed_R`, consent, per-relayer deposit scripts, `FLOOR` as a distinct parameter and all
> of peg-out are designed and not built.** DAA is **actively rejected in the built client** (F7),
> not merely absent. The entries for those are the vocabulary of the design, not a description of
> running code.

**Atomic (in "atomic wrapper")** — here it means the wrapper is 1:1 and minting is authorised purely by a proof: the deposit either verifies on Solana or it doesn't, with no discretionary approval. It does **not** mean redemption is instantaneous.

**The book** — the **order book of underwriting**. Stakers post sell orders — so much liquidity, at such a rate, at such a confirmation depth — and incoming deposits are matched against them by price then time, partially filled, with unused stake left with the staker. It replaces a chosen fee with a discovered one and makes **depth a term of the bid** rather than a protocol constant. Designed, not built.

**Bond (`bond_R`)** — collateral a **single relayer `R`** lodges on Solana: **`solBSV` held by the program, and therefore seizable on-chain**. The constraint is **`bond_R ≥ k × owed_R`** with `k = 1` — `k` is a **sizing multiple on `owed_R`**, not a price hedge and not a discount for a hoped-for detection probability. `owed_R` is what the relayer has been credited and not discharged, **including staged mints still in the vault**, and it comes from proofs the program verified itself, so the inequality is checkable without an attestation. It is **locked** — bonded `solBSV` cannot be redeemed, and release requires an unbonding period. It is a **performance bond** against a relayer's **deliberate theft or abandonment** of what it owes: it is *not* reorg insurance, it does *not* answer a failed redemption (where the escrow is returned instead), and it does *not* cover the relayer's own float. It secures that individual relayer's `owed_R`, not a pooled hot wallet.

**Burn** — destroying `solBSV` on Solana in order to redeem the underlying BSV. The burn and the requested BSV destination are recorded by the bridge program. A reorg that is followed also causes the **vault** to burn tokens that were minted but never released.

**Challenger / watchtower** — anyone who watches for a payout that is later reorged away, or a redemption that was never paid, and submits proof to the Solana program. Permissionless and automated; needs only gas money. A challenger earns a **bounty for a successful payout challenge**. There is **no bounty for detecting a reorg**; that is incentivised only indirectly, because an undetected fraud costs stakers (F3). *(With per-relayer deposits the pooled watchdog role is largely replaced by the deadline and the challenge window, which report a failure without anyone being appointed to look for it.)*

**Checkpoint (light client)** — a recent, deeply buried BSV block header that the on-chain light client treats as its starting point, so it does not have to store the entire header chain.

**Cold reserve / covenant** — *(superseded)* the pooled, covenant-locked BSV backing controlled by a single key. The design no longer has a pooled reserve: deposits pay individual relayers, so there is **no bridge address and nothing for a covenant to constrain**. The roadmap target in [Trust model](04-trust-model.md) — a signerless reserve — remains the destination, but per-relayer deposits are the path that does not depend on it landing.

**DAA (Difficulty Adjustment Algorithm)** — BSV's rule for adjusting mining difficulty, recalculated every block over a 144-block window. The light client must verify it, or a fake header could be accepted — but the shipped client **does not implement it, and actively rejects it**: `check_daa` is a stub with no caller and `push_header` requires `bits` to equal the value fixed at initialization, so the client **halts permanently at the first retarget** (F7). DAA is a designed component that is rejected in code, not merely absent; implementing it (or letting `expected_bits` advance at a retarget) is a named gap before testnet.

**Exposure (`owed_R`)** — what a **single relayer `R`** has been credited and must stand behind: the sum accumulated from deposit proofs **the program verified itself**, grown by each mint that relayer consented to — **including staged mints still in the vault** — and reduced as rights settle. The bond constraint is `bond_R ≥ k × owed_R`. Unlike the earlier pooled `k × (hot float + releasable tranche)`, this is a quantity the program measures rather than one a custodian attests — see *bond*.

**`FLOOR`** — the **minimum deposit confirmation depth**, 12 BSV blocks for the PoC, **fixed in code** (D4). Depositors and bids may commit to *more*, never less: the program requires both `confirmations ≥ committed_depth` and `committed_depth ≥ FLOOR`. It is a **safety** parameter and a **backstop**, not a price — depth is a term of the bid, and `FLOOR` exists to catch a bid nobody should accept. A depth below it is refused rather than discouraged. **Fixed in code also means there is no governance mechanism in the PoC at all** (D7): the change mechanism is a **named gap** deferred to final implementation, not a switch that exists. And as a *distinct parameter* `FLOOR` is designed, not built — the shipped mint uses a fixed `MIN_CONFIRMATIONS = 12` and parses no committed depth from the `OP_RETURN`.

**Hot wallet** — a small BSV float a relayer holds to pay redemptions. With per-relayer deposits there is no shared hot wallet; each relayer's own float is the only freely spendable tier, and it is bounded by that relayer's `HOT_FLOAT_CAP`. **It is the relayer's own money and is not covered by its bond** — the bond answers `owed_R`, the liability the program measures, not the off-chain float it cannot read (F4). It should hold nothing outside outstanding redemption commitments — see *naked spend*.

**Light client** — a program that verifies a chain's headers and transaction inclusion without downloading the whole chain. SOLBEAM runs a BSV light client **on Solana**.

**Maturity** — the **time a staged mint must stay unreorged before the vault releases it** to the depositor. Depth and maturity do different jobs: **depth sets the cost of attacking** (a reorg must out-mine `FLOOR`), while **maturity sets the time available to detect** one. A deposit that is followed by a reorg of depth `≥ FLOOR` is burned instead of released. See *vault* and *the two gates*.

**Merkle proof** — a short cryptographic path showing that a transaction is included in a block, without needing all the block's transactions.

**Mint** — creating `solBSV` on Solana against a proven BSV deposit.

**Naked spend** — spending a relayer's float with **no redemption outstanding**. Unlike a theft attached to a redemption, there is no deadline to miss, so nothing reports it automatically. In the pooled design a challenger was the entire enforcement mechanism; with per-relayer deposits what bounds it is that each float is small and capped, and that a relayer which spends BSV it owes is failing `owed_R`, which the seizable bond secures. The float itself is **not covered by the bond**, so it should never hold anything outside outstanding commitments. See [Trust model](04-trust-model.md).

**OP_RETURN** — a BSV output that can carry arbitrary data. SOLBEAM uses it to attach your Solana address to a deposit, together with the **confirmation depth you commit to** — the commitment is what stops anyone downstream undercutting it to `FLOOR`.

**`owed_R`** — what relayer `R` has been **credited**, accumulated from deposit proofs the program verified itself. It is the liability side of the per-relayer constraint `bond_R ≥ k × owed_R`, and unlike an off-chain reserve balance it is a chain fact. See *exposure* and *bond*.

**Optimistic** — a design where an action is accepted immediately but can be challenged and reversed (or punished) within a window. SOLBEAM's redemption is optimistic.

**Peg-in / peg-out** — moving value into the wrapper (BSV → `solBSV`) and back out (`solBSV` → BSV).

**Proof-of-reserves** — a published, independently checkable statement that the BSV held by the peg matches the `solBSV` supply.

**Relayer** — permissionless, bonded software that receives deposits at **its own BSV script**, pays redemptions in BSV and proves the payout to Solana. Not a committee, not a company, not an operator — a role. Because deposits pay individual relayers there is **no pooled bridge address and no shared hot wallet**, and a relayer's exposure is its own `owed_R`.

**Relayer consent** — the **signature a relayer gives accepting the liability of a mint** that names its script. Without it, a fraudulent mint could credit `owed_R` to an innocent relayer whose bond is then slashed for an attack it never agreed to and could not have detected. Consent converts the reorg risk from an externality into a term the relayer priced and chose, and it is what makes the per-transaction capacity check meaningful.

**SIGHASH_FORKID** — the signature-hash scheme BSV requires for transaction signing. SOLBEAM implements it directly.

**Slashing** — seizing a relayer's bond as punishment for proven misbehaviour: a deliberate theft or abandonment of what the relayer owes (`owed_R`). It is **not** the settlement for a failed redemption: there the escrow is **returned to the holder** and supply is unchanged, and the bond is **not additionally transferred**, because the returned escrow already makes the holder whole. Because the bond is `solBSV`, a punished theft leaves the peg no worse collateralised.

**solBSV** — the wrapped BSV token on Solana: a classic SPL token, 8 decimals, no freeze authority.

**SPV (Simplified Payment Verification)** — verifying that a transaction is in a chain by checking block headers and Merkle proofs, rather than validating the whole chain yourself.

**The two gates** — the checks that surround the **vault**, on both directions of the peg. On **peg-in** the gate requires **no recent reorg at or above the committed depth, a non-stale tip, and a client that is not catching up** before the vault releases a matured mint. On **peg-out** the gate requires **capacity** — some relayer with sufficient bond accepting the request — before the escrow is burned. Both directions have the same shape: enter the vault, then leave it either to the counterparty or back to the sender. A gate that fails **delays** rather than refunds, and no failure path mints. **Designed, not built.** Whether `RECENT_REORG_WINDOW` is enforced as an on-chain gate or only monitored and published is a known open item.

**Tranche** — *(superseded)* a scheduled slice of a pooled cold reserve released to a hot wallet. The design no longer has a pooled reserve, so there is no tranche schedule; per-relayer deposits make each relayer's own float the only spendable tier. Retained here so older documents that use the term are understood as historical.

**Trustless** — correct without trusting any participant: verification rests on cryptography and on-chain code. Minting qualifies; redemption is *trust-minimised* rather than trustless, because paying BSV needs a signature.

**Trust-minimised** — a small, bounded trust assumption remains, but it is enforced economically (bonds, slashing) and by on-chain proofs rather than by promises.

**Unbonding period** — the notice period a relayer must wait before its bond is released, longer than the redemption deadline plus the challenge window. Without it a relayer could take a job, withdraw its bond, and be gone before anyone could slash. A bond withdrawable on demand is not a bond.

**Vault** — a **program-owned token account that every mint lands in first, never with the user**. It is the mechanism that makes the design work: because the tokens sit in the program's *own* account, the program can **burn them if a reorg is followed** or release them on maturity, all without a freeze authority and without Token-2022 hooks. It is also why a staged token is not liquid, so there is no window in which a fraudulent mint can be sold to an innocent buyer. Peg-out escrows into the same vault before the escrow is burned or returned. Designed, not built.

**Veto-only cosigner** — a second key on a relayer's float (2-of-2 bare multisig) held by a separate party that can only *refuse* a spend, never redirect one. It cannot steal, because stealing needs the relayer's key too. It reduces the trust assumption from "can take the money" to "can delay a redemption". *(This is the pooled-era mitigation; with per-relayer deposits the bounds are the seizable bond against `owed_R` and the float cap, and whether a cosigner is still wanted is open.)*

---

Next: [Brand & visual language](10-brand.md)
