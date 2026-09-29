# 4. Trust model & security

SOLBEAM is deliberately asymmetric: **trustless in, trust-minimised out.** This chapter states
exactly what you are trusting, and why. The division is the design, and blurring it is the failure
mode this document exists to prevent.

> **Built or designed?** The built set is exactly four things: the **light client** (cw-144
> difficulty verification, verified against 324/324 real mainnet headers, and Merkle inclusion), the
> **`solBSV` token**, the **mint**, and **fork staging with chainwork** — 27 passing on-chain tests.
> **The vault, the federation, threshold custody, governance, slashing and all of peg-out are
> designed and not built**; the shipped program mints straight to the depositor's token account.
> Every property below is therefore one of three things, and they are labelled:
>
> - **built** — the light client, the token, the mint and fork staging, as they stand;
> - **designed** — the vault, the federation, threshold custody, governance, slashing, peg-out and
>   the parameters that depend on them. Specified, not coded;
> - **trusted** — off-chain, and not enforceable by the program at all.

## Summary

**The trust division, and it should not be blurred:**

| | |
|---|---|
| **Minting** | **Trustless, given the deployed program.** A Solana program verifies BSV proof of work (cw-144) and Merkle inclusion directly. **No member's signature, no committee vote and no oracle mints anything.** The program's **upgrade authority** is the one exception — it can re-anchor the checkpoint — so production must hold it under a threshold and a timelock |
| **Reversal** | **Trustless.** The program compares its own stored header hash against the one a deposit was proven with. **A reorg is a fact about headers, not a report from anyone** |
| **The reserve** | **Trusted, and bounded.** BSV under a **threshold key** held by the federation. No single member can move it. What protects you is a **bond that anyone can seize by proving misbehaviour on-chain**, not the absence of trust |

| Property | Status |
|---|---|
| Backing (1 `solBSV` = 1 BSV) | `custodied BSV ≥ outstanding solBSV` — **monitored, not enforced.** The reserve is off-chain BSV the program cannot read (D8). The mint path enforces its half of it today |
| Reserve custody | **Threshold key.** No single member can move it. *Designed, not built* |
| Membership | **Open.** A 1,000 BSV bond, software rather than manual approval. *Designed, not built* |
| Bond | `bond ≥ k × owed`, **`k = 1`**, in seizable `solBSV`. `owed` is the liability the program has credited from proofs it verified itself. Both sides are `solBSV` the program holds or measures, so the inequality is checkable on-chain with no oracle. *Designed, not built* |
| Slashing | **Self-proving equivocation** on individually-signed payout intents. *Designed, not built* |
| Reversibility | The vault: a program-owned account, so a staged mint can be burned or released with **no freeze authority**. *Designed, not built* |
| Censorship of mints | **None** — anyone can mint, for anyone |
| Censorship of redemptions | **None by construction** — redemptions **can never be paused**. The exit window is the floor |
| Governance | 85% of pledged coins, 30 days, live signal, holding the upgrade authority. **All parameters**, not constants. *Designed, not built* |
| Market / price | External (Raydium/Orca) |
| Exit speed vs entry | The fast direction is the one that needs no trust — **minting** — and the slow direction is the one where trust is substituted with collateral |

**The one trust assumption: a threshold of federation members do not collude.** Everything else is
verified. That assumption is not eliminated — it is **bounded**, by bonds that exceed what they
could take, and by proofs anyone can submit.

## What is trustless, and why

**Solana can verify BSV.** BSV uses double-SHA-256 proof-of-work over an 80-byte header, and Solana
exposes a native SHA-256 syscall. So a BSV light client on Solana can check, from first principles:

- the header chain links correctly,
- each header meets its difficulty target — and the target is the one **BSV's own algorithm derives
  for that block**. **cw-144** is implemented in `difficulty.rs` and replayed against real mainnet
  headers at **324/324 exact**, with no tolerance and no fitting. **The caveat that remains (X3):**
  the rule is hard-coded, and BSV's own documentation says it will revert to 2016-block retargeting
  at some point, so a consensus change would halt the bridge until governance acts;
- a given transaction is included in a given block via its Merkle branch.

Minting is authorised by that proof alone. There is no attestor to bribe, no oracle to spoof, no
committee to capture, and **no way to censor a mint** — anyone can submit a valid proof for anyone.

**What is trustless is the authorisation, not the address it credits.** The proof says *this deposit
exists on BSV*; it does not say *the federation agreed to honour it*. That gap is what the bond and
the threshold signature close, and it is why the rest of this chapter exists.

### Reversal is a fact about headers, not a report

The vault stages every mint for `MATURITY` (144 blocks). Whether it is released or burned is decided
by comparing two things **the program itself holds**: the block hash recorded when the deposit was
proven, and the hash the client stores at that height now.

| The program finds | It concludes | It does |
|---|---|---|
| Hash still matches, tip advanced past the deposit | Canonical | `release_mint` pays the recipient |
| Hash differs | Reorged | `burn_staged` burns the staged tokens |
| Height has left the window | Survived the window | `release_mint` still succeeds; burning is no longer possible |

**No oracle, no reporter, no discretion.** Burning is not confiscation either: the tokens were in an
account the program owns, so it is disposing of what it holds. That is what makes a mint reversible
**without a freeze authority** — and it is why a **fraudulent mint is unsellable**: a staged token
is not in anyone's wallet, so there is nothing to dump and no innocent buyer to inherit the loss.

## Why redemption cannot be fully trustless (today)

Releasing native BSV requires a valid BSV signature. Solana programs cannot sign BSV transactions,
and **BSV Script cannot verify Solana's ed25519 consensus** — BSV has only secp256k1
`OP_CHECKSIG`/`OP_CHECKMULTISIG`, and stake-weighted validator aggregation is not
script-expressible.

So at the instant of redemption, *some key must exist*. This is a property of the two chains, not a
shortcut in the design. SOLBEAM's response is to make that key:

- **a threshold, not a key** — no single member can move the reserve, so the object worth
  compromising is a quorum rather than one operator;
- **bounded by a bond that exceeds what it could take** — the bond is `solBSV`, the same unit as the
  exposure, so no BSV price move shrinks it relative to what it protects;
- **attributable** — members sign payout **intents individually and on Solana**, so every approval
  is on record and equivocation is self-proving, even though the final payment is a threshold
  signature;
- **bounded in time** — a payout that is never made does not trap the holder: after the deadline the
  escrow returns, permissionlessly, and **supply is unchanged**;
- **checked after the fact** — the payout is proved against the light client before the escrow
  settles, so a payment that never happened cannot be claimed.

Bounding can make collusion *unprofitable*. It cannot make it *impossible*, and it does nothing at
all against an attacker who never posted a bond. The rest of this chapter is about how much that
actually costs.

## Who checks what

| Step | Verified by |
|---|---|
| Deposit (BSV → mint) | **The Solana program**, against the BSV light client. *Built* |
| Burn / redemption request | **The Solana program** — native state, nothing to prove. *Designed, not built* |
| Reorg of a staged deposit | **The Solana program**, from its own stored hashes. *Designed, not built* |
| Payout (BSV → redeem) | **The Solana program**, against the BSV light client — this is what makes settlement verifiable rather than a report. *Designed, not built* |
| Threshold signature on the reserve | **The federation's key**, with quorum arithmetic the program cannot read. **Bounded by the bond**, not verified. *Designed, not built* |
| Failed redemption | **Automatic** — the escrow returns to the holder after the deadline; supply is unchanged. *Designed, not built* |

Verification is always done by deterministic on-chain code, and it is only as complete as the paths
that exist. **The deposit path is built; the redemption path is not.** On the built path no
committee is needed and no watcher is needed at all: the program checks every deposit proof itself.

## Core invariants

1. **Backing.** Custodied BSV ≥ outstanding `solBSV` at all times. **Monitored rather than
   enforced** (D8): the reserve is off-chain BSV the program cannot read, so the website publishes
   the ratio and the program does not check it. The mint path enforces its half of it today.
2. **Exposure.** `bond ≥ k × owed`, **with `k = 1`**. Both sides are quantities the program holds
   or measures in `solBSV` — the bond it can seize, and the liability it credited from proofs it
   verified itself. Because the bond and the exposure are the same asset, the inequality holds at
   every BSV price: **no oracle, no governor, no reaction window.** *Designed, not built.*
3. **The reserve needs a quorum.** No single member can move it. This is a property of the threshold
   key, and it is why the custody assumption is *a threshold of members*, not *an operator*.
4. **Reversibility without a freeze authority.** Every mint lands in a program-owned vault, released
   after `MATURITY`, or **burned** if a reorg is followed. *Designed, not built.*
5. **Solvency after a failed redemption.** The escrow is **returned to the holder** and supply is
   unchanged, so the redeemer is made whole **without touching the bond** — paying both would
   compensate twice. **Solvency must therefore not depend on anyone submitting a proof** — which is
   what invariant 2 is for.
6. **Holder protection.** Every redemption either completes or the escrow is automatically returned
   after the deadline. Supply is unchanged either way. **This is why the exit window can substitute
   for an immutable floor.** *Designed, not built.*
7. **Bonds lock.** A bond withdrawable on demand is not a bond. Release requires settling outstanding
   commitments and waiting out the unbonding period, which outlasts the payout deadline; and
   `bond ≥ k × owed` must hold, so **a member cannot leave while owing.**

## Governance, and the floor that is an exit

Governance holds the **upgrade authority**, and a change needs **85% of pledged coins** and takes
effect after **30 days**, signalled **live from the moment it is raised**.

**There is no immutable floor, deliberately.** A hostile change needs 85% *and* 30 days, and
**redemptions run throughout — they can never be paused** — so a proposal that would harm holders
**empties the bridge before it lands.**

> **The floor is the exit window, not a constitution.** The protection was never that the rules are
> frozen. It is that you can always leave before they change.

**Pause is bounded on purpose: mints can be paused, redemptions cannot.** Pausing inbound is a safety
valve; pausing outbound is taking hostages. Because the power is bounded, a pause carries a lower
threshold than a governance change. *Designed, not built.*

**The residual, stated plainly:** a holder who does not watch and does not act within 30 days is
exposed. That is a disclosure obligation, not a mechanism.

## Slashing — self-proving misbehaviour

You cannot deduce who was at fault from an opaque threshold signature. **So the design does not try
to.** Members sign payout intents **individually**, so misbehaviour produces **its own evidence**:

| Misbehaviour | Provable? |
|---|---|
| A member signs **two conflicting payout intents** | **Yes — self-proving.** Two signatures, one member, conflicting statements. Anyone submits it; anyone can be paid the bounty |
| A **threshold** of members signs something invalid | Attributable, since every signature is on record — but this is a governance matter, not a cryptographic one |

**One row was deleted, not corrected: "an intent matching no authorised redemption."** A *closed*
`PegOut` is indistinguishable from one that never existed, so the program cannot decide the
predicate, and checking it would false-positive against an honest member who attested before a
cancel (audit F8).

**This is copied from what RenVM actually shipped**, where `slashDuplicatePropose` and its siblings
take a node's own two conflicting signatures as the entire proof. **Only that cryptographic half was
ever built** — RenVM's own documentation says the slashing contract *"will become a voting system for
darknodes to deregister other misbehaving darknodes. Right now, it is a placeholder."* **We copy the
half that shipped and say plainly that the rest has no precedent.**

**The design rule this implies:** *make misbehaviour produce a self-incriminating signed artifact,
rather than trying to infer guilt from an aggregate.* Attribution stops being a hard cryptographic
problem because nobody has to solve it. *Designed, not built.*

## Threats and answers

| Threat | Answer |
|---|---|
| Fake deposit proof | **Rejected by the light client** — proof of work under cw-144 and Merkle inclusion. The retarget is implemented, so a real chain is followed rather than stalled; the open item is X3, that the rule is hard-coded |
| Mint staged, then a reorg is followed | The vault **burns** the staged tokens, from its own stored header hash. The depositor's BSV is reorged away with the deposit, and they end where they started. Nobody else is affected. *Designed, not built* |
| Reorg after the vault has released | `FLOOR` (12 blocks) and `MATURITY` (144 blocks) are what make out-mining the honest chain cost more than the fraud is worth |
| A single member tries to move the reserve | It cannot: the reserve is under a **threshold key**. *Designed, not built* |
| A **threshold** of members colludes | The assumed risk, and the one the whole design is bounded against. Their bonds — `solBSV`, `k = 1`, seizable on proof — are what make it expensive; equivocation is self-proving. It is **not** made impossible |
| A member signs two conflicting payout intents | **Self-proving.** Two signatures are the entire proof; anyone can submit and take the bounty. *Designed, not built* |
| Nobody fulfils a redemption | The deadline passes and the escrow is **returned to the holder**, permissionlessly. Supply is unchanged and the bond is not additionally transferred, because the returned escrow already makes them whole |
| A payout is reorged away | It must be paid again; if it is not, the deadline returns the escrow. An ordinary reorg of a valid signed transaction self-heals, because the transaction returns to the mempool and re-mines |
| A member exits to dodge a slash | The bond cannot be withdrawn instantly, and `bond ≥ k × owed` must hold — **a member cannot leave while owing** |
| BSV price rises sharply | **Not a solvency risk.** Bond and exposure are both `solBSV`, so they move together — no top-up demand, no governor, and no window for an attacker to strike in |
| Weak BSV hash rate | SPV security inherits the most-work assumption; BSV's hash rate is low relative to Bitcoin's. Mitigated by `FLOOR` and conservative caps |
| The DAA changes (X3) | The rule is hard-coded, so a BSV consensus change would halt the bridge until governance acts. Recoverable, not a theft — and the honest gap in the design |
| Bridge program upgrade | Governance holds the upgrade authority, so this is **not** out of scope any more: it is the power the 85% / 30-day / live-signal / unpausable-redemptions structure exists to bound. The window is the protection, not immutability |

## The residual the bond cannot close

**A quorum that colludes can take the reserve, and bonding does not make that impossible.**

This is the honest shape of what is left over once everything provable has been proved. The
threshold key removes the single key — no member can move the reserve alone — but it does not
remove the *quorum*. A `t`-of-`n` set that signs together can pay the reserve anywhere, and that act
is not something the program can detect from a threshold signature: **you cannot deduce who was at
fault from an aggregate.**

Three things bound it, and none of them closes it:

1. **The bonds.** `bond ≥ k × owed`, in `solBSV`, seizable on proof. With `k = 1` the bond covers the
   liability the program measures and no more, so a colluding quorum that takes more than the
   pledged bonds is **not** answered by them. The bond is sized against the exposure, not against
   the reserve.
2. **Attribution.** Members sign payout **intents individually and on record**, so equivocation
   produces its own proof and a member can be slashed without anyone judging intent. That makes the
   *individual* act punishable; it does not make the *quorum* act detectable.
3. **The exit.** Redemptions cannot be paused, so a holder who sees a hostile direction can leave
   — but that protects against a *visible* change, not against a reserve that is simply gone.

**The residual, stated plainly:** you trust that a threshold of members do not collude. It is
bounded, priced and disclosed; it is not eliminated. The design's answer is the one this document
has given throughout — **make the assumption a threshold rather than a single party, make the
individual acts self-proving, and make the bond exceed what the program can measure as owed.**

**This is also where the earlier "naked option" argument went.** That analysis asked what happens
when a key holder spends an idle float with no redemption attached, and answered: the float is the
operator's own money, the bond does not cover it, and no `k` reaches it. Under the federation that
object no longer exists — **there is one reserve, and it is under a threshold key rather than in
someone's hot wallet** — so the question is no longer about a float at all. It is the quorum
question above, and it is answered the same way: state it, bound it, do not pretend it is closed.

## Why the bond is `solBSV` and not a stablecoin

A stablecoin bond against a BSV liability is not a bond. It is a **written call option on the
reserve, struck at the ratio of the stablecoin bond to the BSV exposure it must cover**. Post
`$500k` against a `10,000 BSV` reserve and the member is short `5,000 BSV`. Move BSV from `$50` to
`$100` and the option is in the money: absconding becomes the *rational* trade. The attacker does
not even need to time it well, because a large holder can help the price along and manufacture the
strike.

A price governor cannot fix a written option. It is reactive by construction, it needs an oracle — a
new trust assumption and a new manipulation surface — and there is always a window between the move
and the throttle. That window *is* the trade.

Denominating the bond in `solBSV` removes the position instead of hedging it, and it costs a member
nothing in optionality, because holding BSV is the business. This is also the answer to "just hedge
the collateral with perps": you do not hedge the position, you delete it. The bond is compared
against `owed`, a quantity the program measures in `solBSV`, so the inequality is not merely
price-invariant in principle — **it is one the program can evaluate for itself, on-chain, with
nothing external consulted.**

### A slashed theft is deflationary

Because the bond is `solBSV`, a slash removes supply while the reserve falls by the stolen amount.
Where the bond covers the liability (`bond ≥ owed`), backing per remaining token **rises**. Honest
holders are not merely protected — they end up marginally better collateralised. A stablecoin bond
has the opposite property: it has to be sold at a price to make holders whole, so the system absorbs
the theft *and* the market move.

**The two cases, kept distinct.** When the theft is caught while the mint is still staged, the vault
**burns** it: supply falls, nothing was sold, and the backing behind every remaining token is
strictly better. When the theft is a member absconding with BSV it owes, the seizable `solBSV` bond
answers the liability and can be burned, so supply falls against a reserve that also fell. A **failed
redemption is not one of these cases**: the escrow is returned to the holder, supply is unchanged,
and **the bond is not additionally transferred**, because the returned escrow already makes the
holder whole. Had the bond been a stablecoin, the program would have had to sell it at a market
price, and the system would have absorbed the theft *and* the market move together.

## The roadmap to a signerless reserve

The threshold signature exists only because a BSV key cannot verify a Solana burn. On BSV that is
*expressible*: the reserve can be locked by a covenant that releases funds only against a
**zero-knowledge proof of the burn**, verified inside BSV Script. BSV is unusually suited to this —
`OP_CAT` and `OP_MUL` are active, script size is effectively unlimited, and in-script Groth16/STARK
verification has been demonstrated (BSVM, MIT, pre-mainnet and unaudited).

If it works, there is no threshold key, no bond against custody and no price mismatch — the reserve
releases itself against a valid proof. That is the research track, and it is why SOLBEAM keeps the
redemption authority behind a replaceable boundary: the token, the mint path and the light client do
not change when it lands.

Until it lands, the threshold key is the honest answer, and the bond is what bounds it.

## What we ask you to trust — plainly

1. **The checkpoint.** The light client starts from a block hash taken on faith. It is published,
   buried deep, and the only thing not proven.
2. **The reserve threshold.** BSV sits under a threshold key. **No single member can move it**, but
   a quorum that colludes can. What bounds that is the bond and the on-chain proof of
   misbehaviour — not the absence of trust. There is no covenant, and BSV has no timelocks to fall
   back on.
3. **The code being correct.** Not independently audited. Our own adversarial review found three
   critical defects fixed (a vacuous proof-of-work check, an unauthenticated checkpoint path, an
   unconstrained mint) and two serious ones (a replay key that double-minted after a reorg, and a
   fork-staging point that could be spliced — P2, now fixed by recording and re-checking the
   branch's fork point). Three criticals remain open (A4, A5, A6). Separately: the **vault design
   has failed two audits** and is being re-audited against this model; **the genesis bootstrap has
   no path** (members bond `solBSV`, which does not exist until a mint happens); **sharding is
   undecided**; and **the DAA rule is hard-coded (X3)**.
4. **The bond being large enough.** `k = 1` covers the liability the program measures and no more.
   A colluding threshold that takes more than the pledged bonds is not answered by the bond.
5. **That honest headers get pushed within the window.** Detection is what makes a reorg visible;
   the program decides correctly once headers arrive, but headers must arrive. Pushing them is
   permissionless and cheap, and the parties with the most to lose have the most reason to do it.

Everything else — deposits, backing, minting, maturity, reversal and the payout proof — is enforced
by code. **Except where it is not yet written:** the vault, the federation, threshold custody,
governance, slashing and all of peg-out are designed and not built, and this list will not be
shorter than reality until they are. The built set is the light client, `solBSV`, the mint and fork
staging — 27 passing on-chain tests.

---

Next: [The federation](05-federation.md)
