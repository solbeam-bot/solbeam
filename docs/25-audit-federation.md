# 25. Audit — the federation model

Adversarial audit of `3_federation_model`. **Verdict: not sound to build.** The blocking set is
**F1, F3, F4, F5, F7, F8** — and it is *not* the set doc 21 named.

The single most important result is **F1**, because it invalidates a claim the project had been
treating as settled: the light client, believed built and verified, **cannot run on a real BSV chain.**

---

## Blocking

### F1 — The light client cannot start on mainnet · **critical, confirmed in code**

`required_bits()` returns `None` while the window holds fewer than **147** records — cw-144's
lookback. `difficulty_for` then falls back to requiring `bits == expected_bits`, the checkpoint's own
target.

**BSV changes difficulty every block** (measured: 471/471 distinct values in the fixture). So the
header after any mainnet checkpoint carries different `bits`, is rejected `UnexpectedRetarget`, the
window never reaches 147, and the fallback applies permanently. **The client deadlocks at
`initialize` and never advances.**

The only configuration where it advances is a checkpoint at `0x207fffff` (regtest), where
`no_retargeting` is true and the difficulty rule does not apply.

> **What this invalidates.** The **324/324 result validated a pure function in `difficulty.rs`, not
> the instruction path.** The 20 on-chain tests are all regtest. So "F7 fixed" and "the client
> follows a real chain's difficulty" are **both false as system claims** — exactly the error class
> this project keeps repeating: *verifying a function and claiming a system.*

**Fix:** the first 147 headers need a trusted target, not an equality test that cannot be satisfied —
either a governance-supplied 147-header seed at `initialize`, or an explicit fallback that trusts the
checkpoint's target for a bounded window and *then* switches, rather than one that can never switch.

### F3 — Reorg-following cannot replace a height, so `burn_staged` is unreachable

Staged branch headers are checked against `required_bits()` derived from the **incumbent tip**, not
from the branch's own height. For any fork point below the tip the branch's first block needs the
target for `fork_height + 1`, and `bits` differs every block — so it is rejected. The only acceptable
fork point is the tip, and `commit_fork` can then only *extend* the window.

**Therefore no stored hash can ever change**, `release_mint`'s "hash still matches" is always true,
and **`burn_staged` is unreachable on mainnet.** "Trustless reversal" is not implemented.

The 72-header reorg test passes only because regtest's `no_retargeting` makes the target constant.

### F4 — The mint gate is a single instant key

`authority` is the initialising payer; `set_checkpoint` and `set_paused` are `has_one = authority`,
with **no timelock, no threshold, and no path to governance** (there is no `set_authority`). Rewrite
the checkpoint, prove a fake deposit, mint anything.

**This falsifies "no signature, no committee and no oracle can mint anything"** — stated in docs 13,
04, 03 and the README. Doc 06 correctly called it a live critical; doc 12 marked it "fixed".

### F5 — TVL is not capped by bonds, and nothing checks the bond on mint

Supply growth is permissionless and unbounded. No instruction on the mint, release or withdraw path
checks `aggregate_bond ≥ k × supply`. With supply = N × bond, a colluding threshold nets ≈ (N−1)/N of
the reserve.

Doc 13 says both "TVL is capped by bonds pledged" **and** "minting is gated by nothing at all."
**Both cannot hold.** Fixable on-chain, since supply is readable — a specification omission.

### F7 — The exit-as-floor is defeated in two steps

`gov.delay` is a **parameter**. Proposal 1 sets it to zero, or the threshold to 51%. **That harms
nobody during the 30 days, so nobody exits.** Once effective, proposal 2 lands instantly.

A change harming only *future* users (lower `FLOOR`, change the deposit script) gives current holders
no reason to exit at all. And the exit is a service supplied by the same electorate that benefits from
the theft — `set_paused` halts `push_header` **and** `verify_deposit`, stalling release.

**Fix:** `gov.delay` must be **immutable or non-decreasing** — a ratchet. Otherwise "the floor is the
exit" is a slogan.

### F8 — Slashing cannot prove the case that matters

The BSV payout is a **threshold signature produced off-chain**, and a threshold signature does not
reveal who signed. An invalid intent is **rejected at record time**, so a misbehaving threshold simply
records nothing. And a *closed* `PegOut` is indistinguishable from one that never existed, so
"matches no authorised redemption" is undecidable — and would false-positive against an honest member
who attested before a cancel.

**Doc 13's third slashing row is false.** Docs 23, 05, 14 and 12 all say this is "a governance matter,
not a cryptographic one" — four writers against one, and the one was the commit that added the row.

**Fix:** delete the row and restate collusion as unbounded, or produce an enforceable predicate. There
is none for an off-chain threshold signature over BSV.

---

## N5 — adjudicated: **the counter-argument was correct**

The audit worked the ledger identity and confirmed it:

```
D = all BSV paid to the script      P = paid to redeemers      X = unauthorised outflow
M = mints executed                  B = escrows burned         U = provable-but-unminted deposits

R = D − P − X          S = M − B          hence   R − S = U − X
```

**Minting a spent deposit output decrements `U` and increments `M` by the same amount — it changes
nothing.** Worked at `k = 1`, `R = S = 1000`, `U = 0`: a member-controlled depositor adds 100
(`R = 1100`), the threshold spends it to pay an honest redeemer (`R = 1000`, `S = 900` after the
burn), the depositor mints (`S = 1000`). **`R = 1000`, `S = 1000` — the legs cancel exactly.**

The unauthorised variant: the threshold sends the same 100 to itself. Shortfall **= 100 = X**, not
200. **The mint did not create it; the unauthorised spend did** — and that is the quorum-collusion
case the bond already prices.

**So N5 does not create unbacked supply.** Correct classification: **medium, inherent, unfixable from
Solana.** Its irreducible content is that the program **verifies deposits, not backing** — and the
honest statement is exactly that.

---

## The blocking list doc 21 gave was wrong

Doc 21 said *"N1 and N5 must be specified before any vault code."* Both are non-blocking:

- **N1 is not a security defect.** The program reads `claim.amount` from the output, so 30 bp of it is
  computable; "mint net + credit FeeAccount" is a split of the minted supply, and the pool stays fully
  backed (100 = 99.7 + 0.3). It is a **wording gap**, not a hole.
- **N3 is not a defect.** Class 4 was about escrowed principal; a shared account holding fully-backed
  fee `solBSV` breaks a rule the author invented.
- **N5** — downgraded, above.

**The actual blockers are F1/F3/F4/F5/F7/F8** — all in the light client and the authority model, none
in the vault.

---

## Minimum blocking set

1. **F1** — make `initialize` able to reach a working difficulty rule on mainnet
2. **F3** — compute each branch header's target from the *branch's own* records; test with two bits changes inside a reorg
3. **F2** — `set_checkpoint` must re-derive `expected_bits`/`no_retargeting`/`pow_limit_bits`, or be removed
4. **F4** — checkpoint and pause authority must be threshold and timelocked, and appear in doc 24
5. **F5/F9** — define `owed` once, and enforce `aggregate_bond ≥ k × owed` on the mint and withdraw paths
6. **F7** — `gov.delay` immutable or a ratchet; redemptions need an attestation timeout-and-rotate rule
7. **F8** — delete doc 13's third slashing row and restate collusion as unbounded
8. **F10** — replace the P2PKH check with a real threshold script, or state that the reserve is one key
9. **Genesis** — no path exists for the first bond; blocking for deployment, not for code

N1, N3 and N5 need **restatement, not code**.
