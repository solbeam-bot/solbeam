//! BSV's difficulty adjustment algorithm — **cw-144**, as implemented in the
//! node's `src/pow.cpp`.
//!
//! This is the whole of the retarget rule, deliberately kept in its own module
//! so it has **no Anchor types and no Solana dependency**. That is not tidiness:
//! it is what lets `poc/solana/difficulty-vectors/` `#[path]`-include this exact
//! file into a plain `cargo test` crate and replay `workstreams/data/headers_mainnet.json`
//! — 471 real headers, hashes and all — without a validator, a cluster or an
//! `anchor build`. The module under test and the module on chain are the same
//! bytes.
//!
//! ## The algorithm, verbatim from `GetNextWorkRequired`
//!
//! ```cpp
//! const int32_t nHeight = pindexPrev->GetHeight();
//! const CBlockIndex *pindexLast  = GetSuitableBlock(pindexPrev);
//! const CBlockIndex *pindexFirst = GetSuitableBlock(pindexPrev->GetAncestor(nHeight - 144));
//! const arith_uint256 nextTarget  = ComputeTarget(pindexFirst, pindexLast, params);
//!
//! // GetSuitableBlock: "In order to avoid a block with a very skewed timestamp
//! // having too much influence, we select the median of the 3 top most blocks."
//! //
//! // ComputeTarget:
//! arith_uint256 work = pindexLast->GetChainWork() - pindexFirst->GetChainWork();
//! work *= params.nPowTargetSpacing;                       // 600
//! int64_t nActualTimespan = pindexLast->GetBlockTime() - pindexFirst->GetBlockTime();
//! if (nActualTimespan > 288 * 600) nActualTimespan = 288 * 600;
//! else if (nActualTimespan < 72 * 600) nActualTimespan = 72 * 600;
//! work /= nActualTimespan;
//! return (-work) / work;                                  // (2^256 - work) / work
//! ```
//!
//! `2^256` does not fit in a `u256`, so the last line is the `(-work) / work`
//! identity exactly as the node has it: in Rust, `(!work) / work` on a `U256`
//! is the same number.
//!
//! ## The off-by-one that cost an hour
//!
//! `GetAncestor(nHeight - 144)` takes a **height**, and `pindexLast` is at
//! `nHeight - 1`. The ancestor is therefore at height `nHeight - 145` in
//! absolute terms — *not* "the block 144 back from the tip". Selecting the
//! first block 144 back from the tip is off by one and reproduces **0 of 471**
//! real mainnet headers, while the correct height arithmetic reproduces every
//! block from the window filling onward. See `FIRST_PREDICTED_INDEX` in the
//! vectors harness.

use uint::construct_uint;

construct_uint! {
    /// 256-bit unsigned arithmetic. This is the Rust spelling of the node's
    /// `arith_uint256`.
    pub struct U256(4);
}

construct_uint! {
    /// 128-bit unsigned arithmetic.
    pub struct U128(2);
}

/// BSV's target block spacing in seconds. `params.nPowTargetSpacing`.
pub const BLOCK_SPACING: u64 = 600;
/// `nPowTargetTimespan` = 2 weeks; retained because the node derives its
/// difficulty period from it, though cw-144 does not use it directly.
pub const TARGET_TIMESPAN: u64 = 14 * 24 * 60 * 60;
/// Number of blocks in the averaging window: `nPowAveragingWindow`.
pub const AVERAGING_WINDOW: u64 = 144;

/// Lower clamp on the measured timespan: `72 * nPowTargetSpacing` = 43,200s.
///
/// The "72" and "288" in the node multiply `nPowTargetSpacing`, **not** the
/// 144-block averaging window. Writing them as `72 * 144 * 600` — which this did
/// at first — makes both bounds 144 times too large, so every real timespan looks
/// short, every target is pinned to the lower clamp, and the bound is inverted: a
/// window that took 25 hours is treated as though it took 6.2 million seconds.
/// The values are asserted against the node's in the vector harness.
pub const MIN_ACTUAL_TIMESPAN: i64 = 72 * BLOCK_SPACING as i64; // 43,200
/// Upper clamp: `288 * nPowTargetSpacing` = 172,800s.
pub const MAX_ACTUAL_TIMESPAN: i64 = 288 * BLOCK_SPACING as i64; // 172,800

/// How many records the client must hold for the algorithm to be computable.
///
/// The bound is exact, and it is **147** — one more than the obvious count.
/// `GetSuitableBlock(x)` reads three blocks ending at `x`, and the older
/// argument is at height `nHeight - 144`, where `nHeight` is the *parent's*
/// height. For the header at height `h`:
///
///   * `pindexLast = GetSuitableBlock(h - 1)` reads records `h-3 .. h-1`;
///   * `pindexFirst = GetSuitableBlock(h - 145)` reads records `h-147 .. h-145`.
///
/// The oldest record required is `h - 147`, so 147 records cover `h-146 .. h`:
/// the parent and everything the two medians reach into. With one fewer,
/// `next_target` refuses to answer — which it must, because inventing a value
/// there would be inventing a difficulty rule.
///
/// This was measured, not reasoned. Against the mainnet fixture, 146 records
/// reproduce **0 of 324** headers and 147 reproduce **324 of 324**. The task
/// description said 146; the arithmetic says 147, and the real headers agree.
pub const LOOKBACK: u64 = AVERAGING_WINDOW + 3; // 147

/// What the chain's parent expected of the next block.
///
/// Split from `ComputeTarget` so the two halves can be tested separately: the
/// median selection and the work arithmetic are different mistakes and should
/// not be able to hide each other.
pub fn compute_target(first_work: u128, first_time: u32, last_work: u128, last_time: u32) -> U256 {
    // Saturating rather than wrapping. The node can subtract in 256 bits and
    // never wraps on a real chain; on a corrupted or hostile window u128 work
    // is monotone by construction, but if it ever were not, a wrap would hand
    // back a *harder* target rather than a panic, which is the worse failure.
    let work_delta = last_work.saturating_sub(first_work);
    // `work *= params.nPowTargetSpacing` — ONCE. Multiplying by 600 twice (which
    // this did, through `saturating_mul` chained twice) makes every target 600
    // times harder and still produces a plausible-looking number, which is why
    // the fixture replay is asserted in exact matches and not in tolerance.
    let work = U256::from(work_delta).saturating_mul(U256::from(BLOCK_SPACING));

    let mut actual = last_time as i64 - first_time as i64;
    if actual > MAX_ACTUAL_TIMESPAN {
        actual = MAX_ACTUAL_TIMESPAN;
    } else if actual < MIN_ACTUAL_TIMESPAN {
        actual = MIN_ACTUAL_TIMESPAN;
    }
    let divisor = U256::from(actual as u64);

    let scaled = work / divisor;
    if scaled.is_zero() {
        // Infinite target: the easiest value that exists. Only reachable from
        // an all-zero window, which the median selection cannot produce on a
        // real chain; saturating here keeps it from being a division by zero.
        return U256::max_value();
    }
    // (2^256 - work) / work, via the two's-complement identity the node uses.
    (!scaled) / scaled
}

/// `arith_uint256::GetCompact()`.
///
/// Compact form is `(size << 24) | mantissa`, where `size` is the length of the
/// big-endian number in bytes and the mantissa is its top three bytes. The
/// special case is the `0x00800000` bit: if the top byte of those three has its
/// high bit set, the mantissa is treated as negative and the node shifts it down
/// a byte and grows `size` by one, even though that byte is zero and the extra
/// size is an artifact of a signed representation. Omitting that branch makes
/// this disagree with the node on every target whose third byte is >= 0x80.
pub fn target_to_compact(target: U256) -> u32 {
    let bytes = target.to_big_endian();
    let mut size = 0usize;
    while size < 32 && bytes[size] == 0 {
        size += 1;
    }
    if size == 32 {
        return 0;
    }
    let mut compact = 0u32;
    for i in 0..3usize {
        if size + i < 32 {
            compact |= (bytes[size + i] as u32) << (8 * (2 - i));
        }
    }
    size = 32 - size;
    if compact & 0x0080_0000 != 0 {
        compact >>= 8;
        size += 1;
    }
    compact | ((size as u32) << 24)
}

/// The consensus `powLimit` for mainnet, in compact form (`0x1d00ffff`).
///
/// This is a **network parameter**, not a constant of the algorithm. Regtest's
/// is far easier (`0x207fffff`), and using mainnet's on regtest would make the
/// cap bind on a chain the node considers legal — which is a good reason to
/// keep it in one named place rather than inline.
pub const MAINNET_POW_LIMIT_BITS: u32 = 0x1d00_ffff;
/// Regtest's `powLimit`, the easiest value the compact encoding can express.
pub const REGTEST_POW_LIMIT_BITS: u32 = 0x207f_ffff;

/// `SetCompact` — compact `bits` back to a 256-bit target. The inverse of
/// `target_to_compact` for canonical values.
pub fn compact_to_target(compact: u32) -> U256 {
    let size = (compact >> 24) as usize;
    let mut mantissa = compact & 0x007f_ffff;
    let mut out = [0u8; 32];
    if size <= 3 {
        mantissa >>= 8 * (3 - size);
        out[29..32].copy_from_slice(&mantissa.to_be_bytes()[1..4]);
    } else if size <= 32 {
        let at = 32 - size;
        out[at..at + 3].copy_from_slice(&mantissa.to_be_bytes()[1..4]);
    }
    // size > 32 is not an encodable target. Returning zero means "unmeetable",
    // which is the same refusal the node's overflow branch makes.
    U256::from_big_endian(&out)
}

/// The target the next block must carry, from the last `LOOKBACK` records.
///
/// `records` is the window, oldest first, and `records.last()` is the parent of
/// the block being checked. `oldest_height` is the height of `records[0]`, so a
/// caller that has begun pruning can still find the ancestor by height rather
/// than by position — which is the bug this signature exists to prevent.
///
/// `pow_limit` is applied last, exactly where the node applies it.
///
/// Returns `None` when the window does not yet reach the ancestor block, i.e.
/// the client has fewer than [`LOOKBACK`] records. That is a real state in the
/// first blocks after `initialize` or `set_checkpoint`, and the caller must
/// choose an explicit policy for it rather than this function inventing one.
pub fn next_target(records: &[Record], oldest_height: u64, pow_limit: U256) -> Option<U256> {
    if records.len() < LOOKBACK as usize {
        return None;
    }
    let last_index = suitable_index(records, records.len() - 1)?;
    let parent_height = oldest_height + (records.len() as u64 - 1);
    // `pindexPrev->GetAncestor(nHeight - nPowAveragingWindow)`, where
    // `nHeight` is the parent's height. Absolute, not "144 back from the tip".
    let ancestor_height = parent_height.checked_sub(AVERAGING_WINDOW)?;
    let ancestor_index = ancestor_height.checked_sub(oldest_height)? as usize;
    let first_index = suitable_index(records, ancestor_index)?;

    let target = compute_target(
        records[first_index].chainwork,
        records[first_index].time,
        records[last_index].chainwork,
        records[last_index].time,
    );
    Some(if target > pow_limit { pow_limit } else { target })
}

/// `GetSuitableBlock` — the median of the three blocks ending at `index`, **by
/// timestamp**.
///
/// The tie handling matters and is deliberately stable: two blocks with the
/// same timestamp leave the sorted order as `[a, b, c]` and the median is `b`,
/// the newer of the two. The node sorts the same three pointers and takes
/// index 1, so this matches.
pub fn suitable_index(records: &[Record], index: usize) -> Option<usize> {
    if index >= records.len() {
        return None;
    }
    let lo = index.saturating_sub(2);
    let mut candidates: [usize; 3] = [lo, lo, lo];
    let mut n = 0usize;
    for i in lo..=index {
        candidates[n] = i;
        n += 1;
    }
    // Straight insertion sort over at most three elements: no allocation, which
    // matters because this runs inside the SBF heap-free path.
    for i in 1..n {
        let mut j = i;
        while j > 0 && records[candidates[j]].time < records[candidates[j - 1]].time {
            candidates.swap(j, j - 1);
            j -= 1;
        }
    }
    Some(candidates[n / 2])
}

/// One header's contribution to the DAA: what the client must keep per block.
///
/// `chainwork` is cumulative work from the checkpoint, not from genesis. The
/// node uses genesis-absolute work, but cw-144 only ever *subtracts* two
/// cumulative values, so any common offset cancels — and a checkpoint header
/// cannot supply the genesis total. See `push_header` for how each new record's
/// value is derived from its own `bits`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Record {
    pub time: u32,
    pub chainwork: u128,
}

/// How many blocks in [`LOOKBACK`] a caller must be able to reach back for.
/// Asserted at compile time against `WINDOW` in `lib.rs`.
pub const _LOOKBACK_CHECK: () = assert!(LOOKBACK < 200);
