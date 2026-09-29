# 25. Audit — the federation model

Adversarial audit of `3_federation_model`. **Verdict at the time: not sound to build.** The blocking
set is **F1, F3, F4, F5, F7, F8** — and it is *not* the set doc 21 named.

The single most important result is **F1**, because it invalidates a claim the project had been
treating as settled: the light client, believed built and verified, **cannot run on a real BSV chain.**

> **Status update, current.** **F1, F2 and F3 are fixed and verified:** **34 on-chain tests pass with
> 0 failing**, including **160 real mainnet headers driven through `push_header`** and a real mainnet
> branch through `push_fork_header`. The **initialiser vulnerability is fixed** (see F4).
> **F10 was originally closed as a documentation error, and that closure is itself REVERSED:** with
> the **Greycore adopted as a 2-of-2 co-signer** on the reserve script, the deposit script genuinely
> **is** a multisig, so the P2PKH check must change and `DepositScript::SPACE` must grow. See F10
> below. The single-bond formula F5 leaned on is **superseded** by the two-sided bonds, which are now
> stated as **the float**, not a capacity rule. The findings below are preserved as history; each
> carries its current status.

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
> the instruction path.** At the time of this audit the on-chain tests were all regtest. So "F7
> fixed" and "the client follows a real chain's difficulty" are **both false as system claims** —
> exactly the error class this project keeps repeating: *verifying a function and claiming a
> system.*
>
> **Partly superseded since.** The F1/F2/F3 fix added on-chain tests that drive **160 real mainnet
> headers through `push_header`** and a **real mainnet branch through `push_fork_header`**, so the
> instruction path — not only the pure function — is now exercised against difficulty that changes
> every block. The F1 deadlock is closed. The residual is that this is still a local validator, not
> a real BSV node.

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

> **Fixed since.** The F1/F2/F3 fix computes each branch header's target from the **branch's own**
> records and drives a **real mainnet branch through `push_fork_header`**, so a stored hash can now
> change and `burn_staged` is reachable. Verified by the 27-test suite, 0 failing.

### F4 — The mint gate is a single instant key

`authority` **was** the initialising payer; `set_checkpoint` and `set_paused` are `has_one = authority`,
with **no timelock, no threshold, and no path to governance** (there is no `set_authority`). Rewrite
the checkpoint, prove a fake deposit, mint anything.

**This falsifies "no signature, no committee and no oracle can mint anything"** — stated in docs 13,
04, 03 and the README. Doc 06 correctly called it a live critical; doc 12 marked it "fixed".
**That blanket claim is now retired for a second reason as well:** with the federation reporting
**spent deposit outpoints**, the federation *is* in the loop for backing verification, so the honest
claim is **"The program verifies deposits. The federation reports backing"** (doc 26 §3's correction;
doc 13, *What is trustless*).

**Partly fixed since.** `initialize` and `initialize_bridge` now require the program's **upgrade
authority** (verified on-chain against the loader's `ProgramData` account), so the first caller of a
fresh deployment no longer becomes `authority`. The residual is F4 itself: the upgrade key can still
rewrite the checkpoint with **no timelock and no threshold**. That fix is specified in doc 24
(`gov.authority_threshold`, `gov.authority_timelock`) and is **not built**. The four overclaims have
been corrected to name the upgrade authority as the exception.

### F5 — TVL is not capped by bonds, and nothing checks the bond on mint

Supply growth is permissionless and unbounded. No instruction on the mint, release or withdraw path
checks `aggregate_bond ≥ k × supply`. With supply = N × bond, a colluding threshold nets ≈ (N−1)/N of
the reserve.

Doc 13 says both "TVL is capped by bonds pledged" **and** "minting is gated by nothing at all."
**Both cannot hold.** Fixable on-chain, since supply is readable — a specification omission.

> **Fix specified, and the formula superseded.** The gate now uses **two-sided bonds, neither inside
> the reserve**: the **BSV-side bond** covers the BSV held (`bsv_bond ≥ k × (BSV held)`) and the
> **`solBSV`-side bond**, seizable on Solana, covers the `solBSV` held (`solbsv_bond ≥ k × (solBSV
> held)`). The single `aggregate_bond ≥ k × supply` formula this finding proposed is **superseded** —
> it could not be satisfied, because bonded `solBSV` is itself part of the supply, so `B ≥ k × supply`
> demands `B ≥ B + H` (doc 13, *The two bonds*). Still **not built**.

### F7 — The exit-as-floor is defeated in two steps

`gov.delay` is a **parameter**. Proposal 1 sets it to zero, or the threshold to 51%. **That harms
nobody during the 30 days, so nobody exits.** Once effective, proposal 2 lands instantly.

A change harming only *future* users (lower `FLOOR`, change the deposit script) gives current holders
no reason to exit at all. And the exit is a service supplied by the same electorate that benefits from
the theft — `set_paused` halts `push_header` **and** `verify_deposit`, stalling release.

**Fix:** `gov.delay` must be **immutable or non-decreasing** — a ratchet. Otherwise "the floor is the
exit" is a slogan.

**Decided otherwise (F7 closed).** The ratchet was rejected in favour of a **floor**: `gov.delay`
stays reducible by governance, but never below **`gov.delay_min`** (doc 24). The floor's **value is
now `open`** — 7 days proposed — with both readings stated: *with a floor*, proposal 1 can shorten the
delay only to the floor, so proposal 2 still has to be exited during it; *without*, the exit window is
whatever the current majority allows. The choice is a judgement about how much the majority is
trusted, not a correctness question.

### F8 — Slashing cannot prove the case that matters

The BSV payout is a **threshold signature produced off-chain**, and a threshold signature does not
reveal who signed. An invalid intent is **rejected at record time**, so a misbehaving threshold simply
records nothing. And a *closed* `PegOut` is indistinguishable from one that never existed, so
"matches no authorised redemption" is undecidable — and would false-positive against an honest member
who attested before a cancel.

**Doc 13's "intent matching no authorised redemption" row is false.** Docs 23, 05, 14 and 12 all say
this is "a governance matter, not a cryptographic one" — four writers against one, and the one was
the commit that added the row.

**Fix:** delete the row and restate collusion as unbounded, or produce an enforceable predicate. There
is none for an off-chain threshold signature over BSV.

**Status: done.** The row has been deleted from docs 13, 14, 23, 12, 09, 08, 04, 05 and 22; only
self-proving equivocation and the governance-matter caveat remain. (An earlier pass deleted the
separate *collusion* row instead and left this one standing — the two rows were different findings.)

### F10 — The reserve script: P2PKH was right for the old model, and is **wrong now** · **REVERSED**

**F10 was closed as "a documentation error, not a code defect", and that closure is reversed.**

The finding originally claimed the deposit script "is hard-required to be P2PKH, so it cannot be the
threshold script the docs describe". The first correction said the federation uses **threshold ECDSA**,
the reserve address **is** an ordinary P2PKH address, the key is never assembled in one place, and
therefore `is_p2pkh` and `DepositScript::SPACE = 38` were **correct**. **That correction assumed a
reserve with no second quorum — and the Greycore is now adopted as a co-signer.**

**The reserve script is a 2-of-2 `OP_CHECKMULTISIG`:**

```
OP_2  <gateway threshold key>  <greycore key>  OP_2  OP_CHECKMULTISIG
```

One leg is the gateway's threshold ECDSA key (one signature, however many members signed); the other
is the Greycore's. **Both must sign**, so the gateway majority cannot move funds alone, the Greycore
cannot move funds alone, and the Greycore polices every reserve spend (doc 26 §3).

**So F10's conclusion — "the code is right, the docs are wrong" — was itself too quick:**

- `is_p2pkh` **must change**; it can no longer require a 25-byte P2PKH script
- `DepositScript::SPACE` **must grow** to ~71 bytes for the 2-of-2, against the current 38
- **"No single member can move funds" is now enforced by a script as well as by the shared key** —
  the gateway's threshold ECDSA, plus the Greycore's signature
- The phrase **"threshold script"** is no longer wrong: the deposit script genuinely is a multisig.
  What remains wrong is using it for a *single-key* design

**What still holds from the first correction.** The *gateway* key is threshold ECDSA, so changing the
gateway's `t` or `n` is a **re-sharing** — the gateway key's address does not change. **Changing the
Greycore's key does change the deposit script**, so that one membership change requires moving the
reserve.

**Restated status: the code is right only for a P2PKH reserve, and the reserve is no longer P2PKH.
Reversed.**

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

**So N5 does not create unbacked supply.** The ledger identity above is unchanged. **The classification
changes, though: N5 is no longer "unfixable from Solana".** Solana cannot read the BSV UTXO set, so the
federation's software **reports spent deposit outpoints to Solana** and the program **checks mints
against that record**. That is not a new trust assumption — the federation is already trusted with the
reserve, and a party that can take the whole reserve is not meaningfully constrained by an honesty
request. **Its irreducible content remains that the program verifies deposits and the federation
reports backing** — now stated as two sentences rather than one overclaimed one.

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

1. **F1** — make `initialize` able to reach a working difficulty rule on mainnet *(fixed and verified: 160 real mainnet headers through `push_header`; 27 tests, 0 failing)*
2. **F3** — compute each branch header's target from the *branch's own* records; test with two bits changes inside a reorg *(fixed and verified: a real mainnet branch through `push_fork_header`)*
3. **F2** — `set_checkpoint` must re-derive `expected_bits`/`no_retargeting`/`pow_limit_bits`, or be removed *(fixed)*
4. **F4** — checkpoint and pause authority must be threshold and timelocked, and appear in doc 24 *(initialiser vulnerability fixed; the authority residual is specified, not built)*
5. **F5/F9** — define `owed` once, and enforce the bond on the mint and withdraw paths *(formula superseded: the two-sided bonds replace `aggregate_bond ≥ k × owed`; specified, not built)*
6. **F7** — `gov.delay` gets a **floor**, not a ratchet; redemptions need an attestation timeout-and-rotate rule *(the floor is now `open` — `gov.delay_min` carries both readings in doc 24; the ratchet is rejected)*
7. **F8** — delete doc 13's **"intent matching no authorised redemption"** row and restate collusion as unbounded *(done: the row is gone from docs 13, 14, 23, 12, 09, 08, 04, 05, 22; the separate collusion row was already deleted)*
8. **F10** — **REVERSED.** With the **Greycore adopted as a 2-of-2 co-signer**, the reserve script genuinely **is** a multisig: `is_p2pkh` **must change** and `DepositScript::SPACE` **must grow** to ~71 bytes, against the current 38. The earlier "documentation error, not a code defect" closure assumed a reserve with no second quorum and is superseded (see F10 above; doc 23, *The reserve script*; doc 26 §3)
9. **Genesis** — **decided: a BSV-side bond.** Members post BSV at genesis, so no `solBSV` needs to exist first; a capped, explicitly-unbonded first mint is recorded as a later option, not chosen

N1, N3 and N5 need **restatement, not code**.
