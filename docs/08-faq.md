# 8. FAQ

> **What exists, and what does not.** The light client, the token and the mint are **built and
> tested**. **The vault, the order book, per-relayer deposits and all of peg-out are designed and
> not built.** Where an answer below describes a staged mint, a bid or a redemption, it is
> describing the specification; the shipped program still mints straight to the depositor's
> wallet. See [`13-summary.md`](13-summary.md).

**Is SOLBEAM trustless?**
Half of it. **Minting is trustless** — a BSV light client on Solana verifies deposits, so no one approves them. **Redemption is trust-minimised** — a bonded relayer holds a small hot float and can be slashed. We describe it exactly that way rather than claiming more.

**What does "atomic" mean in the name?**
Two things, stated plainly: the wrapper is **1:1** (one `solBSV` always represents one BSV of backing), and **minting is atomic in the proof sense** — the deposit proof either verifies on Solana or it doesn't; there is no discretionary approval. Redemption is *optimistic* with a challenge window, not atomic. That distinction matters and is not hidden.

**Where does my `solBSV` go after I deposit?**
Into a **program-owned vault**, never straight to your wallet. The program mints there once your deposit reaches the depth your bid named, and the vault releases to you after a **maturity window** passes with the deposit still canonical. If BSV reorgs and the program follows the heavier branch, the still-staged tokens are **burned** and your BSV returns to you, because the deposit itself was reorged away. You end where you started; nobody else is affected. That staging is also why no freeze authority is needed: the program can burn or return tokens in its *own* account, which is disposing of what it holds rather than confiscating a balance.

**Why can't redemption be trustless too?**
Because releasing real BSV requires a BSV signature, and BSV Script cannot verify Solana's ed25519 consensus. Solana can check BSV; BSV cannot check Solana. That asymmetry is a property of the two chains. The roadmap target — a BSV covenant that releases funds against a zero-knowledge proof of the Solana burn — would remove the key entirely.

**What happens if a relayer disappears?**
The redemption deadline expires and the program **returns your escrowed `solBSV` to you**. On failure the escrow comes back rather than being re-minted, so supply is unchanged, and the relayer's bond is slashed. Either way you keep the same amount.

**What if BSV's price rises sharply?**
Nothing breaks. The bond is held in **`solBSV`** — the same asset as the exposure — so a price move scales both sides together. There is no oracle to feed and no governor to react, which also means there is no window in which an attacker could strike. A sharp move only changes relayer fee revenue in dollar terms. (An earlier draft used a stablecoin bond with a price governor; that was a written call option on the reserve. See [Parameters](06-parameters.md#a-caution-on-the-stablecoin-bond).)

**Who controls SOLBEAM? Is there an operator?**
**No — and that is the design, not a stage it is passing through.** Minting has no trusted participant at all: the program reacts only to BSV headers and Solana slots, and a relayer is a role anyone may run rather than a privileged party. There is **no operator**, and there is **no freeze authority** on `solBSV`: nobody, including whoever builds it, can confiscate a balance or blacklist a holder. Reversibility comes from the vault instead — the program burns or returns tokens in its own account, which is not confiscation.

On parameters, the honest answer is that **the PoC has no governance mechanism at all** (D7). There is no multisig, vote or timelock to describe. What remains in its place is the **program upgrade authority**, which can override every parameter and is therefore an unconditional mint voucher; it is out of scope for the PoC and recorded rather than hidden. The upgrade path is a **named gap**, deliberately deferred until the system is better understood.

**What are the fees?**
**Discovered by an order book, not set by governance.** Stakers post sell orders — so much liquidity, at such a rate, at such a confirmation depth — and a depositor takes the terms on offer; the fee is what the market quotes rather than a fixed percentage someone chooses. Redemption fees pay the relayer that serves the request; there is no hidden spread. (The order book is designed, not built.)

**Why does a deposit wait, and why does a redemption take time?**
Twelve blocks is **`FLOOR`** — the minimum depth, fixed in code for the PoC — and six hours is the redemption deadline `D`, measured in **Solana slots** rather than wall-clock so a cluster halt freezes the clock instead of burning a relayer who could not act. Two things are worth being precise about. First, **depth is a term of the trade, not a constant**: a bid names the depth its staker will accept, so the market prices reorg risk, and `FLOOR` is only the backstop beneath it. Second, **coinbase maturity is not relevant here** — that is a rule about spending newly mined coins, and a bridge deposit is an ordinary transaction long past it. Slower than an exchange UI, far faster than exchange withdrawal policy.

**What if BSV's hash rate is low?**
It is a real consideration: light-client security inherits BSV's most-work assumption, and BSV's hash rate is lower than Bitcoin's. That is exactly why depth is a market term that can be committed *above* `FLOOR`, why `FLOOR` is set high enough to catch a bid nobody should accept, and why supply caps start conservative. Note also that six confirmations do not carry the same meaning on BSV as on Bitcoin, so depth has to be calibrated to BSV rather than inherited.

**Is it open source?**
Yes. The light client, bridge program, token and relayer apps are FOSS, and dependencies are chosen for permissive licensing (ISC / MIT / Apache-2.0).

**Do I need an account or KYC?**
No. Minting and redeeming are permissionless — a BSV wallet and a Solana wallet are all you need. Compliance posture around the *market* layer is a matter for the venues where `solBSV` trades.

**Can I run a relayer?**
Yes. Install the desktop or mobile app, register your own BSV script, fund a small BSV float, post a bond on Solana, and fulfil redemptions for fees. No whitelist, no committee, no permission. Deposits pay a relayer's own script rather than a shared address, so there is no pooled reserve to join and no operator to apply to — and because a mint credits your `owed_R` and puts your bond at risk, the program requires **your signature accepting that liability**. Consent is what makes the risk a priced term you chose rather than an attack an innocent relayer absorbs.

**How do I verify the reserve?**
There is **no pooled reserve** — each relayer receives deposits at its own script and holds its own BSV. What is public and checkable: the `solBSV` supply on Solana, the program's own accounting (each relayer's `owed_R` and seizable `bond_R`), and every relayer's BSV address. The system-wide invariant **`custodied BSV ≥ outstanding solBSV` is published and monitored, and the protocol cannot enforce it**, because the reserve is off-chain BSV the program cannot read. The website shows the ratio; anyone can check it independently. (D8 — monitored, not enforced.)

**What if the bridge program has a bug?**
Caps are staged and a pause is the intended stop for new minting while holders redeem, and the light client and program are open source so the checks are not taken on faith. Two honest qualifications: **there has been no independent audit** — the four critical defects found so far were found by our own adversarial review, which is not the same thing — and there is **no timelocked multisig**; the upgrade authority is out of scope for the PoC and is itself a recorded gap (A5). See [`14-decisions.md`](14-decisions.md) for the full list of what is deferred.

**Is `solBSV` the same as BSV?**
No — it is a claim on BSV, redeemable 1:1 through the peg. It trades on Solana venues and carries the trust assumptions described in [Trust model](04-trust-model.md).

**Where can I trade `solBSV`?**
On Solana venues such as Raydium and Orca, through P2P orderbooks in apps and wallets, with market makers who settle instantly for a spread, and eventually on centralised exchanges. See [Markets & liquidity](11-markets-and-liquidity.md).

**Do I have to wait to trade?**
No. Only the peg has latency. A pool trade settles in seconds, a P2P atomic swap in roughly 10–60 minutes with no third party, and a market maker can pay you immediately while it absorbs the peg delay. The peg's own windows — the depth your bid names, then maturity on the way in; the redemption deadline on the way out — only apply if you choose to use the peg directly.

**Do I need to run a BSV node?**
No — relayers and users need only wallets, an RPC endpoint and chain data. Proofs are verified on Solana, so data sources need not be trusted.

---

Next: [Glossary](09-glossary.md)
