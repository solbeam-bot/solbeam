# 8. FAQ

**Is SOLBEAM trustless?**
Half of it. **Minting is trustless** — a BSV light client on Solana verifies deposits, so no one approves them. **Redemption is trust-minimised** — a bonded relayer holds a small hot float and can be slashed. We describe it exactly that way rather than claiming more.

**What does "atomic" mean in the name?**
Two things, stated plainly: the wrapper is **1:1** (one `solBSV` always represents one BSV of backing), and **minting is atomic in the proof sense** — the deposit proof either verifies on Solana or it doesn't; there is no discretionary approval. Redemption is *optimistic* with a 6-hour settlement window, not atomic. That distinction matters and is not hidden.

**Why can't redemption be trustless too?**
Because releasing real BSV requires a BSV signature, and BSV Script cannot verify Solana's ed25519 consensus. Solana can check BSV; BSV cannot check Solana. That asymmetry is a property of the two chains. The roadmap target — a BSV covenant that releases funds against a zero-knowledge proof of the Solana burn — would remove the key entirely.

**What happens if a relayer disappears?**
The redemption deadline expires and the program **re-mints your `solBSV` automatically**. In the commitment model, the relayer's bond is also slashed. You are made whole either way.

**What if BSV's price rises sharply?**
The bond is held in a stable unit while exposure is in BSV, so a large move makes the bond relatively smaller. The **price governor** responds by shrinking the hot float and slowing cold tranches, and the bond must be topped up. The result is **slower redemptions, never unbacked tokens**. Backing is 1:1 regardless of price.

**Who controls SOLBEAM?**
No one controls the funds. Governance — initially a timelocked team multisig — can change **parameters** (fees, caps, deadlines, tranche schedules, pause). It **cannot move the reserve**: the cold reserve is covenant-locked to the hot wallet, and the hot float is bonded and fraud-checkable. Control migrates toward on-chain governance as the system matures.

**Is it open source?**
Yes. The light client, bridge program, token and relayer apps are FOSS, and dependencies are chosen for permissive licensing (ISC / MIT / Apache-2.0).

**Do I need an account or KYC?**
No. Minting and redeeming are permissionless — a BSV wallet and a Solana wallet are all you need. Compliance posture around the *market* layer is a matter for the venues where `solBSV` trades.

**What are the fees?**
A percentage of the redeemed amount, set by governance and published. Redemption fees pay relayers; there is no hidden spread.

**Why 12 confirmations and 6 hours?**
To make reorgs a non-issue. Twelve BSV confirmations (about two hours) make an un-mint impossible in practice; a six-hour redemption window covers payout propagation and any challenge. Slower than an exchange UI, far faster than exchange withdrawal policy.

**What if BSV's hash rate is low?**
It is a real consideration: light-client security inherits BSV's most-work assumption, and BSV's hash rate is lower than Bitcoin's. That is exactly why minting waits 12 confirmations, why large mints can require more, and why supply caps start conservative.

**Can I run a relayer?**
Yes. Install the desktop or mobile app, fund a small BSV gas float, post a bond on Solana, and fulfil redemptions for fees. No whitelist, no committee, no permission.

**How do I verify the reserve?**
The cold reserve address and its covenant are public on BSV; the `solBSV` supply is public on Solana; the bridge program's accounting is on-chain. SOLBEAM publishes continuous proof-of-reserves matching the two, and anyone can check it independently.

**What if the bridge program has a bug?**
Programs are audited before caps are raised, upgrade authority is a timelocked multisig, caps are staged, and a pause can stop new minting while holders redeem. The light client and program are open source so the checks are not taken on faith.

**Is `solBSV` the same as BSV?**
No — it is a claim on BSV, redeemable 1:1 through the peg. It trades on Solana venues and carries the trust assumptions described in [Trust model](04-trust-model.md).

**Where can I trade `solBSV`?**
On Solana venues such as Raydium and Orca, through P2P orderbooks in apps and wallets, with market makers who settle instantly for a spread, and eventually on centralised exchanges. See [Markets & liquidity](11-markets-and-liquidity.md).

**Do I have to wait to trade?**
No. Only the peg has latency. A pool trade settles in seconds, a P2P atomic swap in roughly 10–60 minutes with no third party, and a market maker can pay you immediately while it absorbs the peg delay. The two-hour mint and six-hour redemption windows only apply if you choose to use the peg directly.

**Do I need to run a BSV node?**
No — relayers and users need only wallets, an RPC endpoint and chain data. Proofs are verified on Solana, so data sources need not be trusted.

---

Next: [Glossary](09-glossary.md)
