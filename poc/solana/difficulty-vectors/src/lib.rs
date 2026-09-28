//! W1.6 acceptance test — replay **real BSV mainnet headers** through the
//! production difficulty rule.
//!
//! The module under test is `programs/solbeam/src/difficulty.rs`, pulled in
//! whole by `#[path]` below. It is not a copy: it is the same file the on-chain
//! program compiles, so a change that breaks the fixture breaks this test and a
//! change that makes this test pass makes the program agree with real BSV.
//!
//! What is being asserted:
//!
//! 1. **Exactness.** For every header in the fixture, the target the client
//!    derives from the preceding 144 blocks' chainwork and timestamps must
//!    compact to *that header's own `bits`*. Not a tolerance, not a fit: the
//!    real header's value.
//! 2. **The lookback height.** `GetAncestor(nHeight - 144)` is absolute, not
//!    "144 back from the tip". The off-by-one is asserted directly, because it
//!    is the mistake that produces zero matches rather than a near miss.
//! 3. **Compact round-trip.** `bits -> target -> bits` for every fixture header,
//!    which is what makes the comparison in (1) a comparison of the same thing.
//!
//! The window is modelled locally rather than pushed through a validator: the
//! algorithm needs 146 records of history, and a Solana transaction carries
//! twelve, so an on-chain replay of 471 mainnet headers is hundreds of
//! transactions to test arithmetic that needs no chain at all. The on-chain
//! tests cover the window and the instructions; this covers the rule.

// The production module, by path. Everything it needs is `uint` and core.
#[path = "../../programs/solbeam/src/difficulty.rs"]
mod difficulty;

use difficulty::{
    compact_to_target, next_target, target_to_compact, Record, U256, REGTEST_POW_LIMIT_BITS,
};

/// 471 contiguous mainnet headers, heights 968,230–968,700, each with its hash,
/// `bits`, `time` and cumulative `chainwork`. Fetched in W1.1.
const HEADERS_JSON: &str = include_str!("../../../../workstreams/data/headers_mainnet.json");

/// BSV mainnet's `powLimit`. The cap is applied by `next_target`; on these
/// heights it never binds, and the test asserts that separately so a future
/// fixture from a different part of the chain cannot silently start exercising
/// it without anyone noticing.
const POW_LIMIT: u32 = difficulty::MAINNET_POW_LIMIT_BITS;

#[derive(Clone, Copy, Debug)]
struct Header {
    height: u64,
    bits: u32,
    time: u32,
    chainwork: u128,
}

/// Minimal reader for this one flat fixture. Deliberately hand-rolled rather
/// than pulling in `serde_json`: the test crate's whole point is that it builds
/// with nothing but the arithmetic crate, on any machine, without a toolchain
/// the build box may not have.
struct Json<'a> {
    bytes: &'a [u8],
    at: usize,
}

/// One `{...}` object's fields, parsed leniently.
struct Obj {
    height: u64,
    bits: u32,
    time: u32,
    chainwork: u128,
}

impl<'a> Json<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Json { bytes, at: 0 }
    }

    fn skip_ws(&mut self) {
        while self.at < self.bytes.len() && (self.bytes[self.at] as char).is_whitespace() {
            self.at += 1;
        }
    }

    fn peek(&mut self) -> Option<u8> {
        self.skip_ws();
        self.bytes.get(self.at).copied()
    }

    fn take(&mut self, expected: u8) {
        self.skip_ws();
        assert_eq!(
            self.bytes.get(self.at).copied(),
            Some(expected),
            "fixture parse: expected {:?} at byte {}",
            expected as char,
            self.at
        );
        self.at += 1;
    }

    /// The next JSON string value.
    fn string(&mut self) -> String {
        self.take(b'"');
        let start = self.at;
        while self.bytes[self.at] != b'"' {
            self.at += 1;
        }
        let s = String::from_utf8_lossy(&self.bytes[start..self.at]).into_owned();
        self.at += 1;
        s
    }

    /// A string value as a `u128`, from hexadecimal. `chainwork` is 32 bytes of
    /// big-endian hex; only its low 16 bytes matter, which is exactly what the
    /// client stores (`u128`, and BSV's cumulative work is around 2^87, so the
    /// top half is always zero on any real chain).
    ///
    /// `u128::from_str_radix` is deliberately not used: it rejects the 32-digit
    /// string even though the value fits, because the string carries leading
    /// zeros the parser still counts. Leading zeros are skipped and at most 32
    /// significant digits are read.
    fn hex_u128(&mut self) -> u128 {
        let s = self.string();
        let digits = s.trim_start_matches('0');
        assert!(
            digits.len() <= 32,
            "chainwork has more than 128 significant bits: {s}"
        );
        let mut value: u128 = 0;
        for c in digits.chars() {
            value = value
                .checked_mul(16)
                .and_then(|v| v.checked_add(c.to_digit(16).expect("hex digit") as u128))
                .expect("chainwork overflows u128");
        }
        value
    }

    /// A string value as a `u32`, from hexadecimal — the `bits` field.
    fn hex_u32(&mut self) -> u32 {
        let s = self.string();
        u32::from_str_radix(&s, 16).expect("bits is 8 hex digits")
    }

    fn number(&mut self) -> u64 {
        self.skip_ws();
        let start = self.at;
        while self.at < self.bytes.len() && self.bytes[self.at].is_ascii_digit() {
            self.at += 1;
        }
        std::str::from_utf8(&self.bytes[start..self.at])
            .unwrap()
            .parse()
            .unwrap()
    }

    /// One header object. Fields are matched by key name, so the fixture can
    /// gain columns without this silently reading the wrong one — a positional
    /// parser here would be a way for the test to pass while parsing `time`
    /// where it meant `bits`.
    fn object(&mut self) -> Obj {
        self.take(b'{');
        let mut height = None;
        let mut bits = None;
        let mut time = None;
        let mut chainwork = None;
        loop {
            match self.peek() {
                Some(b'}') => {
                    self.at += 1;
                    break;
                }
                Some(b',') => {
                    self.at += 1;
                    continue;
                }
                Some(_) => {
                    let key = self.string();
                    self.take(b':');
                    match key.as_str() {
                        "height" => height = Some(self.number()),
                        "bits" => bits = Some(self.hex_u32()),
                        "time" => time = Some(self.number() as u32),
                        "chainwork" => chainwork = Some(self.hex_u128()),
                        // "hash" and "prev" are display strings this test does
                        // not need: the algorithm is a function of work and
                        // time, and linkage is asserted by the fixture's own
                        // construction (W1.1: 0 gaps in 471 headers).
                        _ => {
                            if self.peek() == Some(b'"') {
                                self.string();
                            } else {
                                self.number();
                            }
                        }
                    }
                }
                None => break,
            }
        }
        Obj {
            height: height.expect("height"),
            bits: bits.expect("bits"),
            time: time.expect("time"),
            chainwork: chainwork.expect("chainwork"),
        }
    }

    fn headers(&mut self) -> Vec<Header> {
        self.take(b'[');
        let mut out = Vec::new();
        loop {
            match self.peek() {
                Some(b']') => break,
                Some(b',') => {
                    self.at += 1;
                }
                Some(b'{') => {
                    let o = self.object();
                    out.push(Header {
                        height: o.height,
                        bits: o.bits,
                        time: o.time,
                        chainwork: o.chainwork,
                    });
                }
                other => panic!("fixture parse: unexpected {other:?} at {}", self.at),
            }
        }
        out
    }
}

fn fixture() -> Vec<Header> {
    let mut json = Json::new(HEADERS_JSON.as_bytes());
    let headers = json.headers();
    assert!(
        headers.len() > 400,
        "expected the full 471-header fixture, parsed {}",
        headers.len()
    );
    // Contiguity is the fixture's own precondition, and the local window model
    // below assumes it: the ancestor of the parent is found by height, so a gap
    // would make the model look up the wrong block rather than fail loudly.
    for pair in headers.windows(2) {
        assert_eq!(
            pair[1].height,
            pair[0].height + 1,
            "fixture is not contiguous at height {}",
            pair[1].height
        );
    }
    headers
}

/// The rule the client applies, verbatim: compute the target from the window,
/// then compare its compact form with the `bits` the header declares.
fn required_bits(headers: &[Header], i: usize) -> Option<u32> {
    required_bits_with(headers, i, LOOKBACK)
}

/// The number of records the algorithm needs. Note 147, not 146: see
/// [`difficulty::LOOKBACK`].
const LOOKBACK: u64 = difficulty::LOOKBACK;

/// The same rule with the lookback offset supplied, so the off-by-one can be
/// exercised against everything else held equal. `lookback` here is the
/// distance from the header being checked to the oldest *window* record.
fn required_bits_with(headers: &[Header], i: usize, lookback: u64) -> Option<u32> {
    let depth = lookback as usize;
    let start = i.checked_sub(depth)?;
    let records: Vec<Record> = headers[start..i]
        .iter()
        .map(|h| Record {
            time: h.time,
            chainwork: h.chainwork,
        })
        .collect();
    next_target(&records, headers[start].height, compact_to_target(POW_LIMIT)).map(target_to_compact)
}

#[test]
fn every_mainnet_header_is_predicted_exactly() {
    let headers = fixture();
    let mut checked = 0usize;
    let mut first: Option<usize> = None;
    let mut mismatches = Vec::new();

    for i in 1..headers.len() {
        let Some(required) = required_bits(&headers, i) else {
            continue;
        };
        if first.is_none() {
            first = Some(i);
        }
        checked += 1;
        if required != headers[i].bits {
            // Keep going: one mismatch is a bug, three hundred is a wrong idea,
            // and the difference is worth seeing in the output.
            mismatches.push((headers[i].height, headers[i].bits, required));
        }
    }

    assert!(
        first.is_some(),
        "no header had a full lookback window — the fixture is too short"
    );
    let first = first.unwrap();
    assert_eq!(
        headers[first].height, 968_377,
        "the first header with a full lookback window behind it should be 968,377"
    );
    assert!(
        mismatches.is_empty(),
        "{} of {} headers disagreed with cw-144; first: height {} declares {:08x}, \
         algorithm says {:08x}",
        mismatches.len(),
        checked,
        mismatches[0].0,
        mismatches[0].1,
        mismatches[0].2
    );
    // The number the workstream's independent Python verification produced:
    // 324/324 over heights 968,377..968,700. Asserted as a number rather than
    // only as "no mismatches", so a future fixture that silently shrank cannot
    // turn this into a much weaker test that still passes.
    assert_eq!(
        checked, 324,
        "expected 324 headers with a full lookback window, checked {checked}"
    );
    println!(
        "cw-144: {checked} of {checked} mainnet headers predicted exactly, \
         heights {}..{}",
        headers[first].height,
        headers[headers.len() - 1].height
    );
}

#[test]
fn the_lookback_height_is_absolute_not_relative_to_the_tip() {
    // `GetAncestor(nHeight - 144)` takes a HEIGHT, and the parent is at
    // `nHeight - 1`. Taking the ancestor 144 back from the parent instead of
    // 145 is an off-by-one block, and it reproduces nothing at all — which is
    // why it is worth an assertion rather than a comment. The earlier version of
    // this module had exactly that bug and matched 0 of 471 headers.
    let headers = fixture();

    let correct = headers
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(i, h)| required_bits(&headers, *i) == Some(h.bits))
        .count();

    // Same window width, same everything, except the oldest record the medians
    // can reach: the off-by-one passes a window one record shorter and so asks
    // the ancestor to be one block later.
    let off_by_one = headers
        .iter()
        .enumerate()
        .skip(1)
        .filter(|(i, h)| required_bits_with(&headers, *i, LOOKBACK - 1) == Some(h.bits))
        .count();

    assert!(
        correct > 300,
        "expected a large exact-match count, got {correct}"
    );
    assert_eq!(
        off_by_one, 0,
        "the off-by-one lookback matched {off_by_one} headers; it must match none, \
         or this test is not measuring what it claims"
    );
}

#[test]
fn compact_round_trips_every_fixture_header() {
    // If `bits -> target -> bits` failed, the exactness test above would be
    // comparing two different quantities and could pass or fail for the wrong
    // reason.
    let headers = fixture();
    for h in &headers {
        let target = compact_to_target(h.bits);
        assert_eq!(
            target_to_compact(target),
            h.bits,
            "compact round-trip failed at height {} for {:08x}",
            h.height,
            h.bits
        );
        assert!(!target.is_zero(), "height {} has a zero target", h.height);
    }
}

#[test]
fn pow_limit_never_binds_on_this_fixture() {
    // The cap is real code and is asserted in the module; this records that the
    // fixture does not depend on it, so a later fixture that *does* would be a
    // visible change rather than a silent one.
    let headers = fixture();
    for i in 1..headers.len() {
        let Some(start) = i.checked_sub(LOOKBACK as usize) else {
            continue;
        };
        let records: Vec<Record> = headers[start..i]
            .iter()
            .map(|h| Record {
                time: h.time,
                chainwork: h.chainwork,
            })
            .collect();
        let oldest_height = headers[start].height;
        let Some(raw) = next_target(&records, oldest_height, U256::max_value()) else {
            continue;
        };
        let capped = next_target(&records, oldest_height, compact_to_target(POW_LIMIT)).unwrap();
        assert_eq!(
            raw, capped,
            "the powLimit cap bound at height {}",
            headers[i].height
        );
    }
}

#[test]
fn single_header_trace_matches_the_reference() {
    // The whole-chain test says "everything disagrees", which is a statement
    // about a formula. This says which formula, with the intermediate values
    // that the independent Python verification produced for height 968,377 —
    // measured from the fixture, not copied from this implementation.
    let headers = fixture();
    let i = headers
        .iter()
        .position(|h| h.height == 968_377)
        .expect("fixture covers 968,377");
    let start = i - LOOKBACK as usize;
    let records: Vec<Record> = headers[start..i]
        .iter()
        .map(|h| Record {
            time: h.time,
            chainwork: h.chainwork,
        })
        .collect();
    let oldest_height = headers[start].height;

    // pindexLast = GetSuitableBlock(parent) = the block at 968,375;
    // pindexFirst = GetSuitableBlock(parent - 144) = the block at 968,231.
    let last = difficulty::suitable_index(&records, records.len() - 1).unwrap();
    let parent_height = oldest_height + records.len() as u64 - 1;
    let ancestor_index = (parent_height - 144 - oldest_height) as usize;
    let first = difficulty::suitable_index(&records, ancestor_index).unwrap();
    assert_eq!(oldest_height + last as u64, 968_375);
    assert_eq!(oldest_height + first as u64, 968_231);

    let work_delta = records[last].chainwork - records[first].chainwork;
    // The fixture's chainwork column is 32 bytes, but only the low 16 matter and
    // the difference between two cumulative values is what the algorithm uses.
    // Independently derivable: `sum over the range of 2^256 / (target(bits)+1)`.
    assert_eq!(work_delta, 19_799_344_252_901_577_219_061);
    assert_eq!(records[last].time - records[first].time, 90_211);

    println!("TRACE fw={} ft={} lw={} lt={}", records[first].chainwork, records[first].time, records[last].chainwork, records[last].time);
    let target = difficulty::compute_target(
        records[first].chainwork,
        records[first].time,
        records[last].chainwork,
        records[last].time,
    );
    println!("TRACE target={:x}", target);
    println!("TRACE scaled({:x})", (U256::from(work_delta) * U256::from(600u64)) / U256::from(90_211u64));
    // Python's independent implementation of the same formula for this height:
    //   work   = 19,799,344,252,901,577,219,061 * 600 / 90,211
    //          = 131,686,895,741,549,770,332
    //   target = (2^256 - work) / work
    //          = 0x23dc4d << 192
    let mut expected_be = [0u8; 32];
    expected_be[29..32].copy_from_slice(&[0x23, 0xdc, 0x4d]);
    assert_eq!(
        target,
        U256::from_big_endian(&expected_be) << 192,
        "the target computed for height 968,377 must be the reference target"
    );
    assert_eq!(target_to_compact(target), 0x1823_dc4d);
}

#[test]
fn regtest_is_recognised_as_a_no_retarget_chain() {
    // `initialize` sets `no_retargeting` when the checkpoint's compact target is
    // the maximum the encoding can express. That value is the node's own
    // `UintToArith256(params.powLimit).GetCompact()` for regtest; if it were
    // wrong, every regtest header after the checkpoint would be checked against
    // cw-144 and rejected, and the on-chain suite would fail rather than this
    // test.
    let limit = compact_to_target(REGTEST_POW_LIMIT_BITS);
    assert_eq!(target_to_compact(limit), REGTEST_POW_LIMIT_BITS);
    // The node's regtest `powLimit` is `7fff...ff`, i.e. mantissa `0x7fffff` at
    // size 32. It is the *largest* encodable because the compact encoding treats
    // the top mantissa bit as a sign bit: pushing the mantissa any higher sets
    // `0x00800000`, and `GetCompact` then shifts it down and grows the size
    // instead of producing a bigger number. So this is a ceiling on easiness —
    // which is exactly why no chain anchored at a harder target can acquire the
    // no-retarget flag by accident.
    assert_eq!(limit, U256::from(0x7f_ffffu64) << 232);
    assert_eq!(target_to_compact(U256::from(0x80_0000u64) << 232), 0x2100_8000u32,
        "a larger mantissa is not encodable at size 32; it grows the size instead");
}
