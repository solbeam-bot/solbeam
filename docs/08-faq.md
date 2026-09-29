# 8. FAQ

> **What exists, and what does not.** The light client with cw-144, the token, the mint and fork
> staging are **built and tested** (24 on-chain tests). **The vault, the bonded federation,
> threshold custody, governance, slashing and all of peg-out are designed and not built.** Where
> an answer below describes a staged mint or a redemption, it is describing the specification; the
> shipped program still mints straight to the depositor's wallet. See
> [`13-summary.md`](13-summary.md).

**Is SOLBEAM trustless?**
Not all of it. **Minting is trustless** — a BSV light client on Solana verifies deposits under BSV's real difficulty rule (cw-144), so no one approves them. **Reversal is trustless too** — the program compares its own stored header hash, so a reorg is a fact about headers rather than a report from anyone. **The reserve is trusted, and bounded:** the BSV is held under a threshold key by a bonded federation, and what protects you is a bond that anyone can seize by proving misbehaviour on-chain. The one trust assumption is that **a threshold of federation members do not collude.** We describe it exactly that way rather than claiming more.

**What does "atomic" mean in the name?**
Two things, stated plainly: the wrapper is **1:1** (one `solBSV` always represents one BSV of backing), and **minting is atomic in the proof sense** — the deposit proof either verifies on Solana or it doesn't; there is no discretionary approval. Redemption is not atomic: it is settled by a federation threshold signature and proved back to the program. That distinction matters and is not hidden.

**Where does my `solBSV` go after I deposit?**
Into a **program-owned vault**, never straight to your wallet. As designed, the program mints there once your deposit reaches **12 confirmations**, and the vault releases to you after a **maturity window of 144 blocks** passes with the deposit still canonical. If BSV reorgs and the program follows the heavier branch, the still-staged tokens are **burned** and your BSV returns to you, because the deposit itself was reorged away. You end where you started; nobody else is affected. That staging is also why no freeze authority is needed: the program can burn or return tokens in its *own* account, which is disposing of what it holds rather than confiscating a balance. **One real cliff, not dressed up:** the proof must be submitted while the deposit's block is still inside the light client's **32-hour window** (192 records); after that it can never verify, and the BSV is with the reserve.

**Why can't redemption be trustless too?**
Because releasing real BSV requires a BSV signature, and BSV Script cannot verify Solana's ed25519 consensus. Solana can check BSV; BSV cannot check Solana. That asymmetry is a property of the two chains. Nor can an opaque threshold signature tell you *which* member misbehaved — which is why members sign payout intents **individually**, so misbehaviour produces its own evidence.

**What happens if a redemption fails, or a member disappears?**
The escrow comes back. A redemption escrows `solBSV` into the vault with a deadline; if the payout does not settle, **anyone may cancel after the deadline** and the escrow returns to the holder. Supply is unchanged and **the bond is not slashed for a failed redemption** — the returned escrow already makes the holder whole, and paying the bond as well would compensate twice. The bond answers proven misbehaviour, not a missed payout.

**What is the bond actually for, then?**
**Self-proving equivocation.** Members sign payout intents individually and those intents are recorded on Solana, so a member who signs **two conflicting intents** has produced its own proof of guilt: two signatures, one member, conflicting statements. Anyone may submit it and be paid the bounty. It does **not** catch "an intent matching no authorised redemption" — that predicate is undecidable, because a closed `PegOut` is indistinguishable from one that never existed, and it would false-positive against an honest member who attested before a cancel (audit F8). A threshold of members signing something invalid is attributable because every signature is on record, but that is a governance matter, not a cryptographic one — we say so rather than implying the bond catches everything.

**What if a threshold of federation members collude?**
That is the trust assumption, and it is not eliminated. It is **bounded**: the bond must exceed what the members could take (`bond ≥ k × owed`, so a member cannot leave while owing), the reserve sits under a threshold key so no single member can move it, and misbehaviour leaves on-chain evidence anyone can act on. There is no design in which a colluding threshold is harmless; the design makes it expensive and provable.

**What if BSV's price rises sharply?**
Nothing in the mechanism breaks. The bond is held in **`solBSV`** — the same asset as the exposure — so a price move scales both sides together. There is no oracle to feed and no price governor to react, which also means there is no window in which an attacker could strike. A sharp move changes the dollar value of fees and of the capacity cap; it does not change the ratio the bond protects.

**Who controls SOLBEAM? Is there an operator?**
There is no single operator, but there **is** a federation and there **is** governance — earlier drafts of this FAQ said otherwise, and that is no longer true. Membership is **open**: anyone with a **1,000 BSV bond** may join, and members run software rather than exercising judgement, so there is no manual approval of any transaction. Governance passes changes at **85% of pledged coins with a 30-day delay**, signalled live from the moment a proposal is raised, and it **holds the upgrade authority**. It **cannot touch redemptions**: they are never pausable. A pause stops **mints only**.

There is **no immutable floor, deliberately.** The protection was never that the rules are frozen; it is that **you can always leave before they change**. A hostile proposal is visible while it is only a proposal, and redemptions run throughout, so a change that would harm holders empties the bridge before it lands. **The residual, stated plainly:** a holder who does not watch and does not act within 30 days is exposed. That is a disclosure obligation, not a mechanism. On `solBSV` itself there is still **no freeze authority** — nobody can confiscate a balance or blacklist a holder.

**What are the fees?**
**30 bp, governed.** The fee is a parameter, not a discovery: an earlier draft had an order book quote it, and the order book has been **removed**. Mint and redeem fees each default to 30 bp and are changeable by the same 85% / 30-day governance process as any other parameter. There is no book, no matching and no market-making.

**Why does a deposit wait, and why does a redemption take time?**
Twelve blocks is **`FLOOR`**, the minimum confirmation depth, and it is what a deposit waits — depth is no longer a term of a bid, because there are no bids. Maturity is then **144 blocks**, chosen so the program has time to see a reorg and burn the staged tokens instead of releasing them. The deposit's whole proof must land inside the light client's **32-hour window**. On the way out, the redemption deadline is measured in **Solana slots** rather than wall-clock, so a cluster halt freezes the clock instead of punishing a member who could not act. Slower than a pool trade, far faster than exchange withdrawal policy.

**What if BSV's hash rate is low?**
It is a real consideration: light-client security inherits BSV's most-work assumption, and BSV's hash rate is lower than Bitcoin's. `FLOOR` is set high enough to make out-mining the honest chain expensive, and capacity starts conservative — with `k = 1` it is capped by bonds pledged, so the system cannot grow past what the members have put at risk. Note also that six confirmations do not carry the same meaning on BSV as on Bitcoin, so depth has to be calibrated to BSV rather than inherited.

**Is it open source?**
Yes. The light client, bridge program, token and node software are FOSS, and dependencies are chosen for permissive licensing (ISC / MIT / Apache-2.0).

**Do I need an account or KYC?**
No. Minting and redeeming are permissionless — a BSV wallet and a Solana wallet are all you need. Compliance posture around any *market* layer is a matter for the venues where `solBSV` trades, which the protocol does not run.

**Can I run a federation member?**
Yes. Post a **1,000 BSV bond** — held as `solBSV`, because it must be seizable on Solana — run the node software, and earn fees pro rata to stake. No committee approves you and there is no whitelist. **One thing is genuinely undefined:** the genesis bootstrap. Members bond `solBSV`, but none exists until a mint happens, so the first members need a path that the current design does not specify. That is recorded as open in [`13-summary.md`](13-summary.md) rather than glossed.

**How do I verify the reserve?**
The reserve is native BSV held by the federation under a threshold key, and **the Solana program cannot read it**. The `solBSV` supply is on-chain; the bonds, governance record and slashing evidence are designed to be, and do not exist yet. **The model does not specify a published proof-of-reserves mechanism** — treat any figure shown outside the chain as an assertion, not a proof.

**What if the bridge program has a bug?**
Caps start conservative, and the **pause** is the intended stop for new minting while holders redeem — because **minting can be paused and redemptions cannot**, a pause is a safety valve rather than a hostage-taking. Two honest qualifications: the vault's design has **failed two audits** and is being re-audited against this model, and there has been no external audit. The upgrade authority is no longer an unowned gap: it is held by **governance** at 85% / 30 days. See [`15-audit-2.md`](15-audit-2.md) for what was found.

**Is `solBSV` the same as BSV?**
No — it is a claim on BSV, redeemable 1:1 through the peg. It carries the trust assumptions described in [Trust model](04-trust-model.md).

**Where can I trade `solBSV`?**
Wherever SPL tokens trade — pools on Solana venues, apps and wallets, market makers, and eventually centralised exchanges. **SOLBEAM runs none of that:** it deploys no pool, quotes no price and provides no liquidity, and any such market is outside the protocol's control. See [Markets & liquidity](11-markets-and-liquidity.md), which now records that the protocol's own market layer was removed.

**Do I have to wait to trade?**
No. Only the peg has latency. A market trade settles in seconds; a market maker can pay you immediately while it absorbs the peg delay. The peg's own windows — 12 confirmations and 144-block maturity on the way in, the deadline on the way out — only apply if you use the peg directly.

**Do I need to run a BSV node?**
Users do not — a wallet and an RPC endpoint are enough, because the proofs are verified on Solana and data sources need not be trusted. Federation members run the node software, and each verifies independently with its own light client.

---

Next: [Glossary](09-glossary.md)
