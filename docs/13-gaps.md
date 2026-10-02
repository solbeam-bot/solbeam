# 13. What is left to close

**The honest inventory as of this commit.** Not a roadmap — a list of what stands between the
current state and a deployment that could be shown to strangers.

**The built program is 27 instructions and 82 passing tests.** Everything below is what is *not*
in it, sorted by what it blocks rather than by size.

---

## 1. Blocking — unsafe or misleading without it

### 1.1 The reserve keys do not exist

**This is the largest gap, and it is not code.** The program accepts the reserve script's **shape**
(`is_reserve_multisig`, a 2-of-2), and there is a federation *registry* — member accounts, a
Greycore set, bonds, thresholds. **But the keys are off-chain and have never been generated.**

So every sentence in the docs about the reserve being *"held by a federation under a 2-of-2"* is a
statement about a **specification**. **The program holds records, not custody.**

**What closes it:** a DKG ceremony among the founding members, on real hardware, with the resulting
script published so `initialize_bridge` can be given it. **Nothing in the program changes.**

### 1.2 Governance does not exist

85% of pledged coins, a 30-day delay, live signalling, holding the **upgrade authority**. All
decided, none built.

**Why it blocks:** the upgrade authority today is **one key with a 32-slot timelock**. Every claim
that *"governance holds the upgrade authority"* is therefore false, and the docs say so — but a
reader who sees "governance" in a table will assume it exists.

### 1.3 Slashing does not exist

`seize_solbsv_bond` checks **no proof**, pays **no bounty**, and is **one authority key's
assertion.** **There is no enforceable predicate for a bad mint or a bad release** — the original
audit's finding F8 stands: an off-chain threshold signature reveals nothing about who signed, and a
colluding majority simply signs the same fraudulent thing consistently and never equivocates.

**This is not closeable by code alone.** It is either an accepted risk, or it needs the covenant.

### 1.4 The covenant — the thing the bond depends on

A BSV covenant is the only mechanism that makes the **BSV-side bond collateral rather than a
promise**: a script that releases only against a proof, so a key alone cannot move it. **It is
research, and it is on the critical path for both the reserve and the bond.**

### 1.5 ~~Nothing runs the suite when a parameter changes~~ — **BUILT**

**Found the hard way.** `v.maturity_blocks` was changed from 0 to 144 and the suite was not run;
`origin/main` was **red for several commits** (70 passing / 7 failing). `gen.py` checks that the
**projections agree with each other** — nothing checks the **compiled artifact**, and a stale
`tests/params.json` hid two of the failures.

**Closed by `config/check.sh`**, which regenerates, runs `gen.py --check`, compares
`config/params.json` with `HEAD`, and **runs the suite when the sheet differs.** It exits non-zero
if the suite fails, or if `--no-suite` is used while the sheet is dirty, and it prints what it did
and did not verify.

**One residual, stated rather than implied:** it is a **maintainer script** — the repo has no CI
and no hook — so it works only if someone runs it. **A check nobody runs is a convention, not a
control.**

---

## 2. Decided, and not built

| | |
|---|---|
| ~~**N5's write path**~~ | **BUILT.** `report_spent` now requires a live `GatewayMember` (`NotGatewayMember` otherwise), so the record is written by the member set rather than one key. **The bootstrap hole is real and stated:** the writer cannot be called until the first `admit_member`, and again if every member leaves. Between `initialize_bridge` and that admission, a deposit the reserve spends can still be minted — the record that would refuse it cannot be written. **Nothing enforces admit-before-deposits, and at genesis the Greycore is the upgrade authority, so the first admission is single-key** |
| ~~**`MIN_PEG_IN`**~~ | **BUILT.** `MIN_PEG_IN = 100,000,000` **base units** (1 BSV; `claim.amount` is satoshis), enforced in `verify_deposit` with `BelowMinPegIn` after the `> 0` check and before the script check |
| **`lc.cluster_id` / `lc.pow_limit_bits`** | P11: binds a deployment to one chain. A regtest-target checkpoint must be rejected on mainnet. The values are partly in code; the binding is not built |
| **The burn bounty** | `fee.bounty_share` is `open`. Without it, `burn_staged` relies on someone acting for free |
| **Raising maturity on a deployed instance** | The default is 144; a **running** program still carries 0 until the timelocked authority raises it. That is a procedure, not code — and it must be done |
| **Sharding** | None. Blast radius is 100% against the reference's `1/N` |

---

## 3. Undecided — and it is policy, not code

**The program's job here is only to not hard-code an answer**, and it largely does: `N` is a stored
floor plus a live roster, admission is a recorded Greycore action, and there is **deliberately no
capacity rule** rather than an invented one.

| | |
|---|---|
| **Who may be admitted** | A Greycore decision. The code records it; it does not define it |
| **Whether admission can be compelled** | Unspecified, and possibly unenforceable |
| **The re-sharing ceremony** | `admit_member` records the admission and issues **no key share**. The ceremony itself is unspecified and needs the departing member's cooperation |
| **`N`** | `fed.roster_size` is `open`; the program reads it rather than fixing it |
| **The capacity rule** | Deliberately none. Every figure claiming one has been withdrawn with the reason |
| **`fed.unbond_slots`, `fed.removal`, `gov.delay_min`, `gov.pause_duration`** | `open` |

**None of these block the code.** They block a *launch*, and they need people, not commits.

---

## 4. Accepted risks — stated, deliberately not fixed

**The distinction that matters: these are decisions, not omissions.**

| | |
|---|---|
| **Collusion is unprevented** | A majority acting with the Greycore can take the reserve. No bond stops it and none is claimed to |
| **A 51% attack is affordable** | ~$10k/day. BSV's difficulty sits at mining break-even, so the attacker is nearly made whole by the rewards they mine, and the rental market holds ~120× the network |
| **Leaver-shares degrade the threshold** | A departing member **keeps a valid share**. At 4-of-N, four former members together still hold four valid shares. Rotation is unimplementable today (`initialize_bridge` fixes `deposit_script` once) and proactive re-sharing is unspecified |
| **Key extraction at the threshold** | A quorum can reconstruct the key. A stolen key is not un-stealable by slashing |
| **The seed at `initialize`** | 147 records of trusted difficulty history, supplied by the authority. **Nothing verifies it** |
| **`StaleClient` is untested** | 54,000 slots cannot be exercised on a local validator. Implemented, never reached |
| **No numeric capacity rule** | The bond is the float; the Greycore co-signature is what bounds the reserve |

---

## 5. What actually stands between here and a launch

**In order:**

1. **Run the suite whenever a parameter changes** — hours, and it prevents the class of error that
   put `main` in the red this week.
2. **The founding DKG and the reserve script** — the single thing that turns the federation from
   records into custody.
3. **Governance, holding the upgrade authority** — otherwise one key still controls everything.
4. **Decide the `open` parameters**, and accept or decline the risks in §4 **explicitly**.
5. **The covenant and slashing** — the research track, and the only thing that would bite a
   colluding majority.

**Items 2–5 are people and time, not code.** Item 1 is code, and it is small.
