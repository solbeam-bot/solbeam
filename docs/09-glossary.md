# 9. Glossary

Plain-English definitions of the terms used in this documentation.

**Atomic (in "atomic wrapper")** — here it means the wrapper is 1:1 and minting is authorised purely by a proof: the deposit either verifies on Solana or it doesn't, with no discretionary approval. It does **not** mean redemption is instantaneous.

**Bond** — collateral a relayer lodges on Solana before it is allowed to hold the hot key. It is denominated in **`solBSV`** — the same asset as the exposure — so no BSV price move can shrink it relative to what it protects, and it is **locked**: bonded `solBSV` cannot be redeemed, and release requires an unbonding period. It is seized if the relayer steals from the float or fails to deliver after accepting a redemption.

**Burn** — destroying `solBSV` on Solana in order to redeem the underlying BSV. The burn and the requested BSV destination are recorded by the bridge program.

**Challenger / watchtower** — anyone who watches for unauthorised reserve spends and submits proof to the Solana program, earning a share of the slashed bond. Permissionless, automated, needs only gas money. For a *naked spend* the challenger is the entire enforcement mechanism, not a backstop.

**Checkpoint (light client)** — a recent, deeply buried BSV block header that the on-chain light client treats as its starting point, so it does not have to store the entire header chain.

**Cold reserve** — the large, covenant-locked part of the BSV backing. It can only pay the hot wallet address, in scheduled tranches. A stolen cold key cannot divert it.

**Covenant** — a locking script on BSV that constrains what any spend of those coins may look like. SOLBEAM uses one to force "cold reserve → hot wallet only".

**DAA (Difficulty Adjustment Algorithm)** — BSV's rule for adjusting mining difficulty, recalculated every block over a 144-block window. The light client must verify it, or a fake header could be accepted.

**Exposure** — the most a single relayer can steal before anyone can react: `k × (hot float + releasable tranche)`. The bond must be at least this large. It is measured in `solBSV`, so it does not move when BSV's price does.

**Hot wallet** — a small BSV float held by a relayer's key, used to pay redemptions. This is the only freely spendable tier, and it is what the bond covers. It should hold nothing outside outstanding redemption commitments — see *naked spend*.

**Light client** — a program that verifies a chain's headers and transaction inclusion without downloading the whole chain. SOLBEAM runs a BSV light client **on Solana**.

**Merkle proof** — a short cryptographic path showing that a transaction is included in a block, without needing all the block's transactions.

**Mint** — creating `solBSV` on Solana against a proven BSV deposit.

**Naked spend** — spending the hot float with **no redemption outstanding**. Unlike a theft attached to a redemption, there is no deadline to miss, so nothing reports it automatically and only a challenger can trigger the slash. This is why the float is never left idle and why the hot wallet should require a veto-only cosigner. See [Trust model](04-trust-model.md#the-naked-option-attack).

**OP_RETURN** — a BSV output that can carry arbitrary data. SOLBEAM uses it to attach your Solana address to a deposit.

**Optimistic** — a design where an action is accepted immediately but can be challenged and reversed (or punished) within a window. SOLBEAM's redemption is optimistic.

**Peg-in / peg-out** — moving value into the wrapper (BSV → `solBSV`) and back out (`solBSV` → BSV).

**Proof-of-reserves** — a published, independently checkable statement that the BSV held by the peg matches the `solBSV` supply.

**Relayer** — permissionless, bonded software that pays redemptions in BSV and proves the payout to Solana. Not a committee, not a company — a role.

**SIGHASH_FORKID** — the signature-hash scheme BSV requires for transaction signing. SOLBEAM implements it directly.

**Slashing** — seizing a relayer's bond as punishment for proven misbehaviour. Because the bond is `solBSV`, a slash removes supply as well as collateral, so a punished theft leaves the peg slightly *better* collateralised, never worse.

**solBSV** — the wrapped BSV token on Solana: a classic SPL token, 8 decimals, no freeze authority.

**SPV (Simplified Payment Verification)** — verifying that a transaction is in a chain by checking block headers and Merkle proofs, rather than validating the whole chain yourself.

**Tranche** — a scheduled slice of the cold reserve that becomes releasable to the hot wallet. Slow tranches bound how much can be stolen in one go.

**Trustless** — correct without trusting any participant: verification rests on cryptography and on-chain code.

**Trust-minimised** — a small, bounded trust assumption remains, but it is enforced economically (bonds, slashing) and by on-chain proofs rather than by promises.

**Unbonding period** — the notice period a relayer must wait before its bond is released, longer than the redemption deadline plus the challenge window. Without it a relayer could take a job, withdraw its bond, and be gone before anyone could slash. A bond withdrawable on demand is not a bond.

**Veto-only cosigner** — a second key on the hot wallet (2-of-2 bare multisig) held by a separate party that can only *refuse* a spend, never redirect one. It cannot steal, because stealing needs the relayer's key too. It reduces the trust assumption from "can take the money" to "can delay a redemption", and it is the only defence against someone who steals the hot key and never posted a bond.

---

Next: [Brand & visual language](10-brand.md)
