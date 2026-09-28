# W1 — Light client verification against real BSV data

**Opened because:** three vault designs were audited and failed, and the final audit found that the
design rested on a factual error — that BSV retargets every 2016 blocks. It does not. Its difficulty
adjusts **every block**, and the shipped light client requires `bits == expected_bits`, a value set
once and never refreshed.

**The uncomfortable consequence:** the light client is the only component believed finished, and it
has **never been tested against a real difficulty**. Regtest uses the maximum target and never
adjusts, so the constant-difficulty assumption has held for the entire PoC and would have failed on
contact with testnet.

**Doctrine for this workstream:** *measure before designing.* Every premise about BSV in the document
set is treated as unverified until it is checked against real headers. No further design work on
anything above the light client until this closes.

---

## Tasks

| | Task | Status |
|---|---|---|
| **W1.1** | Fetch a contiguous run of real BSV mainnet headers (hash, bits, time, height) | ✅ **done** — 300 headers, 968,401–968,700, 0 linkage gaps |
| **W1.2** | Establish empirically how often `bits` changes and by how much | ✅ **done** — **100% of blocks** |
| **W1.3** | Identify the actual DAA and verify it predicts real headers | ✅ **done — cw-144, 324/324 exact** |
| **W1.4** | Determine what the client must store per header, and whether the window fits the 10,240-byte cap | ✅ **done — 52 B/record, WINDOW = 192, 10,094 of 10,240** |
| **W1.5** | Whether the DAA is computable from the client's own window | ✅ **done — yes, needs **147** records of lookback (not 146)** |
| **W1.6** | Rewrite `push_header`'s difficulty check against the real algorithm, with a test using real headers | ✅ **done** — `difficulty.rs`, **324/324 mainnet headers exact** (`difficulty-vectors/`) |
| **W1.7** | Fix **P2** — `commit_fork` does not re-anchor the staged branch — which is still unfixed and is the one defect doc 18 called the genuine forgery vector | ✅ **done** — `fork_parent_hash` recorded at `init_staging`, re-checked at `commit_fork` (`ForkPointMoved`) |

## What is already known, and should not be re-litigated

- `push_header` (`lib.rs:207`) and `push_fork_header` (`:355`) require `bits == expected_bits`
- `expected_bits` is set from the checkpoint header at `initialize` (`:175`) and never refreshed
- `commit_fork` (`:401`) compares **height**, not chainwork
- `LightClient::SPACE` = 9,322 (verified), against a 10,240-byte account cap
- The window held 288 records of 32 bytes before this workstream; it is now **192 records of 52 bytes**

## What must be established, not assumed

1. **Which DAA.** ASERT (anchor + incoming timestamp) and a 144-block moving average imply *very*
   different storage. ASERT needs one anchor; a moving average needs 144 timestamps, which would
   force the window down to roughly 253 records at 40 bytes each.
2. **Whether `bits` changes every block in practice**, and by how much.
3. **Whether the DAA is verifiable from data the client already holds**, or needs new stored state.
4. **The chainwork baseline** — cumulative work from genesis is not derivable from a checkpoint
   header, so it must be a trusted scalar at `initialize` and re-anchored by `set_checkpoint`.

---

## Measured results

### W1.1 — real data

**300 contiguous mainnet headers, heights 968,401–968,700**, fetched from a public BSV API and saved
to `workstreams/data/headers_mainnet.json`. Linkage verified: **0 gaps** — every header's
`previousblockhash` equals its predecessor's `hash`.

### W1.2 — the premise is dead · **100% of blocks change difficulty**

```
distinct bits values : 300 of 300
bits changed         : 299 times in 299 transitions  ->  100.0% of blocks
mean block interval  : 592.6s          (min 3s, max 3,107s)
per-block target ratio: 0.9442 .. 1.0352, mean 1.000096
```

**Every single header carries a different difficulty.** The claim in doc 21 — *"retargets are every
2016 blocks and the window holds 288, so at most one difficulty boundary can ever sit inside the
window"* — is not merely imprecise, it is the opposite of the truth.

This is corroborated by BSV's own documentation: *"the original Bitcoin client updated the difficulty
target every 2016 blocks… however the current difficulty adjustment algorithm changes the rate every
block in an attempt to compensate for the dynamics of the multiple competing SHA256 chains that
currently exist."* ([BSV Hub](https://hub.bsvblockchain.org/higher-learning/bsv-academy/bsv-theory/proof-of-work/controlling-the-block-discovery-rate.md))

The same source notes the algorithm **"will be adjusted back to the original 2016 block adjustment
rate in the near future"** — which means the difficulty rule is *not stable over time either*, and
the client cannot hard-code either behaviour without a way to handle a change.

### What this means for the shipped client

`push_header` (`lib.rs:207`) requires `bits == expected_bits`, where `expected_bits` is set from the
checkpoint header at `initialize` and never refreshed. **On mainnet it would reject the very next
header.** F7 was described as "halts permanently at the first retarget"; the truth is that it halts
**immediately**, and it has never been exercised because regtest uses the maximum target and never
adjusts.

### W1.3 — the algorithm is **not yet identified**, and no guess should be shipped

Two hypotheses were tested against the real headers. **Neither fits:**

| Hypothesis | Mean error | Max error |
|---|---|---|
| ASERT, fixed anchor, halflife 86,400s | 4.87% | 10.7% |
| ASERT, fixed anchor, halflife 172,800s | 5.40% | 12.0% |
| ASERT, fixed anchor, halflife 345,600s | 5.83% | 13.1% |
| Moving average, W = 144 | 7.62% | — |
| Moving average, best W found (259) | 1.97% | — |

The moving-average fit *improves monotonically* as the window widens, which is the signature of a
model that is wrong rather than of a window that is large — a wider window simply means less
adjustment. **So the algorithm is neither ASERT-with-a-fixed-anchor nor a plain moving average over
block timestamps.**

**This is the correct place to stop guessing.** The next step is the algorithm as BSV actually
implements it — from the node source or the specification — and then a fit against these 300 headers
as the acceptance test. Note also that the per-block magnitude (±3.5%) is consistent with a window
of roughly 144 blocks, so the *window* may be right while the *form* is wrong.

**Doctrine check:** this workstream was opened because a premise was asserted without verification.
The right outcome here is a **measured** statement — "100% of blocks change difficulty, and the exact
rule is not yet identified" — rather than a second confident guess.

---

## W1.3 — the algorithm, identified and verified

**It is cw-144, not ASERT**, and it is in the node's `src/pow.cpp`. The reason my earlier fits failed is
visible the moment you read it: **it does not use raw block timestamps — it uses a median-of-three
" suitable block" at each end.**

```cpp
// GetNextWorkRequired, src/pow.cpp
const int32_t nHeight = pindexPrev->GetHeight();
const CBlockIndex *pindexLast  = GetSuitableBlock(pindexPrev);
const CBlockIndex *pindexFirst = GetSuitableBlock(pindexPrev->GetAncestor(nHeight - 144));
const arith_uint256 nextTarget = ComputeTarget(pindexFirst, pindexLast, params);

// GetSuitableBlock: median of the 3 topmost blocks by time
//   "In order to avoid a block with a very skewed timestamp having too much influence,
//    we select the median of the 3 top most blocks as a starting point."

// ComputeTarget:
arith_uint256 work = pindexLast->GetChainWork() - pindexFirst->GetChainWork();
work *= params.nPowTargetSpacing;                       // 600
int64_t nActualTimespan = pindexLast->GetBlockTime() - pindexFirst->GetBlockTime();
if (nActualTimespan > 288 * 600) nActualTimespan = 288 * 600;   // clamp [0.5x, 2x]
else if (nActualTimespan < 72 * 600) nActualTimespan = 72 * 600;
work /= nActualTimespan;
return (-work) / work;                                  // (2^256 - work) / work
```

### Verified against real mainnet headers

Implemented in Python and run against the fetched chain:

```
VERIFY cw-144 over heights 968377..968700
324 match / 0 mismatch   ->  100.00%
*** every block predicted exactly ***
```

**324 of 324 blocks predicted exactly, with no tolerance and no fitting.** This is not a plausible
model; it is the algorithm.

### W1.6 / W1.7 — what was built

**The rule lives in `difficulty.rs`**, with no Anchor types, so the acceptance test can replay the
real headers without a validator: `poc/solana/difficulty-vectors/` `#[path]`-includes that exact
source file and runs 471 mainnet headers through it with a plain `cargo test`.

```
cw-144: 324 of 324 mainnet headers predicted exactly, heights 968377..968700
```

**Two details the task description had wrong, both caught by the replay and not by reading:**

1. **The lookback is 147 records, not 146.** `GetSuitableBlock(x)` reads three blocks ending at `x`,
   and the older argument is `GetAncestor(nHeight - 144)` where `nHeight` is the *parent's* height.
   The header at `h` therefore needs records `h-147 .. h-145`. With 146 the replay matches **0 of
   324** headers; with 147 it matches **324 of 324**. An off-by-one in a lookback is exactly the kind
   of error that stays invisible until testnet, which is what the replay exists to prevent.

2. **The clamps are `72 * nPowTargetSpacing` and `288 * nPowTargetSpacing`** — 43,200 s and
   172,800 s — and *not* 72 and 288 multiplied by the 144-block averaging window. The larger reading
   makes both bounds 144 times too big, so every real timespan looks short and every target pins to
   the lower clamp. The replay catches it because no header matches.

**The window is 192 records, and it is fixed by arithmetic rather than chosen.**

```
LIGHT_CLIENT_FIXED = 110 bytes   (8 discriminator, 8+8 heights, 32 tip hash, 8 window_start,
                                  32 authority, 4 expected_bits, 1 no_retargeting,
                                  4 pow_limit_bits, 8 last_push_slot, 1 paused, 1 bump, 4 Vec len)
HEADER_RECORD_SIZE = 52 bytes    (32 hash + 16 chainwork u128 + 4 time)
SPACE = 110 + 192 x 52 = 10,094  of 10,240   -> 146 bytes of margin
```

193 records gives 94 bytes of margin and 194 gives 42, so **192 is the largest that leaves a real
margin** — and it is 45 records above the 147 the DAA itself needs. The deposit lifetime is therefore
**32 hours at 600 s/block**, down from the 48 the design assumed. `WINDOW_HOURS = 32`, asserted at
compile time along with `WINDOW > difficulty::LOOKBACK`.

**One rule was not in the task description and had to be mirrored:** the node's
`GetNextWorkRequired` opens with `if (params.fPowNoRetargeting) return pindexPrev->GetBits();`, and
regtest sets that flag. `LightClient::no_retargeting` is that rule, derived deterministically at
`initialize` (the checkpoint's compact target is the largest the encoding can express). Without it
the regtest fixture — fixed target `0x207fffff`, which cw-144 does not reproduce — would be rejected
block by block.

**W1.7 — P2.** `init_staging` records `fork_parent_hash` once; `push_fork_header` links the branch's
first header to the recorded hash; `commit_fork` requires the chain still to hold that hash at that
height and otherwise fails `ForkPointMoved`. Two tests: one that stages a branch, moves the fork
point with a heavier branch, and asserts the stale commit is refused; one that asserts an
uncontested commit still succeeds.

### What the client needs, and what that costs — **W1.4 / W1.5**

Three requirements fall straight out, and they contradict the previous design:

1. **Per-block `chainwork` must be stored.** The target is derived from the *work difference* between
   two suitable blocks. There is no way to compute it from `bits` alone or from a single scalar.
2. **146 blocks of lookback** (144 + 2 for the median), at *both* ends.
3. **`time` must be stored**, since the clamps are on the time difference.

At 32 (hash) + 16 (chainwork, `u128` — the value is ~2^87 for BSV, so `u64` is too small) + 4 (time)
= **52 bytes per record**, and 10,134 usable bytes against the 10,240 cap:

| Per-record | Max window | Hours at 600s |
|---|---|---|
| 52 B (hash + chainwork + time) | **194** | ~32 h |
| 56 B (＋bits) | **180** | ~30 h |

**So the window cannot be 288.** The previous design's 48-hour deposit lifetime was never achievable:
storing what cw-144 needs forces the window down to roughly **180–194 records, about 30 hours** — and
146 of those are consumed by the DAA's own lookback.

**This is a real product consequence, not a detail:** the deposit deadline in doc 18 (P3, "48 hours
accepted") is **already too long** and has to come down to under 30 hours, or the header store has to
span two accounts.

### And F7 is worse than recorded, in a specific way

`push_header` requires `bits == expected_bits`. With cw-144 the target changes **every block**, so
this rejects **every header after the checkpoint** — not "at the next retarget". Implementing the fix
means storing per-header chainwork and time, which is what forces the window down above.

---

## Corrections to this workstream's own numbers

### The lookback is 147, not 146 — and it was measured

My figure of 146 was **wrong by one**, and the way it was caught is the point. `GetSuitableBlock(x)`
reads three blocks *ending at* `x`, and the older argument is `GetAncestor(nHeight - 144)` where
`nHeight` is the parent's height. For the header at height `h`:

```
pindexLast  = GetSuitableBlock(h - 1)     -> needs records h-3   .. h-1
pindexFirst = GetSuitableBlock(h - 145)   -> needs records h-147 .. h-145
oldest record needed: h - 147  ->  147 records, not 146
```

**Verified against the fixture rather than reasoned about:** with a 146-record window the
implementation reproduces **0 of 324** mainnet headers; at 147 it reproduces **324 of 324.**

An off-by-one in a lookback is exactly the class of error that would have been invisible until
testnet — a client that halts, for a reason nobody could see from the constants. It surfaced here only
because the acceptance test is a replay of real headers.

### `WINDOW = 192`, and the arithmetic

```
LIGHT_CLIENT_FIXED = 8 (discriminator) + 8 + 8 + 32 + 8 + 32 (authority)
                   + 4 (expected_bits) + 1 (no_retargeting) + 4 (pow_limit_bits)
                   + 8 (last_push_slot) + 1 (paused) + 1 (bump) + 4 (Vec len)  = 110
HEADER_RECORD_SIZE = 32 (hash) + 16 (chainwork, u128) + 4 (time)               =  52
SPACE              = 110 + 192 × 52 = 10,094   of 10,240  ->  146 bytes margin
```

At 193 it is 10,146 (94 margin); at 194, 10,198 (42 margin). **192 is the largest with real margin**,
and it is **45 records of slack over the 147 the DAA itself needs.**

**Deposit lifetime: 32 hours** at 600 s/block, down from the 48 the documents assumed.

### Regtest has an explicit no-retarget rule, which is worth knowing

The node has `if (params.fPowNoRetargeting) return pindexPrev->GetBits();` in
`GetNextWorkRequired` — a case the specification reading did not surface. It is mirrored with a
`no_retargeting` flag derived **deterministically** at `initialize` (checkpoint `bits` equal to the
maximum encodable compact target), which is what keeps the existing regtest fixture chain
(`0x207fffff`, fixed) working **without weakening the mainnet path.** Worth noting because the
obvious way to keep regtest green would have been to weaken the check, which would have re-opened F7
by the back door.
