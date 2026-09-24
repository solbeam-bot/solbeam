# 9. Glossary

Plain-English definitions of the terms used in this documentation.

**Atomic (in "atomic wrapper")** — here it means the wrapper is 1:1 and minting is authorised purely by a proof: the deposit either verifies on Solana or it doesn't, with no discretionary approval. It does **not** mean redemption is instantaneous.

**Bond** — collateral a relayer lodges on Solana before it is allowed to hold the hot key. It is seized if the relayer steals from the float or fails to deliver after accepting a redemption.

**Burn** — destroying `solBSV` on Solana in order to redeem the underlying BSV. The burn and the requested BSV destination are recorded by the bridge program.

**Challenger / watchtower** — anyone who watches for unauthorised reserve spends and submits proof to the Solana program, earning a share of the slashed bond. Permissionless, automated, needs only gas money.

**Checkpoint (light client)** — a recent, deeply buried BSV block header that the on-chain light client treats as its starting point, so it does not have to store the entire header chain.

**Cold reserve** — the large, covenant-locked part of the BSV backing. It can only pay the hot wallet address, in scheduled tranches. A stolen cold key cannot divert it.

**Covenant** — a locking script on BSV that constrains what any spend of those coins may look like. SOLBEAM uses one to force "cold reserve → hot wallet only".

**DAA (Difficulty Adjustment Algorithm)** — BSV's rule for adjusting mining difficulty, recalculated every block over a 144-block window. The light client must verify it, or a fake header could be accepted.

**Hot wallet** — a small BSV float held by a relayer's key, used to pay redemptions. This is the only freely spendable tier, and it is what the bond covers.

**Light client** — a program that verifies a chain's headers and transaction inclusion without downloading the whole chain. SOLBEAM runs a BSV light client **on Solana**.

**Merkle proof** — a short cryptographic path showing that a transaction is included in a block, without needing all the block's transactions.

**Mint** — creating `solBSV` on Solana against a proven BSV deposit.

**OP_RETURN** — a BSV output that can carry arbitrary data. SOLBEAM uses it to attach your Solana address to a deposit.

**Optimistic** — a design where an action is accepted immediately but can be challenged and reversed (or punished) within a window. SOLBEAM's redemption is optimistic.

**Peg-in / peg-out** — moving value into the wrapper (BSV → `solBSV`) and back out (`solBSV` → BSV).

**Proof-of-reserves** — a published, independently checkable statement that the BSV held by the peg matches the `solBSV` supply.

**Relayer** — permissionless, bonded software that pays redemptions in BSV and proves the payout to Solana. Not a committee, not a company — a role.

**SIGHASH_FORKID** — the signature-hash scheme BSV requires for transaction signing. SOLBEAM implements it directly.

**Slashing** — seizing a relayer's bond as punishment for proven misbehaviour.

**solBSV** — the wrapped BSV token on Solana: a classic SPL token, 8 decimals, no freeze authority.

**SPV (Simplified Payment Verification)** — verifying that a transaction is in a chain by checking block headers and Merkle proofs, rather than validating the whole chain yourself.

**Tranche** — a scheduled slice of the cold reserve that becomes releasable to the hot wallet. Slow tranches bound how much can be stolen in one go.

**Trustless** — correct without trusting any participant: verification rests on cryptography and on-chain code.

**Trust-minimised** — a small, bounded trust assumption remains, but it is enforced economically (bonds, slashing) and by on-chain proofs rather than by promises.

---

Next: [Brand & visual language](10-brand.md)
