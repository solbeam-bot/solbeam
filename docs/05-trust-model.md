# 05. Trust model

SOLBEAM is deliberately asymmetric: **trustless in, trust-minimised out.** This chapter states exactly
what you are trusting, and why — and then records the audit findings against it.

Every property below is one of three things, and they are labelled:

- **built** — the light client, the token, the mint, fork staging, the nullifier, the timelocked
  authority and the vault, as they stand;
- **designed** — the federation, the Greycore, peg-out, governance and the parameters that depend on
  them;
- **trusted** — off-chain, and not enforceable by the program at all.

**The built set is 16 instructions and 37 passing / 0 failing.** Membership, the reserve script, the
Greycore, governance and peg-out are specified in [03. The federation](03-the-federation.md), not
coded.

---

## Summary

**The trust division, and it should not be blurred:**

| | |
|---|---|
| **Minting** | **The deposit is verified; the backing is reported.** A Solana program verifies BSV proof of work (cw-144) and Merkle inclusion directly. **The program verifies deposits. The federation reports backing** — Solana cannot read the BSV UTXO set, so the software reports **spent deposit outpoints** and the program checks mints against that record. Not a new trust assumption: the federation is already trusted with the reserve. The **upgrade authority** is the other exception — it can re-anchor the checkpoint — so production must hold it under a threshold and a timelock |
| **Reversal** | **Trustless.** The program compares its own stored header hash against the one a deposit was proven with. **A reorg is a fact about headers, not a report from anyone.** The mechanism is built; its protective window ships at 0 |
| **The reserve** | **Trusted, and bounded.** BSV under a **2-of-2 `OP_CHECKMULTISIG`** — the federation's **threshold ECDSA** gateway key (never assembled in one place) **and** the **Greycore**'s key, and **both must sign**. No gateway majority and no Greycore can move it alone. What protects you is a **two-sided bond under the collective key** — the program seizes the `solBSV` side automatically; the members seize the BSV side by signing, which is a **collective action by the majority and a social duty, not an on-chain guarantee**, not the absence of trust |

| Property | Status |
|---|---|
| Backing (1 `solBSV` = 1 BSV) | `custodied BSV ≥ outstanding solBSV` — **monitored, not enforced.** The reserve is off-chain BSV the program cannot read. The mint path enforces the *staged/held* half of it in code |
| Reserve custody | **2-of-2 `OP_CHECKMULTISIG`** — the gateway's threshold ECDSA key plus the Greycore's. *Designed; the script's shape is accepted by code* |
| Membership | **Admitted by the Greycore.** Two-sided bonds, software rather than manual approval. *Designed, not built* |
| Bonds | **Two-sided, one per direction, neither inside the reserve, and the bond is the float.** `bsv_bond ≥ k × (BSV held)` in native BSV outside the reserve; `solbsv_bond ≥ k × (solBSV held)` in `solBSV`. The BSV side is seized by the members collectively; the `solBSV` side by the program. `k = 1`; no price oracle; the `n/t` capacity arithmetic and the `~$180k`/`$300k` figures are **withdrawn**. *Designed, not built* |
| Slashing | **Self-proving equivocation** on individually-signed payout intents. *Designed, not built* |
| Reversibility | The vault: a program-owned account, so a staged mint can be burned or released with **no freeze authority**. *Built, shipped at maturity 0* |
| Censorship of mints | **None** — anyone can mint, for anyone |
| Censorship of redemptions | **None by construction** — redemptions **can never be paused**. The exit window is the floor |
| Governance | 85% of pledged coins, 30 days, live signal, holding the upgrade authority. **All parameters.** *Designed, not built* — the authority timelock is built |
| Market / price | External (Raydium/Orca). The protocol runs none of it |
| Exit speed vs entry | The fast direction is the one that needs no trust — **minting** — and the slow direction is the one where trust is substituted with collateral |

**The one trust assumption: a threshold of federation members do not collude, together with the
Greycore.** Everything else is verified, except the **reported spent-outpoint record**. That
assumption is not eliminated — it is **bounded**, by two-sided bonds that cover what their sides hold,
by the Greycore co-signature, and by proofs anyone can submit. **The maximum loss from a colluding
threshold is the entire non-member supply**; continuous publication of the reserve and supply is the
mitigation, promoted to an early deliverable.

---

## What is trustless, and why

**Solana can verify BSV.** BSV uses double-SHA-256 proof-of-work over an 80-byte header, and Solana
exposes a native SHA-256 syscall. So a BSV light client on Solana can check, from first principles:

- the header chain links correctly;
- each header meets its difficulty target — and the target is the one **BSV's own algorithm derives
  for that block**. **cw-144** is implemented in `difficulty.rs` and replayed against real mainnet
  headers at **324/324 exact**, with no tolerance and no fitting. **The caveat that remains (X3):**
  the rule is hard-coded, and BSV's own documentation says it will revert to 2016-block retargeting at
  some point, so a consensus change would halt the bridge until governance acts;
- a given transaction is included in a given block via its Merkle branch.

Minting is authorised by that proof alone. There is no attestor to bribe, no oracle to spoof, no
committee to capture, and **no way to censor a mint** — anyone can submit a valid proof for anyone.
The instruction path — not only the pure function — is exercised by **160 real mainnet headers through
`push_header`**.

**What is trustless is the authorisation, not the address it credits.** The proof says *this deposit
exists on BSV*; it does not say *the federation agreed to honour it*. That gap is what the bond and
the threshold signature close, and it is why the rest of this chapter exists.

### Reversal is a fact about headers, not a report

The vault stages every mint. Whether it is released or burned is decided by comparing two things **the
program itself holds**: the block hash recorded when the deposit was proven, and the hash the client
stores at that height now.

| The program finds | It concludes | It does |
|---|---|---|
| Hash still matches, tip advanced past the deposit, client fresh | Canonical | `release_mint` pays the recipient |
| Hash differs, and the height has matured | Reorged | `burn_staged` burns the staged tokens |
| Height has left the window | Survived the window | `release_mint` still succeeds; burning is no longer possible |

**No oracle, no reporter, no discretion.** Burning is not confiscation either: the tokens were in an
account the program owns, so it is disposing of what it holds. That is what makes a mint reversible
**without a freeze authority** — and why a **fraudulent mint is unsellable**.

**The window is the caveat.** At the shipped maturity of 0 the burn predicate is satisfied
*instantly* rather than unreachable, so release and burn are a **race** and release normally wins.
**What protects a deposit today is `MIN_CONFIRMATIONS = 12` — prevention, not reversal.** See
[02. How it works §4](02-how-it-works.md#the-maturity-0-consequence-stated-up-front).

---

## Why redemption cannot be fully trustless (today)

Releasing native BSV requires a valid BSV signature. Solana programs cannot sign BSV transactions, and
**BSV Script cannot verify Solana's ed25519 consensus** — BSV has only secp256k1
`OP_CHECKSIG`/`OP_CHECKMULTISIG`, and stake-weighted validator aggregation is not script-expressible.

So at the instant of redemption, *some key must exist*. This is a property of the two chains, not a
shortcut in the design. SOLBEAM's response is to make that key:

- **a 2-of-2 script, with the gateway leg a threshold** — no gateway majority and no Greycore can move
  the reserve, so the object worth compromising is the gateway quorum **plus** the Greycore;
- **bounded by bonds that cover their own sides** — the redeem side is `solBSV`, the same unit as that
  exposure; the mint side is native BSV held **outside the reserve**, so neither bond is funded by a
  deposit into the thing it covers;
- **attributable** — members sign payout **intents individually and on Solana**, so every approval is
  on record and equivocation is self-proving;
- **bounded in time** — a payout that is never made does not trap the holder: after the deadline the
  escrow returns, permissionlessly, and **supply is unchanged**;
- **checked after the fact** — the payout is proved against the light client before the escrow
  settles, so a payment that never happened cannot be claimed.

Bounding can make collusion *unprofitable*. It cannot make it *impossible*, and it does nothing at all
against an attacker who never posted a bond.

---

## Who checks what

| Step | Verified by |
|---|---|
| Deposit (BSV → mint) | **The Solana program**, against the BSV light client. *Built* |
| Reorg of a staged deposit | **The Solana program**, from its own stored hashes. *Built* |
| Burn / redemption request | **The Solana program** — native state, nothing to prove. *Designed* |
| Payout (BSV → redeem) | **The Solana program**, against the BSV light client — settlement is verified, not reported. *Designed* |
| Threshold signature on the reserve | **The federation's key**, with quorum arithmetic the program cannot read. **Bounded by the bond**, not verified. *Designed* |
| Failed redemption | **Automatic** — the escrow returns to the holder after the deadline; supply is unchanged. *Designed* |

Verification is always deterministic on-chain code, and it is only as complete as the paths that
exist. **The deposit path is built; the redemption path is not.** On the built path no committee is
needed and no watcher is needed at all.

---

## Core invariants

1. **Backing.** Custodied BSV ≥ outstanding `solBSV` at all times. **Monitored rather than enforced**:
   the reserve is off-chain BSV the program cannot read, so the site publishes the ratio and the
   program does not check it.
2. **Exposure.** **Two-sided bonds**, one per direction and **neither inside the reserve**: `bsv_bond
   ≥ k × (BSV held)` and `solbsv_bond ≥ k × (solBSV held)`, **with `k = 1`**. Each side is sized in
   the asset it protects, so the inequalities hold at every BSV price: **no oracle, no governor, no
   reaction window.** *Designed, not built.*
3. **The reserve needs a quorum.** No single member can move it — a property of a threshold key plus
   the 2-of-2 script, not of a promise. `fed.threshold` sizes nothing on-chain.
4. **Reversibility without a freeze authority.** Every mint lands in a program-owned vault, released
   after maturity, or **burned** if a reorg is followed. *Built; maturity ships at 0.*
5. **Solvency after a failed redemption.** The escrow is **returned to the holder** and supply is
   unchanged, so the redeemer is made whole **without touching the bond** — paying both would
   compensate twice. **Solvency must therefore not depend on anyone submitting a proof** — which is
   what invariant 2 is for.
6. **Holder protection.** Every redemption either completes or the escrow is automatically returned
   after the deadline. **This is why the exit window can substitute for an immutable floor.**
7. **Bonds lock.** A bond withdrawable on demand is not a bond. Release requires settling outstanding
   commitments and waiting out the unbonding period, and **both bonds must still cover what their
   sides hold** — **a member cannot leave while owing.**

---

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
threshold than a governance change.

**The residual, stated plainly:** a holder who does not watch and does not act within the delay is
exposed. That is a disclosure obligation, not a mechanism.

---

## Slashing — self-proving misbehaviour

You cannot deduce who was at fault from an opaque threshold signature. **So the design does not try
to.** Members sign payout intents **individually**, so misbehaviour produces **its own evidence**:

| Misbehaviour | Provable? |
|---|---|
| A member signs **two conflicting payout intents** | **Yes — self-proving.** Two signatures, one member, conflicting statements. Anyone submits it; anyone can be paid the bounty |
| A **threshold** of members signs something invalid | Attributable, since every signature is on record — but this is a governance matter, not a cryptographic one |

**One row was deleted, not corrected: "an intent matching no authorised redemption."** A *closed*
`PegOut` is indistinguishable from one that never existed, so the program cannot decide the predicate,
and checking it would false-positive against an honest member who attested before a cancel.

**This is copied from what RenVM actually shipped**, where `slashDuplicatePropose` and its siblings
take a node's own two conflicting signatures as the entire proof. **Only that cryptographic half was
ever built** — RenVM's own documentation says the slashing contract *"will become a voting system for
darknodes to deregister other misbehaving darknodes. Right now, it is a placeholder."* **We copy the
half that shipped and say plainly that the rest has no precedent.**

**The design rule this implies:** *make misbehaviour produce a self-incriminating signed artifact,
rather than trying to infer guilt from an aggregate.*

---

## Threats and answers

| Threat | Answer |
|---|---|
| Fake deposit proof | **Rejected by the light client** — proof of work under cw-144 and Merkle inclusion. The open item is X3, that the rule is hard-coded |
| Mint staged, then a reorg is followed | The vault **burns** the staged tokens, from its own stored header hash. The depositor's BSV is reorged away with the deposit; they end where they started. *Built — but at maturity 0 the burn is a race* |
| Reorg after the vault has released | `FLOOR` (12 blocks) and maturity are what make out-mining the honest chain cost more than the fraud is worth. Today, only `FLOOR` is doing that work |
| A single member — or the gateway majority — tries to move the reserve | It cannot alone: the reserve is under a **2-of-2 `OP_CHECKMULTISIG`** and **both must sign**. *Designed, not built* |
| The gateway quorum colludes **with the Greycore** | The assumed risk, and the one the whole design is bounded against. Two-sided bonds price it, equivocation is self-proving, and continuous publication makes the theft visible. It is **not** made impossible |
| A member signs two conflicting payout intents | **Self-proving.** Two signatures are the entire proof; anyone can submit and take the bounty |
| Nobody fulfils a redemption | The deadline passes and the escrow is **returned to the holder**, permissionlessly. The bond is not additionally transferred, because the returned escrow already makes them whole |
| A payout is reorged away | It must be paid again; if it is not, the deadline returns the escrow. An ordinary reorg of a valid signed transaction self-heals |
| A member exits to dodge a slash | The bond cannot be withdrawn instantly, and both bonds must still cover what their sides hold |
| BSV price rises sharply | **Not a solvency risk.** Each bond is denominated in the asset its side holds, so bond and exposure move together |
| Weak BSV hash rate | SPV security inherits the most-work assumption; BSV's hash rate is low relative to Bitcoin's. Mitigated by `FLOOR` and conservative caps |
| The DAA changes (X3) | The rule is hard-coded, so a BSV consensus change would halt the bridge until governance acts. Recoverable, not a theft |
| Bridge program upgrade | Governance holds the upgrade authority — in the design. In the built code the checkpoint/pause path is **timelocked (32 slots) but still one key**. The exit window is the protection, not immutability |

---

## The residual the bond cannot close

**A quorum that colludes can take the reserve, and bonding does not make that impossible.**

The threshold key removes the single key — no member can move the reserve alone — but it does not
remove the *quorum*. A `t`-of-`n` set that signs together can pay the reserve anywhere, and that act is
not something the program can detect from a threshold signature: **you cannot deduce who was at fault
from an aggregate.**

Three things bound it, and none closes it:

1. **The bonds.** Two-sided, `k = 1`, neither inside the reserve. They cover what their sides hold and
   no more, so a colluding quorum that takes the reserve is **not** fully answered by them. **The
   maximum loss is the entire non-member supply.**
2. **Attribution.** Members sign payout intents individually and on record, so equivocation produces
   its own proof. That makes the *individual* act punishable; it does not make the *quorum* act
   detectable.
3. **The exit and visibility.** Redemptions cannot be paused, so a holder who sees a hostile direction
   can leave — but that protects against a *visible* change, not against a reserve that is simply
   gone. **Continuous publication of the reserve and supply is what forces the theft to be visible**,
   and it is an early deliverable for exactly this case.

**The residual, stated plainly:** you trust that a threshold of members do not collude. It is bounded,
priced and disclosed; it is not eliminated.

---

## Why the bonds are denominated in what they protect, not a stablecoin

A stablecoin bond against a BSV liability is not a bond. It is a **written call option on the reserve,
struck at the ratio of the stablecoin bond to the BSV exposure it must cover.** Post `$500k` against a
`10,000 BSV` reserve and the member is short `5,000 BSV`. Move BSV from `$50` to `$100` and the option
is in the money: absconding becomes the *rational* trade. The attacker does not even need to time it
well, because a large holder can help the price along and manufacture the strike.

A price governor cannot fix a written option. It is reactive by construction, it needs an oracle — a
new trust assumption and a new manipulation surface — and there is always a window between the move
and the throttle. That window *is* the trade.

**The two-sided bonds delete the position on both sides.** The **redeem-side bond is `solBSV`** — the
same unit as that exposure — and the **mint-side bond is native BSV held outside the reserve**, so it
is not the same asset *or* the same pool as the liability it covers. Each bond is compared against
what its own side holds — quantities the program measures on-chain, with nothing external consulted.

### A slashed theft is deflationary

Because the **redeem-side** bond is `solBSV`, a slash of it removes supply while the reserve falls by
the stolen amount. Where that bond covers the liability, backing per remaining token **rises**. The
**mint-side bond moves native BSV**, so that half is not deflationary — it is returned to the affected
side or paid as the bounty, and it does not change the `solBSV` supply.

**The cases, kept distinct.** When a theft is caught while the mint is still staged, the vault
**burns** it: supply falls, nothing was sold, and the backing behind every remaining token improves.
When a member absconds with BSV it owes, the seizable `solBSV` bond answers the liability and can be
burned, so supply falls against a reserve that also fell. **A failed redemption is not one of these
cases**: the escrow is returned, supply is unchanged, and the bond is not additionally transferred.

---

## The roadmap to a signerless reserve

The threshold signature exists only because a BSV key cannot verify a Solana burn. On BSV that is
*expressible*: the reserve can be locked by a covenant that releases funds only against a
**zero-knowledge proof of the burn**, verified inside BSV Script. BSV is unusually suited to this —
`OP_CAT` and `OP_MUL` are active, script size is effectively unlimited, and in-script Groth16/STARK
verification has been demonstrated (BSVM, MIT, pre-mainnet and unaudited).

If it works, there is no threshold key, no Greycore co-signature, no bond against custody and no price
mismatch — the reserve releases itself against a valid proof. That is the research track, and it is
why the redemption authority stays behind a replaceable boundary.

Until it lands, the gateway threshold key plus the **Greycore co-signature** is the honest answer, and
the two-sided bonds are what price misbehaviour.

---

## What we ask you to trust — plainly

1. **The checkpoint.** The light client starts from a block hash taken on faith. It is published,
   buried deep, and the only thing not proven.
2. **The reserve script.** BSV sits under a **2-of-2 `OP_CHECKMULTISIG`**. No gateway majority and no
   Greycore can move it alone, but both colluding can. There is no covenant, and BSV has no timelocks
   to fall back on.
3. **The code being correct.** **Not independently audited.** Our own adversarial review found three
   critical defects fixed (a vacuous proof-of-work check, an unauthenticated checkpoint path, an
   unconstrained mint) and two serious ones (a replay key that double-minted after a reorg, and a
   fork-staging point that could be spliced). Separately: the vault's pre-federation design **failed
   two audits** and was rebuilt against the current model; **sharding is undecided**; and **the DAA
   rule is hard-coded (X3)**.
4. **The bond being large enough.** `k = 1` covers the liability the program measures and no more. A
   colluding threshold that takes more than the pledged bonds is not answered by the bond.
5. **That honest headers get pushed within the window.** Detection is what makes a reorg visible; the
   program decides correctly once headers arrive, but headers must arrive. Pushing them is
   permissionless and cheap, and the parties with the most to lose have the most reason to do it.
6. **The reported spent-outpoint record.** The mint checks it, but it is a **federation assertion**.
   Solana cannot read the BSV UTXO set; that is the accepted oracle.

Everything else — deposits, backing, minting, reversal and the payout proof — is enforced by code.
**Except where it is not yet written:** the federation, the Greycore, governance and all of peg-out are
designed and not built, and this list will not be shorter than reality until they are.

---

## The audit findings, and their status

Three adversarial passes ran against the design and the code. Findings are preserved with their
current status; the **narrative** — how each was found, what was wrong, and the errors this project
made and corrected — is in [10. Audit history](10-audit-history.md).

### The federation audit — the blocking set

**F1, F3, F4, F5, F7, F8** were the blockers, and it was *not* the set the vault audit named.

| ID | Finding | Status |
|---|---|---|
| **F1** | **The light client could not start on mainnet.** `required_bits()` returned `None` while the window held fewer than **147** records, and the caller fell back to "the difficulty matches" — which fails on a chain whose difficulty changes every block. The client **deadlocked at `initialize`.** The 324/324 result had validated a pure function, not the instruction path | **Fixed and verified.** A trusted 147-record seed, and **160 real mainnet headers through `push_header`** plus a real mainnet branch through `push_fork_header`. The residual is that this is still a local validator, not a real BSV node |
| **F2** | `set_checkpoint` updated the height and hash but left the expected difficulty, the retargeting flag and the proof-of-work limit stale — a client initialised on regtest would accept **everything** at the easiest target | **Fixed.** Re-derived through one shared path. The immediate `set_checkpoint` instruction was later **removed** and folded into the timelocked authority path |
| **F3** | Branch headers were checked against the **incumbent tip's** target, so no fork below the tip could ever stage and `burn_staged` was **unreachable**. "Trustless reversal" was not implemented | **Fixed.** Each branch header is computed at its own height, verified with a real mainnet branch. This required an allocation-free in-place window reader, because the borsh decode of a 192-entry window costs ~28 KB of a 32 KB per-instruction heap |
| **F4** | The mint gate was a **single instant key**: `authority` was the initialising payer, with no timelock, no threshold and no path to governance. Rewrite the checkpoint, prove a fake deposit, mint anything. This falsified *"no signature, no committee and no oracle can mint anything"* | **Partly fixed.** `initialize` and `initialize_bridge` now require the program's **upgrade authority**, and checkpoint/pause changes go through a **timelocked** propose/execute/cancel path (`TIMELOCK_SLOTS = 32`). **The residual is the upgrade key itself**: it can still rewrite the program, with no threshold. Held by design under governance. The four overclaims were corrected to name the upgrade authority as the exception |
| **F5** | TVL is not capped by bonds, and nothing checked the bond on mint. With supply = N × bond, a colluding threshold nets ≈ (N−1)/N of the reserve | **Formula superseded, still not built.** The gate now uses two-sided bonds, neither inside the reserve. The single `aggregate_bond ≥ k × supply` formula was itself unsatisfiable — bonded `solBSV` is part of the supply, so it demanded `B ≥ B + H` |
| **F7** | **The exit-as-floor is defeated in two steps.** `gov.delay` is a parameter: proposal 1 sets it to zero, or the threshold to 51%. That harms nobody during the 30 days, so nobody exits; then proposal 2 lands instantly | **Decided otherwise.** The ratchet was rejected in favour of a **floor** (`gov.delay_min`), whose **value is `open`** with both readings stated. It is a judgement about how much the majority is trusted, not a correctness question |
| **F8** | **Slashing cannot prove the case that matters.** An off-chain threshold signature does not reveal who signed, an invalid intent is rejected at record time, and a *closed* `PegOut` is indistinguishable from one that never existed | **Done.** The "intent matching no authorised redemption" row has been **deleted** everywhere; only self-proving equivocation and the governance-matter caveat remain. There is no enforceable predicate for an off-chain threshold signature over BSV |
| **F10** | The reserve script: P2PKH was right for the old model and is **wrong now** | **Reversed, and now partly honoured in code.** With the Greycore adopted as a 2-of-2 co-signer the deposit script genuinely **is** a multisig. The committed code accepts a 71-byte `OP_CHECKMULTISIG` reserve script (`is_reserve_multisig`, `MAX_SCRIPT_LEN = 71`, `DepositScript::SPACE = 84`). The keys and the Greycore behind it are not built |

**N5, adjudicated.** The audit worked the ledger identity — `R = D − P − X`, `S = M − B`, hence
`R − S = U − X` — and confirmed that **minting a spent deposit output does not create unbacked
supply**: the mint decrements `U` and increments `M` by the same amount. The real shortfall is an
**unauthorised spend by the threshold**, which is the quorum-collusion case the bond already prices.
The classification still changed: N5 is **not** unfixable from Solana. The federation's software
**reports spent deposit outpoints** and the program checks mints against that record.

**The blocking list the vault audit gave was wrong.** It said N1 and N5 must be specified before any
vault code. **N1 is a wording gap, not a security defect** (the program reads `claim.amount`, so 30 bp
of it is computable; "mint net + credit a fee" is a split of the minted supply and the pool stays fully
backed). **N3 is not a defect** (a shared account holding fully-backed fee `solBSV` breaks a rule the
author invented). **N5 was downgraded.** The actual blockers were all in the light client and the
authority model, none in the vault.

### The mechanism audit — A1–A18

A full adversarial review of the peg mechanism against the code. **Fixed** means addressed with a
test where one was possible.

| ID | Finding | Severity | Status |
|---|---|---|---|
| **A1** | **The difficulty target was read from the header being checked** — `bits` came from the submitted header and `check_daa` returned `true` unconditionally, so proof of work was vacuous and every "forging `FLOOR` blocks must out-mine the chain" claim was false. Regtest masked it | critical | **Fixed** — target computed per block by cw-144, verified 324/324 |
| **A2** | `set_checkpoint` and `set_paused` were **unauthenticated**: `authority` was a bare `Signer` compared to nothing | critical | **Fixed** — authority stored, `has_one` enforced; later replaced by the timelocked path. The deploy-time race on `initialize` remains (P1) |
| **A3** | `verify_deposit` never constrained `mint`, so anyone could submit a valid public deposit against a counterfeit mint and burn the replay slot | critical | **Fixed** — pinned to the `[b"mint"]` PDA |
| **A4** | The staking buffer is off-chain BSV — unverifiable and unseizable | critical | **Superseded** — the bonded stake is `solBSV` the program holds and can seize; the reserve's BSV is still off-chain and monitored, not enforced |
| **A5** | **The program upgrade authority is an unconditional mint voucher** — safety parameters were Rust `const`s, so "loosening needs a new program" and "ship a new program" were the same power | critical | **Owned by governance by design; not removed.** In the built code the upgrade key remains one key. It is the first thing to attack |
| **A6** | The peg-in destination is a **P2PKH key, not a covenant** | critical | **Reversed by design and then again.** The reserve is a 2-of-2 `OP_CHECKMULTISIG`, and the code now **accepts** that script shape. The covenant track remains the destination |
| **A7** | **The replay key included `height`**, so a deposit re-included at a different height after a reorg minted twice | serious | **Fixed** — identity is `(txid, vout)`; height is stored only for pruning |
| **A8** | The staging escrow was **not implemented**, and `UsedDeposits` stored no recipient or amount | serious | **Closed** — the vault and the `StagedMint` record are built |
| **A9** | `MAX_USED = 200` against a `WINDOW` of 192 was a cheap peg-in shutdown; `MIN_PEG_IN`/`MAX_PEG_IN` unimplemented | serious | **Decided and built** — the fixed list is replaced by a **nullifier PDA** per minted deposit, removing the ceiling and the pruning logic at once. `MIN_PEG_IN` as an enforced value is still not in code |
| **A10** | The aggregate cap is not tied to the buffer | serious | **Open** — the aggregate cap is still policy, not implemented |
| **A11** | `commit_fork` never re-anchored the staged branch, so an intervening commit could splice the window from two chains | serious | **Fixed** — `fork_parent_hash` recorded at `init_staging`, linked from `push_fork_header`, re-checked at `commit_fork` (`ForkPointMoved`). Commit compares chainwork, not height |
| **A12** | `commit_fork` only emits an event — no depth recorded, no pause, no bounty | serious | **Partly addressed by the federation model** — detection is a funded member job, and the program decides release from its own headers. Recording depth on commit is still to do |
| **A13** | "No oracles, by construction" is false for quantities that gate funds: the reserve is off-chain BSV | serious | **Open, and now stated** — the reserve is named as trusted rather than pretending otherwise; it is bounded, not verified |
| **A14** | The committed confirmation depth is not parsed, and would not bind an attacker anyway — the fraud's depositor *is* the attacker | serious | **Closed by reversal** — the committed-depth market term is removed with the order book; `FLOOR` is a governed floor |
| **A15** | Return-to-sender is not an on-chain path: no refund instruction, `parse_outputs` reads only outputs, and spending the deposit needs a key | serious | **Open** |
| **A16** | The bond asset contradicts across documents, and only the Solana leg is seizable | serious | **Resolved by decision** — two-sided bonds, neither inside the reserve. The asymmetry is stated, not hidden |
| **A17** | Pausing froze `push_header` too, so the tip stalled and unpausing needed the missed headers replayed one at a time | minor | **Open** — under the design the pause is mints-only and should not touch `push_header` at all |
| **A18** | Minor mismatches — `bits_to_target_be` masks the sign bit, and the adversary playbook expects an error that does not exist | minor | **Partly fixed** — `used_deposits` was pinned before the list was replaced; the sign-bit mask remains |

**The lesson worth keeping.** A1 and A2 were invisible to a green suite: the tests asserted that bad
proof of work was rejected, and it *was* — against the target the test itself supplied. A2 had no test
at all. Passing tests demonstrated that the code did what the tests did, not that the client was
secure.

### The vault audits — V, W, T, N

Three vault redesigns each failed audit before the current build. The full history is in
[10. Audit history](10-audit-history.md); the findings that survive as **live open items** are:

| ID | Finding | Status |
|---|---|---|
| **N1** | **The peg-in fee's physical location is unspecified**, and the `OP_RETURN` has no amount. The BSV arrived at the reserve address; the program can mint `net` and credit an accrual, but it cannot make the BSV-side 30 bp true | **Unresolved, critical, and the first thing to specify** |
| **N2** | **Redemption can be stalled by member inaction, and nothing is slashable** — refusal leaves no signed artifact. A timeout-and-rotate rule is the obvious candidate and is **not in the design** | **Open, structural** |
| **N3** | `FeeAccount` is a shared pool on the Solana side. Bounded (0.3% of verified deposits, distributed pro rata), but the design set a "no shared token account" rule and this is an exception to it | **Open** — either distribute per item or restate the rule |
| **N4** | `PayoutIntent` is one account per member per redemption: `m × n` accounts, each paying rent, plus one instruction per member per redemption | **Open, accepted cost** |
| **N5** | The deposit script is the mint gate; a spent deposit output remains provable | **Resolved in principle** by the reported spent-outpoint record; the **format and write path are unspecified and it is not built** |
| **N6** | The bond is denominated in the thing it protects — the redeem-side bond devalues exactly when it is seized | **Half answered** by the two-sided bonds (the mint side is BSV outside the reserve). The residual is stated, not fixed |
| **T6** | `request_redeem` must require `deadline_slot >= now + D_MIN`, or a holder can be paid **and** refunded | **Open** — must be in the instruction |
| **V5 / T8** | The real detection budget is `WINDOW − MATURITY` (48 blocks at the designed values, zero at maximum committed depth), and freshness measures update recency, not honesty | **Stated, not solved** |

---

## The honest limitations, in one place

These are the things this project has repeatedly got wrong by softening. They are stated here as
limitations, not as features:

- **The federation is an accepted oracle for spent outpoints.** *"The program verifies deposits. The
  federation reports backing."* It is not a new trust — the federation already holds the reserve — but
  it is a trust, and the mint path depends on the report.
- **Leaver-shares are open, and they degrade the threshold.** A departing member keeps a valid share;
  at `4-of-N`, four former members together still hold four valid shares. Key rotation is currently
  **unimplementable** because `deposit_script` is fixed once; proactive re-sharing is not specified.
- **There is no numeric capacity rule.** The bond is the float; the `n/t` arithmetic and the
  `~$180k`/`$300k` figures are withdrawn; nothing has replaced them. **A reviewer cannot compute our
  capacity from our documents.**
- **Collusion is unprevented.** A gateway quorum acting with the Greycore can take the reserve.
- **Sharding: none.** The blast radius is 100% of the reserve, not `1/N`.
- **The vault's protective window ships at 0.** What protects a deposit today is
  `MIN_CONFIRMATIONS = 12` — prevention, not reversal.
- **No independent audit.** The critical defects found so far were found by our own adversarial
  review, which is not the same as an audit by someone with no stake in the answer.
