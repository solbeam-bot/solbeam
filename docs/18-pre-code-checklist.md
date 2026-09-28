# 18. Before we code — the critical list

Consolidated from three adversarial audits (findings A1–A18, F1–F7, C1–C4, plus the
documentation verification). Organised by **when it has to be dealt with**, not by severity,
because that is the useful question now.

---

## Status at a glance — read this first

**Nothing on this page is fixed.** Every item is either *decided* (the approach is agreed, no
code written) or *open* (needs a decision). The code is unchanged from the A1/A2/A3/A7 fixes.

| | Item | Status | What it needs |
|---|---|---|---|
| **P1** | Checkpoint race + self-declared difficulty | **Decided** — race accepted | Deploy privately; address-link later if wanted. No code |
| **P2** | `commit_fork` does not re-anchor | **Specified** | Store `fork_parent_hash` at init; link from it; re-check at commit |
| **P3** | Deposits have a hard ~32 h life | **Decided** — 48 h accepted, but **the window cannot hold it** | W1 measured the real limit at **32 h** (192 records). The app automates the mint; deadline disclosed; unproven receipts published off-chain. **The 48 is now wrong and needs re-deciding** |
| **P4** | Retarget halts the client (F7) | **Specified** | Store the difficulty-period anchor; compute and check the new target. Testable on regtest |
| **P5** | Replay-list shutdown and hard ceiling | **Decided** | `MIN_PEG_IN = 1 BSV`, **and the fixed list replaced by a nullifier PDA per minted deposit** |
| **P6–P10** | Vault-era items | **Design-in** | Depend on the vault, which is unbuilt |

Only **P1** is closed by a decision rather than by work. Everything else is work still to do.

## What is materially outstanding

After the decisions on P1–P7, four things carry real weight, in order:

1. **P2 — window splicing.** The one genuine forgery vector. Code, light client, testable on
   regtest.
2. **P4 — the retarget halt.** Blocks testnet; self-contained; testable on regtest.
3. **The vault has no instruction set designed.** Release, burn, escrow and return do not exist
   even on paper, so there is nothing to audit and P8/P10 depend on it. **This is the largest
   design gap left.**
4. **Cross-deployment replay.** Nothing binds a BSV deposit to *this* Solana deployment. If
   SOLBEAM is ever deployed twice, the same deposit could mint on both — the recipient address is
   a Solana pubkey and would resolve identically. **Not previously recorded anywhere.**

Plus three smaller items: who pays the **first-time ATA rent** (~$0.11 per new user, the largest
per-user cost and the only one that does not come back on its own); **A7's missing test**; and the
**five unexamined areas** in section 4, of which the order book's economics matter most because
D2's auto-approve rests on reasoning that has never been adversarially tested.

### P11 — Cross-deployment replay · **fix defined, cheap now and awkward later**

**Yes — redeploying the same code is the bug.** Nothing in a deposit commits to *which* Solana
deployment it is for.

Concretely: the `OP_RETURN` carries the **recipient's Solana pubkey**. A second deployment of the
same program — a fresh program id, its own light client, its own checkpoint — would verify the
same BSV deposit, because the proof is against the BSV chain and the recipient pubkey resolves
identically on both. The same BSV would then back `solBSV` on **two** deployments.

It is not a double-spend of one token: the two deployments have **different mint addresses**, so
they are different tokens. It is worse in a quieter way — **two tokens each claiming the same
backing**, and only one of them is backed. The standard wrapped-asset failure.

**The fix, and it is small:** bind the deployment into the deposit. The `OP_RETURN` carries a
**domain separator** — the program id, or a short hash of it — alongside the recipient, and
`verify_deposit` rejects a claim whose separator is not this deployment's. A deposit made for
deployment A then cannot mint on B, because B sees a commitment to A.

**Why now rather than later:** the separator has to be in the *deposit*, which is written by the
depositor. Adding it after launch means every depositor must change what they sign, and every
deposit made before the change stays replayable forever. **Cheap to add while `verify_deposit` is
being touched for P5; painful to retrofit.**

### The first-time ATA rent · **decision needed**

A recipient who has never held `solBSV` needs an associated token account created, which costs
**0.00149 SOL (~$0.11)** in rent. It is the **largest per-user cost in the system** — against
$0.0004 for the mint that triggers it — and the only one that does not come back on its own; it
is recoverable only by closing the account.

Against a 1 BSV minimum deposit (~$30) that is about **0.37%**, comparable to the fee itself.

| Option | Effect |
|---|---|
| **Submitter pays (status quo)** ✅ | Minting is permissionless, so whoever submits pays. The app absorbs ~$0.11 per new user as an onboarding cost, and the depositor can reclaim it by closing the account |
| Protocol reimburses | Cleaner for users; adds accounting and a withdrawal path |
| Depositor pre-creates the account | Shifts the cost and the rent-reclaim to them, and adds a step before the first deposit |

**Recommended: submitter pays, and say so in the docs.** It is small, it is recoverable, and it
avoids inventing a reimbursement mechanism for eleven cents.

## 1. Live in shipped code — fix or decide before writing anything new

### P1 — The checkpoint race · **critical, unfixed**
`initialize` accepts any 80-byte header that meets **its own declared `bits`**, and it is
unauthenticated. Whoever calls it first chooses both the trusted root **and the difficulty for
the client's entire life.** On a fresh deploy the transaction is front-runnable.

The checkpoint being trusted is inherent to the design. **The race is not.** Options: deploy and
initialize atomically; gate `initialize` on a known key; or accept it and document that the
deploy transaction must be private. *Decision needed.*

### P2 — `commit_fork` does not re-anchor the staged branch · **critical, unfixed**
Linkage is validated when each branch header is *pushed*. At *commit* it only checks that the
fork height is still in the window, then splices `headers[..=fork_idx] ++ staging.hashes`
**without re-checking that `headers[fork_idx].hash` is still the block the branch links to.**

If another fork commits in between, the stored window is spliced from two different chains.
`verify_deposit` then proves against a record whose linkage is broken — **the only invariant the
light client has.** This is a potential mint-forgery vector, not merely untidy. Fix: store the
expected parent hash in the staging account at `init_staging` and re-verify it at commit.

### P3 — A deposit has a hard ~32-hour life, and then it is unspendable · **critical, decided: 32 h**
`verify_deposit` requires the deposit's height to be **inside the window** (`index_of(height)`).
The window holds **192** headers and `window_start` advances with every header pushed. (W1 measured this: 147 records are consumed by cw-144 itself, and 192 is the largest that fits the 10,240-byte cap with margin.)

So a deposit must be minted within roughly 32 hours of its block. After that the proof can never
be verified again — **and the BSV is already with the relayer.** An honest depositor whose mint is
delayed — because the advancer stalled, a gate closed, or they simply waited — loses the deposit
permanently, with no on-chain refund path (A15).

### P3 broken down

Four separate things get tangled here, so take them one at a time.

**(a) The window advances by design.** Every BSV header pushed moves `window_start` forward. The
window holds **192 hashes** — about 32 hours at ten-minute blocks. This is normal operation, not
a reorg, not an attack.

**(b) So every deposit has a deadline.** `verify_deposit` asks "is the block at height H in the
window?" After ~288 blocks the answer is permanently no. The proof can never be verified again.

**(c) The BSV does not come back.** The deposit sits in the relayer's own script. Moving it needs
a BSV transaction signed by **that relayer's key**. The protocol is on Solana and cannot sign it,
and **BSV has no timelocks** — `OP_CLTV` and `OP_CSV` are no-ops — so "refundable after 24 hours"
cannot be written into the script. A refund is therefore a **rule, not a guarantee**.

**(d) The incentive points the wrong way.** Until a deposit is *proven*, `owed_R` is zero: the
relayer holds the BSV, carries no liability, and its bond is untouched. **The relayer profits by
never minting.**

**What protects the depositor is that minting is permissionless — and that minting is itself the
enforcement.** Proving the deposit is what creates `owed_R` and makes the bond bind. So the
depositor's own action is simultaneously the remedy and the thing that puts the relayer on the
hook. They never need the relayer's cooperation to be made whole; they need only to act.

| Option | Extends the deadline? | Cost | Verdict |
|---|---|---|---|
| **Automate the mint** in the app | No — but the deadline stops mattering | trivial | **Do it.** The primary answer |
| **Disclose the deadline** (48 h) in the UI | No | trivial | **Do it** |
| **Publish unproven receipts** off-chain | No | low | **Do it.** This is the "24-hour rule" — monitoring and reputation, not code |
| Historic-header bridging | By ~12 blocks (tx limit) | medium | Marginal |
| Multiple window accounts (4 × 192) | Yes, 4× | 4× rent (~$16) + complexity | Possible if 48 h proves too short |
| A refund path | — | — | Needs the relayer's key. A rule, not code |
| A covenant | — | research | The structural fix: removes the relayer's discretion entirely |

**The residual, stated plainly:** a depositor who does not use our app, does not mint, and does not
watch for 32 hours can lose the deposit to a dishonest relayer. There is **no code fix for that
while the reserve is key-controlled.** It belongs in the trust model explicitly.
Options: size the window against `depth + maturity` with margin and state the deadline in the UI;
allow a historic header to be supplied with a chain of headers; or add the refund path. *Decision
needed, and it interacts directly with the maturity length.*

### P4 — `FLOOR` and the retarget · **F7, blocks testnet**
`push_header` requires `bits == expected_bits`, set once at `initialize` and never refreshed.
**The client halts permanently at the first difficulty retarget.** Invisible on regtest; fatal on
testnet or mainnet. Needs a stored difficulty-period anchor — BSV retargets every 2016 blocks and
we store 192, of which 147 are consumed by the DAA.

### P5 — The replay list is a cheap shutdown and a hard ceiling · **F6, unfixed**
`MAX_USED = 200` with `MIN_PEG_IN` unimplemented. Two hundred dust deposits block every peg-in for
the rest of the window, repeatably. Independently, it caps the protocol at **200 peg-ins per
48 hours** with no attacker at all.

---

## P5 — settled: a minimum, plus a nullifier per deposit

Two different problems were filed together, and only one is answered by a minimum deposit.

**(a) Dust griefing — decided.** `MIN_PEG_IN = 1 BSV` is enforced on-chain. Two hundred one-satoshi
self-deposits previously cost dust; they now cost 200 BSV. That is a real deterrent.

**But it is not a full one, and it is worth seeing why:** the attacker *receives tokens* for every
deposit. So this is **a capital-lockup attack, not a fee attack** — the cost is the opportunity
cost of 200 BSV tied up for up to 32 hours, plus fees and slippage if they sell on a DEX rather
than waiting to redeem. Real, but not prohibitive.

**(b) The 200-per-window ceiling — NOT fixed.** `MAX_USED = 200` with a 192-block window means the
protocol can process **at most 200 peg-ins per 32 hours, with no attacker at all.** A minimum
deposit does nothing about this; it bounds the *cost* of filling the list, not the *size* of it.

And it cannot simply be raised. The account is `8 + 4 + (MAX_USED × 44) + 1` bytes against a
10,240-byte cap, so one account holds at most **232** entries — ~100 mints a day. That is a
capacity limit, not a safety one, and it would bite in ordinary use.

| Option | Ceiling | Cost | Verdict |
|---|---|---|---|
| Keep the fixed list at 200 | 200 per 48 h | — | Fine for a PoC; not for production |
| Shrink `DepositKey` (u32 height) | ~255 | trivial | Marginal |
| **A nullifier PDA per minted deposit** ✅ | **none** | ~0.002 SOL rent per mint, refundable when closed | **The structural fix.** Solana cannot enumerate PDAs, but it does not need to — replay is checked by looking up a derived address |
| Multiple list accounts | 200 × n | n × rent | Works, more moving parts |

**Decided: adopt the nullifier.** A deposit is "used" if its derived PDA exists, which removes
the ceiling, the pruning logic and the accidental capacity cap at once. Solana cannot enumerate
PDAs but does not need to — replay is checked by *deriving* an address, not by scanning a list.
Rent is ~0.002 SOL per mint, refundable when the account is closed after the window.

**Separately, the aggregate mint cap is still unimplemented.** `MAX_MINT_PER_WINDOW` was designed
as the safety parameter; the replay list is a *different* thing that has been accidentally
doubling as a capacity cap. They should not be conflated any longer.

---

## 2. Live the moment the vault is built — design them in, not after

### P6 — Closed by the nullifier decision · **was F1**

**Superseded by P5.** P6 said a burn must release the replay entry so a re-mined deposit can be
minted again. With a nullifier PDA, burning the staged mint **closes the PDA**, so the entry is
released as a side effect. No separate work.

*On the suggestion of pinning a transaction to a specific block:* BSV has `nLockTime`, but it
sets a **lower bound only** — "not valid before height H". A transaction with `nLockTime = H` can
still be mined in any later block, so it cannot be pinned to one block, and there is no native
"expires after" either (`OP_CLTV`/`OP_CSV` are no-ops). The underlying problem is the height
changing on re-inclusion, and the fix is to key replay on `(txid, vout)` — which is already done
— plus the nullifier. The accepted posture, "even if mints fail sometimes it is safer", is
right, and it is what keying on identity rather than height already gives us.

<details><summary>Original entry, kept for the record</summary>

### P6 — The vault must release the replay entry when it burns · **F1**
Pruning is by height, and a natural reorg does not remove a transaction — it returns it to the
mempool to be mined again. So the ordinary case is: block orphaned, vault correctly burns the
staged mint, transaction re-mined, **and the depositor can never mint again.** Their BSV is in the
reserve and their tokens are gone. This is the most likely honest-user loss in the *designed*
system and it needs no attacker.

</details>

### P7 — Largely dissolves · **was F3**

**Downgraded after review, because the framing was wrong.** P7 was written as though the risk were
that someone relays *wrong* information. That was never the risk: the relayer is entirely
untrusted and the program **verifies every header**. A header that does not link to the tip, that
declares its own difficulty, or that misses the target is rejected outright, and the relayer pays
the fee for the privilege. Relaying bad information does not work.

What remained was **liveness** — whether anyone bothers to deliver headers at all — and that is
much smaller than it looked:

- The cost is about **$0.39 a week**.
- **Anyone who wants to mint needs the tip current**, because a deposit is proven against it.
- On the unbacked path, which is the **initial liquidity provision**, the party with a pending
  deposit is exactly the party motivated to advance the chain, and at genesis there is nothing
  worth attacking yet.

So this is a chore nobody minds doing, not a design flaw. **No bounty is needed for the PoC.**
The genuine residual is narrower and already covered by P2: a *valid fork* block can be pushed and
taken as the tip, and it is replaced only by a strictly heavier branch — which is where window
splicing becomes possible.

**Context, for the record: this is a Solana-side job.** The light client lives on Solana and stores BSV block
hashes in its account. For it to know a new BSV block exists, **someone must submit a Solana
transaction calling `push_header`.** That is what "pushing headers" means — one Solana
transaction per BSV block, ~$0.0004 each, about **$0.39 a week** in total.

Two separate roles, neither paid:

1. **Advancing** — one `push_header` per BSV block. Permissionless, cheap, unstaked.
2. **Fork staging** — when BSV reorganises, building and committing the competing branch. More
   work, and *this* is the act of detection.

**The incentive that does exist**, and it is probably enough: anyone whose deposit is affected
wants the honest chain followed. A depositor whose mint was orphaned cannot re-mint until the
client switches to the honest branch — so **they** are motivated to stage the fork. Stakers are
too, since an undetected fraud eats their buffer. On the D6 unbacked path there are no stakers,
which is where the incentive runs thinnest.

**So the question is narrower than "should detection be paid":** is the affected-depositor
incentive sufficient, or is an explicit bounty wanted? A formal reward is optional, not
obviously necessary.

<details><summary>Original entry</summary>

Pushing headers is permissionless, unpaid and unstaked. Staging a competing branch — the actual
detection act — has no bounty, while payout challenging does. On the unbacked path (D6) there is
no staker whose buffer is at risk either, so **there is no incentive at all.** If detection is the
backstop, it needs paying for. *Decision needed before the vault ships.*

### P8 — The unbacked path has no backstop · **F2, accepted by decision**
D6 allows a peg-in with no underwriter. Then the vault and detection are the *entire* defence: no
bond, no buffer, nothing to slash. That is a deliberate risk acceptance, but it should be
implemented as an explicit, visible mode rather than as the default.

### P9 — Self-dealing is profitable when detection fails · **F5, `k = 1`**
At `k = 1` a staker underwriting its own fraudulent deposit is roughly break-even **only if the
shortfall is detected and the bond is slashed.** With detection failing, nothing is noticed to
slash and the attacker keeps the mint *and* the bond.

### P10 — The bond must cover staged mints · **F4**
`owed_R` must include mints still maturing in the vault. A depositor whose mint is maturing has
paid BSV and holds no tokens, so omitting them leaves exactly that window unbonded. Defined in
doc 12; **must be implemented that way.**

---

## 3. Known and accepted — do not re-litigate

| | |
|---|---|
| **The program upgrade authority** (A5) | Can mint arbitrarily by replacing the program. Out of scope for the PoC; the fix is governance, possibly tied to staking |
| **The reserve is keys, not a covenant** (A6) | No covenant, and no timelocks on BSV to fall back on. Per-relayer deposits are the mitigation that does not require the covenant track |
| **The buffer is off-chain** (A4) | Tracked, not verified. Its integrity rests on the custodian |
| **Refunds need a key** (A15) | No on-chain return-to-sender path exists |
| **Detection depends on somebody pushing headers** | A liveness condition anyone can satisfy — but see P7, since nobody is paid to |

---

## 4. Audit gaps — not yet examined at all

These have had **no adversarial attention** and are worth a pass before mainnet, if not before
code:

1. **The order book's economics.** No analysis of whether a staker can be systematically picked
   off, griefed out of capacity, or manipulated by a large depositor. The auto-approve decision
   rests on the vault reversing a reorged fill — untested reasoning.
2. **The vault's instruction set.** No design yet for release, burn, escrow and return — so no
   audit of their authorisation, recipients or edge cases.
3. **Cross-chain replay.** Whether a BSV transaction used on one Solana deployment can be replayed
   against another, and whether a chain-id or programme-id is bound into the deposit.
4. **The `used_deposits` counterfactual.** X2 argued the missing `seeds` constraint was not
   exploitable. It is now pinned, but **no test creates a counterfeit account and asserts
   rejection** — the closure is by argument, not by evidence.
5. **`verify_deposit`'s parsing.** No adversarial pass on malformed transactions, unusual output
   counts, or `OP_PUSHDATA` handling in the `OP_RETURN`.

---

## The three decisions this document needs

1. **P1** — how is the first `initialize` protected?
2. **P3** — how long may a deposit wait before it is unmintable, and what happens then?
3. **P7** — is detection paid for, or is it assumed to happen?

Everything else here is either fixed, scheduled, or explicitly accepted.

</details>
