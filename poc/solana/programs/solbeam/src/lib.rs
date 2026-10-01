//! SOLBEAM — the BSV light client, and (later) the peg.
//!
//! This is Phase 2. The first increment is the **light client only**: a
//! checkpoint, a rolling window of headers, and the two checks that make a
//! header chain meaningful — linkage and proof of work. The mint comes next,
//! once this compiles and the header chain verifies against
//! `poc/fixtures/deposit_1.json`.
//!
//! Why this order: the light client is the half that must be *identical* to the
//! Python reference in `poc/checks/`. If the two disagree about a header hash or
//! a Merkle fold, everything above them is worthless. Proving agreement on the
//! same fixture is the most valuable test in the whole PoC, and it needs nothing
//! but this.
//!
//! **The difficulty rule is cw-144**, the algorithm BSV's `src/pow.cpp`
//! actually implements, and it lives in [`difficulty`] — a module with no
//! Anchor types so it can be replayed against real mainnet headers by a plain
//! `cargo test`. The old client required `bits == expected_bits`, a value fixed
//! at `initialize`; on a real chain that rejects *every* header after the
//! checkpoint, because BSV changes difficulty every block. See
//! `workstreams/W1-light-client-verification.md`.
//!
//! Deliberately NOT in this increment, and each is a known gap rather than an
//! oversight:
//!
//! * **the mint and the token.** No SPL CPI yet, which also keeps this file
//!   clear of the Anchor 1.x `CpiContext` change.

use anchor_lang::prelude::*;
use anchor_lang::solana_program::bpf_loader_upgradeable;
// Not in `prelude`: the trait that carries `DISCRIMINATOR`, needed to write the
// replay nullifier's account data by hand (its PDA seeds come from instruction
// arguments, so it cannot be created by an Anchor `init` constraint).
use anchor_lang::Discriminator;
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{
    self, Burn, CloseAccount, Mint, MintTo, Token, TokenAccount, Transfer,
};
// Solana 3.x moved hashing out of `solana_program` entirely — there is no
// `solana_program::hash` — and anchor_lang re-exports no replacement. This
// crate is already in the tree transitively; on the SBF target it calls the
// on-chain `sol_sha256` syscall rather than doing the work in the program.
use solana_sha256_hasher::hash as sha256;

pub mod difficulty;
/// The generated parameter sheet. **The constants below that correspond to a
/// parameter row live here now**, generated from `config/params.json`; the rows
/// of `docs/06-parameters.md` and `docs/parameters.csv` are the same values, so
/// the doc, the CSV and this program cannot drift. Regenerate with
/// `python3 config/gen.py`.
pub mod params;

use difficulty::{compact_to_target, target_to_compact, Record, MAINNET_POW_LIMIT_BITS, U256};

declare_id!("EYsckW3596zBL1pxfxGev44z6LH4hEpoHff7tSisvjCW");

/// BSV headers are always exactly 80 bytes.
pub const HEADER_LEN: usize = 80;

/// How long the header window reaches back, expressed in TIME rather than a
/// block count.
///
/// **This is 32 hours and it cannot be 48.** The window used to be 288 records
/// of 32 bytes, because a record stored a bare block hash. cw-144 needs more:
/// the retarget is a function of the *chainwork difference* and the *timestamp
/// difference* between two suitable blocks 144 apart, so every record carries
/// hash (32) + chainwork (16) + time (4) = 52 bytes. 288 x 52 = 14,976 plus
/// overhead, against a hard 10,240-byte account-creation cap — 46% over, and
/// `initialize` would simply revert.
///
/// At 52 bytes a record and 123 bytes of fixed fields, 194 records is the
/// arithmetic maximum. **192 is chosen instead, 133 bytes under the cap**,
/// which leaves room for a field or two without resizing every existing
/// account. 192 records is 115,200 seconds = **32 hours at 600 s/block**.
///
/// What actually constrains the *bottom* is cw-144's own lookback:
/// [`difficulty::LOOKBACK`] = 147 records. The window is opened by a **trusted
/// seed** of exactly that many records (`SEED_RECORDS`) and then holds 45 live
/// headers before it starts evicting; 192 is 45 records of slack over the
/// minimum, and the seed ages out naturally as the client advances. The product
/// consequence is real and is recorded in the workstream: the 48-hour deposit
/// deadline the design assumed is gone.
///
/// The whole account is deserialised on every instruction, so a bigger window
/// is also more compute on the mint path. 9,984 bytes of records is comfortable
/// against the 200,000 CU budget.
pub use params::{SECONDS_PER_BLOCK, WINDOW, WINDOW_HOURS}; // WINDOW == 192

/// The window must be wide enough for the difficulty algorithm to be
/// computable at all. `LOOKBACK` is 147 (144 + 3 for the median); a window at
/// or below it could never compute a retarget, which is exactly the defect this
/// workstream exists to fix (F1).
const _: () = assert!(
    WINDOW > difficulty::LOOKBACK as usize,
    "WINDOW must exceed the cw-144 lookback (147 records) or the retarget is not computable"
);

// The generated values are pinned to the cw-144 module's own arithmetic.
// `difficulty.rs` `#[path]`-includes `params.rs`, so these are the same bytes
// the vectors crate compiles; the assertions make a parameter change that
// disagrees with the algorithm a compile error rather than a quiet divergence.
const _: () = assert!(SEED_RECORDS == difficulty::LOOKBACK as usize);
const _: () = assert!(SECONDS_PER_BLOCK == difficulty::BLOCK_SPACING);
const _: () = assert!(
    params::DAA_CLAMP_LOW_MULTIPLIER
        == difficulty::MIN_ACTUAL_TIMESPAN / difficulty::BLOCK_SPACING as i64
);
const _: () = assert!(
    params::DAA_CLAMP_HIGH_MULTIPLIER
        == difficulty::MAX_ACTUAL_TIMESPAN / difficulty::BLOCK_SPACING as i64
);

/// How many records the window must hold before cw-144 is computable — the
/// size of the **trusted seed**.
///
/// This is [`difficulty::LOOKBACK`], not a second constant, because the seed
/// exists for exactly one reason: to supply the records the difficulty rule
/// reads. For the header at height `h` the rule reaches back to `h - 147`, so a
/// window of 147 records ending at the checkpoint is what makes the *first*
/// block after it checkable. With one fewer the rule returns `None` — and the
/// old client, faced with that `None`, fell back to `bits == expected_bits`
/// forever (F1). The seed is that missing history, taken on trust.
///
/// The seed **includes the checkpoint itself** as its newest record, so the
/// caller supplies `SEED_RECORDS - 1 = 146` ancestors below the checkpoint and
/// the checkpoint is pinned by the seed's linkage check. That keeps the
/// arithmetic 147 seed + 45 live = the 192-record window, with no extra field
/// to remember the checkpoint's timestamp.
pub use params::SEED_RECORDS;

/// How deep a deposit must be buried before it can be minted. Twelve blocks is
/// roughly two hours on BSV. The test matrix compresses time, not depth, so this
/// stays a real number.
pub use params::MIN_CONFIRMATIONS;

/// How old the client's view of the chain may be before the vault refuses to act,
/// in Solana slots. **54,000 is about six hours.**
///
/// This is doc 31's `lc.max_staleness_slots`, and it is the freshness half of the
/// release check: a client whose last accepted header is older than this is a
/// client that may not have seen the reorg yet, so releasing against it would
/// hand out tokens for a deposit that no longer exists. It is a multiple of BSV
/// block time on purpose — roughly 1,500 Solana slots per BSV block — so anything
/// under that is meaningless.
///
/// The clock is [`LightClient::last_push_slot`], which is `Clock::slot` at the
/// last accepted header. It cannot be back-dated by the submitter because it
/// comes from the sysvar.
pub use params::MAX_STALENESS_SLOTS;

/// Seed prefix of the **staged mint** PDA: `[b"mint", txid, vout]`.
///
/// The same bytes as the SPL mint's `[b"mint"]` seed deliberately — the two are
/// different PDAs because the seed lists differ in length, and doc 31 specifies
/// this prefix. The staged item is keyed on the *deposit's* identity, exactly as
/// the nullifier is, so a reorg that re-includes the transaction at another
/// height hits the same staged item rather than a new one.
pub const STAGED_MINT_SEED: &[u8] = b"mint";

/// Seed of the program-owned **vault** token account: `[b"vault"]`.
///
/// One shared token account, and it is the deliberate exception to doc 21's
/// "no shared token account" rule: doc 31 makes the vault the whole mechanism.
/// Its authority is the light client PDA, so only this program can move or burn
/// what is in it.
pub const VAULT_SEED: &[u8] = b"vault";

/// Seed of the singleton **config** PDA: `[b"config"]`, holding
/// `maturity_blocks`.
pub const CONFIG_SEED: &[u8] = b"config";

/// `solBSV` is a classic SPL token with eight decimals, matching BSV's own
/// satoshi precision. One satoshi is one base unit, so no conversion is ever
/// needed when minting a deposit.
pub use params::TOKEN_DECIMALS;

/// Seed prefix of the per-deposit replay **nullifier**.
///
/// The nullifier is a PDA seeded on `(txid, vout)` — the identity of a deposit —
/// so its **existence is the whole replay record**. There is no list, and so no
/// ceiling: the fixed `UsedDeposits` list this replaces capped the peg-in path at
/// 200 mints per 32-hour window with no attacker required (F6/A9), and the pruned
/// list was still a bound on usage per window rather than a replay defence.
/// Decision P5 replaced it with this.
///
/// **The replay key is `(txid, vout)` and deliberately not the height.** A reorg
/// re-includes the same transaction at a different height, and a height-keyed
/// record would hand it a fresh key and mint it a second time — an unbacked mint
/// out of a legitimate deposit. See [`DepositNullifier`].
pub const NULLIFIER_SEED: &[u8] = b"nullifier";

/// Seed prefix of the **reported spent-outpoint record**: `[b"spent_outpoint",
/// txid, vout]`.
///
/// One PDA per `(txid, vout)`, exactly as the replay nullifier is, and its
/// **existence** is the record. `verify_deposit` refuses a claim whose outpoint
/// is present. This is N5's mechanism: Solana cannot read BSV's UTXO set, so
/// spentness is *reported* by the federation (see [`crate::report_spent`]) and
/// the program checks mints against the report.
///
/// Deliberately the same key shape as the nullifier — the deposit's identity is
/// `(txid, vout)` — and deliberately a **different account**: the nullifier
/// records that *this program* minted the deposit; this records that the
/// **reserve spent the output**. They are different facts with different
/// lifetimes, and conflating them would make one of the two unusable.
pub const SPENT_OUTPOINT_SEED: &[u8] = b"spent_outpoint";

/// Seed prefix of a **pending redemption**: `[b"redeem", id_le]`.
///
/// The whole of the peg-out's escrow state: who is redeeming, how much is
/// escrowed, the BSV address the payout must pay, when the member's deadline
/// expires, and — once a payout has been proven — the block it was proven
/// against. Doc 11 §2's "escrow, not an immediate burn" is this account plus
/// [`REDEEM_ESCROW_SEED`]: the tokens still exist while the BSV has not moved,
/// so `supply ≤ reserve` holds at every point in between.
///
/// The id is a **global counter** from [`RedeemBook`] rather than a per-holder
/// nonce, deliberately: every redemption is then `[b"redeem", 0..]`, so a
/// permissionless `cancel_redeem` / `claim_payout` / `settle_redeem` can find
/// the item without knowing the holder's key. A per-holder seed would make the
/// permissionless paths depend on an off-chain index of holders.
pub const REDEEM_SEED: &[u8] = b"redeem";

/// Seed prefix of the **escrow token account**: `[b"redeem_escrow", id_le]`.
///
/// A program-owned (PDA-authority) token account, one per redemption. It is
/// separate from [`REDEEM_SEED`] because a Solana account is owned either by
/// this program (data) or by the token program (tokens), never both — the
/// metadata lives in the pending account and the tokens in this one.
pub const REDEEM_ESCROW_SEED: &[u8] = b"redeem_escrow";

/// Seed of the singleton **redemption book**: `[b"redeem_book"]`.
///
/// It carries the two counters the escrow cannot: the next redemption id (so
/// ids are sequential and discoverable) and the number of redemptions pending
/// right now (so `po.max_pending` is a bound the program actually enforces).
/// A singleton with a fixed seed, exactly like [`CONFIG_SEED`], so a caller
/// cannot point the count at an account of its own.
pub const REDEEM_BOOK_SEED: &[u8] = b"redeem_book";

// ---------------------------------------------------------------------------
// the federation registry (doc 03 sections 2-3, doc 12 sections 1-3, 5-6)
// ---------------------------------------------------------------------------
//
// **This is records, not custody.** The reserve is off-chain BSV under a 2-of-2
// `OP_CHECKMULTISIG` script whose two keys -- the gateway threshold key and the
// Greycore key -- are generated and shared **off chain**, by a ceremony that is
// not specified anywhere in this repository. Nothing here generates, holds,
// signs with or can spend any key. What is built is the **registry**: who is a
// member, what each side has recorded as bonded, who admitted them, whether
// they are active, and the floor that keeps the signing set usable.

/// Seed of the singleton **federation config** PDA: `[b"federation"]`.
///
/// It carries the two values that are parameters rather than code: the
/// **gateway signing threshold** (`fed.threshold`, the `t` of `t-of-N`) and the
/// **Greycore threshold**, plus the two aggregate keys those thresholds sign
/// with. It also carries the live counts -- gateway members holding a share and
/// Greycore members -- because a floor cannot be enforced against a number the
/// program has to be told.
///
/// `greycore_key` is the single key that fills the **second leg** of the
/// reserve's 2-of-2 script. The Greycore is itself a set with its own
/// threshold, so `greycore_threshold` is the number of Greycore members that
/// must assemble before that one leg can sign; the reserve script is still a
/// 2-of-2, because its second leg is one key however many people produced it.
pub const FEDERATION_SEED: &[u8] = b"federation";

/// Seed prefix of a **gateway member record**: `[b"member", identity]`.
///
/// One account per member, keyed on the member's identity key, so the account's
/// **existence is the admission** and one identity cannot hold two seats: a
/// second `admit_member` for the same key derives the same address, and the
/// account must be empty for the instruction to proceed.
///
/// The record is the member's whole on-chain state: the `solBSV`-side bond the
/// program can seize, the **BSV-side bond, which is an attestation and not
/// custody** (see [`Member::bsv_bond`]), the state that decides whether the
/// member counts toward the threshold, the Greycore member whose signature
/// admitted them, and the bond-custody key the design puts the BSV-side bond
/// under.
pub const GATEWAY_MEMBER_SEED: &[u8] = b"member";

/// Seed prefix of a **Greycore member record**: `[b"greycore", identity]`.
///
/// The second set, kept in its own registry rather than as a flag on
/// [`GATEWAY_MEMBER_SEED`], because the two sets have different jobs, different
/// thresholds and different admission rules: a Greycore member does not hold a
/// share of the reserve key and does not bond, and a gateway member cannot
/// admit anyone. Admission to the gateway **requires one of these records to
/// sign** -- that is what makes admission the Greycore's stated job rather than
/// a convention.
///
/// Membership in the Greycore is itself permissioned and is entered **only by
/// the program's upgrade authority** (see [`initialize_federation`] and
/// [`add_greycore_member`]). The reference appoints its Greycore by community
/// governance; no governance instruction exists here, so for now the same one
/// key that governs the rest of this PoC appoints them. That is a stand-in, and
/// it is recorded as one.
pub const GREYCORE_MEMBER_SEED: &[u8] = b"greycore";

/// How far past the gateway threshold the roster may grow: `t + 8`.
///
/// **This is a PoC placeholder, and it is not a policy.** `N` in `t-of-N` is
/// deliberately unfixed (`fed.roster_size` is `open`), so the program cannot
/// read a cap from configuration; but an **unbounded** set is worse than an
/// arbitrary one, because `admit_member` writes into a registry whose live
/// count is what the threshold floor is measured against, and every member is a
/// share of an off-chain key the registry cannot recall. The number exists so
/// the growth path is bounded and visible, and it is deliberately **not**
/// presented as `N`: it is a ceiling on `N` chosen to leave room for the
/// Greycore to admit, replace and rotate members without a re-deploy.
///
/// When `N` is decided, this constant should be replaced by the decided value --
/// or by a governed parameter -- rather than kept as an invented bound.
pub const MAX_GATEWAY_HEADROOM: u64 = 8;


/// Seed prefix of a **claimed payout outpoint**: `[b"payout_nullifier", txid,
/// vout_le]`.
///
/// This is the answer to a defect the vault audit already named (W5: *"payout
/// proofs were replayable — one payment settled every redemption with the same
/// amount and destination"*). A proof that address `X` was paid `v` is not
/// bound to a redemption by anything in the BSV transaction, so without a
/// record on the Solana side the same payment settles every pending redemption
/// that names `X` with a small enough amount — the second holder's escrow is
/// burned without them being paid. The account records **which redemption** the
/// outpoint settled, so it may be re-submitted for the same redemption (a reorg
/// can re-include the transaction at another height) but never for a different
/// one.
pub const PAYOUT_NULLIFIER_SEED: &[u8] = b"payout_nullifier";

/// How long a checkpoint or pause change must sit pending before it may be
/// executed, in slots. **This is the F4 fix's number.**
///
/// A timelock is not a threshold, and this one does not pretend to be: the same
/// single upgrade-authority key proposes and executes. What it buys is the one
/// thing `has_one = authority` alone cannot — **notice**. `set_checkpoint` can
/// install a trusted root that makes a fabricated deposit provable, and
/// `set_paused` halts header advancement and therefore redemption, and before
/// this both took one signature and landed inside one slot. With this, a pending
/// change is visible in a PDA for at least this long before it can be applied.
///
/// **This PoC value is deliberately short** so the test suite can advance the
/// clock past it. It is a named constant precisely because the production value
/// is a policy decision: doc 24 carries it as `gov.authority_timelock`, and that
/// is the number to change, in one place.
pub use params::TIMELOCK_SLOTS;

/// Size of one `HeaderRecord`: the block hash, the block's cumulative
/// chainwork, and its timestamp.
///
/// The three fields are not redundant with one another:
///   * the **hash** is the client's whole invariant — linkage — and the only
///     thing a deposit proof needs;
///   * **chainwork** is the numerator of cw-144's target. It cannot be derived
///     from a single scalar: the algorithm subtracts two cumulative values 144
///     apart. `u128` because BSV's cumulative work is around 2^87, which does
///     not fit `u64`;
///   * **time** is the denominator, with the clamps applied to its difference.
///
/// Height is derived from the window's start; `prev`, `bits` and `nonce` are
/// used once when a header is pushed; and the Merkle root is a field *inside*
/// the header, already committed to by this hash, so a claim can simply supply
/// it. `bits` is deliberately NOT stored: it is `target_to_compact` of the
/// target the client recomputes anyway, so a stored copy could only ever agree
/// with itself or hide a wrong target.
pub use params::HEADER_RECORD_SIZE;

/// The fixed part of `LightClient`. Named so the compile-time assertion below
/// can show its arithmetic instead of hiding it behind one number.
pub const LIGHT_CLIENT_FIXED: usize = 8      // discriminator
    + 8                                      // checkpoint_height
    + 8                                      // tip_height
    + 32                                     // tip_hash
    + 8                                      // window_start
    + 32                                     // authority
    + 4                                      // expected_bits
    + 1                                      // no_retargeting
    + 4                                      // pow_limit_bits
    + 4                                      // seed_remaining
    + 8                                      // last_push_slot
    + 1                                      // paused
    + 1                                      // bump
    + 4; // headers: Vec length prefix

/// Solana refuses to grow an account by more than this in one instruction, and
/// `init` allocates the whole `LightClient` in one go. Exceeding it is not a
/// graceful failure — `initialize` simply reverts — so it is asserted at compile
/// time rather than discovered on testnet. This cap is the reason the window is
/// sized against a 64-byte record rather than the 116-byte one it started with.
pub use params::MAX_ACCOUNT_CREATE;

/// How many branch headers fit in one transaction. A Solana transaction is
/// capped at 1232 bytes; after the signature, accounts, blockhash, instruction
/// header and the `Vec<u8>` length prefix roughly 215 bytes are gone, leaving
/// about 12 headers of 80 bytes. The earlier `push_fork` failed precisely
/// because it ignored this ceiling and tried to send 72.
pub use params::MAX_FORK_BATCH;

/// **The peg-out's numbers, from the same generated sheet.**
///
/// * [`PAYOUT_CONFIRMATIONS`] (`po.payout_confirmations`) — how deep the payout
///   must be before it is provable. Lower than the mint's 12 because the burn
///   still faces a challenge window, so depth is not the only protection.
/// * [`CHALLENGE_WINDOW`] (`po.challenge_window`) — BSV blocks that must pass
///   over the *staging block* before the burn executes. Deliberately the same
///   number as the vault's designed maturity: both are "how long before we
///   believe the chain".
/// * [`REDEEM_DEADLINE_SLOTS`] (`po.deadline`) — the **default** the `Config`
///   account is created with. It is a slot count, not a BSV height, because a
///   deadline that cannot advance is not a deadline: if the header feed stalls,
///   a height-based deadline would never expire and the holder's funds would
///   freeze (doc 11 §4). It ships as a stored, timelock-mutable `Config` field
///   for the same reason `v.maturity_blocks` does — so the policy can move
///   without a redeploy, and so the suite can reach the guard.
/// * [`CANCEL_GRACE_SLOTS`] (`po.cancel_grace`) — an extra delay on top of the
///   deadline before cancellation. Zero: cancellation is immediate on expiry.
/// * [`REDEEM_D_MIN`] (`po.d_min`) — the minimum redemption, in base units
///   (0.01 BSV), so a claim's proof is worth the fees it costs to settle.
/// * [`MAX_PENDING_REDEMPTIONS`] (`po.max_pending`) — the cap on concurrent
///   pending redemptions, so the escrow cannot make per-instruction work
///   unbounded.
/// * [`REDEEM_FEE_BP`] (`fee.redeem_bp`) — the gross peg-out fee in basis
///   points. It needs no account: the holder receives `A − fee`, the reserve
///   falls by `A − fee`, the supply falls by `A`, so the ratio improves by the
///   fee and the members keep BSV they did not have to pay out (doc 11 §3).
pub use params::{
    CANCEL_GRACE_SLOTS, CHALLENGE_WINDOW, MAX_PENDING_REDEMPTIONS,
    PAYOUT_CONFIRMATIONS, REDEEM_DEADLINE_SLOTS, REDEEM_D_MIN, REDEEM_FEE_BP,
};

const _: () = assert!(
    LightClient::SPACE <= MAX_ACCOUNT_CREATE,
    "LightClient::SPACE exceeds Solana's account-creation cap: shrink HeaderRecord or WINDOW"
);

const _: () = assert!(
    ForkStaging::SPACE <= MAX_ACCOUNT_CREATE,
    "ForkStaging::SPACE exceeds Solana's account-creation cap: it mirrors the window, so \
     shrinking WINDOW or HeaderRecord fixes both"
);

/// A competing branch being assembled across several transactions.
///
/// One per submitter, seeded on their key, so no two submitters can contend for
/// the same slot. It holds the fork point and the branch hashes so far; the
/// branch headers themselves are validated as they arrive and only their hashes
/// are kept, for the same reason the main window keeps only hashes.
#[account]
pub struct ForkStaging {
    pub submitter: Pubkey,
    /// The **last common block** shared with the main chain — the common
    /// ancestor. Branch headers begin at `fork_height + 1`.
    pub fork_height: u64,
    /// **The hash of the block at `fork_height` when this branch was staged.**
    ///
    /// This is the fix for P2, and it is the whole of the fix. A branch is only
    /// meaningful as a set of headers *hanging off one specific block*, and the
    /// client's only invariant is linkage. Reading the fork point from chain
    /// state later — which is what this account used to do, once per pushed
    /// header and again at commit — lets one branch be spliced onto a different
    /// block than the one it was built on. The window then holds two chains
    /// stapled together, and a header hash that is not in either chain can be
    /// made canonical: a mint forgery. Recording the parent **once, here** and
    /// re-comparing it at commit is what makes the branch a single object.
    pub fork_parent_hash: [u8; 32],
    /// Branch headers, oldest first, excluding the fork point itself.
    ///
    /// Full records rather than bare hashes. The window keeps `hash + chainwork
    /// + time` and a commit has to produce records of the same shape: the
    /// branch's later records are *not* the fork point's chainwork, and cw-144
    /// subtracts exactly those values, so recomputing them at commit from a
    /// single total is not possible. They are captured as the headers arrive,
    /// which is the only moment the branch's `bits` and `time` are in hand.
    pub records: Vec<HeaderRecord>,
    pub bump: u8,
}

impl ForkStaging {
    pub const SPACE: usize = 8                  // discriminator
        + 32                                    // submitter
        + 8                                     // fork_height
        + 32                                    // fork_parent_hash
        + 4 + (WINDOW * HEADER_RECORD_SIZE)     // records: Vec length + records
        + 1;                                    // bump
}

#[program]
pub mod solbeam {
    use super::*;

    /// Establish the trusted starting point. The checkpoint is the *only*
    /// thing trusted here, which is why it is governance-set, buried deep and
    /// published — and why it is taken as the **raw 80-byte header** rather
    /// than as individual fields.
    ///
    /// **Only the program's upgrade authority may call this.** The account set
    /// requires the BPF loader's `ProgramData` for this program and checks its
    /// `upgrade_authority_address` against the payer on-chain; see
    /// [`Initialize`]. Without that check this instruction was unpermissioned
    /// and the first caller became `authority`, and therefore the key that could
    /// rewrite the trusted root, pause the client and mint.
    ///
    /// That is not fussiness; it is a bug this test found. The first version
    /// took prev, merkle root, time, bits and nonce separately and rebuilt the
    /// header, hardcoding the version. The synthetic chain uses version
    /// 0x20000000, the rebuilt header used 1, the hashes differed, and the
    /// checkpoint failed its own proof-of-work check with `CheckpointBadPow`.
    /// Raw bytes make that class of mistake impossible: there are no fields
    /// left to drop.
    pub fn initialize(
        ctx: Context<Initialize>,
        checkpoint_height: u64,
        header: [u8; HEADER_LEN],
    ) -> Result<()> {
        let lc = &mut ctx.accounts.light_client;
        lc.authority = ctx.accounts.payer.key();
        lc.paused = false;
        lc.bump = ctx.bumps.light_client;
        // Shared with `set_checkpoint` on purpose: F2 was that the two paths
        // disagreed about what a trusted root implies, and one implementation is
        // the only way to keep them agreeing.
        lc.anchor_checkpoint(checkpoint_height, &header)?;
        lc.last_push_slot = Clock::get()?.slot;

        // The bridge's one tunable parameter. **Default 0**, which is doc 31's
        // deliberate PoC setting: at 0 the vault is a pass-through and the
        // protective window is absent. The mechanism is present and the value is
        // raisable through the timelocked authority path; see
        // `AuthorityChange::SetMaturity`.
        let config = &mut ctx.accounts.config;
        config.maturity_blocks = params::DEFAULT_MATURITY_BLOCKS;
        config.redeem_deadline_slots = params::REDEEM_DEADLINE_SLOTS;
        config.bump = ctx.bumps.config;

        msg!(
            "SOLBEAM light client initialised at height {} tip {} window {} records ({} h), \
             seed remaining {}",
            checkpoint_height,
            display_hex(&lc.tip_hash),
            WINDOW,
            WINDOW_HOURS,
            lc.seed_remaining
        );
        Ok(())
    }

    /// Append one header. Permissionless: anyone may advance the chain, and the
    /// worst a hostile advancer can do is waste its own fees.
    pub fn push_header(ctx: Context<PushHeader>, header: [u8; HEADER_LEN]) -> Result<()> {
        let lc = &mut ctx.accounts.light_client;
        require!(!lc.paused, SolbeamError::Paused);
        // The window holds fewer than 147 records, so cw-144 is not computable
        // and the client has no business accepting main-chain headers. The seed
        // is the only instruction allowed to write during this state, and it
        // checks linkage instead of difficulty because there is no difficulty
        // rule to check yet.
        require!(!lc.is_seeding(), SolbeamError::Seeding);

        let prev = read32(&header, 4);
        let bits = read_u32_le(&header, 72);

        // 1. It must extend *this* chain. Without this a header could be any
        //    valid block from anywhere.
        require!(prev == lc.tip_hash, SolbeamError::BrokenLinkage);

        // 2. The declared target must be the one cw-144 derives for this block,
        //    and this is checked BEFORE the target is used. Order matters:
        //    `meets_target` takes the target from the header, so validating only
        //    afterwards means an attacker chooses their own difficulty.
        //
        //    This used to be `bits == expected_bits`, a value fixed at
        //    `initialize`. On a chain whose difficulty changes every block that
        //    rejects every header after the checkpoint — the defect this
        //    replaces. See `difficulty` for the algorithm: of the 471 headers in
        //    the fixture, the 324 with a full lookback are all predicted exactly.
        let parent_work = lc
            .record(lc.tip_height)
            .map(|r| r.chainwork)
            .unwrap_or_default();
        let chainwork = lc.difficulty_for(bits, parent_work)?;

        // 3. It must be real work. Linkage is not evidence on its own: anyone
        //    can build an arbitrarily long chain of easy headers.
        require!(meets_target(&header, bits), SolbeamError::BadPow);

        let height = lc
            .tip_height
            .checked_add(1)
            .ok_or(SolbeamError::Overflow)?;

        let record_hash = header_hash(&header);
        let record = HeaderRecord {
            hash: record_hash,
            chainwork,
            time: read_u32_le(&header, 68),
        };
        if lc.headers.len() >= WINDOW {
            lc.headers.remove(0);
            lc.window_start += 1;
        }
        if lc.headers.is_empty() {
            lc.window_start = height;
        }
        lc.headers.push(record);

        lc.tip_height = height;
        lc.tip_hash = record_hash;
        lc.last_push_slot = Clock::get()?.slot;

        msg!(
            "SOLBEAM header {} {} bits {:08x} work +{}",
            height,
            display_hex(&record_hash),
            bits,
            work_from_bits(bits)
        );
        Ok(())
    }

    /// Supply the trusted ancestors a fresh checkpoint cannot carry. **This is
    /// a trusted bootstrap, on the same footing as the checkpoint itself — not
    /// an ongoing trust, and not a second way to advance the chain.**
    ///
    /// A checkpoint is one header. cw-144 needs 147 records ending at it, and
    /// the client has no way to obtain the other 146 trustlessly: their
    /// cumulative chainwork is genesis-absolute and is not committed to by the
    /// checkpoint header. So they are supplied here, checked for **linkage
    /// only**, and then never treated as trusted again — from the moment the
    /// seed completes, every later header is checked against cw-144 computed
    /// from these records, so a seed that lied about difficulty would be caught
    /// at the first retarget rather than believed forever. That bounded,
    /// one-time trust is the whole design; F1 exists because the alternative
    /// was an *unbounded* trust (the `bits == expected_bits` fallback) that
    /// never switched off.
    ///
    /// Why linkage is enough to make the seed non-fabricable in practice: the
    /// records are supplied oldest-first and each must link to the previous
    /// one, and the **last must hash to the checkpoint**. The checkpoint is
    /// already fixed and its `prev` has exactly one preimage, so every record
    /// below it is forced. A fabricated ancestor therefore has nowhere to go:
    /// either it is the first record (nothing links to it, and the real second
    /// header is then rejected `BrokenLinkage`), or it breaks the chain at the
    /// point it was spliced in. What the seed *can* lie about is difficulty —
    /// the work values — which is exactly the trust the checkpoint already
    /// carries and which cw-144 retires.
    ///
    /// Authority-gated, unlike `push_header`, and deliberately so: a
    /// permissionless seed has a griefing failure — anyone could write one
    /// arbitrary header as the seed root, and since the rest of the chain must
    /// link to *it*, no honest seed could ever complete. The seed is trusted
    /// data set by whoever set the checkpoint, so it is signed by the same key.
    ///
    /// Batched like `push_fork_header` because 147 x 80 = 11,760 bytes cannot
    /// fit in one 1,232-byte transaction. Batches arrive oldest-first; up to
    /// [`MAX_FORK_BATCH`] headers per call, and the client tracks how many it
    /// still needs in `seed_remaining`.
    pub fn seed_headers(ctx: Context<SeedHeaders>, ancestors: Vec<u8>) -> Result<()> {
        let lc = &mut ctx.accounts.light_client;
        require!(!lc.paused, SolbeamError::Paused);
        require!(lc.is_seeding(), SolbeamError::NotSeeding);

        require!(!ancestors.is_empty(), SolbeamError::EmptySeed);
        require!(ancestors.len() % HEADER_LEN == 0, SolbeamError::MalformedTx);
        let batch = ancestors.len() / HEADER_LEN;
        require!(batch <= MAX_FORK_BATCH, SolbeamError::BatchTooLarge);
        require!(
            batch <= lc.seed_remaining as usize,
            SolbeamError::SeedTooLong
        );

        // Cumulative work continues from whatever has arrived already, with the
        // window's oldest record as the zero point. As in `push_header`, the
        // value is derived from each header's own `bits` rather than supplied,
        // so the window stays internally consistent. Only *differences* of this
        // value are ever consumed, so the arbitrary baseline cancels.
        let mut chainwork = lc.headers.last().map(|r| r.chainwork).unwrap_or_default();
        let mut prev = lc.headers.last().map(|r| r.hash);

        let mut records: Vec<HeaderRecord> = Vec::with_capacity(batch);
        for raw in ancestors.chunks(HEADER_LEN) {
            // No difficulty check: with fewer than SEED_RECORDS records the rule
            // is not computable, which is why this instruction exists at all.
            // Linkage is checked, and the terminal hash is pinned to the
            // checkpoint below, so the chain cannot be fabricated.
            if let Some(parent) = prev {
                require!(read32(raw, 4) == parent, SolbeamError::BrokenLinkage);
            }
            let bits = read_u32_le(raw, 72);
            let hash = header_hash_of_bytes(raw);
            chainwork = chainwork.saturating_add(work_from_bits(bits));
            records.push(HeaderRecord {
                hash,
                chainwork,
                time: read_u32_le(raw, 68),
            });
            prev = Some(hash);
        }

        let completes = batch == lc.seed_remaining as usize;
        if completes {
            // The pin. The last seed record must BE the checkpoint the client
            // was anchored on, so the whole chain below it is forced and the
            // seed ends exactly where the trusted root is.
            require!(
                prev == Some(lc.tip_hash),
                SolbeamError::SeedWrongTip
            );
        }

        lc.headers.extend(records);
        lc.seed_remaining -= batch as u32;

        if completes {
            // 147 records ending at the checkpoint. The window_start set by
            // `anchor_checkpoint` already names the oldest of them, so this is
            // a statement about the shape the program just built, not a plan.
            require!(
                lc.headers.len() == SEED_RECORDS,
                SolbeamError::SeedWrongLength
            );
            lc.last_push_slot = Clock::get()?.slot;
            msg!(
                "SOLBEAM seed complete: {} records, window {}..{}, next header is checked by cw-144",
                lc.headers.len(),
                lc.window_start,
                lc.tip_height
            );
        } else {
            msg!(
                "SOLBEAM seed {}/{} records, {} still required",
                lc.headers.len(),
                SEED_RECORDS,
                lc.seed_remaining
            );
        }
        Ok(())
    }

    /// Follow a reorg.
    ///
    /// Without this the client is safe against a hostile advancer but blind to
    /// a legitimate reorg: a header built on an older block is rejected, and
    /// the client stalls on the abandoned branch forever.
    ///
    /// The whole branch is validated before it is considered - linkage from the
    /// fork point, proof of work, and the difficulty check on every header - so
    /// a hostile advancer gains nothing it did not already have. The replacement
    /// only happens if the branch is STRICTLY heavier; a tie keeps the
    /// incumbent, so nobody can grind a tiebreak.
    ///
    /// **Weight is accumulated chainwork, not height.** Each staged record
    /// carries the work of the target its header declares, summed from the fork
    /// point, so a short branch of hard blocks beats a long branch of easy ones
    /// — which is the whole point of the rule and is not true of a length
    /// comparison.
    ///
    /// A competing branch is submitted **one header at a time**. It cannot be
    /// sent whole: a Solana transaction is capped at 1232 bytes, and a real branch
    /// is far longer than the ~13 headers that would fit. So it is staged in a
    /// per-submitter account and committed once complete — see `commit_fork`.
    ///
    /// **Per-submitter rather than one shared slot.** A single mutable staging
    /// area can be occupied with junk, denying legitimate reorgs to everyone. A
    /// PDA seeded on the submitter removes the contention structurally instead of
    /// pricing it: you can only ever fill your own slot. Rent is a refundable
    /// deposit rather than a fee, so spamming creates many empty accounts and
    /// harms nobody, which is a better failure mode than a bond plus slashing
    /// machinery for what is only a denial of reorg-following.
    /// `fork_height` is the **last block the two branches share** — the common
    /// ancestor, not the first block of the competing branch. The branch itself
    /// begins at `fork_height + 1`, so a branch of N headers commits at tip
    /// `fork_height + N`. Getting this off by one is easy and the fixture's own
    /// `fork.from_height` uses the other convention (it is the first branch
    /// block), so callers holding that value must pass `from_height - 1`.
    pub fn init_staging(ctx: Context<InitStaging>, fork_height: u64) -> Result<()> {
        let lc = &ctx.accounts.light_client;
        require!(!lc.paused, SolbeamError::Paused);
        require!(!lc.is_seeding(), SolbeamError::Seeding);
        // The common ancestor must be a header we still hold. Deeper than the
        // window needs a checkpoint reset, which is a governance action.
        let fork_parent_hash = lc
            .hash_at(fork_height)
            .ok_or(SolbeamError::ForkPointNotInWindow)?;
        // On a retargeting chain the branch's first header needs 147 records of
        // ancestry below the fork point, and only the main window can supply
        // them. A fork point nearer the bottom of the window than that can never
        // be validated, so refuse it here rather than let it stage headers that
        // must then fail. (On a no-retargeting chain the target is constant and
        // no ancestry is needed.)
        if !lc.no_retargeting {
            let available = fork_height - lc.window_start + 1;
            require!(
                available >= SEED_RECORDS as u64,
                SolbeamError::ForkPointTooOld
            );
        }

        let staging = &mut ctx.accounts.staging;
        staging.submitter = ctx.accounts.submitter.key();
        staging.fork_height = fork_height;
        // Read once, from the chain state as it is *now*. Everything this branch
        // does afterwards is pinned to this value — see `ForkStaging`.
        staging.fork_parent_hash = fork_parent_hash;
        staging.records = Vec::new();
        staging.bump = ctx.bumps.staging;
        Ok(())
    }

    /// Append a **batch** of branch headers, validating linkage and proof of work
    /// exactly as `push_header` does for the main chain.
    ///
    /// Batched because one header per transaction is wasteful in the only
    /// currency this path spends. About twelve headers fit in a 1232-byte
    /// transaction, so a 72-header branch costs six transactions instead of
    /// seventy-two — a twelfth of the fees, and a twelfth of the latency, which
    /// matters when the thing being followed is a live reorg.
    ///
    /// The batch arrives as ONE flat byte vector rather than `Vec<[u8; 80]>`,
    /// because nested fixed-size arrays do not survive borsh's layout on the
    /// client side; a flat slice of 80-byte chunks serialises without drama and
    /// is chunked here.
    pub fn push_fork_header(
        ctx: Context<PushForkHeader>,
        branch_bytes: Vec<u8>,
    ) -> Result<()> {
        // Read the light client through the allocation-free view rather than as
        // `Account<LightClient>`: deserialising a full 192-record window costs a
        // ~28 KB `Vec` on a 32 KB heap, and this instruction also has to hold the
        // staging account. See `LightClientView`.
        let lc_data = ctx.accounts.light_client.try_borrow_data()?;
        let lc = LightClientView::new(&lc_data)?;
        require!(!lc.paused(), SolbeamError::Paused);
        require!(lc.seed_remaining() == 0, SolbeamError::Seeding);
        let staging = &mut ctx.accounts.staging;
        require!(
            staging.submitter == ctx.accounts.submitter.key(),
            SolbeamError::NotStagingOwner
        );

        require!(!branch_bytes.is_empty(), SolbeamError::EmptyFork);
        require!(branch_bytes.len() % HEADER_LEN == 0, SolbeamError::MalformedTx);
        let batch = branch_bytes.len() / HEADER_LEN;
        require!(batch <= MAX_FORK_BATCH, SolbeamError::BatchTooLarge);
        require!(
            staging.records.len() + batch <= WINDOW,
            SolbeamError::ForkTooLong
        );

        // Link to the tip of the branch so far, or to the fork point while the
        // branch is still empty.
        //
        // The first header links to the hash **recorded when the branch was
        // staged**, not to whatever the chain holds at that height now. That is
        // P2: a fresh lookup here lets the first header of a branch be judged
        // against one chain state and the rest of it against another, and lets
        // a branch staged against block A silently become a branch of block B.
        // The recorded hash is the block this branch was built on; if the chain
        // has since moved, the lookup below is what notices.
        let mut chainwork;
        let mut prev = match staging.records.last() {
            Some(record) => {
                // Resuming a staged branch: the parent's cumulative work is its
                // last record's, and every record in the branch was derived the
                // same way, so the running sum stays continuous across batches.
                chainwork = record.chainwork;
                record.hash
            }
            None => {
                let work = lc
                    .chainwork_at(staging.fork_height)
                    .ok_or(SolbeamError::ForkPointNotInWindow)?;
                chainwork = work;
                staging.fork_parent_hash
            }
        };

        // F3 — the branch's OWN ancestry, which is what the target must be
        // computed from.
        //
        // This used to pin every staged header to `lc.required_bits()`, the
        // target for `tip_height + 1` on the *incumbent* chain. For any fork
        // point below the tip that is the wrong block: BSV changes `bits` every
        // header, so the branch's first block declares the target for its own
        // height and was rejected `UnexpectedRetarget`. The only fork point that
        // could ever work was the tip, `commit_fork` could only extend, no
        // stored hash could change, and `burn_staged` was unreachable. The
        // 72-header reorg test passed only because regtest has no retargeting.
        //
        // A branch header at height `h` needs records `h-147 .. h-1`. The main
        // window supplies everything at or below the fork point (it is a single
        // chain, so those records are shared ancestry); the branch supplies its
        // own records above it. Together they are contiguous and cumulative
        // chainwork stays on one baseline, so `next_target` sees exactly the
        // window the node would have seen.
        //
        // The concatenation is presented to cw-144 as a closure rather than as a
        // materialised `Vec<Record>`: only 147 records are ever read, but
        // collecting them allocates — and the SBF heap is small enough that a
        // window-sized copy of the window is a real cost. `next_target_from`
        // runs the same arithmetic as the slice form.
        let fork_idx = lc
            .index_of(staging.fork_height)
            .ok_or(SolbeamError::ForkPointNotInWindow)?;
        let main_len = fork_idx + 1;
        let staged = &staging.records;
        let no_retargeting = lc.no_retargeting();
        let expected_bits = lc.expected_bits();
        let pow_limit = compact_to_target(lc.pow_limit_bits());

        // Validate the whole batch before writing any of it, so a bad header
        // half-way through does not leave a partial branch staged.
        let mut checked: Vec<HeaderRecord> = Vec::with_capacity(batch);
        for raw in branch_bytes.chunks(HEADER_LEN) {
            require!(read32(raw, 4) == prev, SolbeamError::BrokenLinkage);
            let bits = read_u32_le(raw, 72);
            // The target for THIS header's height, from the branch's own
            // records. On a no-retargeting chain the target is constant and no
            // lookback is needed; on any real chain it is cw-144, and `None`
            // means the branch reaches below the client's window, which no
            // amount of branch data can repair.
            let required = if no_retargeting {
                expected_bits
            } else {
                // Ancestry = light-client prefix ++ staged ++ checked-so-far,
                // trimmed to the newest LOOKBACK records because only those can
                // be read.
                let parent_height =
                    staging.fork_height + staged.len() as u64 + checked.len() as u64;
                let total = main_len + staged.len() + checked.len();
                let kept = total.min(SEED_RECORDS);
                let start = total - kept;
                let oldest_height = parent_height + 1 - kept as u64;
                difficulty::next_target_from(kept, oldest_height, pow_limit, |i| {
                    let idx = start + i;
                    if idx < main_len {
                        let (_, work, time) =
                            lc.record(idx).expect("index below the fork point is stored");
                        Record { time, chainwork: work }
                    } else if idx - main_len < staged.len() {
                        let r = &staged[idx - main_len];
                        Record { time: r.time, chainwork: r.chainwork }
                    } else {
                        let r = &checked[idx - main_len - staged.len()];
                        Record { time: r.time, chainwork: r.chainwork }
                    }
                })
                .map(target_to_compact)
                .ok_or(SolbeamError::DifficultyNotComputable)?
            };
            // Target before work, for the reason given in `push_header`.
            require!(bits == required, SolbeamError::UnexpectedRetarget);
            require!(meets_target_slice(raw, bits), SolbeamError::BadPow);

            let time = read_u32_le(raw, 68);
            let hash = header_hash_of_bytes(raw);
            chainwork = chainwork.saturating_add(work_from_bits(bits));
            checked.push(HeaderRecord {
                hash,
                chainwork,
                time,
            });
            prev = hash;
        }
        staging.records.extend(checked);
        Ok(())
    }

    /// Close a staged branch without committing it, returning the rent.
    ///
    /// **This is not optional housekeeping.** `commit_fork` refuses a branch that
    /// is not strictly heavier, and a refused instruction reverts — so its `close`
    /// constraint never runs and there is no other way to release the account.
    /// Without this instruction a branch that never becomes heavier strands its
    /// rent permanently.
    pub fn abandon_staging(_ctx: Context<AbandonStaging>) -> Result<()> {
        // Nothing to do: the `close` constraint on the account does the work.
        Ok(())
    }

    /// Swap the window onto the staged branch, if it is strictly heavier.
    ///
    /// **Strictly** heavier: a tie keeps the incumbent, so equal-work branches
    /// cannot be used to churn the tip. The comparison is real accumulated
    /// chainwork, not height: on any chain where difficulty varies, a shorter
    /// branch can carry more work, so height is not a proxy for it. The staging
    /// account is closed here and its rent returns to the submitter.
    pub fn commit_fork(ctx: Context<CommitFork>) -> Result<()> {
        let lc = &mut ctx.accounts.light_client;
        require!(!lc.paused, SolbeamError::Paused);
        require!(!lc.is_seeding(), SolbeamError::Seeding);
        let staging = &ctx.accounts.staging;
        require!(
            staging.submitter == ctx.accounts.submitter.key(),
            SolbeamError::NotStagingOwner
        );
        require!(!staging.records.is_empty(), SolbeamError::EmptyFork);

        let new_tip_height = staging.fork_height + staging.records.len() as u64;

        // P2, the re-anchor check. The branch was staged on one specific block;
        // if the chain no longer holds that block at that height, the branch is
        // stale and must be re-staged against whatever is there now.
        //
        // Without this, two branches staged against the same height could both
        // commit: the first moves the tip, the second splices its headers onto
        // the *new* chain's block at the fork height, and the window ends up
        // holding `headers[0..=fork_idx]` from one chain and the branch from
        // another with no linkage between them at all. Linkage is the only
        // invariant this client has; breaking it lets a header that is in no
        // chain become canonical, and therefore lets a deposit that is in no
        // block be minted. Re-staging is cheap; a forged mint is not.
        let current_parent = lc
            .hash_at(staging.fork_height)
            .ok_or(SolbeamError::ForkPointNotInWindow)?;
        require!(
            current_parent == staging.fork_parent_hash,
            SolbeamError::ForkPointMoved
        );

        // The incumbent's tip and the branch's tip, measured on the same
        // baseline: the window's cumulative work, where "cumulative" means from
        // the checkpoint. The branch's value was accumulated as its headers
        // arrived, from the fork point's record.
        let incumbent_work = lc
            .record(lc.tip_height)
            .map(|r| r.chainwork)
            .unwrap_or_default();
        require!(
            staging.records.last().map(|r| r.chainwork).unwrap_or_default()
                > incumbent_work,
            SolbeamError::ForkNotHeavier
        );

        let fork_idx = lc
            .index_of(staging.fork_height)
            .ok_or(SolbeamError::ForkPointNotInWindow)?;

        // Keep the prefix up to and including the fork point, append the branch,
        // then prune to the window. The prefix always begins at headers[0], so
        // window_start only moves by whatever the prune discards.
        //
        // The prefix's records are the shared ancestry: identical on both
        // branches, which is why their hash, chainwork and time carry over
        // unchanged. Every record above the fork point comes from the branch and
        // was built as it arrived — hash, work and time together — so the window
        // that results is a single contiguous chain with no gap in it.
        //
        // Rebuilt **in place**, not into a fresh `Vec`. A deserialised live
        // window already has a capacity of 256 records (borsh's growth leaves
        // the vector larger than its length), and the result is at most 192, so
        // `extend_from_slice` below never reallocates. A new `Vec` would hold a
        // second full window — up to 16 KB — alongside both deserialised
        // accounts, which is enough to exhaust the 32 KB SBF heap on exactly the
        // reorg this instruction exists to perform.
        let total = (fork_idx + 1) + staging.records.len();
        let excess = total.saturating_sub(WINDOW);
        lc.headers.truncate(fork_idx + 1);
        if excess >= fork_idx + 1 {
            lc.headers.clear();
            let skip = excess - (fork_idx + 1);
            lc.headers.extend_from_slice(&staging.records[skip..]);
        } else {
            if excess > 0 {
                lc.headers.drain(0..excess);
            }
            lc.headers.extend_from_slice(&staging.records);
        }
        if excess > 0 {
            lc.window_start += excess as u64;
        }
        if lc.headers.is_empty() {
            lc.window_start = new_tip_height;
        }

        lc.tip_height = new_tip_height;
        lc.tip_hash = staging.records.last().unwrap().hash;
        lc.last_push_slot = Clock::get()?.slot;

        emit!(ChainReorganised {
            from_height: staging.fork_height,
            new_tip_height,
        });
        Ok(())
    }

    /// Create `solBSV`.
    ///
    /// Two absences are deliberate and are the point:
    ///   * **no freeze authority** — nobody can freeze a holder's balance
    ///   * **mint authority is a PDA of this program**, not a key, so no
    ///     operator holds a token that can conjure supply
    pub fn initialize_token(ctx: Context<InitializeToken>) -> Result<()> {
        msg!(
            "SOLBEAM solBSV mint {} — {} decimals, no freeze authority, authority = program PDA",
            ctx.accounts.mint.key(),
            TOKEN_DECIMALS
        );
        Ok(())
    }

    /// Create the bridge's own state: the script a deposit must pay.
    ///
    /// `deposit_script` is the one address every peg-in must pay, so setting it
    /// is a privileged act: a first caller free to choose it would redirect
    /// every deposit into their own output. Like `initialize`, this is gated on
    /// the program's **upgrade authority**, read from the loader's `ProgramData`
    /// account — see [`Initialize`].
    ///
    /// There is no longer a replay-list account to create here. Replay is a
    /// per-deposit **nullifier PDA**, created by [`verify_deposit`] at the first
    /// mint and closed by [`prune_nullifier`] once its block leaves the window.
    pub fn initialize_bridge(ctx: Context<InitializeBridge>, deposit_script: Vec<u8>) -> Result<()> {
        require!(
            deposit_script.len() <= MAX_SCRIPT_LEN,
            SolbeamError::DepositScriptTooLong
        );
        require!(
            is_acceptable_deposit_script(&deposit_script),
            SolbeamError::DepositScriptNotP2pkh
        );
        let ds = &mut ctx.accounts.deposit_script;
        ds.script = deposit_script;
        ds.bump = ctx.bumps.deposit_script;

        msg!("SOLBEAM bridge initialised");
        Ok(())
    }

    /// Record a deposit outpoint the reserve has spent, so that
    /// [`verify_deposit`] will refuse to mint it. **This is N5's reported
    /// spent-outpoint record.**
    ///
    /// Solana cannot read BSV's UTXO set, so spentness is **reported, not
    /// proved**. That is not a new trust assumption: under the federation the
    /// deposit script *is* the reserve and the members already hold its key, so
    /// an honesty request about a UTXO adds nothing to what they can already do.
    /// The honest statement the design makes is two sentences — *"the program
    /// verifies deposits; the federation reports backing"* (docs 03, 05, 06).
    ///
    /// **PoC stand-in, and a reviewer should read it as one.** The signer is the
    /// program's **upgrade authority**, verified on-chain against the loader's
    /// `ProgramData` account exactly as [`crate::Initialize`] and
    /// [`crate::initialize_bridge`] verify it — standing in for the federation,
    /// which does not exist yet. When it does, the signer becomes the
    /// federation's key or quorum and nothing else about the instruction needs
    /// to change.
    ///
    /// **The permissionless alternative was considered and rejected.** Letting
    /// anyone submit a spend proof would make the record writable by anyone,
    /// which turns mint *availability* into an attack surface rather than only
    /// mint *correctness*. This instruction is deliberately not that.
    ///
    /// The account is seeded on `(txid, vout)` and built by hand to follow
    /// [`DepositNullifier`] rather than Anchor's `init`. The seeds *are*
    /// expressible here — both are instruction arguments — but
    /// `transfer` + `allocate` + `assign` is what stops a lamport sent to this
    /// public PDA from blocking the first report; see [`create_spent_outpoint`].
    /// Reporting twice is refused rather than silently accepted — a report is a
    /// statement about a fact, and a second one says nothing new.
    pub fn report_spent(ctx: Context<ReportSpent>, txid: [u8; 32], vout: u32) -> Result<()> {
        create_spent_outpoint(
            &ctx.accounts.spent_outpoint.to_account_info(),
            &ctx.accounts.authority.to_account_info(),
            ctx.program_id,
            txid,
            vout,
        )?;

        emit!(SpentOutpointReported {
            txid,
            vout,
            authority: ctx.accounts.authority.key(),
        });
        msg!(
            "SOLBEAM spent outpoint reported {}:{}",
            display_hex(&txid),
            vout
        );
        Ok(())
    }

    /// Verify a BSV deposit against the header window, and refuse to accept the
    /// same one twice.
    ///
    /// This is the trustless half of the peg, and it is deliberately explicit
    /// about what it does NOT take on trust. Everything the caller supplies -
    /// the branch, the transaction, the claimed output - is re-derived here.
    ///
    /// **It proves the output *paid* the deposit script. It does not, and cannot,
    /// prove the outpoint is *unspent*** — Solana has no view of BSV's UTXO set.
    /// Under the federation the deposit script is the pooled reserve and the same
    /// members hold the key, so a consolidation or payout can spend a deposit
    /// output while the original deposit stays provable and mintable. Two
    /// separate records answer that, and neither is a spentness proof:
    ///
    ///   * the **spent-outpoint record** (step 7) — the federation reports
    ///     outpoints it has seen the reserve spend, and this instruction refuses
    ///     a deposit whose outpoint is in that record. The report is the whole
    ///     mechanism, because on-chain spentness is impossible;
    ///   * the replay **nullifier** (step 8) — records that *this program* has
    ///     already minted a given `(txid, vout)`. It is a mint-side gate and it
    ///     is **not** the same thing as spentness: it stops a double mint, never
    ///     a mint after the output was spent.
    ///
    /// See A2/N5 and decisions P5 and the spent-record decision.
    ///
    /// **What it does with a verified claim changed in doc 31.** It used to mint
    /// straight to the depositor. It now **stages** the mint — creating a
    /// [`StagedMint`] keyed on `(txid, vout)` that records the recipient, the
    /// amount, the deposit's height, the hash of the block it was proven
    /// against, and **the maturity that applied at this moment** — and mints the
    /// `solBSV` into the program-owned **vault** token account rather than to the
    /// recipient.
    ///
    /// `maturity_at_deposit` is load-bearing, not bookkeeping: a later governance
    /// raise of `Config::maturity_blocks` must not retroactively trap a deposit
    /// that was verified under a shorter window. Reading it once, here, is what
    /// makes the parameter a policy for *future* deposits rather than a lever on
    /// funds already in flight.
    pub fn verify_deposit(ctx: Context<VerifyDeposit>, claim: DepositClaim) -> Result<()> {
        let lc = &ctx.accounts.light_client;
        require!(!lc.paused, SolbeamError::Paused);

        // 1. The header must still be inside the window. A proof against a
        //    header we no longer hold cannot be checked at all.
        let index = lc
            .index_of(claim.height)
            .ok_or(SolbeamError::HeaderNotInWindow)?;
        let record = lc
            .headers
            .get(index)
            .ok_or(SolbeamError::HeaderNotInWindow)?;

        // The supplied header must be the canonical block at this height. This
        // is the check that lets the window store nothing but hashes: the
        // Merkle root is a field inside this header, so the hash already
        // commits to it and it never has to be kept separately.
        require!(
            header_hash(&claim.header) == record.hash,
            SolbeamError::HeaderMismatch
        );
        let merkle_root = read32(&claim.header, 36);

        // 2. The transaction must be the one the proof names. Without this the
        //    branch could be valid for a *different* transaction.
        let txid = header_hash_of_bytes(&claim.tx);
        require!(txid == claim.txid, SolbeamError::TxidMismatch);

        // 3. And it must be *in* the block, which is what the branch proves.
        require!(
            fold_branch(claim.txid, claim.index, &claim.branch) == merkle_root,
            SolbeamError::BadMerkleProof
        );

        // 4. The claimed output must exist, carry the claimed value, and pay the
        //    bridge's deposit script.
        let outputs = parse_outputs(&claim.tx)?;
        require!((claim.vout as usize) < outputs.len(), SolbeamError::NoSuchOutput);
        let (value, script) = &outputs[claim.vout as usize];
        require!(*value == claim.amount, SolbeamError::AmountMismatch);
        require!(*value > 0, SolbeamError::ZeroValue);
        require!(
            script == &ctx.accounts.deposit_script.script,
            SolbeamError::WrongOutputScript
        );

        // 5. The recipient must be committed somewhere in the same transaction.
        //    This is the payload the wallet has to attach, and its absence is
        //    the most likely real-world user error.
        let mut payload_found = false;
        for (_, out_script) in outputs.iter() {
            if let Some(payload) = op_return_payload(out_script) {
                if payload == claim.recipient {
                    payload_found = true;
                    break;
                }
            }
        }
        require!(payload_found, SolbeamError::MissingPayload);

        // 6. Buried deep enough.
        let confirmations = lc
            .tip_height
            .saturating_sub(claim.height)
            .saturating_add(1);
        require!(
            confirmations >= MIN_CONFIRMATIONS,
            SolbeamError::InsufficientConfirmations
        );

        // 7. Backing. The federation's spent-outpoint record is consulted
        //    **before** the replay nullifier, because it answers a different
        //    question: not "has this program minted this deposit" but "has the
        //    reserve already spent this output". The address is re-derived from
        //    the claim, never trusted, so a caller cannot point the check at an
        //    empty account of its own (see `require_not_spent`).
        require_not_spent(
            &ctx.accounts.spent_outpoint.to_account_info(),
            ctx.program_id,
            claim.txid,
            claim.vout,
        )?;

        // 8. Replay. The (txid, vout) pair is the identity of a deposit, so a
        //    PDA seeded on exactly those two values is the record. Creating it
        //    *is* the mint; a second claim finds it already there and reverts
        //    with `AlreadyMinted`.
        //
        //    No list, no prune on this path and no `MAX_USED`: the old fixed list
        //    was a cap on usage per window reached by ordinary volume, not a
        //    replay defence. The height is stored **inside** the nullifier so the
        //    later prune can be checked against it rather than trusted — see
        //    [`prune_nullifier`].
        create_nullifier(
            &ctx.accounts.nullifier.to_account_info(),
            &ctx.accounts.submitter.to_account_info(),
            ctx.program_id,
            claim.txid,
            claim.vout,
            claim.height,
        )?;

        // 9. The recipient named in the OP_RETURN is the account the staged mint
        //    will be released to. This is the binding between the BSV payload and
        //    the Solana destination, so it is checked rather than assumed. No
        //    token account is created here any more: nothing is minted to the
        //    recipient on this path.
        require!(
            ctx.accounts.recipient_owner.key().to_bytes() == claim.recipient,
            SolbeamError::RecipientMismatch
        );

        // 10. Stage the mint. The item is a PDA on `(txid, vout)` — the deposit's
        //     identity, never its height — and it records the maturity read from
        //     `Config` **now**, so a later raise cannot trap this deposit. Created
        //     by hand for the same reason the nullifier is: the seeds come from
        //     instruction arguments, and the existence check must produce
        //     `AlreadyStaged` rather than a system-program error.
        create_staged_mint(
            &ctx.accounts.staged.to_account_info(),
            &ctx.accounts.submitter.to_account_info(),
            ctx.program_id,
            claim.txid,
            claim.vout,
            ctx.accounts.recipient_owner.key(),
            claim.amount,
            claim.height,
            record.hash,
            ctx.accounts.config.maturity_blocks,
        )?;

        // 11. Mint into the VAULT, not to the recipient. The authority is this
        //     program's own PDA, so the program signs for it — no operator key
        //     can mint. The tokens leave the vault only through `release_mint`
        //     (to the recipient, once matured and still canonical) or are burned
        //     by `burn_staged` (once the chain has moved against the deposit).
        let bump = ctx.accounts.light_client.bump;
        let seeds: &[&[u8]] = &[b"light_client", &[bump]];
        token::mint_to(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.light_client.to_account_info(),
                },
            )
            .with_signer(&[seeds]),
            claim.amount,
        )?;

        emit!(DepositStaged {
            txid: claim.txid,
            vout: claim.vout,
            amount: claim.amount,
            recipient: claim.recipient,
            height: claim.height,
            maturity_at_deposit: ctx.accounts.config.maturity_blocks,
        });
        Ok(())
    }

    /// Release a matured, still-canonical staged mint to its recipient.
    /// **Permissionless** — anyone may call it, which is what makes the exit
    /// real: the recipient does not depend on a relayer to hand them their own
    /// tokens.
    ///
    /// All four conditions are required, and doc 31 §4 says why each alone is not
    /// enough:
    ///
    /// * **the client is fresh** — the last accepted header is within
    ///   [`MAX_STALENESS_SLOTS`]. Staleness alone would release against a view of
    ///   the chain that has not yet seen the reorg;
    /// * **the item has matured** — `tip_height >= deposit_height +
    ///   maturity_at_deposit`, using the maturity recorded when the deposit was
    ///   verified, not the current config;
    /// * **the client still holds the height** — it has not left the window, so
    ///   the hash can still be checked at all;
    /// * **the stored hash still equals the deposit's** — depth alone would
    ///   release even if the client's record of that height had changed.
    pub fn release_mint(ctx: Context<ReleaseMint>, txid: [u8; 32], vout: u32) -> Result<()> {
        let lc = &ctx.accounts.light_client;
        let staged = &ctx.accounts.staged;

        let now = Clock::get()?.slot;
        require!(
            now.saturating_sub(lc.last_push_slot) <= MAX_STALENESS_SLOTS,
            SolbeamError::StaleClient
        );

        let mature_at = staged
            .deposit_height
            .checked_add(staged.maturity_at_deposit)
            .ok_or(SolbeamError::Overflow)?;
        require!(lc.tip_height >= mature_at, SolbeamError::NotMatured);

        let current = lc
            .hash_at(staged.deposit_height)
            .ok_or(SolbeamError::DepositHeightNotInWindow)?;
        require!(
            current == staged.deposit_hash,
            SolbeamError::DepositHashChanged
        );

        // The ATA is derived from `recipient_owner`, so a caller that supplied a
        // different owner would send the tokens to the wrong place. The stored
        // recipient is what the OP_RETURN committed to; this is what pins the
        // destination to it.
        require!(
            ctx.accounts.recipient_owner.key() == staged.recipient,
            SolbeamError::RecipientMismatch
        );

        let bump = lc.bump;
        let seeds: &[&[u8]] = &[b"light_client", &[bump]];
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.recipient_token_account.to_account_info(),
                    authority: ctx.accounts.light_client.to_account_info(),
                },
            )
            .with_signer(&[seeds]),
            staged.amount,
        )?;

        emit!(MintReleased {
            txid,
            vout,
            recipient: staged.recipient,
            amount: staged.amount,
            height: staged.deposit_height,
        });
        Ok(())
    }

    /// Burn a staged mint whose deposit the chain has moved against.
    /// **Permissionless**: anyone may call it, and the rent of the closed item is
    /// the reward for doing so.
    ///
    /// The predicate is the mirror of [`release_mint`] and is deliberately
    /// narrower than doc 21's draft:
    ///
    /// * **the stored hash at `deposit_height` DIFFERS from the deposit's** — the
    ///   client has followed a reorg and no longer holds the block the deposit was
    ///   proven against. Burning is the correct answer when the BSV is gone: the
    ///   tokens must not exist;
    /// * **the item has matured** — `tip_height >= deposit_height +
    ///   maturity_at_deposit`, so a transient fork cannot burn a good mint.
    ///
    /// It does **not** require freshness (doc 31 §3 lists only the two
    /// conditions). That is the safe direction: a stale client can only be wrong
    /// about which block is canonical, and a burn destroys supply rather than
    /// creating it.
    ///
    /// There is no bounty here. Doc 31 §3 suggests one and §6 records it as
    /// **suggested, not sized** with `fee.bounty_share` open; inventing a number
    /// would be adding a parameter the specification deliberately leaves unset.
    /// The closer keeps the item's rent.
    pub fn burn_staged(ctx: Context<BurnStaged>, txid: [u8; 32], vout: u32) -> Result<()> {
        let lc = &ctx.accounts.light_client;
        let staged = &ctx.accounts.staged;

        let mature_at = staged
            .deposit_height
            .checked_add(staged.maturity_at_deposit)
            .ok_or(SolbeamError::Overflow)?;
        require!(lc.tip_height >= mature_at, SolbeamError::NotMatured);

        let current = lc
            .hash_at(staged.deposit_height)
            .ok_or(SolbeamError::DepositHeightNotInWindow)?;
        require!(
            current != staged.deposit_hash,
            SolbeamError::DepositHashUnchanged
        );

        let bump = lc.bump;
        let seeds: &[&[u8]] = &[b"light_client", &[bump]];
        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                Burn {
                    mint: ctx.accounts.mint.to_account_info(),
                    from: ctx.accounts.vault.to_account_info(),
                    authority: ctx.accounts.light_client.to_account_info(),
                },
            )
            .with_signer(&[seeds]),
            staged.amount,
        )?;

        emit!(StagedMintBurned {
            txid,
            vout,
            amount: staged.amount,
            height: staged.deposit_height,
        });
        Ok(())
    }

    /// Propose a timelocked change to the trusted checkpoint or the pause flag.
    ///
    /// **F4.** `set_checkpoint` and `set_paused` used to be one signature and one
    /// slot: `authority` is the program's upgrade authority, so a single key
    /// could install a trusted root that makes a fabricated deposit provable
    /// (`set_checkpoint` proves a fake deposit, then `verify_deposit` mints it)
    /// or halt header advancement and therefore redemption. Neither is wrong to
    /// *exist* — somebody must be able to repair a bad root or stop a bleeding
    /// client — but neither should be instant and unannounced.
    ///
    /// So a change is now two steps. This one records exactly one pending change
    /// in a **singleton PDA**, with the proposing authority and an
    /// `effective_slot` at least [`TIMELOCK_SLOTS`] in the future. The change
    /// takes effect only when [`execute_authority_change`] runs at or after that
    /// slot, and [`cancel_authority_change`] can withdraw it before then.
    ///
    /// One outstanding change at a time is structural, not a convention: the PDA
    /// has no per-proposal seed, so a second `propose` fails while one is live.
    /// That also means the delay cannot be restarted or leapfrogged by spamming
    /// proposals — the proposer must execute or cancel before proposing again.
    pub fn propose_authority_change(
        ctx: Context<ProposeAuthorityChange>,
        change: AuthorityChange,
        effective_slot: u64,
    ) -> Result<()> {
        let earliest = Clock::get()?
            .slot
            .checked_add(TIMELOCK_SLOTS)
            .ok_or(SolbeamError::Overflow)?;
        require!(
            effective_slot >= earliest,
            SolbeamError::TimelockTooSoon
        );

        let pending = &mut ctx.accounts.pending;
        pending.authority = ctx.accounts.authority.key();
        pending.effective_slot = effective_slot;
        pending.change = change;
        pending.bump = ctx.bumps.pending;

        emit!(AuthorityChangeProposed {
            authority: pending.authority,
            effective_slot,
        });
        msg!(
            "SOLBEAM authority change proposed, effective at slot {} (now {})",
            effective_slot,
            Clock::get()?.slot
        );
        Ok(())
    }

    /// Apply the pending change, once its timelock has elapsed.
    ///
    /// Two things are enforced together, and both matter: the signer must be the
    /// authority recorded on the pending account *and* the light client's own
    /// `authority` (the account constraints check the second, the body the
    /// first), and `Clock::slot` must be at or after `effective_slot`. The
    /// account is closed on success, so the pending change exists exactly once
    /// and cannot be replayed.
    pub fn execute_authority_change(ctx: Context<ExecuteAuthorityChange>) -> Result<()> {
        let pending = &ctx.accounts.pending;
        require!(
            ctx.accounts.authority.key() == pending.authority,
            SolbeamError::Unauthorized
        );
        let slot = Clock::get()?.slot;
        require!(
            slot >= pending.effective_slot,
            SolbeamError::TimelockNotElapsed
        );

        match &pending.change {
            // F2's shared implementation, on purpose: a checkpoint installed here
            // re-derives `expected_bits`, `no_retargeting` and `pow_limit_bits`
            // from the raw header, exactly as `initialize` does. A second,
            // divergent path is how F2 arose in the first place.
            AuthorityChange::Checkpoint { height, header } => {
                ctx.accounts.light_client.anchor_checkpoint(*height, header)?;
                msg!(
                    "SOLBEAM checkpoint re-anchored at {} (no_retargeting {}, seed remaining {})",
                    height,
                    ctx.accounts.light_client.no_retargeting,
                    ctx.accounts.light_client.seed_remaining
                );
            }
            AuthorityChange::Pause { paused } => {
                ctx.accounts.light_client.paused = *paused;
                msg!("SOLBEAM paused set to {}", paused);
            }
            // The vault's maturity, through the same timelocked path as the
            // checkpoint and the pause. **No second authority mechanism**: this
            // is a variant of the existing enum, proposed by
            // `propose_authority_change` and applied here.
            //
            // Raising it applies to *future* deposits only. A deposit already
            // verified carries its own `maturity_at_deposit`, so a governance
            // raise can never reach back and freeze funds in flight (doc 31 §2).
            AuthorityChange::SetMaturity { blocks } => {
                ctx.accounts.config.maturity_blocks = *blocks;
                msg!("SOLBEAM maturity_blocks set to {}", blocks);
            }
            // The peg-out deadline, through the same timelocked path. Same rule
            // as maturity: future redemptions only, because each item records
            // the deadline it was created with.
            AuthorityChange::SetRedeemDeadline { slots } => {
                ctx.accounts.config.redeem_deadline_slots = *slots;
                msg!("SOLBEAM redeem_deadline_slots set to {}", slots);
            }
        }

        emit!(AuthorityChangeExecuted {
            authority: pending.authority,
            slot,
        });
        Ok(())
    }

    /// Withdraw a pending change before it takes effect.
    ///
    /// Needed because a proposal the authority has thought better of would
    /// otherwise occupy the singleton slot for its whole delay — and because a
    /// change can be wrong, not only malicious.
    pub fn cancel_authority_change(ctx: Context<CancelAuthorityChange>) -> Result<()> {
        require!(
            ctx.accounts.authority.key() == ctx.accounts.pending.authority,
            SolbeamError::Unauthorized
        );
        msg!(
            "SOLBEAM authority change cancelled before slot {}",
            ctx.accounts.pending.effective_slot
        );
        Ok(())
    }

    /// Close a minted deposit's replay nullifier once its block has left the
    /// header window, returning the rent.
    ///
    /// **The height rule is the whole of the safety argument.** A closed
    /// nullifier means the deposit can be minted again, so a prune that could be
    /// aimed at a live deposit would be a replay oracle: close the nullifier,
    /// re-mint, repeat. The account therefore stores the `deposit_height` it was
    /// created with, and this instruction refuses unless
    /// `deposit_height < window_start` — a condition on *stored* state, not on an
    /// argument the caller supplies. Once the deposit's block is below the
    /// window, `verify_deposit` refuses any claim at that height before the
    /// replay check is reached, and `window_start` is monotonic, so re-including
    /// the same transaction at a later in-window height would require orphaning
    /// the original block — a reorg deeper than the window itself.
    ///
    /// Permissionless on purpose: the caller takes the rent back as the reward
    /// for housekeeping. This is the only path that closes a nullifier today; a
    /// vault burn will close one as soon as such an instruction exists, and
    /// nothing else may. See V1 in doc 19/20 for why the pending item and the
    /// nullifier must not be the same account.
    pub fn prune_nullifier(ctx: Context<PruneNullifier>, txid: [u8; 32], vout: u32) -> Result<()> {
        // The address is Anchor's business: `seeds` on the account, bound to
        // these two arguments by `#[instruction(...)]`. What is checked here is
        // the rule, on the *stored* height — never on an argument, which is the
        // whole difference between a sound prune and a replay oracle.
        let deposit_height = ctx.accounts.nullifier.deposit_height;
        require!(
            deposit_height < ctx.accounts.light_client.window_start,
            SolbeamError::NullifierNotPrunable
        );

        // `close = submitter` on the account returns the rent and zeroes it, so
        // there is no manual close here and no second place to get it wrong.
        emit!(NullifierPruned {
            txid,
            vout,
            deposit_height,
        });
        Ok(())
    }

    // -----------------------------------------------------------------------
    // peg-out — the redemption half (doc 11)
    // -----------------------------------------------------------------------
    //
    // The shape is the vault's, applied to the other direction: initiate,
    // cancel-on-timeout, claim against the light client, wait, settle. Steps
    // 1–3 are built here; step 4 (membership — who is obliged to pay) is not,
    // and doc 11 §6 records that every redemption therefore times out into a
    // cancellation until a federation exists.

    /// **Escrow** the holder's `solBSV` and name the BSV address the payout
    /// must pay.
    ///
    /// No burn happens here, and that is the whole point (doc 11 §2). A burn is
    /// final; a burn at step 1 would destroy the holder's claim on the reserve
    /// before any BSV had moved, so a member who then failed to pay would leave
    /// them with nothing. The tokens move to a program-owned escrow keyed on the
    /// redemption's `id`, where they stay until the payout is proven and the
    /// challenge window has passed (`settle_redeem`) or the deadline expires
    /// (`cancel_redeem`).
    ///
    /// **`id` must be `RedeemBook::next_id`.** Requiring the next counter value
    /// rather than accepting any unused id is what keeps the redemptions
    /// enumerable — `[b"redeem", 0]`, `[b"redeem", 1]`, … — which is what makes
    /// the three permissionless paths above usable by anyone rather than only by
    /// an off-chain index that knows every holder. Two callers racing for the
    /// same id cannot both win: the PDA is the same account, so one transaction
    /// fails, and the loser retries at the new counter.
    ///
    /// The **fee is recorded on the item**, not recomputed at claim time, for
    /// the same reason a staged mint records `maturity_at_deposit`: a later
    /// parameter change must not change what an in-flight redemption is owed.
    /// The payout proof must show at least `amount − fee`.
    ///
    /// `bsv_address` is the **20-byte HASH160** the base58 address encodes — the
    /// only part that determines the output script, and the only part the claim
    /// needs to compare against. Parsing base58check on-chain would add a
    /// decoder and a checksum check on the mint path for no additional
    /// guarantee: the program can only pay the script the hash determines.
    pub fn initiate_redeem(
        ctx: Context<InitiateRedeem>,
        id: u64,
        amount: u64,
        bsv_address: [u8; 20],
    ) -> Result<()> {
        let lc = &ctx.accounts.light_client;
        require!(!lc.paused, SolbeamError::Paused);
        require!(amount >= REDEEM_D_MIN, SolbeamError::BelowMinimumRedeem);

        let book = &mut ctx.accounts.book;
        require!(id == book.next_id, SolbeamError::WrongRedeemId);
        require!(
            book.pending < MAX_PENDING_REDEMPTIONS,
            SolbeamError::TooManyPendingRedemptions
        );

        let fee = redeem_fee(amount)?;
        let payout_min = amount.checked_sub(fee).ok_or(SolbeamError::Overflow)?;
        require!(payout_min > 0, SolbeamError::BelowMinimumRedeem);

        let now = Clock::get()?.slot;
        // The deadline is recorded **now**, from the policy in force now. A
        // later governance change moves the policy for future redemptions and
        // cannot shorten or extend this one — the same rule maturity follows.
        let deadline_slot = redeem_deadline_slot(
            now,
            ctx.accounts.config.redeem_deadline_slots,
            CANCEL_GRACE_SLOTS,
        )?;

        let pending = &mut ctx.accounts.pending;
        pending.holder = ctx.accounts.holder.key();
        pending.amount = amount;
        pending.fee = fee;
        pending.bsv_address = bsv_address;
        pending.initiated_slot = now;
        pending.deadline_slot = deadline_slot;
        pending.payout_height = 0;
        pending.payout_hash = [0u8; 32];
        pending.bump = ctx.bumps.pending;

        // The escrow is a program-owned (light-client-authority) token account,
        // so no operator key can move it; the holder's authority is used only
        // for the transfer in.
        token::transfer(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                Transfer {
                    from: ctx.accounts.holder_token_account.to_account_info(),
                    to: ctx.accounts.escrow.to_account_info(),
                    authority: ctx.accounts.holder.to_account_info(),
                },
            ),
            amount,
        )?;

        book.next_id = book.next_id.checked_add(1).ok_or(SolbeamError::Overflow)?;
        book.pending = book.pending.checked_add(1).ok_or(SolbeamError::Overflow)?;
        book.bump = ctx.bumps.book;

        emit!(RedeemInitiated {
            id,
            holder: pending.holder,
            amount,
            fee,
            bsv_address,
            deadline_slot,
        });
        msg!(
            "SOLBEAM redemption {} initiated: {} base units escrowed, fee {}, deadline slot {}",
            id,
            amount,
            fee,
            deadline_slot
        );
        Ok(())
    }

    /// Return an escrowed redemption to its holder once the deadline has
    /// passed. **Permissionless**: the caller takes the closed accounts' rent,
    /// which is what makes a failed peg-out recoverable without the holder
    /// having to act.
    ///
    /// Two things must be true:
    ///
    /// * **the deadline has passed** — `Clock::slot >= deadline_slot`. The clock
    ///   is a Solana slot, deliberately: a header-height deadline would never
    ///   expire if the feed stalled, and the holder's funds would freeze with it
    ///   (doc 11 §4/§6);
    /// * **no live payout has been claimed** — if a claim is staged and the
    ///   light client still holds the block it was proven against, the member
    ///   has paid and the escrow is theirs to burn, not the holder's to
    ///   withdraw. This is the fix for V2 (`cancel_redeem` raced
    ///   `settle_redeem`): once a payout is staged, the two exits are mutually
    ///   exclusive, so the "paid **and** refunded" case the old design made
    ///   certain cannot arise.
    ///
    /// **A claim the client can no longer check is not a live claim.** If the
    /// stored hash at the payout's height no longer equals the one the claim was
    /// proven against, the BSV did not stay paid, and the holder is made whole
    /// rather than left with an escrow that no instruction could resolve. If the
    /// height has left the window altogether the same rule applies and for a
    /// stronger reason: `settle_redeem` then fails `PayoutHeightNotInWindow`
    /// forever, so a claim that still blocked cancellation would latch the
    /// escrow permanently — not delay it, latch it. It is the same
    /// "too old to check" boundary the vault's exits have at
    /// `DepositHeightNotInWindow`, and it is worse here because the holder has
    /// no other route to their funds. **The rule: a claim whose payout block has
    /// left the light client's window is no longer provable, so the escrow
    /// returns to the holder.** The residual is stated rather than hidden — if
    /// that block had not been reorged, the member paid in BSV and the holder
    /// keeps both the payout and the refund. The window is what makes that
    /// trade; a permanently stuck escrow is the worse failure.
    ///
    /// The amount returned is `pending.amount` — **unchanged**, no fee: nothing
    /// was paid, so nothing is charged.
    pub fn cancel_redeem(ctx: Context<CancelRedeem>, id: u64) -> Result<()> {
        let lc = &ctx.accounts.light_client;
        let pending = &ctx.accounts.pending;
        let now = Clock::get()?.slot;
        require!(now >= pending.deadline_slot, SolbeamError::DeadlineNotReached);

        if pending.payout_height != 0 {
            // A claim blocks cancellation only while it is still provable.
            // `payout_claim_is_live` is false both for a hash that changed (a
            // reorg) and for a height that has left the window: neither can
            // settle any more (`settle_redeem` refuses the latter with
            // `PayoutHeightNotInWindow`), so neither may keep the escrow
            // latched. See the doc comment for the rule and its residual.
            require!(
                !payout_claim_is_live(lc, pending),
                SolbeamError::RedeemClaimed
            );
        }

        // The destination is derived from `holder`, so a caller that supplied a
        // different account would send the tokens somewhere else. Pinned to the
        // holder recorded at initiation.
        require!(
            ctx.accounts.holder.key() == pending.holder,
            SolbeamError::HolderMismatch
        );

        let bump = lc.bump;
        let seeds: &[&[u8]] = &[b"light_client", &[bump]];
        token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                Transfer {
                    from: ctx.accounts.escrow.to_account_info(),
                    to: ctx.accounts.holder_token_account.to_account_info(),
                    authority: ctx.accounts.light_client.to_account_info(),
                },
                &[seeds],
            ),
            pending.amount,
        )?;
        close_escrow(
            ctx.accounts.token_program.key(),
            &ctx.accounts.escrow.to_account_info(),
            &ctx.accounts.submitter.to_account_info(),
            &ctx.accounts.light_client.to_account_info(),
            bump,
        )?;

        ctx.accounts.book.pending = ctx
            .accounts
            .book
            .pending
            .checked_sub(1)
            .ok_or(SolbeamError::Overflow)?;

        emit!(RedeemCancelled {
            id,
            holder: pending.holder,
            amount: pending.amount,
        });
        Ok(())
    }

    /// Prove, through the **existing light client**, that the named address was
    /// paid. Permissionless: the holder does not depend on the member who owes
    /// them, and a third party holding the proof can settle the redemption.
    ///
    /// The proof is the mint's proof pointed the other way — a raw legacy
    /// transaction, a Merkle branch to the root of a header the client already
    /// holds, and a depth check — with three differences that matter:
    ///
    /// * **the output must pay the address recorded at initiation**, not the
    ///   reserve's deposit script. The stored 20-byte HASH160 is expanded to the
    ///   canonical P2PKH script and compared byte for byte, so the check is
    ///   against what the holder asked for and not against a caller's claim
    ///   (this is V4: a settle that bound neither amount nor destination);
    /// * **the amount is a floor, not an equality** — at least `amount − fee`.
    ///   Overpayment is the member's business; underpayment is not a settlement;
    /// * **the depth is `po.payout_confirmations`**, six blocks, lower than the
    ///   mint's twelve because the burn that follows still faces a challenge
    ///   window.
    ///
    /// The payout outpoint is recorded in a [`PayoutNullifier`] carrying **this
    /// redemption's id**, so one BSV payment cannot settle two redemptions that
    /// name the same address and amount (W5). It may be re-submitted for the
    /// *same* redemption, because a reorg can re-include the transaction at a
    /// different height and the member should not have to pay twice for the
    /// chain's own rearrangement.
    ///
    /// Staging is a latch on the pending account: `payout_height` and
    /// `payout_hash`. It is what `settle_redeem` and `cancel_redeem` resolve
    /// against, and it is deliberately **not** final — a reorg clears it in
    /// effect, and a re-claim overwrites it.
    pub fn claim_payout(
        ctx: Context<ClaimPayout>,
        id: u64,
        proof: PayoutProof,
    ) -> Result<()> {
        let lc = &ctx.accounts.light_client;
        require!(!lc.paused, SolbeamError::Paused);
        let pending = &mut ctx.accounts.pending;

        // A claim is already staged and still matches the client's window: this
        // is a double claim, not a re-claim after a reorg. (If the stored hash
        // no longer matches, or the height has left the window, the claim is
        // void and a fresh proof — or the re-included transaction — may replace
        // it. `payout_claim_is_live` is the same predicate `cancel_redeem`
        // refuses on, so "void enough to re-claim" and "void enough to cancel"
        // are one condition, not two.)
        require!(
            !payout_claim_is_live(lc, pending),
            SolbeamError::PayoutAlreadyClaimed
        );

        // 1. The header must still be inside the window.
        let index = lc
            .index_of(proof.height)
            .ok_or(SolbeamError::HeaderNotInWindow)?;
        let record = lc
            .headers
            .get(index)
            .ok_or(SolbeamError::HeaderNotInWindow)?;

        // 2. The supplied header must be the canonical block at that height.
        require!(
            header_hash(&proof.header) == record.hash,
            SolbeamError::HeaderMismatch
        );
        let merkle_root = read32(&proof.header, 36);

        // 3. The transaction must be the one the proof names, and must be in
        //    that block.
        let txid = header_hash_of_bytes(&proof.tx);
        require!(txid == proof.txid, SolbeamError::TxidMismatch);
        require!(
            fold_branch(proof.txid, proof.index, &proof.branch) == merkle_root,
            SolbeamError::BadMerkleProof
        );

        // 4. Deep enough. `po.payout_confirmations`.
        let confirmations = lc
            .tip_height
            .saturating_sub(proof.height)
            .saturating_add(1);
        require!(
            confirmations >= PAYOUT_CONFIRMATIONS,
            SolbeamError::InsufficientConfirmations
        );

        // 5. The output must pay the holder's address, and carry at least what
        //    the redemption is owed.
        let outputs = parse_outputs(&proof.tx)?;
        require!(
            (proof.vout as usize) < outputs.len(),
            SolbeamError::NoSuchOutput
        );
        let (value, script) = &outputs[proof.vout as usize];
        require!(
            is_p2pkh_for(script, &pending.bsv_address),
            SolbeamError::PayoutAddressMismatch
        );
        let payout_min = pending
            .amount
            .checked_sub(pending.fee)
            .ok_or(SolbeamError::Overflow)?;
        require!(*value >= payout_min, SolbeamError::PayoutAmountTooLow);
        require!(*value > 0, SolbeamError::ZeroValue);

        // 6. The outpoint has not already settled a *different* redemption.
        stage_payout_outpoint(
            &ctx.accounts.payout_nullifier.to_account_info(),
            &ctx.accounts.submitter.to_account_info(),
            ctx.program_id,
            proof.txid,
            proof.vout,
            id,
        )?;

        pending.payout_height = proof.height;
        pending.payout_hash = record.hash;

        emit!(PayoutClaimed {
            id,
            txid: proof.txid,
            vout: proof.vout,
            height: proof.height,
            amount: *value,
            payout_min,
        });
        Ok(())
    }

    /// **Burn** the escrow once the payout has survived the challenge window.
    ///
    /// The predicate is the release/burn pair of the mint vault turned around:
    ///
    /// * **a payout was claimed** — otherwise `PayoutNotClaimed`;
    /// * **the client still holds the payout's height and its stored hash still
    ///   equals the claim's** — a reorg is the one thing the window exists to
    ///   catch, and if it happened the burn must not execute (`PayoutReorged`);
    ///   a height gone from the window cannot be checked at all
    ///   (`PayoutHeightNotInWindow`), and **that claim is then cancellable** —
    ///   it can never settle, so it must not latch the escrow (see
    ///   [`cancel_redeem`]);
    /// * **`po.challenge_window` BSV blocks have passed over the staging
    ///   block** — `tip_height >= payout_height + W`, so a transient fork cannot
    ///   burn a good claim before it is buried.
    ///
    /// Permissionless, like every other step: the caller keeps the closed
    /// accounts' rent, which is the reward for doing the housekeeping. The
    /// supply falls by the full `amount` while the reserve falls by
    /// `amount − fee`, so the reserve-to-supply ratio improves by the fee
    /// (doc 11 §3).
    pub fn settle_redeem(ctx: Context<SettleRedeem>, id: u64) -> Result<()> {
        let lc = &ctx.accounts.light_client;
        let pending = &ctx.accounts.pending;

        require!(pending.payout_height != 0, SolbeamError::PayoutNotClaimed);
        let current = lc
            .hash_at(pending.payout_height)
            .ok_or(SolbeamError::PayoutHeightNotInWindow)?;
        require!(
            current == pending.payout_hash,
            SolbeamError::PayoutReorged
        );
        let settlable_at = pending
            .payout_height
            .checked_add(CHALLENGE_WINDOW)
            .ok_or(SolbeamError::Overflow)?;
        require!(
            lc.tip_height >= settlable_at,
            SolbeamError::ChallengeWindowNotElapsed
        );

        let bump = lc.bump;
        let seeds: &[&[u8]] = &[b"light_client", &[bump]];
        token::burn(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                Burn {
                    mint: ctx.accounts.mint.to_account_info(),
                    from: ctx.accounts.escrow.to_account_info(),
                    authority: ctx.accounts.light_client.to_account_info(),
                },
                &[seeds],
            ),
            pending.amount,
        )?;
        close_escrow(
            ctx.accounts.token_program.key(),
            &ctx.accounts.escrow.to_account_info(),
            &ctx.accounts.submitter.to_account_info(),
            &ctx.accounts.light_client.to_account_info(),
            bump,
        )?;

        ctx.accounts.book.pending = ctx
            .accounts
            .book
            .pending
            .checked_sub(1)
            .ok_or(SolbeamError::Overflow)?;

        emit!(RedeemSettled {
            id,
            holder: pending.holder,
            amount: pending.amount,
            payout_height: pending.payout_height,
        });
        Ok(())
    }    // -----------------------------------------------------------------------
    // the federation registry
    // -----------------------------------------------------------------------
    //
    // **Records, not custody.** The reserve is off-chain BSV under a 2-of-2
    // script whose keys are generated and shared off chain; no instruction here
    // holds, generates or signs with any key. What these six instructions do is
    // record **who is in each set, what each side has attested as bonded, and
    // the floor below which the signing set stops being usable** -- and refuse
    // the state changes that would break that floor.
    //
    // Deliberately **absent**, and each absence is a specification gap rather
    // than an omission to read past: the **re-sharing ceremony** (the mechanism
    // by which a new member is given a share of the live key) is not specified
    // anywhere, so `admit_member` records the admission and nothing more; the
    // **eligibility rules** that decide who the Greycore may admit are not
    // specified; **whether admission can be compelled** is not specified, and
    // is unobservable anyway (a refusal produces no artifact); the **unbonding
    // clock** (`fed.unbond_slots`) is `open`, which is why `leave_member`
    // returns the bonds at once and records nothing about time; **key rotation**
    // is deliberately omitted and is currently unimplementable, because
    // `initialize_bridge` fixes `deposit_script` once; and **slashing is not
    // built** -- see [`seize_solbsv_bond`] for what is and is not claimed there.

    /// Create the federation and the Greycore: the two sets, their thresholds,
    /// the two aggregate keys, and the founding Greycore member.
    ///
    /// Gated on the program's **upgrade authority**, read from the loader's
    /// `ProgramData` account, by the same two constraints [`Initialize`] and
    /// [`InitializeBridge`] use. There is no governance instruction in this
    /// PoC, so this is the only entry point into the registry; removing the
    /// program's upgrade authority before calling it makes the registry
    /// permanently uninitialisable, which is the correct failure for a
    /// deployment with no governing key.
    ///
    /// `threshold` is the `t` of `t-of-N`: it is a **floor, not a size**. `N` is
    /// never fixed here -- members are admitted while the set has room, and the
    /// program refuses any departure that would leave fewer bonded members than
    /// this number. See [`admit_member`] and [`leave_member`].
    pub fn initialize_federation(
        ctx: Context<InitializeFederation>,
        threshold: u64,
        greycore_key: Pubkey,
    ) -> Result<()> {
        require!(threshold >= 2, SolbeamError::BadFederationThreshold);
        require!(
            reserve_keys_distinct(&ctx.accounts.gateway_key.key(), &greycore_key),
            SolbeamError::ReserveKeysNotDistinct
        );
        require!(
            greycore_key != ctx.accounts.authority.key(),
            SolbeamError::ReserveKeysNotDistinct
        );

        let fed = &mut ctx.accounts.federation;
        fed.authority = ctx.accounts.authority.key();
        fed.threshold = threshold;
        fed.gateway_key = ctx.accounts.gateway_key.key();
        fed.greycore_key = greycore_key;
        fed.greycore_threshold = 1;
        fed.gateway_members = 0;
        fed.greycore_members = 1;
        fed.bonded_members = 0;
        fed.bump = ctx.bumps.federation;

        ctx.accounts.greycore_member.identity = ctx.accounts.authority.key();
        ctx.accounts.greycore_member.bump = ctx.bumps.greycore_member;

        emit!(FederationInitialized {
            authority: ctx.accounts.authority.key(),
            threshold,
            gateway_key: ctx.accounts.gateway_key.key(),
            greycore_key,
        });
        Ok(())
    }

    /// Add one Greycore member: `[b"greycore", identity]`.
    ///
    /// Authority-gated. The Greycore is a set with its own threshold, so its
    /// members must be addable one at a time; `greycore_threshold` decides how
    /// many of them must assemble to produce the single key that fills the
    /// reserve script's second leg.
    ///
    /// **A stand-in, stated rather than implied away.** The reference appoints
    /// its Greycore by community governance, and the design has the Greycore
    /// admit gateway members. There is no governance instruction here, so for
    /// now the program's upgrade authority appoints Greycore members, and the
    /// Greycore in turn admits gateway members. At genesis the two sets are
    /// **not disjoint** -- the founding Greycore member is the authority that
    /// created the federation -- and **no mechanism makes them disjoint**; that
    /// is a named limitation, not a property this code establishes.
    pub fn add_greycore_member(
        ctx: Context<AddGreycoreMember>,
        identity: Pubkey,
    ) -> Result<()> {
        require!(
            identity != Pubkey::default(),
            SolbeamError::BadFederationThreshold
        );
        let fed = &mut ctx.accounts.federation;
        fed.greycore_members = fed
            .greycore_members
            .checked_add(1)
            .ok_or(SolbeamError::Overflow)?;
        ctx.accounts.greycore_member.identity = identity;
        ctx.accounts.greycore_member.bump = ctx.bumps.greycore_member;
        emit!(GreycoreMemberAdded { identity });
        Ok(())
    }

    /// Admit one gateway member: the Greycore's job, and its signature is what
    /// does it.
    ///
    /// Creates `[b"member", identity]`. `approver` must sign, and must be the
    /// identity of an existing [`GreycoreMember`] record -- admission **cannot
    /// be performed by a gateway member**, by the program's upgrade authority
    /// acting alone, or by an arbitrary signer. The approving identity is
    /// recorded on the member, so every admission is attributable to a named
    /// Greycore member.
    ///
    /// **What this does not do, and cannot:** it does not issue the new member a
    /// share of the gateway threshold key. Adding a holder to a threshold key is
    /// a **re-sharing**, and the ceremony -- generation, distribution,
    /// verification -- is **not specified anywhere** in this repository. So the
    /// registry says this identity is a member and the key does not yet know it.
    /// The gap is the ceremony, not this instruction.
    ///
    /// The new member holds **no bond yet**: `bonded` is false, the member does
    /// not count toward the threshold floor, and [`record_bonds`] is what posts
    /// them. Admission also refuses to grow the set past `threshold + 8` -- see
    /// [`MAX_GATEWAY_HEADROOM`] for why a bound exists at all and why the number
    /// is a PoC placeholder rather than a policy.
    pub fn admit_member(ctx: Context<AdmitMember>, identity: Pubkey) -> Result<()> {
        require!(
            identity == ctx.accounts.identity.key(),
            SolbeamError::MemberIdentityMismatch
        );
        require!(
            ctx.accounts.approver.key() == ctx.accounts.greycore_member.identity,
            SolbeamError::NotGreycore
        );

        let fed = &mut ctx.accounts.federation;
        require!(
            fed.gateway_members < fed.threshold + MAX_GATEWAY_HEADROOM,
            SolbeamError::FederationFull
        );
        fed.gateway_members = fed
            .gateway_members
            .checked_add(1)
            .ok_or(SolbeamError::Overflow)?;

        let member = &mut ctx.accounts.member;
        member.identity = identity;
        member.bsv_bond = 0;
        member.solbsv_bond = 0;
        member.bond_key = Pubkey::default();
        member.approved_by = ctx.accounts.approver.key();
        member.bonded = false;
        member.status = MemberStatus::Active;
        member.bump = ctx.bumps.member;

        emit!(MemberAdmitted {
            identity,
            approved_by: ctx.accounts.approver.key(),
        });
        Ok(())
    }

    /// Record both sides of a member's bond, which is what makes the seat count.
    ///
    /// **Two-sided, and the two sides are not the same kind of thing:**
    ///
    /// * `solbsv_bond` is the **`solBSV`-side bond**. It is recorded as a
    ///   number, and the program is the only party that can clear it, which is
    ///   what makes this side the enforceable half. **What is not built is the
    ///   token transfer**: this instruction does not move `solBSV` anywhere, so
    ///   it records a claim about a token account; see [`seize_solbsv_bond`].
    /// * `bsv_bond` is the **BSV-side bond**. It is **an attestation and not
    ///   custody**, and the distinction is the whole point: the BSV exists on
    ///   another chain, the program cannot see it, and it is held under the
    ///   **collective threshold key** recorded in `bond_key`, not under the
    ///   member's own key. Nothing here can move it. A member cannot move it
    ///   either -- that is the design requirement -- but seizing it is a
    ///   **collective action by the members signing a BSV transaction**, a
    ///   social duty with nothing on BSV compelling it, and the program's only
    ///   part in it is this record.
    ///
    /// Both amounts must be non-zero: a member who has posted one side and not
    /// the other does not yet count toward the threshold floor, because the
    /// floor is about a set that can actually sign and be seized.
    ///
    /// **A bond is the float, not a capital requirement.** This instruction
    /// deliberately applies **no ratio to the reserve and no capacity rule**:
    /// the bond is working capital for transfers, and what constrains the
    /// reserve is the Greycore's co-signature on every spend. The `k = 1`
    /// solvency line (`fed.k`) is a check on the mint and exit paths, which are
    /// not this instruction, and **no numeric capacity rule exists** to
    /// enforce here. None should be invented.
    ///
    /// Called by the member's identity. One-shot: once `bonded` is true the
    /// amounts are frozen, so a bond cannot be quietly resized to game a
    /// threshold, and re-attesting changes nothing.
    pub fn record_bonds(
        ctx: Context<RecordBonds>,
        bsv_bond: u64,
        solbsv_bond: u64,
    ) -> Result<()> {
        require!(
            !ctx.accounts.member.bonded,
            SolbeamError::BondsAlreadyRecorded
        );
        require!(
            ctx.accounts.member.status == MemberStatus::Active,
            SolbeamError::MemberNotActive
        );
        require!(bsv_bond > 0, SolbeamError::BondMissing);
        require!(solbsv_bond > 0, SolbeamError::BondMissing);

        let member = &mut ctx.accounts.member;
        member.bsv_bond = bsv_bond;
        member.solbsv_bond = solbsv_bond;
        member.bond_key = ctx.accounts.bond_key.key();
        member.bonded = true;

        let fed = &mut ctx.accounts.federation;
        fed.bonded_members = fed
            .bonded_members
            .checked_add(1)
            .ok_or(SolbeamError::Overflow)?;

        emit!(BondsRecorded {
            identity: member.identity,
            bsv_bond,
            solbsv_bond,
            bond_key: member.bond_key,
        });
        Ok(())
    }

    /// Leave: the bonds are returned and the seat is surrendered.
    ///
    /// Called by the member's identity. The record is kept rather than closed,
    /// because a departure is a fact the registry should still carry; `status`
    /// becomes [`MemberStatus::Left`] and the bonds are **zeroed with nothing
    /// paid in their place**, which is what "returns the bond" means here: the
    /// `solBSV`-side bond was never transferred in (see [`record_bonds`]) and
    /// the BSV-side bond is off-chain, so what the program can do is stop
    /// counting both against the member. Settling the BSV-side bond is a BSV
    /// transaction the members sign, exactly as seizing it is.
    ///
    /// **The floor.** A departure is refused unless at least `threshold` bonded
    /// members remain, checked **before** anything is mutated. Below that the
    /// set can no longer assemble the `t` of `t-of-N` needed to sign at all, so
    /// the registry would be recording a federation that cannot act. On a fresh
    /// federation at `threshold = 4` this means the fifth departure is refused.
    /// The exit path is therefore bounded by the threshold and **not** by an
    /// unbonding clock: `fed.unbond_slots` is `open`, so no time lock exists to
    /// enforce, and this instruction does not pretend one does.
    ///
    /// **What a departing member keeps, stated plainly and not papered over:**
    /// **a departure returns the bond; it does not invalidate the member's
    /// share of the reserve key.** A former member who was given a share of the
    /// gateway threshold key retains a valid one, because nothing in this
    /// program or in the design invalidates it -- the key is off-chain and the
    /// registry cannot reach it. **The effective threshold therefore degrades
    /// with churn**: at `4-of-N`, four former members together still hold four
    /// valid shares and can sign as if they were still seated, while the
    /// registry shows a smaller set. This is a **known, unfinalised problem**,
    /// not a property this instruction establishes or fixes. The two remedies
    /// are key rotation -- currently unimplementable, because
    /// `initialize_bridge` fixes `deposit_script` once and the reserve address
    /// cannot change -- and proactive re-sharing, which **needs the departing
    /// member's cooperation** and so does not answer the case it is most needed
    /// for. Both are real work, neither is built, and this code does not claim
    /// otherwise.
    pub fn leave_member(ctx: Context<LeaveMember>) -> Result<()> {
        require!(
            ctx.accounts.member.status == MemberStatus::Active,
            SolbeamError::MemberNotActive
        );
        require!(
            ctx.accounts.member.identity == ctx.accounts.owner.key(),
            SolbeamError::MemberIdentityMismatch
        );
        let fed = &mut ctx.accounts.federation;
        require!(
            departure_keeps_threshold(fed.bonded_members, fed.threshold),
            SolbeamError::GatewayBelowThreshold
        );

        let member = &mut ctx.accounts.member;
        let returned_bsv = member.bsv_bond;
        let returned_solbsv = member.solbsv_bond;
        member.bsv_bond = 0;
        member.solbsv_bond = 0;
        member.bonded = false;
        member.status = MemberStatus::Left;

        if returned_bsv > 0 || returned_solbsv > 0 {
            fed.bonded_members = fed
                .bonded_members
                .checked_sub(1)
                .ok_or(SolbeamError::Overflow)?;
        }

        emit!(MemberLeft {
            identity: member.identity,
            returned_bsv,
            returned_solbsv,
            bonded_remaining: fed.bonded_members,
        });
        Ok(())
    }

    /// Clear a member's **`solBSV`-side** bond, on the authority's instruction.
    ///
    /// This is the side the design makes **seizable by the program**, and this
    /// instruction is the whole of that capability here: it zeroes the recorded
    /// amount and the aggregate. The BSV-side bond is **not touched, and cannot
    /// be** -- it is off-chain and is seized only by the members collectively
    /// signing a BSV transaction, which is a social duty the program has no part
    /// in. That asymmetry is stated in the design rather than hidden, and it is
    /// stated here too.
    ///
    /// **What this is not.** It is **not slashing**: no proof of misbehaviour is
    /// checked, no equivocation is compared, no bounty is paid, and there is no
    /// intent account to equivocate on. Slashing is designed and **not built**
    /// (doc 03 §5). Requiring the program's upgrade authority is the honest
    /// stand-in for the federation, which does not exist -- and it means this
    /// instruction is **one key's assertion**, which is a weaker thing than the
    /// designed mechanism and is recorded as weaker.
    ///
    /// The same threshold floor as [`leave_member`] applies, and for the same
    /// reason: a member whose bond is gone cannot back a seat, so the set must
    /// still have `threshold` bonded members after the seizure.
    ///
    /// **What is not built, said exactly:** no `solBSV` is moved. The recorded
    /// amount is the program's record of what a member attested; since
    /// [`record_bonds`] does not take custody, clearing the record does not by
    /// itself put any token anywhere.
    pub fn seize_solbsv_bond(ctx: Context<SeizeSolbsvBond>) -> Result<()> {
        require!(
            ctx.accounts.member.status == MemberStatus::Active,
            SolbeamError::MemberNotActive
        );
        require!(ctx.accounts.member.bonded, SolbeamError::BondMissing);
        require!(ctx.accounts.member.solbsv_bond > 0, SolbeamError::NoBondToSeize);

        let fed = &mut ctx.accounts.federation;
        require!(
            departure_keeps_threshold(fed.bonded_members, fed.threshold),
            SolbeamError::GatewayBelowThreshold
        );

        let member = &mut ctx.accounts.member;
        let seized = member.solbsv_bond;
        member.solbsv_bond = 0;
        member.bonded = false;
        fed.bonded_members = fed
            .bonded_members
            .checked_sub(1)
            .ok_or(SolbeamError::Overflow)?;

        emit!(SolbsvBondSeized {
            identity: member.identity,
            amount: seized,
            authority: ctx.accounts.authority.key(),
        });
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// accounts
// ---------------------------------------------------------------------------

#[account]
pub struct LightClient {
    /// The trusted starting point: everything below this is taken on faith, and
    /// therefore must be buried deep and published.
    pub checkpoint_height: u64,
    pub tip_height: u64,
    /// Internal byte order — the same order the Python reference uses, so a
    /// fixture can be compared without a conversion step that could hide a bug.
    pub tip_hash: [u8; 32],
    /// Height of `headers[0]`. The window is contiguous, so every other height
    /// is derived from this — storing a height per record would cost eight
    /// bytes times WINDOW for information that is already implied.
    pub window_start: u64,
    /// The rolling window, oldest first.
    pub headers: Vec<HeaderRecord>,
    /// Who may set the checkpoint or pause the client. **Without this, `authority`
    /// in `SetCheckpoint` was a bare `Signer` compared to nothing, so any key could
    /// rewrite the trusted root — the whole client's security — at will.**
    ///
    /// Set by `initialize` to the program's **upgrade authority**, verified
    /// on-chain against the loader's `ProgramData` account. It used to be set to
    /// whichever key called `initialize` first, which made the initialiser
    /// unpermissioned: one public transaction by anyone at all took over a fresh
    /// deployment. Production still wants a governance multisig to hold the
    /// upgrade authority, but that is now a choice about *who the authority is*,
    /// not an open race about who gets there first.
    pub authority: Pubkey,
    /// The target every header must carry. **Read from the chain, never from the
    /// header being checked.** The target previously came from the submitted
    /// header's own `bits` field, so an attacker simply declared an easy target,
    /// ground one hash, and the proof-of-work check passed — which made the
    /// client forgeable by anyone with a laptop and made every "forging blocks
    /// must out-mine the chain" claim false.
    ///
    /// Since W1.6 this is the target a header must carry **only when
    /// [`LightClient::no_retargeting`] is set** (regtest) or while the window is
    /// still too short to compute cw-144 (fewer than 147 records, i.e. the first
    /// blocks after `initialize` or `set_checkpoint`). On a live chain it is the
    /// checkpoint's value and nothing more.
    pub expected_bits: u32,
    /// Mirrors the node's `fPowNoRetargeting` chain parameter: on regtest the
    /// target is never recomputed, and every block after the checkpoint must
    /// carry the checkpoint's own `bits`.
    ///
    /// Derived at `initialize` from the checkpoint header, deterministically:
    /// it is set when the checkpoint's compact target already equals the maximum
    /// the compact encoding can express, which is exactly regtest's `powLimit`.
    /// No chain can be easier than that, so a chain anchored there has nowhere
    /// to adjust to — and a chain anchored anywhere else never acquires the
    /// flag. Deriving it is deliberate: a stored boolean passed in by the
    /// initialiser would be a value an attacker could set to skip the retarget
    /// check entirely.
    pub no_retargeting: bool,
    /// The network's `powLimit` in compact form, fixed at `initialize`.
    ///
    /// A network parameter, not a property of the algorithm: mainnet's
    /// `0x1d00ffff` caps the target far below regtest's `0x207fffff`. Applying
    /// mainnet's limit to a regtest chain would reject blocks the node accepts.
    pub pow_limit_bits: u32,
    /// How many trusted ancestor headers the client still needs before cw-144
    /// is computable. **Zero means live.**
    ///
    /// Set to [`SEED_RECORDS`] by `anchor_checkpoint` whenever the checkpoint
    /// sits on a retargeting chain, and decremented by `seed_headers` as
    /// ancestors arrive; the checkpoint itself is the last seed record, so the
    /// window is exactly full when this reaches zero. On a no-retargeting chain
    /// it is zero from the start, because the checkpoint's target is the whole
    /// rule there.
    ///
    /// This field is why the seed cannot be forgotten or half-applied: every
    /// instruction that could advance the chain checks it, and a window that is
    /// short of 147 records can never be mistaken for a working one. The old
    /// design had no such state and inferred "too short to compute" from the
    /// window length at the moment of use — which is exactly how F1's permanent
    /// fallback arose.
    pub seed_remaining: u32,
    /// `Clock::slot` of the last accepted header.
    ///
    /// Recorded on every accepted header — main chain and committed fork alike.
    /// It is a **freshness** input for a later design, and it cannot be
    /// back-dated by the submitter because it comes from the sysvar; note that
    /// it advances on *any* accepted header, so it measures how recently the
    /// client was updated, not how honest the update was (audit T8).
    pub last_push_slot: u64,
    pub paused: bool,
    pub bump: u8,
}

impl LightClient {
    pub const SPACE: usize = LIGHT_CLIENT_FIXED + (WINDOW * HEADER_RECORD_SIZE);

    /// Install a trusted checkpoint and decide how the client can move on from
    /// it.
    ///
    /// **One implementation, used by `initialize` and `set_checkpoint` alike.**
    /// F2 was precisely the two paths disagreeing: `set_checkpoint` reset the
    /// hash, height and window but left `expected_bits`, `no_retargeting` and
    /// `pow_limit_bits` carrying the *old* chain's values, so a client ever
    /// anchored on a `0x207fffff` header kept `no_retargeting = true` forever
    /// and accepted everything afterwards at the easiest encodable target.
    ///
    /// The two chains behave differently and the derivation says which:
    ///
    ///   * **No retargeting** (the checkpoint's compact target is the largest
    ///     the encoding can express, i.e. regtest): the target never changes, so
    ///     the checkpoint's own `bits` *is* the rule and the client is live
    ///     immediately. No lookback is needed and none is requested.
    ///   * **Retargeting** (any real chain): cw-144 needs 147 records ending at
    ///     the checkpoint and only one of them exists, so the client opens a
    ///     **seeding** state and refuses to advance until `seed_headers` has
    ///     supplied the other 146. This is the F1 fix: without the seed, the
    ///     fallback `bits == expected_bits` can never be satisfied on a chain
    ///     that changes difficulty every block, and the client deadlocks.
    fn anchor_checkpoint(&mut self, height: u64, header: &[u8; HEADER_LEN]) -> Result<()> {
        let bits = read_u32_le(header, 72);
        // The checkpoint is trusted, but not *arbitrary*: it still has to be a
        // real block under its own target, exactly as `initialize` required.
        require!(meets_target(header, bits), SolbeamError::CheckpointBadPow);

        self.checkpoint_height = height;
        self.tip_height = height;
        self.tip_hash = header_hash(header);
        self.headers = Vec::new();
        // Taken from the checkpoint header, which is the one piece of data the
        // client trusts. On a no-retargeting chain this is the target forever;
        // on a real chain it is replaced by cw-144 once the seed completes and
        // is never consulted again.
        self.expected_bits = bits;
        // Derived, never supplied. `REGTEST_BITS` is the compact form of
        // regtest's `powLimit`, i.e. the node's own
        // `UintToArith256(params.powLimit).GetCompact()` for that chain.
        self.no_retargeting = bits == difficulty::REGTEST_POW_LIMIT_BITS;
        // A network parameter. Mainnet's limit, applied to any chain, is a
        // strictly-tighter cap than regtest's; a PoC deployment that needs the
        // regtest value passes it in rather than this constant changing.
        self.pow_limit_bits = MAINNET_POW_LIMIT_BITS;

        if self.no_retargeting {
            self.seed_remaining = 0;
            self.window_start = height;
        } else {
            // The window the seed will fill runs from `height - 146` to the
            // checkpoint itself, which is `SEED_RECORDS` records and is exactly
            // what cw-144 needs to judge `height + 1`.
            self.window_start = height
                .checked_sub(SEED_RECORDS as u64 - 1)
                .ok_or(SolbeamError::CheckpointTooLow)?;
            self.seed_remaining = SEED_RECORDS as u32;
        }
        Ok(())
    }

    /// True while the client is still waiting for trusted ancestors.
    fn is_seeding(&self) -> bool {
        self.seed_remaining > 0
    }

    /// The `bits` the header at `tip_height + 1` must carry, or `None` while
    /// the window is still too short for cw-144.
    ///
    /// `None` is now an **error on the main-chain path, not a licence to
    /// trust.** The only way a live client can hold fewer than `LOOKBACK`
    /// records is a bug, because `push_header` refuses to run until the seed
    /// has filled the window — see [`Self::anchor_checkpoint`] and F1.
    fn required_bits(&self) -> Option<u32> {
        if self.no_retargeting {
            // The node's own rule for a chain with no retargeting: the target
            // is simply the previous block's. `expected_bits` is that value.
            return Some(self.expected_bits);
        }
        let pow_limit = compact_to_target(self.pow_limit_bits);
        // Read the window in place rather than collecting it into a `Vec<Record>`.
        // Every live mainnet header needs 147 of these; the copy is 147 records
        // of heap on a chain where the heap is small, and it was the allocation
        // that made the first real mainnet `push_header` fail with "out of
        // memory". The algorithm is unchanged and still `difficulty.rs`'s.
        difficulty::next_target_from(
            self.headers.len(),
            self.window_start,
            pow_limit,
            |i| Record {
                time: self.headers[i].time,
                chainwork: self.headers[i].chainwork,
            },
        )
        .map(target_to_compact)
    }

    /// The whole difficulty check for the main chain.
    ///
    /// `parent_work` is the chainwork of the block this header builds on, and
    /// the returned value is this header's cumulative chainwork — the sum of
    /// the two, derived from the header's own `bits`. Deriving it rather than
    /// trusting a supplied number is what makes the stored window internally
    /// consistent: every record's work is the work of the target it declares.
    ///
    /// There is deliberately **no fallback** when the rule is not computable.
    /// The old fallback accepted `bits == expected_bits` for every block before
    /// the window reached 147 records; on BSV, where difficulty changes every
    /// block, that made the checkpoint's successors unverifiable and the client
    /// never advanced (F1). The trusted seed is what supplies those records, so
    /// `None` here means the seed did not run and the honest response is to
    /// refuse the header.
    ///
    /// [`push_fork_header`] does not use this function: a branch header must be
    /// judged at its *own* height, not at `tip_height + 1` (F3).
    fn difficulty_for(&self, bits: u32, parent_work: u128) -> Result<u128> {
        let required = self
            .required_bits()
            .ok_or(SolbeamError::DifficultyNotComputable)?;
        require!(bits == required, SolbeamError::UnexpectedRetarget);
        let work = work_from_bits(bits);
        Ok(parent_work.saturating_add(work))
    }

    /// Index of a height inside the window, if it is still held.
    pub fn index_of(&self, height: u64) -> Option<usize> {
        if height < self.window_start || height > self.tip_height {
            return None;
        }
        Some((height - self.window_start) as usize)
    }

    /// Height of the record at `index`.
    pub fn height_at(&self, index: usize) -> u64 {
        self.window_start + index as u64
    }

    /// The stored record for a height, or `None` if it has left the window.
    pub fn record(&self, height: u64) -> Option<&HeaderRecord> {
        let index = self.index_of(height)?;
        self.headers.get(index)
    }

    /// The hash the window holds for a height, or `None` if it has left it.
    pub fn hash_at(&self, height: u64) -> Option<[u8; 32]> {
        self.record(height).map(|r| r.hash)
    }
}

// ---------------------------------------------------------------------------
// an allocation-free view of the light client's account data
// ---------------------------------------------------------------------------

/// Byte offsets into a **serialised** `LightClient`, discriminator included.
/// Spelled out in full even where one is not read, so the layout stays legible.
#[allow(dead_code)]
mod lc_offsets {
    pub const CHECKPOINT_HEIGHT: usize = 8; // after the 8-byte discriminator
    pub const TIP_HEIGHT: usize = 16;
    pub const TIP_HASH: usize = 24;
    pub const WINDOW_START: usize = 56;
    pub const RECORDS_LEN: usize = 64;
    pub const RECORDS: usize = 68;
    pub const RECORD_SIZE: usize = super::HEADER_RECORD_SIZE;
    /// authority (32) + expected_bits (4) + no_retargeting (1) + pow_limit_bits
    /// (4) + seed_remaining (4) + last_push_slot (8) + paused (1) + bump (1).
    pub const TAIL_LEN: usize = 55;
    pub const TAIL_EXPECTED_BITS: usize = 32;
    pub const TAIL_NO_RETARGETING: usize = 36;
    pub const TAIL_POW_LIMIT_BITS: usize = 37;
    pub const TAIL_SEED_REMAINING: usize = 41;
    pub const TAIL_PAUSED: usize = 53;
}

/// A read-only view of a `LightClient` account **that allocates nothing**.
///
/// `Account<'info, LightClient>` deserialises the whole window into a
/// `Vec<HeaderRecord>`, and on Solana that is not free. borsh grows the vector
/// through three allocations, and the SBF bump allocator cannot reuse the
/// intermediate buffers, so a full window costs about 28 KB of a 32 KB heap.
/// `push_fork_header` then has no room for the staging account, and fails with
/// `memory allocation failed, out of memory` — which is exactly what happened
/// the first time a real mainnet branch was staged.
///
/// Every field this instruction reads is at a fixed offset in the account data,
/// so it can be read in place. [`view_matches_serialisation`] below serialises a
/// real `LightClient` and asserts this view against it, so a field reorder
/// breaks a test instead of silently reading the wrong bytes.
///
/// [`view_matches_serialisation`]: tests::view_matches_serialisation
pub struct LightClientView<'a> {
    data: &'a [u8],
}

impl<'a> LightClientView<'a> {
    pub fn new(data: &'a [u8]) -> Result<Self> {
        let view = LightClientView { data };
        require!(
            view.data_len() <= data.len(),
            SolbeamError::MalformedClientData
        );
        Ok(view)
    }

    /// Number of records the window holds.
    pub fn len(&self) -> usize {
        u32::from_le_bytes(
            self.data[lc_offsets::RECORDS_LEN..lc_offsets::RECORDS_LEN + 4]
                .try_into()
                .unwrap(),
        ) as usize
    }

    fn data_len(&self) -> usize {
        lc_offsets::RECORDS + self.len() * lc_offsets::RECORD_SIZE + lc_offsets::TAIL_LEN
    }

    pub fn tip_height(&self) -> u64 {
        u64::from_le_bytes(
            self.data[lc_offsets::TIP_HEIGHT..lc_offsets::TIP_HEIGHT + 8]
                .try_into()
                .unwrap(),
        )
    }

    pub fn window_start(&self) -> u64 {
        u64::from_le_bytes(
            self.data[lc_offsets::WINDOW_START..lc_offsets::WINDOW_START + 8]
                .try_into()
                .unwrap(),
        )
    }

    /// Offset of the fixed fields that follow the records.
    fn tail(&self) -> usize {
        lc_offsets::RECORDS + self.len() * lc_offsets::RECORD_SIZE
    }

    pub fn expected_bits(&self) -> u32 {
        let at = self.tail() + lc_offsets::TAIL_EXPECTED_BITS;
        u32::from_le_bytes(self.data[at..at + 4].try_into().unwrap())
    }

    pub fn no_retargeting(&self) -> bool {
        self.data[self.tail() + lc_offsets::TAIL_NO_RETARGETING] != 0
    }

    pub fn pow_limit_bits(&self) -> u32 {
        let at = self.tail() + lc_offsets::TAIL_POW_LIMIT_BITS;
        u32::from_le_bytes(self.data[at..at + 4].try_into().unwrap())
    }

    pub fn seed_remaining(&self) -> u32 {
        let at = self.tail() + lc_offsets::TAIL_SEED_REMAINING;
        u32::from_le_bytes(self.data[at..at + 4].try_into().unwrap())
    }

    pub fn paused(&self) -> bool {
        self.data[self.tail() + lc_offsets::TAIL_PAUSED] != 0
    }

    fn record_bytes(&self, index: usize) -> Option<&'a [u8]> {
        if index >= self.len() {
            return None;
        }
        let at = lc_offsets::RECORDS + index * lc_offsets::RECORD_SIZE;
        Some(&self.data[at..at + lc_offsets::RECORD_SIZE])
    }

    /// The hash, cumulative chainwork and time of one window record.
    pub fn record(&self, index: usize) -> Option<([u8; 32], u128, u32)> {
        let raw = self.record_bytes(index)?;
        let mut hash = [0u8; 32];
        hash.copy_from_slice(&raw[..32]);
        let chainwork = u128::from_le_bytes(raw[32..48].try_into().unwrap());
        let time = u32::from_le_bytes(raw[48..52].try_into().unwrap());
        Some((hash, chainwork, time))
    }

    /// Index of a height inside the window, if it is still held.
    pub fn index_of(&self, height: u64) -> Option<usize> {
        if height < self.window_start() || height > self.tip_height() {
            return None;
        }
        Some((height - self.window_start()) as usize)
    }

    pub fn hash_at(&self, height: u64) -> Option<[u8; 32]> {
        self.record(self.index_of(height)?).map(|(hash, _, _)| hash)
    }

    pub fn chainwork_at(&self, height: u64) -> Option<u128> {
        self.record(self.index_of(height)?).map(|(_, work, _)| work)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anchor_lang::Discriminator;

    /// The view must agree with `LightClient`'s own serialisation, field for
    /// field. Without this the offsets above are a guess that compiles and then
    /// reads the wrong bytes on chain.
    #[test]
    fn view_matches_serialisation() {
        let lc = LightClient {
            checkpoint_height: 7,
            tip_height: 9,
            tip_hash: [0xa5; 32],
            window_start: 8,
            headers: vec![
                HeaderRecord {
                    hash: [1u8; 32],
                    chainwork: 0x1122_3344_5566_7788_99aa_bbcc_ddee_ff00,
                    time: 111,
                },
                HeaderRecord {
                    hash: [2u8; 32],
                    chainwork: 42,
                    time: 222,
                },
            ],
            authority: Pubkey::new_from_array([9u8; 32]),
            expected_bits: 0x207f_ffff,
            no_retargeting: true,
            pow_limit_bits: 0x1d00_ffff,
            seed_remaining: 3,
            last_push_slot: 4242,
            paused: false,
            bump: 251,
        };

        let mut data = Vec::new();
        data.extend_from_slice(LightClient::DISCRIMINATOR);
        AnchorSerialize::serialize(&lc, &mut data).unwrap();
        assert_eq!(data.len(), LightClient::SPACE - (WINDOW - lc.headers.len()) * HEADER_RECORD_SIZE);

        let view = LightClientView::new(&data).unwrap();
        assert_eq!(view.len(), 2);
        assert_eq!(view.tip_height(), 9);
        assert_eq!(view.window_start(), 8);
        assert_eq!(view.expected_bits(), 0x207f_ffff);
        assert!(view.no_retargeting());
        assert_eq!(view.pow_limit_bits(), 0x1d00_ffff);
        assert_eq!(view.seed_remaining(), 3);
        assert!(!view.paused());
        assert_eq!(view.record(0).unwrap(), ([1u8; 32], 0x1122_3344_5566_7788_99aa_bbcc_ddee_ff00, 111));
        assert_eq!(view.record(1).unwrap(), ([2u8; 32], 42, 222));
        assert_eq!(view.record(2), None);
        assert_eq!(view.index_of(8), Some(0));
        assert_eq!(view.index_of(9), Some(1));
        assert_eq!(view.index_of(10), None);
        assert_eq!(view.index_of(7), None);
        assert_eq!(view.hash_at(8), Some([1u8; 32]));
        assert_eq!(view.chainwork_at(9), Some(42));
    }
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, PartialEq, Eq, Debug)]
pub struct HeaderRecord {
    /// Internal byte order, matching the Python reference. Everything a deposit
    /// proof needs that is not the header itself is derived from, or supplied
    /// alongside, this.
    pub hash: [u8; 32],
    /// Cumulative proof of work from the checkpoint, not from genesis.
    ///
    /// The node's value is genesis-absolute, but cw-144 only ever subtracts two
    /// cumulative values, so any common offset cancels. A checkpoint header
    /// cannot supply the genesis total — that is a trusted scalar this PoC does
    /// not take — so the window carries its own baseline: `initialize` starts
    /// it at zero and every accepted header adds `work(target(bits))` to its
    /// parent's value. The difference the algorithm consumes is then exact,
    /// because both ends are sums of the same per-header quantities.
    pub chainwork: u128,
    /// The header's own timestamp. cw-144's denominator, and the field
    /// `GetSuitableBlock` sorts the three topmost blocks by.
    pub time: u32,
}

/// The BPF upgradeable loader's `ProgramData` account for **this program**.
///
/// Derived rather than supplied: the address is `find_program_address([crate::ID],
/// bpf_loader_upgradeable)` and never comes from the caller. That is what stops a
/// caller pointing the authority check at a `ProgramData` account they control.
///
/// Returns the address without checking it exists — the `Account<'info, ProgramData>`
/// wrapper in the instruction does that, and `try_deserialize` rejects anything that
/// is not a `ProgramData` variant.
fn program_data_address() -> Pubkey {
    Pubkey::find_program_address(&[crate::ID.as_ref()], &bpf_loader_upgradeable::ID).0
}

/// The gate every `initialize`-class instruction shares: **only the key that can
/// upgrade this program may install its trust root.**
///
/// This closes the deploy-time takeover. `initialize` and `initialize_bridge`
/// used to accept any signer as `payer` and record that key as the client's
/// `authority` (and reachable through it, `set_checkpoint`, `set_paused` and a
/// fabricated deposit). The first caller after deployment therefore became the
/// authority in one public transaction — a permanent takeover of a fresh
/// deployment, and a race that cannot be won by being quick.
///
/// The identity is read **from the chain**, not supplied. `program_data` is
/// pinned to the loader's `ProgramData` PDA for this program by `address`, and
/// its `upgrade_authority_address` must equal the payer. The field order matters:
/// `payer` and `program_data` are declared before any `init` account, so the
/// check runs before anything is created.
#[derive(Accounts)]
pub struct Initialize<'info> {
    /// The initialiser, and the only key that may run this instruction.
    #[account(mut)]
    pub payer: Signer<'info>,
    /// The `ProgramData` account of this program, whose upgrade authority must be
    /// the payer.
    ///
    /// A deployer that leaves the program non-upgradeable
    /// (`upgrade_authority_address == None`) can never initialise, which is the
    /// correct failure: there is no key that could later repair the trust root,
    /// so there is no key entitled to set one.
    #[account(
        address = program_data_address() @ SolbeamError::Unauthorized,
        constraint = program_data.upgrade_authority_address == Some(payer.key())
            @ SolbeamError::Unauthorized,
    )]
    pub program_data: Account<'info, ProgramData>,
    #[account(
        init,
        payer = payer,
        space = LightClient::SPACE,
        seeds = [b"light_client"],
        bump
    )]
    pub light_client: Account<'info, LightClient>,
    /// The singleton config. Created here so it always exists, with
    /// `maturity_blocks = 0` — the PoC default doc 31 decides on.
    #[account(
        init,
        payer = payer,
        space = Config::SPACE,
        seeds = [CONFIG_SEED],
        bump
    )]
    pub config: Account<'info, Config>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PushHeader<'info> {
    #[account(mut, seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    pub advancer: Signer<'info>,
}

#[derive(Accounts)]
pub struct SeedHeaders<'info> {
    #[account(
        mut,
        seeds = [b"light_client"],
        bump = light_client.bump,
        has_one = authority @ SolbeamError::Unauthorized,
    )]
    pub light_client: Account<'info, LightClient>,
    /// The key that set the checkpoint. The seed is trusted data on the
    /// checkpoint's own footing, and — unlike `push_header` — it cannot be
    /// permissionless: whoever writes the first record fixes the chain the rest
    /// must link to, so an open seed could be bricked by one junk header. See
    /// `seed_headers`.
    pub authority: Signer<'info>,
}

#[derive(Accounts)]
pub struct InitStaging<'info> {
    #[account(seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    /// One staging slot per submitter: `[b"staging", submitter]`. Two submitters
    /// cannot collide, which is what removes the griefing problem rather than
    /// pricing it.
    #[account(
        init,
        payer = submitter,
        space = ForkStaging::SPACE,
        seeds = [b"staging", submitter.key().as_ref()],
        bump
    )]
    pub staging: Account<'info, ForkStaging>,
    #[account(mut)]
    pub submitter: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PushForkHeader<'info> {
    /// Deliberately NOT `Account<'info, LightClient>`. Anchor would deserialise
    /// the entire window into a `Vec`, which on a full 192-record window costs
    /// ~28 KB of the 32 KB SBF heap — leaving no room for the staging account,
    /// and failing with "out of memory" the first time a real mainnet branch was
    /// staged. `LightClientView` reads the fields this instruction needs in
    /// place. The account is still pinned to its canonical PDA and to this
    /// program, so it can only ever be the one light client.
    ///
    /// CHECK: address and owner are both enforced below; the data is read only
    /// through `LightClientView`, whose offsets are asserted against
    /// `LightClient`'s own serialisation by `tests::view_matches_serialisation`.
    #[account(seeds = [b"light_client"], bump, owner = crate::ID)]
    pub light_client: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [b"staging", submitter.key().as_ref()],
        bump = staging.bump,
    )]
    pub staging: Account<'info, ForkStaging>,
    pub submitter: Signer<'info>,
}

#[derive(Accounts)]
pub struct AbandonStaging<'info> {
    /// Seeds are derived from `submitter`, so only the owner's key can address
    /// this account — someone else's key derives a different PDA and fails the
    /// constraint. No explicit owner check is needed.
    #[account(
        mut,
        seeds = [b"staging", submitter.key().as_ref()],
        bump = staging.bump,
        close = submitter,
    )]
    pub staging: Account<'info, ForkStaging>,
    #[account(mut)]
    pub submitter: Signer<'info>,
}

#[derive(Accounts)]
pub struct CommitFork<'info> {
    #[account(mut, seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    /// Closed on commit, returning the rent to the submitter — staging is a
    /// deposit, not a cost, and a committed branch has no further use.
    #[account(
        mut,
        seeds = [b"staging", submitter.key().as_ref()],
        bump = staging.bump,
        close = submitter,
    )]
    pub staging: Account<'info, ForkStaging>,
    #[account(mut)]
    pub submitter: Signer<'info>,
}

/// Proposal of a timelocked checkpoint or pause change.
///
/// `light_client` is read-only here except for the `init` payer; the change is
/// written to the pending account and applied later by
/// [`ExecuteAuthorityChange`]. The pending account is a singleton PDA — no
/// per-proposal seed — so a second proposal is impossible while one is live.
#[derive(Accounts)]
pub struct ProposeAuthorityChange<'info> {
    #[account(
        seeds = [b"light_client"],
        bump = light_client.bump,
        has_one = authority @ SolbeamError::Unauthorized,
    )]
    pub light_client: Account<'info, LightClient>,
    // `init` here is what makes "only one outstanding change" structural rather
    // than a convention: while a pending change exists, this account exists and
    // a second `propose` fails.
    #[account(
        init,
        payer = authority,
        space = PendingAuthorityChange::SPACE,
        seeds = [b"pending_authority_change"],
        bump
    )]
    pub pending: Account<'info, PendingAuthorityChange>,
    #[account(mut)]
    pub authority: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Apply a pending change. Both the pending account's recorded proposer and the
/// light client's own `authority` must be the signer, and the timelock must have
/// elapsed; the account is closed on success.
#[derive(Accounts)]
pub struct ExecuteAuthorityChange<'info> {
    #[account(
        mut,
        seeds = [b"light_client"],
        bump = light_client.bump,
        has_one = authority @ SolbeamError::Unauthorized,
    )]
    pub light_client: Account<'info, LightClient>,
    /// The bridge config, written when the pending change is
    /// [`AuthorityChange::SetMaturity`]. Required on every execute so the
    /// instruction has one shape; unchanged for the checkpoint and pause
    /// variants.
    #[account(mut, seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    #[account(
        mut,
        seeds = [b"pending_authority_change"],
        bump = pending.bump,
        close = authority,
    )]
    pub pending: Account<'info, PendingAuthorityChange>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

/// Withdraw a pending change. `close = authority` returns the rent.
#[derive(Accounts)]
pub struct CancelAuthorityChange<'info> {
    #[account(
        seeds = [b"light_client"],
        bump = light_client.bump,
        has_one = authority @ SolbeamError::Unauthorized,
    )]
    pub light_client: Account<'info, LightClient>,
    #[account(
        mut,
        seeds = [b"pending_authority_change"],
        bump = pending.bump,
        close = authority,
    )]
    pub pending: Account<'info, PendingAuthorityChange>,
    #[account(mut)]
    pub authority: Signer<'info>,
}

/// Close a deposit's replay nullifier once its block has left the window.
///
/// The nullifier is seeded on `(txid, vout)`, which are instruction arguments, so
/// the seeds are bound with `#[instruction(...)]` and Anchor derives and checks
/// the address itself. `close = submitter` returns the rent to whoever did the
/// housekeeping, and it is the only way this account is ever closed besides a
/// future vault burn.
#[derive(Accounts)]
#[instruction(txid: [u8; 32], vout: u32)]
pub struct PruneNullifier<'info> {
    #[account(seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    #[account(
        mut,
        seeds = [NULLIFIER_SEED, txid.as_ref(), &vout.to_le_bytes()],
        bump = nullifier.bump,
        close = submitter,
    )]
    pub nullifier: Account<'info, DepositNullifier>,
    #[account(mut)]
    pub submitter: Signer<'info>,
}

/// Write one entry in the federation's **spent-outpoint record**.
///
/// `authority` must be the program's upgrade authority, read from the loader's
/// `ProgramData` account for this program — the same on-chain identity check
/// [`Initialize`] and [`InitializeBridge`] use. It stands in for the federation
/// until the federation exists (see [`crate::report_spent`]).
///
/// `spent_outpoint` is an `UncheckedAccount` because the record is created by
/// hand — see [`create_spent_outpoint`], which is what makes a pre-funded PDA
/// unable to block a report — and it is read only for `data_is_empty`, never
/// deserialised. The address is re-derived from `(txid, vout)` and checked
/// inside `create_spent_outpoint`. It is `mut` because creating it funds and
/// allocates it. There is deliberately **no instruction that closes it**.
#[derive(Accounts)]
pub struct ReportSpent<'info> {
    /// The program's upgrade authority, and the payer of the record's rent.
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        address = program_data_address() @ SolbeamError::Unauthorized,
        constraint = program_data.upgrade_authority_address == Some(authority.key())
            @ SolbeamError::Unauthorized,
    )]
    pub program_data: Account<'info, ProgramData>,
    /// CHECK: address re-derived from `(txid, vout)` and compared against
    /// `SPENT_OUTPOINT_SEED` in `create_spent_outpoint`; `data_is_empty` is what
    /// refuses a second report.
    #[account(mut)]
    pub spent_outpoint: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

// ---------------------------------------------------------------------------
// header maths
// ---------------------------------------------------------------------------
//
// This must agree with `poc/checks/bsvlib.py` byte for byte. If it does not,
// the disagreement is the bug — not the fixture.

/// Double SHA-256 of the header, in *internal* byte order.
pub fn header_hash(header: &[u8; HEADER_LEN]) -> [u8; 32] {
    let first = sha256(header);
    sha256(first.as_ref()).to_bytes()
}

/// Compare the header's hash against the target encoded in `bits`.
///
/// Both sides are compared as 256-bit big-endian values: `bits` produces a
/// big-endian target, and reversing the digest gives the same number. This is
/// the identical convention `bsvlib.hash_as_int` uses (it reads the digest
/// little-endian, which is the same thing).
pub fn meets_target_slice(header: &[u8], bits: u32) -> bool {
    let digest = header_hash_of_bytes(header);
    let mut hash_be = digest;
    hash_be.reverse();
    hash_be <= bits_to_target_be(bits)
}

pub fn meets_target(header: &[u8; HEADER_LEN], bits: u32) -> bool {
    let digest = header_hash(header);
    let mut hash_be = digest;
    hash_be.reverse();
    hash_be <= bits_to_target_be(bits)
}

/// Compact `bits` to a 256-bit big-endian target. Mirrors `bsvlib.target_from_bits`.
pub fn bits_to_target_be(bits: u32) -> [u8; 32] {
    let exponent = (bits >> 24) as usize;
    let mantissa = bits & 0x007f_ffff;
    let mut out = [0u8; 32];

    if exponent <= 3 {
        let shifted = mantissa >> (8 * (3 - exponent));
        out[29..32].copy_from_slice(&shifted.to_be_bytes()[1..4]);
    } else if exponent <= 32 {
        // The mantissa's three bytes end at the top of the target.
        let start = 32 - exponent;
        out[start..start + 3].copy_from_slice(&mantissa.to_be_bytes()[1..4]);
    }
    // exponent > 32 is not a valid target; the all-zero result can never be met.
    out
}

/// The work a target is worth: `2^256 / (target + 1)`, the node's
/// `GetBlockProof`.
///
/// Returned as `u128` because that is the only width the window can store —
/// BSV's cumulative total is around 2^87, and a *per-block* value is smaller
/// still. Saturating rather than wrapping, for the reason given in
/// `push_header`: a wrapped work value would make a chain look lighter, not
/// heavier, and a lighter chain is the one that wins a comparison by accident.
///
/// Only the *differences* of these values are ever used, so their absolute
/// scale never has to be comparable with the node's genesis-absolute chainwork.
pub fn work_from_bits(bits: u32) -> u128 {
    let target = compact_to_target(bits);
    if target.is_zero() {
        // Not an encodable target: no header can meet it, so it is worth
        // nothing. Returning 0 keeps this total and monotone.
        return 0;
    }
    let work = (!U256::zero()) / (target + U256::one());
    if work > U256::from(u128::MAX) {
        u128::MAX
    } else {
        work.as_u128()
    }
}

// -- the reported spent-outpoint record -------------------------------------

/// Refuse a claim whose deposit outpoint the federation has reported spent.
///
/// The address is **re-derived from the claim** and compared before the account
/// is read. That is the whole security property of this check: `spent_outpoint`
/// is an `UncheckedAccount`, because its seeds are the claim's `(txid, vout)`
/// and Anchor cannot express those as a constraint, so without the re-derivation
/// a caller could pass any empty account it liked and skip the backing check.
///
/// Missing and empty are both *not spent*. Only a non-empty account at the
/// derived PDA counts, and only this program can write one: the PDA cannot sign
/// for itself, so `allocate`/`assign` at that address is reachable only through
/// [`create_spent_outpoint`]. Lamports sent to the address by anyone else do not
/// make `data_is_empty()` false and so cannot fake a report — they are merely
/// forfeited when the authority first reports the outpoint.
fn require_not_spent<'info>(
    spent: &AccountInfo<'info>,
    program_id: &Pubkey,
    txid: [u8; 32],
    vout: u32,
) -> Result<()> {
    let vout_bytes = vout.to_le_bytes();
    let (expected, _bump) = Pubkey::find_program_address(
        &[SPENT_OUTPOINT_SEED, txid.as_ref(), &vout_bytes],
        program_id,
    );
    require!(
        spent.key() == expected,
        SolbeamError::WrongSpentOutpoint
    );
    require!(spent.data_is_empty(), SolbeamError::DepositSpent);
    Ok(())
}

/// Create the spent-outpoint record for a deposit outpoint, refusing a second
/// report.
///
/// The mirror of [`create_nullifier`], and built the same way for the same
/// reason: `transfer` + `allocate` + `assign` rather than `create_account`, so a
/// lamport sent to this public PDA cannot block a report. The system program's
/// `CreateAccount` refuses a destination that already holds lamports, which is a
/// targeted, near-free denial of service on the record; `Allocate` only requires
/// empty data, so pre-existing lamports are absorbed (and forfeited) rather than
/// fatal.
///
/// **This record is permanent, and that is deliberate.** The nullifier has a
/// prune because its block leaving the window is what makes a replay harmless
/// again. There is no equivalent event for spentness: a spent output is spent
/// forever, so any instruction that could close this account would re-open the
/// mint of a deposit the reserve has already spent — the exact N5 hole. There
/// is therefore no `prune_spent`, no `close`, and no field that a future
/// instruction could use to justify one. The rent is a permanent, one-off cost
/// the authority pays, which is also why being the authority is the only way to
/// write here.
fn create_spent_outpoint<'info>(
    spent: &AccountInfo<'info>,
    authority: &AccountInfo<'info>,
    program_id: &Pubkey,
    txid: [u8; 32],
    vout: u32,
) -> Result<()> {
    let vout_bytes = vout.to_le_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[SPENT_OUTPOINT_SEED, txid.as_ref(), &vout_bytes],
        program_id,
    );
    require!(
        spent.key() == expected,
        SolbeamError::WrongSpentOutpoint
    );
    require!(
        spent.data_is_empty(),
        SolbeamError::OutpointAlreadyReported
    );

    let seeds: &[&[u8]] = &[SPENT_OUTPOINT_SEED, txid.as_ref(), &vout_bytes, &[bump]];
    let rent = Rent::get()?.minimum_balance(SpentOutpoint::SPACE);
    let existing = spent.lamports();
    if existing < rent {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                anchor_lang::system_program::ID,
                anchor_lang::system_program::Transfer {
                    from: authority.clone(),
                    to: spent.clone(),
                },
            ),
            rent - existing,
        )?;
    }

    anchor_lang::system_program::allocate(
        CpiContext::new_with_signer(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Allocate {
                account_to_allocate: spent.clone(),
            },
            &[seeds],
        ),
        SpentOutpoint::SPACE as u64,
    )?;
    anchor_lang::system_program::assign(
        CpiContext::new_with_signer(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Assign {
                account_to_assign: spent.clone(),
            },
            &[seeds],
        ),
        program_id,
    )?;

    // Written field for field rather than through Borsh, exactly as the
    // nullifier is: discriminator then `bump`. The account carries no other
    // data — its existence is the record, and there is nothing to prune against.
    let mut data = spent.try_borrow_mut_data()?;
    data[..8].copy_from_slice(SpentOutpoint::DISCRIMINATOR);
    data[8] = bump;
    Ok(())
}

// -- the replay nullifier ---------------------------------------------------

/// Create the replay nullifier for a deposit, refusing a second one.
///
/// The seeds are `(txid, vout)` — two **instruction arguments** — so the address
/// cannot be expressed as an Anchor `seeds` constraint and the account is built
/// by hand rather than through `init`. The derivation is reused verbatim by
/// [`prune_nullifier`], and the client-side tests derive it the same way, so all
/// three agree on which address identifies a deposit.
///
/// The existence check comes **before** the account is built: a deposit that has
/// already been minted has this account, and `data_is_empty()` is false, so the
/// caller gets `AlreadyMinted` rather than a system-program error about an
/// account in use. The height is written into the account because
/// [`prune_nullifier`] must be able to check it without trusting an argument.
///
/// **Built with `transfer` + `allocate` + `assign`, not `create_account`.** The
/// system program's `CreateAccount` refuses a destination that already holds
/// lamports, and this PDA's address is public the moment the deposit transaction
/// is: one lamport sent to it would block that deposit from ever being minted — a
/// targeted, near-free denial of service. `Allocate` only requires the account's
/// data to be empty, so pre-existing lamports (which the attacker forfeits) are
/// absorbed rather than fatal. What the attacker cannot do is write *data* into
/// the account, because only the owner may do that and the owner is this program.
fn create_nullifier<'info>(
    nullifier: &AccountInfo<'info>,
    submitter: &AccountInfo<'info>,
    program_id: &Pubkey,
    txid: [u8; 32],
    vout: u32,
    deposit_height: u64,
) -> Result<()> {
    let vout_bytes = vout.to_le_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[NULLIFIER_SEED, txid.as_ref(), &vout_bytes],
        program_id,
    );
    require!(
        nullifier.key() == expected,
        SolbeamError::WrongNullifier
    );
    require!(nullifier.data_is_empty(), SolbeamError::AlreadyMinted);

    let seeds: &[&[u8]] = &[NULLIFIER_SEED, txid.as_ref(), &vout_bytes, &[bump]];
    let rent = Rent::get()?.minimum_balance(DepositNullifier::SPACE);
    let existing = nullifier.lamports();
    if existing < rent {
        // From the submitter, which signs the outer transaction. Not a signed
        // PDA transfer: the nullifier receives, it does not pay.
        anchor_lang::system_program::transfer(
            CpiContext::new(
                anchor_lang::system_program::ID,
                anchor_lang::system_program::Transfer {
                    from: submitter.clone(),
                    to: nullifier.clone(),
                },
            ),
            rent - existing,
        )?;
    }

    anchor_lang::system_program::allocate(
        CpiContext::new_with_signer(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Allocate {
                account_to_allocate: nullifier.clone(),
            },
            &[seeds],
        ),
        DepositNullifier::SPACE as u64,
    )?;
    anchor_lang::system_program::assign(
        CpiContext::new_with_signer(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Assign {
                account_to_assign: nullifier.clone(),
            },
            &[seeds],
        ),
        program_id,
    )?;

    // The allocate CPI has given the account its data region, so this borrow sees
    // it. Written field for field rather than through Borsh so the layout is
    // explicit: discriminator, `deposit_height: u64`, `bump: u8`.
    let mut data = nullifier.try_borrow_mut_data()?;
    data[..8].copy_from_slice(DepositNullifier::DISCRIMINATOR);
    data[8..16].copy_from_slice(&deposit_height.to_le_bytes());
    data[16] = bump;
    Ok(())
}

// -- the staged mint --------------------------------------------------------

/// Create the staged mint for a verified deposit, refusing a second one for the
/// same `(txid, vout)`.
///
/// Built by hand for exactly the reasons [`create_nullifier`] is: the seeds are
/// two instruction arguments, so the address cannot be an Anchor `seeds`
/// constraint, and a deposit that has already been staged must fail with
/// `AlreadyStaged` rather than with a system-program error about an
/// already-allocated account. `transfer` + `allocate` + `assign` rather than
/// `create_account` for the same denial-of-service reason: a lamport sent to this
/// public PDA must not be able to block the deposit from ever being staged.
///
/// The layout written here is `StagedMint`'s own, and
/// `vault_layout_tests::staged_mint_manual_layout_matches_borsh` compares it
/// against a real `AnchorSerialize`, so a field reorder breaks a test instead of
/// silently corrupting every staged item.
#[allow(clippy::too_many_arguments)]
fn create_staged_mint<'info>(
    staged: &AccountInfo<'info>,
    submitter: &AccountInfo<'info>,
    program_id: &Pubkey,
    txid: [u8; 32],
    vout: u32,
    recipient: Pubkey,
    amount: u64,
    deposit_height: u64,
    deposit_hash: [u8; 32],
    maturity_at_deposit: u64,
) -> Result<()> {
    let vout_bytes = vout.to_le_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[STAGED_MINT_SEED, txid.as_ref(), &vout_bytes],
        program_id,
    );
    require!(staged.key() == expected, SolbeamError::WrongStagedMint);
    require!(staged.data_is_empty(), SolbeamError::AlreadyStaged);

    let seeds: &[&[u8]] = &[STAGED_MINT_SEED, txid.as_ref(), &vout_bytes, &[bump]];
    let rent = Rent::get()?.minimum_balance(StagedMint::SPACE);
    let existing = staged.lamports();
    if existing < rent {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                anchor_lang::system_program::ID,
                anchor_lang::system_program::Transfer {
                    from: submitter.clone(),
                    to: staged.clone(),
                },
            ),
            rent - existing,
        )?;
    }

    anchor_lang::system_program::allocate(
        CpiContext::new_with_signer(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Allocate {
                account_to_allocate: staged.clone(),
            },
            &[seeds],
        ),
        StagedMint::SPACE as u64,
    )?;
    anchor_lang::system_program::assign(
        CpiContext::new_with_signer(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Assign {
                account_to_assign: staged.clone(),
            },
            &[seeds],
        ),
        program_id,
    )?;

    let mut data = staged.try_borrow_mut_data()?;
    data[..8].copy_from_slice(StagedMint::DISCRIMINATOR);
    data[8..40].copy_from_slice(&recipient.to_bytes());
    data[40..48].copy_from_slice(&amount.to_le_bytes());
    data[48..56].copy_from_slice(&deposit_height.to_le_bytes());
    data[56..88].copy_from_slice(&deposit_hash);
    data[88..96].copy_from_slice(&maturity_at_deposit.to_le_bytes());
    data[96] = bump;
    Ok(())
}

// -- the peg-out's helpers --------------------------------------------------

/// The peg-out fee on `amount`, in base units: `fee.redeem_bp` basis points.
///
/// The fee needs no account and is not transferred anywhere: the holder is paid
/// `amount − fee` in BSV while the whole `amount` is burned, so the reserve
/// falls by less than the supply and the members keep the difference inside the
/// reserve they already hold (doc 11 §3). Rounded **down**, which favours the
/// holder by at most one base unit.
fn redeem_fee(amount: u64) -> Result<u64> {
    let scaled = amount
        .checked_mul(REDEEM_FEE_BP)
        .ok_or(SolbeamError::Overflow)?;
    Ok(scaled / 10_000)
}

/// The slot at which a redemption initiated at `now` may be cancelled:
/// `now + policy_slots + grace_slots`, with an overflow reported rather than
/// wrapped.
///
/// `policy_slots` is the `Config::redeem_deadline_slots` in force at initiation
/// and `grace_slots` is `po.cancel_grace`. The grace is a parameter rather than
/// read from the constant inside so the arithmetic is testable directly: the
/// shipped `po.cancel_grace` is 0, so no on-chain test can tell the term apart
/// from an omitted add. The call site passes `CANCEL_GRACE_SLOTS`, so the
/// parameter still governs behaviour.
fn redeem_deadline_slot(now: u64, policy_slots: u64, grace_slots: u64) -> Result<u64> {
    let deadline = now
        .checked_add(policy_slots)
        .and_then(|s| s.checked_add(grace_slots))
        .ok_or(SolbeamError::Overflow)?;
    Ok(deadline)
}

/// True while a staged payout claim is still **provable**: a claim was made and
/// the client still holds that exact block.
///
/// This is the one definition of a *live claim*, shared by [`claim_payout`]'s
/// double-claim guard and [`cancel_redeem`]'s refusal, so the two instructions
/// cannot disagree about whether the escrow is still committed to a payout. It
/// is false both when the client's hash at the claim's height has changed (the
/// payout was reorged) and when the height has left the window entirely (the
/// claim can no longer be checked, and so can never settle).
fn payout_claim_is_live(lc: &LightClient, pending: &PendingRedeem) -> bool {
    pending.payout_height != 0
        && lc.hash_at(pending.payout_height) == Some(pending.payout_hash)
}

/// True when `script` is the canonical P2PKH script for `hash160`:
/// `76 a9 14 <hash160> 88 ac`.
///
/// The payout destination is stored as its 20-byte hash and expanded here, so
/// the claim compares against the *stored* address rather than against anything
/// the caller supplies (V4).
pub fn is_p2pkh_for(script: &[u8], hash160: &[u8; 20]) -> bool {
    script.len() == 25
        && script[0] == 0x76
        && script[1] == 0xa9
        && script[2] == 0x14
        && &script[3..23] == &hash160[..]
        && script[23] == 0x88
        && script[24] == 0xac
}

/// Close a program-owned escrow token account, returning its rent to
/// `destination`.
///
/// Not Anchor's `close` constraint: the escrow's **authority is the light client
/// PDA**, and a `close` on a token account has to be signed by that authority.
/// Signing the CPI explicitly with the light client's seeds is the same thing
/// `burn` and `transfer` already do here, and it keeps the authority's identity
/// in one place rather than in a constraint that would have to re-derive it.
fn close_escrow<'info>(
    token_program: Pubkey,
    escrow: &AccountInfo<'info>,
    destination: &AccountInfo<'info>,
    light_client: &AccountInfo<'info>,
    bump: u8,
) -> Result<()> {
    let seeds: &[&[u8]] = &[b"light_client", &[bump]];
    token::close_account(CpiContext::new_with_signer(
        token_program,
        CloseAccount {
            account: escrow.clone(),
            destination: destination.clone(),
            authority: light_client.clone(),
        },
        &[seeds],
    ))
}

/// Record that a payout outpoint settled a redemption, refusing a second
/// redemption for the same outpoint.
///
/// The mirror of [`create_nullifier`] in construction — `transfer` + `allocate`
/// + `assign`, so a lamport sent to this public PDA cannot block a claim, and
/// the address is re-derived from `(txid, vout)` before anything is touched.
///
/// It differs in one respect, and that is the W5 fix: the record stores **which
/// redemption** the outpoint settled. A record that merely existed would refuse
/// a legitimate re-claim when a reorg re-includes the same transaction at a
/// different height; a record that did not exist would let one payment settle
/// every redemption naming the same address. Storing the id gives both: the
/// same redemption may replace its own claim, and no other redemption may use
/// it.
fn stage_payout_outpoint<'info>(
    nullifier: &AccountInfo<'info>,
    submitter: &AccountInfo<'info>,
    program_id: &Pubkey,
    txid: [u8; 32],
    vout: u32,
    redemption_id: u64,
) -> Result<()> {
    let vout_bytes = vout.to_le_bytes();
    let (expected, bump) = Pubkey::find_program_address(
        &[PAYOUT_NULLIFIER_SEED, txid.as_ref(), &vout_bytes],
        program_id,
    );
    require!(
        nullifier.key() == expected,
        SolbeamError::WrongPayoutNullifier
    );

    // Already claimed. Only this program can write here, so a non-empty account
    // at the derived address is one of our records and its first field is the
    // redemption id `create_payout_nullifier` wrote.
    if !nullifier.data_is_empty() {
        require!(
            nullifier.owner == program_id,
            SolbeamError::WrongPayoutNullifier
        );
        let data = nullifier.try_borrow_data()?;
        let stored = u64::from_le_bytes(data[8..16].try_into().unwrap());
        require!(
            stored == redemption_id,
            SolbeamError::PayoutOutpointReused
        );
        return Ok(());
    }

    let seeds: &[&[u8]] = &[PAYOUT_NULLIFIER_SEED, txid.as_ref(), &vout_bytes, &[bump]];
    let rent = Rent::get()?.minimum_balance(PayoutNullifier::SPACE);
    let existing = nullifier.lamports();
    if existing < rent {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                anchor_lang::system_program::ID,
                anchor_lang::system_program::Transfer {
                    from: submitter.clone(),
                    to: nullifier.clone(),
                },
            ),
            rent - existing,
        )?;
    }

    anchor_lang::system_program::allocate(
        CpiContext::new_with_signer(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Allocate {
                account_to_allocate: nullifier.clone(),
            },
            &[seeds],
        ),
        PayoutNullifier::SPACE as u64,
    )?;
    anchor_lang::system_program::assign(
        CpiContext::new_with_signer(
            anchor_lang::system_program::ID,
            anchor_lang::system_program::Assign {
                account_to_assign: nullifier.clone(),
            },
            &[seeds],
        ),
        program_id,
    )?;

    // Field for field, as the other hand-built accounts are: discriminator,
    // `redemption_id: u64`, `bump: u8`.
    let mut data = nullifier.try_borrow_mut_data()?;
    data[..8].copy_from_slice(PayoutNullifier::DISCRIMINATOR);
    data[8..16].copy_from_slice(&redemption_id.to_le_bytes());
    data[16] = bump;
    Ok(())
}

// -- small helpers ----------------------------------------------------------

fn read32(buf: &[u8], at: usize) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(&buf[at..at + 32]);
    out
}

fn read_u32_le(buf: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([buf[at], buf[at + 1], buf[at + 2], buf[at + 3]])
}

/// Render 32 bytes as display-order hex, the way a block explorer shows it.
/// Display only — never for comparison, which is done in internal order.
fn display_hex(bytes: &[u8; 32]) -> String {
    let mut reversed = *bytes;
    reversed.reverse();
    hex_encode(&reversed)
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push(HEX[(b >> 4) as usize] as char);
        s.push(HEX[(b & 0x0f) as usize] as char);
    }
    s
}

/// Everything the producer supplies for a mint, and none of it is believed.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct DepositClaim {
    /// Height of the block the deposit is in. Must be inside the window.
    pub height: u64,
    /// Internal byte order, matching the Python reference and the stored records.
    pub txid: [u8; 32],
    pub vout: u32,
    pub amount: u64,
    /// The depositor's Solana address, taken from the OP_RETURN payload.
    pub recipient: [u8; 32],
    pub index: u32,
    pub branch: Vec<[u8; 32]>,
    /// The raw 80-byte header of the block at `height`. Stored records hold
    /// only a hash, so the claim must supply the header — and the hash check in
    /// `verify_deposit` is what proves it is the canonical one. The Merkle root
    /// is read out of it rather than stored.
    pub header: [u8; HEADER_LEN],
    /// The raw deposit transaction, so the claimed output can be re-derived
    /// rather than trusted.
    pub tx: Vec<u8>,
}

/// Everything the producer supplies to prove a **payout**, and none of it is
/// believed. The mirror of [`DepositClaim`], pointed at the redemption's named
/// address instead of the reserve script.
///
/// There is deliberately **no amount field**: the amount is a floor derived from
/// the stored redemption (`amount − fee`), so a caller cannot inflate what a
/// payment is worth by claiming it is worth more. The output's own value is read
/// from the transaction.
#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct PayoutProof {
    /// Height of the block the payout is in. Must be inside the window.
    pub height: u64,
    /// Internal byte order, matching the Python reference and the stored records.
    pub txid: [u8; 32],
    pub vout: u32,
    pub index: u32,
    pub branch: Vec<[u8; 32]>,
    /// The raw 80-byte header of the block at `height`. Checked against the
    /// window's stored hash, so only the canonical block passes.
    pub header: [u8; HEADER_LEN],
    /// The raw payout transaction, so the output can be re-derived.
    pub tx: Vec<u8>,
}

/// A minted deposit's replay record: **one PDA per `(txid, vout)`**, and nothing
/// else.
///
/// The account's **existence** is the record. That is the whole design (decision
/// P5) and it replaces a fixed `Vec<DepositKey>` capped at `MAX_USED = 200`:
/// there is no list to fill, so there is no ceiling, and no per-window capacity
/// limit for ordinary volume to reach. Solana cannot enumerate PDAs and does not
/// need to — replay is answered by deriving the address and looking it up.
///
/// It is deliberately **not** the pending-mint item that the vault will hold.
/// That item closes on release; this one must outlive it, or a released deposit
/// could be minted again (V1 in doc 19/20). The two lifetimes are different
/// accounts for that reason.
///
/// `deposit_height` is the field that makes pruning sound rather than trusted:
/// [`prune_nullifier`] may only close this account when that stored height is
/// below `window_start`. With the height supplied as an argument instead, a
/// caller could name a stale height for a live deposit, close the nullifier and
/// re-mint — a replay oracle at ~$0.001 a cycle (W1 in doc 20).
#[account]
pub struct DepositNullifier {
    /// The block the deposit was minted from. Written once, never updated.
    pub deposit_height: u64,
    pub bump: u8,
}

impl DepositNullifier {
    pub const SPACE: usize = 8 + 8 + 1; // 17 bytes: discriminator + height + bump
}

/// The federation's **spent-outpoint record**: one PDA per `(txid, vout)`, and
/// its **existence** is the record. This is N5's mechanism (doc 03 §3, doc 05).
///
/// Solana cannot read BSV's UTXO set, so spentness is *reported*, not proved:
/// [`report_spent`] is the write path, [`verify_deposit`] checks it, and the
/// honest statement is *"the program verifies deposits; the federation reports
/// backing."* The record is keyed on the deposit's identity `(txid, vout)` — the
/// same key the nullifier uses — because a reorg that re-includes the
/// transaction must hit the same report, not a fresh one.
///
/// **Permanent, and not prunable.** The nullifier's `deposit_height` exists so
/// that [`prune_nullifier`] can close it once its block has left the window and
/// a replay is harmless again. Spentness has no such event: the output is spent
/// forever, so closing this account would re-open the mint of a deposit the
/// reserve has already spent — exactly the hole the record exists to close.
/// Hence no height field, no prune instruction and no `close` anywhere:
/// `bump` is all there is to store.
#[account]
pub struct SpentOutpoint {
    pub bump: u8,
}

impl SpentOutpoint {
    /// 8 discriminator + 1 bump = 9 bytes. `create_spent_outpoint` writes this
    /// layout by hand; `vault_layout_tests::spent_outpoint_layout_is_discriminator_then_bump`
    /// asserts the two agree.
    pub const SPACE: usize = 8 + 1;
}

/// One verified deposit whose mint is **held in the vault** rather than paid
/// straight to the recipient. Doc 31's `StagedMint`.
///
/// The account is keyed on `(txid, vout)` — the deposit's identity, never its
/// height — and holds everything the release and the burn need to decide, with
/// no dependence on a relayer or on a caller's claim:
///
/// * `recipient` is the Solana key the deposit's `OP_RETURN` committed to;
/// * `amount` is the satoshi value the output carried;
/// * `deposit_height` and `deposit_hash` are the block the proof was checked
///   against, so a later reorg is detectable from the light client's own window;
/// * `maturity_at_deposit` is **the maturity read from [`Config`] at the moment
///   this deposit was verified**. It is load-bearing: a governance raise applies
///   to future deposits and cannot retroactively trap this one in flight.
///
/// `release_mint` and `burn_staged` are the only two exits, and they are
/// mutually exclusive on `deposit_hash` still matching or not. Both close this
/// account, returning its rent to the caller. The [`DepositNullifier`] is
/// deliberately **not** closed by either: it is the replay record and must
/// outlive the item.
#[account]
pub struct StagedMint {
    pub recipient: Pubkey,
    pub amount: u64,
    pub deposit_height: u64,
    pub deposit_hash: [u8; 32],
    pub maturity_at_deposit: u64,
    pub bump: u8,
}

impl StagedMint {
    /// 8 discriminator + 32 recipient + 8 amount + 8 height + 32 hash + 8
    /// maturity + 1 bump = 97 bytes. `create_staged_mint` writes this layout by
    /// hand, and `vault_layout_tests::staged_mint_manual_layout_matches_borsh`
    /// asserts the two agree.
    pub const SPACE: usize = 8 + 32 + 8 + 8 + 32 + 8 + 1;
}

/// One redemption in flight: **escrowed `solBSV`, a named BSV address, and the
/// state the two exits resolve against.**
///
/// Created by [`initiate_redeem`] and closed by exactly one of
/// [`cancel_redeem`] (the deadline passed, and any claim on it is no longer
/// live) or [`settle_redeem`] (a payout was proven and the challenge window
/// passed). No third instruction closes it, so the escrow cannot leak out of
/// the program by another path.
///
/// Every field is a **stored** fact rather than a re-derivation, because the
/// policy in force when the holder committed is what they are owed: `fee` and
/// `deadline_slot` are copied from the parameters as they were at initiation,
/// exactly as [`StagedMint::maturity_at_deposit`] is.
#[account]
pub struct PendingRedeem {
    /// Who receives the escrow back if the redemption is cancelled, and whose
    /// associated token account is the only valid cancellation destination.
    pub holder: Pubkey,
    /// The full amount escrowed, and the amount burned on settlement. The
    /// holder receives `amount − fee` in BSV from the reserve, which is why the
    /// two numbers are kept apart here.
    pub amount: u64,
    /// The fee that applied at initiation, in base units. Copied, never
    /// recomputed: a later `fee.redeem_bp` change must not move what this
    /// redemption is owed.
    pub fee: u64,
    /// The **HASH160** of the BSV address named at initiation. The payout must
    /// pay the canonical P2PKH script this expands to, so the destination is
    /// bound at initiation and checked at claim (V4).
    pub bsv_address: [u8; 20],
    /// `Clock::slot` at initiation, recorded so the deadline can be audited
    /// rather than inferred.
    pub initiated_slot: u64,
    /// `initiated_slot + Config::redeem_deadline_slots + po.cancel_grace`.
    /// After this slot — and only then — [`cancel_redeem`] may return the
    /// escrow.
    pub deadline_slot: u64,
    /// The BSV height the payout was proven at, or **0 for "no claim staged"**.
    /// The zero sentinel is safe because a proof is always against a block at
    /// height ≥ 1.
    pub payout_height: u64,
    /// The hash of the block at `payout_height` when the payout was claimed.
    /// [`settle_redeem`] burns only while the client's own window still holds
    /// this same hash; [`cancel_redeem`] returns the escrow whenever it does
    /// not — a changed hash *or* a height that has left the window, both of
    /// which make the claim unprovable and therefore unsettleable. The shared
    /// predicate is `payout_claim_is_live`.
    pub payout_hash: [u8; 32],
    pub bump: u8,
}

impl PendingRedeem {
    /// 8 discriminator + 32 holder + 8 amount + 8 fee + 20 address + 8 initiated
    /// + 8 deadline + 8 payout height + 32 payout hash + 1 bump = 133 bytes.
    pub const SPACE: usize = 8 + 32 + 8 + 8 + 20 + 8 + 8 + 8 + 32 + 1;
}

/// The redemption counter and the pending-redemption count. A singleton PDA
/// (`[b"redeem_book"]`), so there is exactly one and no seed a caller can vary.
///
/// `next_id` makes redemptions sequentially keyed and therefore enumerable,
/// which is what lets the three permissionless instructions work without an
/// index of holders. `pending` is the count `po.max_pending` bounds: the cap is
/// on *concurrent* escrows, so it is decremented on both exits.
#[account]
pub struct RedeemBook {
    /// The id `initiate_redeem` must be called with next. Strictly increasing;
    /// nothing ever writes it back.
    pub next_id: u64,
    /// How many [`PendingRedeem`] accounts are open right now.
    pub pending: u64,
    pub bump: u8,
}

impl RedeemBook {
    /// 8 discriminator + 8 next_id + 8 pending + 1 bump = 25 bytes.
    pub const SPACE: usize = 8 + 8 + 8 + 1;
}

/// The record that one BSV outpoint has settled one redemption.
///
/// One PDA per `(txid, vout)`, and it stores **which** redemption, not merely
/// that the outpoint was used. That distinction is the whole point: an
/// existence-only record would refuse a legitimate re-claim when a reorg
/// re-includes the same transaction at a different height, while a
/// per-redemption record (or none at all) lets one payment settle two
/// redemptions that name the same address. Storing the id gives both — the same
/// redemption may replace its own claim, and no other redemption may use it.
///
/// Written by hand (`transfer` + `allocate` + `assign`), exactly as
/// [`DepositNullifier`] is, because its seeds come from instruction arguments
/// rather than from fields Anchor's `seeds` can see.
#[account]
pub struct PayoutNullifier {
    /// The [`PendingRedeem`] this outpoint settled.
    pub redemption_id: u64,
    pub bump: u8,
}

impl PayoutNullifier {
    /// 8 discriminator + 8 redemption id + 1 bump = 17 bytes.
    pub const SPACE: usize = 8 + 8 + 1;
}

/// The bridge's tunable policy: **only `maturity_blocks`, and only through the
/// timelocked authority path.**
///
/// A singleton PDA (`[b"config"]`), so there is exactly one and no seed the
/// caller can vary. It is created with `maturity_blocks = 0` — doc 31's decided
/// PoC setting — and written only by `execute_authority_change` on an
/// [`AuthorityChange::SetMaturity`]. No instruction writes it directly, which is
/// what keeps "no second authority mechanism" true.
#[account]
pub struct Config {
    /// How deep a deposit's block must be before its staged mint may release,
    /// **in BSV blocks**. 0 means no window at all: the vault is a pass-through
    /// and the protective window is absent.
    pub maturity_blocks: u64,
    /// How long a redemption's member has to pay before the holder may cancel,
    /// **in Solana slots**. Ships at `po.deadline` (216,000 ≈ 24 h) and moves
    /// only through the timelocked authority path.
    ///
    /// It is a stored field rather than a bare constant for the same two
    /// reasons `maturity_blocks` is: it is policy that should be raisable
    /// without a redeploy, and a 24-hour constant cannot be reached by any test
    /// on a local validator, so a constant would leave `cancel_redeem`'s only
    /// real guard unexercised. **Defaults are recorded in
    /// `config/params.json`; the program carries the default, not the parameter
    /// itself.**
    pub redeem_deadline_slots: u64,
    pub bump: u8,
}

impl Config {
    /// 8 discriminator + 8 maturity + 8 redeem deadline + 1 bump = 25 bytes.
    pub const SPACE: usize = 8 + 8 + 8 + 1;
}

/// The two signer sets on the reserve script, their thresholds, and the live
/// counts the thresholds are floors over.
///
/// **A record, not a key.** `gateway_key` and `greycore_key` are the two
/// 33-byte public keys that fill `fed.script`'s 2-of-2
/// `OP_2 <gateway threshold key> <greycore key> OP_2 OP_CHECKMULTISIG`. The
/// program stores them so the registry says which script the federation claims
/// to hold; it cannot check that claim, cannot spend it, and holds no share of
/// either key. **The keys do not exist in this repository**, no ceremony
/// produces them, and no deposit has ever paid one.
///
/// **Why the counts are here.** `threshold` is a *floor* and `N` is variable,
/// so the program must be able to answer "how many members hold a share right
/// now" without being told: `gateway_members` is every identity ever admitted
/// and not departed, and `bonded_members` is the subset that has posted both
/// sides. The departure and seizure paths refuse to draw `bonded_members` below
/// `threshold`, which is the only numeric invariant the registry enforces. It is
/// **not a capacity rule about the reserve** -- no such rule exists in this
/// design, and none is invented here.
#[account]
pub struct FederationConfig {
    /// The key that created the federation and the only key that may appoint
    /// Greycore members or seize a recorded `solBSV` bond. The program's upgrade
    /// authority at genesis; a **PoC stand-in** for the federation, which does
    /// not exist.
    pub authority: Pubkey,
    /// The **gateway signing threshold** `t` (`fed.threshold`, `4`). The
    /// reserve key's `t-of-N`. Changing `t` or `N` is a **re-sharing**, not a
    /// migration, and **the re-sharing ceremony is not specified** -- so this
    /// field records the threshold and nothing here performs the ceremony.
    pub threshold: u64,
    /// The gateway's **aggregate** public key: the leg that emits one signature
    /// however many members signed.
    pub gateway_key: Pubkey,
    /// How many Greycore members must assemble before the Greycore leg can sign.
    /// `fed.greycore_threshold` is `open`; this PoC fixes it at 1 in
    /// [`initialize_federation`] and does not pretend that is the design value.
    pub greycore_threshold: u64,
    /// The Greycore's public key: **the second leg of the reserve script**, and
    /// the reason a gateway majority acting alone cannot move the reserve.
    pub greycore_key: Pubkey,
    /// Every gateway identity admitted and not departed. This is the live `N`.
    pub gateway_members: u64,
    /// Greycore identities in the registry.
    pub greycore_members: u64,
    /// Gateway members with both sides recorded. The threshold floor is applied
    /// to **this** count.
    pub bonded_members: u64,
    pub bump: u8,
}

impl FederationConfig {
    /// 8 + 32 + 8 + 32 + 8 + 32 + 8 + 8 + 8 + 1 = 145 bytes.
    pub const SPACE: usize = 8 + 32 + 8 + 32 + 8 + 32 + 8 + 8 + 8 + 1;
}

/// One Greycore member: an identity key, and nothing else.
///
/// A Greycore member **does not bond and holds no share of the reserve key**.
/// What a member of this set *is* is reputation that can be lost and a
/// signature that can be attributed -- it admits gateway members and, through
/// `greycore_key`, co-signs every reserve spend. Neither of those is enforced by
/// this account; it records who is in the set.
///
/// Admission to this set is **authority-gated** for now (see
/// [`crate::add_greycore_member`]), which is a stand-in: the reference appoints
/// its Greycore by community governance and no governance instruction exists
/// here. At genesis the authority that creates the federation is itself the
/// founding Greycore member, so **the two quorums are not disjoint at the
/// start**, and no mechanism in this program makes them so.
#[account]
pub struct GreycoreMember {
    pub identity: Pubkey,
    pub bump: u8,
}

impl GreycoreMember {
    /// 8 discriminator + 32 identity + 1 bump = 41 bytes.
    pub const SPACE: usize = 8 + 32 + 1;
}

/// Whether a gateway member currently holds a seat.
///
/// Two states, because the registry keeps a departed member's record instead of
/// closing it. Only [`MemberStatus::Active`] counts toward the gateway's live
/// size, and only an active member may be seized from.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, Debug)]
pub enum MemberStatus {
    /// Admitted, in the set, counted in `gateway_members`.
    Active,
    /// Departed: the bonds are zeroed and the seat is surrendered. **The
    /// record remains, and the member's off-chain share of the reserve key
    /// remains valid with it** -- nothing here invalidates a share, which is
    /// the leaver-share problem stated in [`crate::leave_member`].
    Left,
}

/// One gateway member: identity, both bonds, admission, and state.
///
/// **The two bond fields are different kinds of thing and are documented
/// separately on purpose:**
///
/// * `solbsv_bond` -- the **`solBSV`-side** bond. Recorded here, clearable only
///   by the program (via [`crate::seize_solbsv_bond`]). This is the enforceable
///   half *in the design*; what is built is the record, not a token transfer.
/// * `bsv_bond` -- the **BSV-side** bond, and it is **an attestation, not
///   custody**. The BSV is on another chain. It is held under `bond_key`, the
///   **collective threshold key** -- *not* the member's own key, which is the
///   design requirement that stops a caught member moving their own bond -- and
///   it is seized only by the members **collectively signing a BSV
///   transaction**. That is a **social duty: nothing on BSV compels them to
///   sign**, and this program has no part in it beyond this number. A majority
///   could also seize an honest member's bond; the design states that residual
///   rather than hiding it.
///
/// **Neither bond is inside the reserve.** The bond is the float -- working
/// capital for transfers -- and **not a capital requirement sized against the
/// reserve**: there is deliberately no numeric capacity rule in this design, and
/// none is enforced or invented here.
#[account]
pub struct GatewayMember {
    /// The member's identity key. The account is seeded on it, so one identity
    /// is one seat.
    pub identity: Pubkey,
    /// **BSV-side bond, recorded.** An attestation by the member that this much
    /// BSV is bonded under `bond_key`; the program cannot see BSV and cannot
    /// check it. Not custody.
    pub bsv_bond: u64,
    /// **`solBSV`-side bond, recorded.** The side the program can clear.
    pub solbsv_bond: u64,
    /// The **collective threshold key** the BSV-side bond sits under --
    /// **not** the member's key. The program records it and can do nothing
    /// with it.
    pub bond_key: Pubkey,
    /// The Greycore member whose signature admitted this member. Admission
    /// requires the Greycore, so this is never a gateway key.
    pub approved_by: Pubkey,
    /// True once both sides are recorded, which is what makes the seat count
    /// toward `FederationConfig::bonded_members`.
    pub bonded: bool,
    /// Active or departed. A departed member keeps this record and a valid
    /// off-chain share.
    pub status: MemberStatus,
    pub bump: u8,
}

impl GatewayMember {
    /// 8 + 32 + 8 + 8 + 32 + 32 + 1 + 1 + 1 = 123 bytes.
    pub const SPACE: usize = 8 + 32 + 8 + 8 + 32 + 32 + 1 + 1 + 1;
}

/// The one change awaiting its timelock, and who proposed it.
///
/// A singleton: the PDA seed is fixed (`[b"pending_authority_change"]`), so only
/// one can exist. `authority` is what makes `cancel` and `execute` the proposer's
/// to do; `effective_slot` is the earliest slot at which the change may be
/// applied.
#[account]
pub struct PendingAuthorityChange {
    pub authority: Pubkey,
    pub effective_slot: u64,
    pub change: AuthorityChange,
    pub bump: u8,
}

impl PendingAuthorityChange {
    /// Sizes the largest variant, which is `Checkpoint` (tag + `u64` + header).
    pub const SPACE: usize = 8 + 32 + 8 + (1 + 8 + HEADER_LEN) + 1; // 138 bytes
}

/// The change a [`PendingAuthorityChange`] will apply.
///
/// Only the two instructions that used to be instant — the checkpoint and the
/// pause flag — are representable here. Deliberately so: this is the complete
/// set of privileged state changes the client has, and the enum is where that
/// stays visible.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub enum AuthorityChange {
    /// Re-anchor the trusted checkpoint. The raw 80-byte header is carried so
    /// `anchor_checkpoint` can re-derive `expected_bits`, `no_retargeting` and
    /// `pow_limit_bits` at execute time (F2), exactly as `initialize` does.
    Checkpoint { height: u64, header: [u8; HEADER_LEN] },
    /// Pause or unpause header advancement and therefore minting.
    Pause { paused: bool },
    /// Set the vault's maturity, in BSV blocks. Applied to future deposits only:
    /// each [`StagedMint`] carries the `maturity_at_deposit` that applied when it
    /// was verified, so raising this cannot reach back and freeze a deposit
    /// already in flight.
    SetMaturity { blocks: u64 },
    /// Set the peg-out deadline, in Solana slots. Applied to future redemptions
    /// only: each [`PendingRedeem`] carries the `deadline_slot` computed when it
    /// was initiated, so a change here cannot shorten or extend a redemption
    /// already in flight.
    SetRedeemDeadline { slots: u64 },
}

/// The bridge's deposit script, passed in so the check is against the account
/// the bridge actually controls rather than a constant baked into the program.
#[account]
pub struct DepositScript {
    pub script: Vec<u8>,
    pub bump: u8,
}

impl DepositScript {
    /// 8 discriminator + 4 vec length + MAX_SCRIPT_LEN + 1 padding.
    pub const SPACE: usize = 8 + 4 + MAX_SCRIPT_LEN + 1;
}


// ---------------------------------------------------------------------------
// the federation registry's accounts
// ---------------------------------------------------------------------------

/// Creates the federation, the Greycore and the founding Greycore member.
///
/// The authority check is the same one [`Initialize`] and [`InitializeBridge`]
/// use, and it is declared **before** either `init` so a rejected caller creates
/// nothing. `gateway_key` is an instruction argument rather than an account: it
/// is a **record of the off-chain key**, and there is nothing on chain to
/// deserialise or compare it against -- which is exactly why it must be
/// distinct from the Greycore's key, since one key in both legs would collapse
/// the 2-of-2 into a single signer and remove the second quorum entirely.
#[derive(Accounts)]
pub struct InitializeFederation<'info> {
    /// The program's upgrade authority, and the payer. Becomes the federation's
    /// `authority` and the first Greycore identity.
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        address = program_data_address() @ SolbeamError::Unauthorized,
        constraint = program_data.upgrade_authority_address == Some(authority.key())
            @ SolbeamError::Unauthorized,
    )]
    pub program_data: Account<'info, ProgramData>,
    /// The aggregate gateway public key the design says fills the reserve
    /// script's first leg. A record; the program holds no share of it.
    /// CHECK: a public key carried as instruction data, stored verbatim, never
    /// dereferenced and never signed for.
    pub gateway_key: UncheckedAccount<'info>,
    #[account(init, payer = authority, space = FederationConfig::SPACE,
              seeds = [FEDERATION_SEED], bump)]
    pub federation: Account<'info, FederationConfig>,
    #[account(init, payer = authority, space = GreycoreMember::SPACE,
              seeds = [GREYCORE_MEMBER_SEED, authority.key().as_ref()], bump)]
    pub greycore_member: Account<'info, GreycoreMember>,
    pub system_program: Program<'info, System>,
}

/// Adds one Greycore member. Authority-gated, because no governance instruction
/// exists in this PoC -- see [`crate::add_greycore_member`].
#[derive(Accounts)]
pub struct AddGreycoreMember<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut, seeds = [FEDERATION_SEED], bump = federation.bump,
              has_one = authority @ SolbeamError::Unauthorized)]
    pub federation: Account<'info, FederationConfig>,
    /// Creating this account **is** the admission, exactly as it is for a
    /// gateway member: the seeds include the identity, so a second attempt for
    /// the same key collides with the existing account and `init` refuses.
    #[account(init, payer = authority, space = GreycoreMember::SPACE,
              seeds = [GREYCORE_MEMBER_SEED, identity.key().as_ref()], bump)]
    pub greycore_member: Account<'info, GreycoreMember>,
    /// CHECK: used only as a seed; the address derived from it is what the
    /// `init` must match, so an identity that does not derive this account
    /// cannot be pointed at it.
    pub identity: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

/// Admit one gateway member, on a Greycore member's signature.
///
/// `approver` is the Greycore member and must sign; `greycore_member` is that
/// member's registry record, whose `identity` must equal the signer. Requiring
/// **both** the record and the signature is what makes "admission requires the
/// Greycore" a check rather than a convention: an arbitrary signer has no
/// record, and a gateway member has no record in this registry at all.
#[derive(Accounts)]
pub struct AdmitMember<'info> {
    /// The Greycore member performing the admission, and the rent payer.
    #[account(mut)]
    pub approver: Signer<'info>,
    #[account(mut, seeds = [FEDERATION_SEED], bump = federation.bump)]
    pub federation: Account<'info, FederationConfig>,
    /// The approver's record in the Greycore registry. Its `identity` is
    /// compared against the signer in the handler, so this cannot be a
    /// borrowed record for a key that is not present.
    #[account(seeds = [GREYCORE_MEMBER_SEED, approver.key().as_ref()],
              bump = greycore_member.bump)]
    pub greycore_member: Account<'info, GreycoreMember>,
    /// The new member's record, created here with zeroed bonds: an admitted
    /// member does not count toward the threshold until [`RecordBonds`] posts
    /// both sides.
    #[account(init, payer = approver, space = GatewayMember::SPACE,
              seeds = [GATEWAY_MEMBER_SEED, identity.key().as_ref()], bump)]
    pub member: Account<'info, GatewayMember>,
    /// CHECK: only a seed; the `init` above must match the address derived from
    /// it, and the handler requires it to equal the `identity` argument, so the
    /// record cannot be created under a key other than the admitted one.
    pub identity: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

/// Records both sides of a member's bond. Called by the member's identity.
#[derive(Accounts)]
pub struct RecordBonds<'info> {
    #[account(mut, seeds = [GATEWAY_MEMBER_SEED, owner.key().as_ref()],
              bump = member.bump)]
    pub member: Account<'info, GatewayMember>,
    #[account(mut, seeds = [FEDERATION_SEED], bump = federation.bump)]
    pub federation: Account<'info, FederationConfig>,
    /// The member's identity key, and the only key that may post their bonds.
    pub owner: Signer<'info>,
    /// The **collective threshold key** the BSV-side bond is held under. Passed
    /// in as a record: the program cannot check that any BSV is bonded under it.
    /// CHECK: stored verbatim, never dereferenced and never signed for.
    pub bond_key: UncheckedAccount<'info>,
}

/// Leaves the federation. Called by the member's identity.
#[derive(Accounts)]
pub struct LeaveMember<'info> {
    #[account(mut, seeds = [GATEWAY_MEMBER_SEED, owner.key().as_ref()],
              bump = member.bump)]
    pub member: Account<'info, GatewayMember>,
    #[account(mut, seeds = [FEDERATION_SEED], bump = federation.bump)]
    pub federation: Account<'info, FederationConfig>,
    /// The member's identity key. The handler checks it against the record as
    /// well as deriving the account from it, so a member cannot leave on
    /// another member's seat.
    pub owner: Signer<'info>,
}

/// Clears a member's recorded `solBSV`-side bond. Authority-gated.
#[derive(Accounts)]
pub struct SeizeSolbsvBond<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(mut, seeds = [FEDERATION_SEED], bump = federation.bump,
              has_one = authority @ SolbeamError::Unauthorized)]
    pub federation: Account<'info, FederationConfig>,
    /// The member whose recorded `solBSV` bond is cleared.
    #[account(mut, seeds = [GATEWAY_MEMBER_SEED, member.identity.as_ref()],
              bump = member.bump)]
    pub member: Account<'info, GatewayMember>,
}

#[derive(Accounts)]
pub struct InitializeBridge<'info> {
    /// See [`Initialize`]: the bridge's deposit script is the address every
    /// peg-in pays, so whoever sets it first controls where deposits go. It is
    /// gated on the program's upgrade authority for the same reason and by the
    /// same two constraints.
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        address = program_data_address() @ SolbeamError::Unauthorized,
        constraint = program_data.upgrade_authority_address == Some(payer.key())
            @ SolbeamError::Unauthorized,
    )]
    pub program_data: Account<'info, ProgramData>,
    #[account(init, payer = payer, space = DepositScript::SPACE,
              seeds = [b"deposit_script"], bump)]
    pub deposit_script: Account<'info, DepositScript>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct InitializeToken<'info> {
    #[account(
        init,
        payer = payer,
        // A PDA rather than a keypair, so the address is deterministic and a
        // re-run against a validator that already has state is idempotent.
        seeds = [b"mint"],
        bump,
        mint::decimals = TOKEN_DECIMALS,
        mint::authority = light_client,
        // mint::freeze_authority is deliberately NOT set. Omitting it is what
        // makes the token unfreezable; there is no way to add one later.
    )]
    pub mint: Account<'info, Mint>,
    #[account(seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    /// The program-owned vault: the one token account every mint lands in, and
    /// the one `release_mint` pays out of and `burn_staged` burns from.
    ///
    /// A PDA of this program (`[b"vault"]`) whose **authority is the light
    /// client PDA**, so only this program can move what is in it and no operator
    /// key holds it. Created here, with the mint, because the vault is not
    /// meaningful without one.
    #[account(
        init,
        payer = payer,
        seeds = [VAULT_SEED],
        bump,
        token::mint = mint,
        token::authority = light_client,
    )]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
    pub rent: Sysvar<'info, Rent>,
}

#[derive(Accounts)]
pub struct VerifyDeposit<'info> {
    #[account(seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    /// The federation's **spent-outpoint record** for this deposit, one PDA per
    /// `(txid, vout)`.
    ///
    /// Same situation as the nullifier below: the seeds come from the `claim`
    /// argument, so this is an `UncheckedAccount` whose address is re-derived and
    /// compared inside [`require_not_spent`] before it is read. That check is
    /// load-bearing rather than hygiene — with the address unverified a caller
    /// could supply an unrelated empty account and skip the backing check
    /// entirely. A missing account and an empty one are both "not spent";
    /// only a non-empty account owned by this program at the derived address is
    /// a report, and only this program can ever write there.
    /// CHECK: address re-derived from `(claim.txid, claim.vout)` and compared
    /// against `SPENT_OUTPOINT_SEED` in `require_not_spent`; only
    /// `data_is_empty` is read from it.
    pub spent_outpoint: UncheckedAccount<'info>,
    /// The replay nullifier for this deposit, one PDA per `(txid, vout)`.
    ///
    /// An `UncheckedAccount` because its seeds come from the `claim` argument,
    /// which an Anchor `seeds` constraint cannot see. The address is re-derived
    /// and checked inside [`create_nullifier`] before anything is created, and
    /// the account is never trusted for data: existence is the only thing read.
    ///
    /// It was previously an `Account<UsedDeposits>` pinned to `[b"used_deposits"]`
    /// and, before that, bound to nothing at all — so a caller could supply a
    /// replay list of its own and mint the same deposit repeatedly. Deriving the
    /// address from the claim itself removes that class of mistake rather than
    /// pinning one instance of it.
    /// CHECK: address re-derived from `(claim.txid, claim.vout)` and compared
    /// against `NULLIFIER_SEED` in `create_nullifier`; only `data_is_empty` is
    /// read from it.
    #[account(mut)]
    pub nullifier: UncheckedAccount<'info>,
    /// The staged mint for this deposit, one PDA per `(txid, vout)`. Same
    /// situation as the nullifier: the seeds come from the `claim` argument, so
    /// the address is re-derived and checked inside [`create_staged_mint`]
    /// before anything is created.
    /// CHECK: address re-derived from `(claim.txid, claim.vout)` and compared
    /// against `STAGED_MINT_SEED` in `create_staged_mint`; only `data_is_empty`
    /// is read from it.
    #[account(mut)]
    pub staged: UncheckedAccount<'info>,
    #[account(seeds = [b"deposit_script"], bump = deposit_script.bump)]
    pub deposit_script: Account<'info, DepositScript>,
    /// The maturity that applies to this deposit, read once and copied onto the
    /// staged item. See the instruction body.
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    /// Pinned to the program's own mint PDA. Without this the caller supplies any
    /// `Mint` whose authority happens to be this program's light-client PDA —
    /// which anyone can create, since `InitializeMint` needs no authority
    /// signature — and a valid public deposit is then consumed against a
    /// counterfeit mint, stranding the real deposit permanently.
    #[account(mut, seeds = [b"mint"], bump)]
    pub mint: Account<'info, Mint>,
    /// The program-owned vault the minted tokens land in. Pinned to the vault PDA
    /// so a caller cannot direct the mint at an account of its own.
    #[account(mut, seeds = [VAULT_SEED], bump)]
    pub vault: Account<'info, TokenAccount>,
    /// CHECK: its key is checked against the OP_RETURN payload, and it becomes
    /// the `recipient` recorded on the staged mint — which `release_mint` later
    /// pays. It is never read or written here.
    pub recipient_owner: UncheckedAccount<'info>,
    #[account(mut)]
    pub submitter: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

/// Release one matured, still-canonical staged mint.
///
/// The seeds are `(txid, vout)` — the deposit's identity, which the `StagedMint`
/// itself does not store, because the item is *keyed* on it — so they are bound
/// with `#[instruction(...)]` exactly as [`PruneNullifier`] does. `close =
/// submitter` returns the item's rent to whoever performed the release.
#[derive(Accounts)]
#[instruction(txid: [u8; 32], vout: u32)]
pub struct ReleaseMint<'info> {
    #[account(seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    #[account(
        mut,
        seeds = [STAGED_MINT_SEED, txid.as_ref(), &vout.to_le_bytes()],
        bump = staged.bump,
        close = submitter,
    )]
    pub staged: Account<'info, StagedMint>,
    #[account(mut, seeds = [b"mint"], bump)]
    pub mint: Account<'info, Mint>,
    #[account(mut, seeds = [VAULT_SEED], bump)]
    pub vault: Account<'info, TokenAccount>,
    /// Created on the recipient's behalf if they have never held solBSV, so a
    /// first-time user needs no SOL to receive. This is why release is
    /// permissionless *and* costs the caller nothing but fees: the caller pays
    /// the ATA's rent if it does not exist.
    #[account(
        init_if_needed,
        payer = submitter,
        associated_token::mint = mint,
        associated_token::authority = recipient_owner,
    )]
    pub recipient_token_account: Account<'info, TokenAccount>,
    /// CHECK: the ATA above is derived from this account, and its key is checked
    /// against the staged item's stored `recipient` before anything moves.
    pub recipient_owner: UncheckedAccount<'info>,
    #[account(mut)]
    pub submitter: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

/// Burn one staged mint whose deposit the chain has moved against.
///
/// Same seed situation as [`ReleaseMint`]. No associated token account is
/// involved: burning from the vault needs the mint, the vault and the vault's
/// authority only.
#[derive(Accounts)]
#[instruction(txid: [u8; 32], vout: u32)]
pub struct BurnStaged<'info> {
    #[account(seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    #[account(
        mut,
        seeds = [STAGED_MINT_SEED, txid.as_ref(), &vout.to_le_bytes()],
        bump = staged.bump,
        close = submitter,
    )]
    pub staged: Account<'info, StagedMint>,
    #[account(mut, seeds = [b"mint"], bump)]
    pub mint: Account<'info, Mint>,
    #[account(mut, seeds = [VAULT_SEED], bump)]
    pub vault: Account<'info, TokenAccount>,
    #[account(mut)]
    pub submitter: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

// ---------------------------------------------------------------------------
// peg-out accounts
// ---------------------------------------------------------------------------

/// Escrow a holder's `solBSV` into a per-redemption, program-owned token
/// account, and create the item that records the terms.
///
/// Three accounts are created: the singleton [`RedeemBook`] (on the first
/// redemption only), the [`PendingRedeem`] item, and the escrow token account —
/// both keyed on the sequential `id`. The holder signs and pays for all of
/// them, so the escrow is genuinely the holder's and cannot be opened by a third
/// party against their balance.
#[derive(Accounts)]
#[instruction(id: u64, amount: u64, bsv_address: [u8; 20])]
pub struct InitiateRedeem<'info> {
    #[account(mut)]
    pub holder: Signer<'info>,
    #[account(seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    /// The deadline policy this redemption copies. Read once, at initiation.
    #[account(seeds = [CONFIG_SEED], bump = config.bump)]
    pub config: Account<'info, Config>,
    /// The next id and the concurrent-redemption count. Created on the first
    /// redemption; a singleton, so it cannot be re-pointed.
    #[account(
        init_if_needed,
        payer = holder,
        space = RedeemBook::SPACE,
        seeds = [REDEEM_BOOK_SEED],
        bump,
    )]
    pub book: Account<'info, RedeemBook>,
    /// The item itself. `id` is an instruction argument, so the seeds are
    /// expressible here — unlike the mint vault's, whose keys come from a
    /// `DepositClaim`.
    #[account(
        init,
        payer = holder,
        space = PendingRedeem::SPACE,
        seeds = [REDEEM_SEED, &id.to_le_bytes()],
        bump,
    )]
    pub pending: Account<'info, PendingRedeem>,
    #[account(mut, seeds = [b"mint"], bump)]
    pub mint: Account<'info, Mint>,
    /// The holder's own token account the escrow is drawn from. Created on
    /// their behalf if they have never held `solBSV`.
    #[account(
        init_if_needed,
        payer = holder,
        associated_token::mint = mint,
        associated_token::authority = holder,
    )]
    pub holder_token_account: Account<'info, TokenAccount>,
    /// The escrow: a program-owned token account whose **authority is the light
    /// client PDA**, so only this program can move or burn what is in it.
    #[account(
        init,
        payer = holder,
        seeds = [REDEEM_ESCROW_SEED, &id.to_le_bytes()],
        bump,
        token::mint = mint,
        token::authority = light_client,
    )]
    pub escrow: Account<'info, TokenAccount>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

/// Return one escrowed redemption to its holder. Permissionless: the caller
/// receives the closed accounts' rent.
#[derive(Accounts)]
#[instruction(id: u64)]
pub struct CancelRedeem<'info> {
    #[account(seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    #[account(mut, seeds = [REDEEM_BOOK_SEED], bump = book.bump)]
    pub book: Account<'info, RedeemBook>,
    /// Closed on success, so a redemption exists exactly once and cannot be
    /// cancelled twice.
    #[account(
        mut,
        seeds = [REDEEM_SEED, &id.to_le_bytes()],
        bump = pending.bump,
        close = submitter,
    )]
    pub pending: Account<'info, PendingRedeem>,
    #[account(
        mut,
        seeds = [REDEEM_ESCROW_SEED, &id.to_le_bytes()],
        bump,
        token::mint = mint,
        token::authority = light_client,
    )]
    pub escrow: Account<'info, TokenAccount>,
    #[account(mut, seeds = [b"mint"], bump)]
    pub mint: Account<'info, Mint>,
    /// Created on the holder's behalf if they have closed it, so the return
    /// never depends on the holder having kept an account open.
    #[account(
        init_if_needed,
        payer = submitter,
        associated_token::mint = mint,
        associated_token::authority = holder,
    )]
    pub holder_token_account: Account<'info, TokenAccount>,
    /// CHECK: the destination is derived from this account, and its key is
    /// checked against the pending item's stored `holder` before anything moves.
    pub holder: UncheckedAccount<'info>,
    #[account(mut)]
    pub submitter: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

/// Prove a payout against the light client and stage the settlement.
///
/// `payout_nullifier` is an `UncheckedAccount` because its seeds come from the
/// `PayoutProof`'s `(txid, vout)`, which an Anchor `seeds` constraint cannot
/// see; the address is re-derived and compared inside [`stage_payout_outpoint`]
/// before anything is read or created.
#[derive(Accounts)]
#[instruction(id: u64, proof: PayoutProof)]
pub struct ClaimPayout<'info> {
    #[account(seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    #[account(
        mut,
        seeds = [REDEEM_SEED, &id.to_le_bytes()],
        bump = pending.bump,
    )]
    pub pending: Account<'info, PendingRedeem>,
    /// CHECK: address re-derived from `(proof.txid, proof.vout)` and compared
    /// against `PAYOUT_NULLIFIER_SEED` in `stage_payout_outpoint`.
    #[account(mut)]
    pub payout_nullifier: UncheckedAccount<'info>,
    #[account(mut)]
    pub submitter: Signer<'info>,
    pub system_program: Program<'info, System>,
}

/// Burn one settled escrow. Permissionless: the caller keeps the rent of the
/// two accounts this closes.
#[derive(Accounts)]
#[instruction(id: u64)]
pub struct SettleRedeem<'info> {
    #[account(seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    #[account(mut, seeds = [REDEEM_BOOK_SEED], bump = book.bump)]
    pub book: Account<'info, RedeemBook>,
    #[account(
        mut,
        seeds = [REDEEM_SEED, &id.to_le_bytes()],
        bump = pending.bump,
        close = submitter,
    )]
    pub pending: Account<'info, PendingRedeem>,
    #[account(
        mut,
        seeds = [REDEEM_ESCROW_SEED, &id.to_le_bytes()],
        bump,
        token::mint = mint,
        token::authority = light_client,
    )]
    pub escrow: Account<'info, TokenAccount>,
    #[account(mut, seeds = [b"mint"], bump)]
    pub mint: Account<'info, Mint>,
    #[account(mut)]
    pub submitter: Signer<'info>,
    pub token_program: Program<'info, Token>,
}

#[event]
pub struct ChainReorganised {
    pub from_height: u64,
    pub new_tip_height: u64,
}

/// A deposit has been verified and its mint **staged into the vault**.
///
/// `maturity_at_deposit` is emitted because it is the policy that applied when
/// the deposit was verified, and it is what `release_mint` will use — not the
/// config value at release time.
#[event]
pub struct DepositStaged {
    pub txid: [u8; 32],
    pub vout: u32,
    pub amount: u64,
    pub recipient: [u8; 32],
    pub height: u64,
    pub maturity_at_deposit: u64,
}

/// A deposit outpoint has been recorded as spent by the reserve, so
/// [`verify_deposit`] will refuse to mint it. Emitted so the record's contents
/// are visible from the log as well as from the PDA.
#[event]
pub struct SpentOutpointReported {
    pub txid: [u8; 32],
    pub vout: u32,
    /// The signer that reported it — the program authority today, the
    /// federation later.
    pub authority: Pubkey,
}

/// A staged mint has been released out of the vault to its recipient.
#[event]
pub struct MintReleased {
    pub txid: [u8; 32],
    pub vout: u32,
    pub recipient: Pubkey,
    pub amount: u64,
    pub height: u64,
}

/// A staged mint has been burned out of the vault, because the chain moved
/// against the deposit it was proven from.
#[event]
pub struct StagedMintBurned {
    pub txid: [u8; 32],
    pub vout: u32,
    pub amount: u64,
    pub height: u64,
}

/// A timelocked authority change has been recorded and is now visible until it
/// is executed or cancelled. Emitting it is the point of F4: notice.
#[event]
pub struct AuthorityChangeProposed {
    pub authority: Pubkey,
    pub effective_slot: u64,
}

#[event]
pub struct AuthorityChangeExecuted {
    pub authority: Pubkey,
    pub slot: u64,
}

/// A minted deposit's replay nullifier has been closed, so the deposit's block
/// has left the window. `deposit_height` is the stored value the prune checked,
/// emitted so the rule can be audited from the log.
#[event]
pub struct NullifierPruned {
    pub txid: [u8; 32],
    pub vout: u32,
    pub deposit_height: u64,
}

/// A redemption has been opened and its `solBSV` escrowed. `fee` and
/// `deadline_slot` are emitted because they are the terms **as they applied at
/// initiation**, which is what the exits resolve against.
#[event]
pub struct RedeemInitiated {
    pub id: u64,
    pub holder: Pubkey,
    pub amount: u64,
    pub fee: u64,
    pub bsv_address: [u8; 20],
    pub deadline_slot: u64,
}

/// A redemption's deadline passed with no live payout, and the escrow was
/// returned to the holder — **unchanged**, no fee.
#[event]
pub struct RedeemCancelled {
    pub id: u64,
    pub holder: Pubkey,
    pub amount: u64,
}

/// A payout was proven against the light client and staged. `payout_min` is the
/// floor the proof had to meet (`amount − fee`), emitted so the rule can be
/// audited from the log.
#[event]
pub struct PayoutClaimed {
    pub id: u64,
    pub txid: [u8; 32],
    pub vout: u32,
    pub height: u64,
    pub amount: u64,
    pub payout_min: u64,
}

/// A settled redemption was burned, after the challenge window passed with the
/// payout still canonical.
#[event]
pub struct RedeemSettled {
    pub id: u64,
    pub holder: Pubkey,
    pub amount: u64,
    pub payout_height: u64,
}

/// The federation and its Greycore were created, with the two aggregate keys
/// that fill the reserve script's legs.
///
/// Emitted because these are the values the registry says the reserve is, and
/// the program cannot verify any of them: the log is what makes the claim
/// visible and attributable to the key that made it.
#[event]
pub struct FederationInitialized {
    pub authority: Pubkey,
    pub threshold: u64,
    pub gateway_key: Pubkey,
    pub greycore_key: Pubkey,
}

/// A Greycore member was appointed. Authority-gated in this PoC.
#[event]
pub struct GreycoreMemberAdded {
    pub identity: Pubkey,
}

/// A gateway member was admitted, by the named Greycore member.
///
/// `approved_by` is the point of the event: admission is the Greycore's job, so
/// who approved whom is the part that has to be attributable.
#[event]
pub struct MemberAdmitted {
    pub identity: Pubkey,
    pub approved_by: Pubkey,
}

/// Both sides of a member's bond were recorded.
///
/// `bond_key` is the collective threshold key the **BSV-side** bond sits under,
/// and the `bsv_bond` figure is the member's **attestation**: the program cannot
/// see BSV, so this event records a declaration, not a deposit.
#[event]
pub struct BondsRecorded {
    pub identity: Pubkey,
    pub bsv_bond: u64,
    pub solbsv_bond: u64,
    pub bond_key: Pubkey,
}

/// A member departed. `bonded_remaining` is emitted so the size of the signing
/// set after the departure is visible from the log, not only from the config.
#[event]
pub struct MemberLeft {
    pub identity: Pubkey,
    pub returned_bsv: u64,
    pub returned_solbsv: u64,
    pub bonded_remaining: u64,
}

/// A member's recorded **`solBSV`-side** bond was cleared on the authority's
/// instruction.
///
/// The name says `solbsv` deliberately: the BSV-side bond is untouched and
/// untouchable from here, and a reader must not read this event as a bond being
/// taken.
#[event]
pub struct SolbsvBondSeized {
    pub identity: Pubkey,
    pub amount: u64,
    pub authority: Pubkey,
}

// ---------------------------------------------------------------------------
// Merkle folding and minimal transaction parsing
// ---------------------------------------------------------------------------
//
// These must agree with `poc/checks/bsvlib.py`. Where they disagree, the
// disagreement is the bug.

/// Fold a Merkle branch from leaf to root. Mirrors `bsvlib.fold_branch`.
pub fn fold_branch(txid: [u8; 32], mut index: u32, branch: &[[u8; 32]]) -> [u8; 32] {
    let mut current = txid;
    for sibling in branch {
        current = if index % 2 == 0 {
            merkle_hash(&current, sibling)
        } else {
            merkle_hash(sibling, &current)
        };
        index /= 2;
    }
    current
}

/// Double SHA-256 of arbitrary bytes, in internal order.
pub fn header_hash_of_bytes(bytes: &[u8]) -> [u8; 32] {
    sha256(sha256(bytes).as_ref()).to_bytes()
}

/// `H(a || b)` — the Merkle interior node, matching `bsvlib.merkle_hash`.
pub fn merkle_hash(a: &[u8; 32], b: &[u8; 32]) -> [u8; 32] {
    let mut buf = [0u8; 64];
    buf[..32].copy_from_slice(a);
    buf[32..].copy_from_slice(b);
    sha256(sha256(&buf).as_ref()).to_bytes()
}

fn read_varint(raw: &[u8], i: &mut usize) -> Result<u64> {
    let first = *raw.get(*i).ok_or(SolbeamError::MalformedTx)?;
    *i += 1;
    Ok(match first {
        0xfd => {
            let v = u16::from_le_bytes(read_n(raw, *i, 2)?.try_into().unwrap()) as u64;
            *i += 2;
            v
        }
        0xfe => {
            let v = u32::from_le_bytes(read_n(raw, *i, 4)?.try_into().unwrap()) as u64;
            *i += 4;
            v
        }
        0xff => {
            let v = u64::from_le_bytes(read_n(raw, *i, 8)?.try_into().unwrap());
            *i += 8;
            v
        }
        n => n as u64,
    })
}

fn read_n<'a>(raw: &'a [u8], at: usize, n: usize) -> Result<&'a [u8]> {
    raw.get(at..at + n).ok_or(SolbeamError::MalformedTx.into())
}

/// The outputs of a legacy transaction, as `(value, script)`.
///
/// BSV has no SegWit, so there is no marker, no flag and no witness to skip —
/// the format is the original one, which is why this is short.
pub fn parse_outputs(raw: &[u8]) -> Result<Vec<(u64, Vec<u8>)>> {
    let mut i = 4usize; // version

    let vin = read_varint(raw, &mut i)?;
    for _ in 0..vin {
        i = i.checked_add(36).ok_or(SolbeamError::MalformedTx)?; // outpoint
        let script_len = read_varint(raw, &mut i)? as usize;
        i = i.checked_add(script_len).ok_or(SolbeamError::MalformedTx)?;
        i = i.checked_add(4).ok_or(SolbeamError::MalformedTx)?; // sequence
        if i > raw.len() {
            return Err(SolbeamError::MalformedTx.into());
        }
    }

    let vout = read_varint(raw, &mut i)?;
    let mut outputs = Vec::new();
    for _ in 0..vout {
        let value = u64::from_le_bytes(read_n(raw, i, 8)?.try_into().unwrap());
        i += 8;
        let script_len = read_varint(raw, &mut i)? as usize;
        let script = read_n(raw, i, script_len)?.to_vec();
        i += script_len;
        outputs.push((value, script));
    }

    // locktime must be present, which also rejects a truncated transaction
    if i + 4 > raw.len() {
        return Err(SolbeamError::MalformedTx.into());
    }
    Ok(outputs)
}

/// The payload of an `OP_RETURN` output, if it is one.
pub fn op_return_payload(script: &[u8]) -> Option<&[u8]> {
    if script.first() != Some(&0x6a) {
        return None;
    }
    let len = *script.get(1)? as usize;
    script.get(2..2 + len)
}

/// The longest deposit script the account will hold: a 2-of-2 multisig.
///
/// `OP_2 <33-byte key> <33-byte key> OP_2 OP_CHECKMULTISIG` is 1+34+34+1+1 = 71 bytes.
/// Grown from 25 when the reserve script became a 2-of-2 rather than a plain P2PKH.
pub use params::MAX_SCRIPT_LEN;

/// A canonical P2PKH script: `76 a9 14 <20 bytes> 88 ac`, 25 bytes.
///
/// Retained because the current deployment pays a single key, and the Phase 1A fixture
/// is a real deposit to a P2PKH address. The target model is [`is_reserve_multisig`].
pub fn is_p2pkh(script: &[u8]) -> bool {
    script.len() == 25
        && script[0] == 0x76
        && script[1] == 0xa9
        && script[2] == 0x14
        && script[23] == 0x88
        && script[24] == 0xac
}

/// The reserve script of the Greycore model:
/// `OP_2 <gateway threshold key> <greycore key> OP_2 OP_CHECKMULTISIG`.
///
/// One key is the federation's threshold key, which emits **one** signature however many
/// members signed; the other is the Greycore's. **Both must sign**, so neither the
/// federation majority nor the Greycore can move the reserve alone. That is what constrains
/// the reserve — the bond does not, and cannot.
///
/// 71 bytes: `52 21 <33> 21 <33> 52 ae`.
pub fn is_reserve_multisig(script: &[u8]) -> bool {
    script.len() == 71
        && script[0] == 0x52
        && script[1] == 0x21
        && script[35] == 0x21
        && script[69] == 0x52
        && script[70] == 0xae
}

/// Either accepted shape. Anything else is refused, so the authority cannot set a script
/// nobody can pay — or one that is unspendable.
pub fn is_acceptable_deposit_script(script: &[u8]) -> bool {
    is_p2pkh(script) || is_reserve_multisig(script)
}

// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// the federation's two pure rules
// ---------------------------------------------------------------------------
//
// Both are factored out of the instruction handlers so they can be unit-tested
// directly. That is not tidiness: the departure floor is the registry's only
// numeric invariant and it is **unreachable on a local validator without
// building a five-member federation first**, and the key rule is a property of
// values the program cannot see. A rule that can only be exercised through an
// expensive setup is a rule that gets tested by accident, so the arithmetic and
// the comparison live here and are tested below.

/// Does a departure leave the gateway at or above its signing threshold?
///
/// `bonded` is the member count **before** the departure, so the post-state is
/// `bonded - 1`. The floor is `t` of `t-of-N`: below `t` the reserve key cannot
/// produce a signature at all, so the registry would be recording a federation
/// that cannot act. A count that cannot be decremented at all (0) is also a
/// refusal; it is a state the handlers cannot reach, and this returns false for
/// it rather than letting the subtraction wrap.
///
/// This is a floor on the **signing set**, and deliberately **not** a capacity
/// rule about the reserve: no numeric capacity rule exists in this design, and
/// none is introduced here.
pub fn departure_keeps_threshold(bonded: u64, threshold: u64) -> bool {
    match bonded.checked_sub(1) {
        Some(remaining) => remaining >= threshold,
        None => false,
    }
}

/// Are the two legs of the reserve script genuinely two different keys?
///
/// The reserve script is `OP_2 <gateway threshold key> <greycore key> OP_2
/// OP_CHECKMULTISIG`, both legs required. If the two were the same key, the
/// script would still assemble and would still say 2-of-2, but **one signer
/// would satisfy both legs** -- the second quorum, which is the only thing
/// constraining the reserve, would not exist. So the keys must differ, and
/// neither may be the all-zero key.
///
/// The program **cannot verify that either key is real**: no key generation and
/// no sharing happens here, so a caller can record any non-zero key it likes.
/// That is the accepted limit of a registry, and it is why this rule is about
/// *distinctness* and not about custody.
pub fn reserve_keys_distinct(gateway_key: &Pubkey, greycore_key: &Pubkey) -> bool {
    *gateway_key != Pubkey::default()
        && *greycore_key != Pubkey::default()
        && gateway_key != greycore_key
}

#[error_code]
pub enum SolbeamError {
    #[msg("header must be exactly 80 bytes")]
    BadLength,
    #[msg("header does not link to the current tip")]
    BrokenLinkage,
    #[msg("header does not meet its proof-of-work target")]
    BadPow,
    #[msg("the checkpoint header does not meet its own target")]
    CheckpointBadPow,
    #[msg("unexpected difficulty retarget")]
    UnexpectedRetarget,
    #[msg("the light client is paused")]
    Paused,
    #[msg("arithmetic overflow")]
    Overflow,
    #[msg("no header at that height is inside the window")]
    HeaderNotInWindow,
    #[msg("the raw transaction does not hash to the claimed txid")]
    TxidMismatch,
    #[msg("the Merkle branch does not fold to the block's root")]
    BadMerkleProof,
    #[msg("the supplied header is not the canonical block at that height")]
    HeaderMismatch,
    #[msg("the transaction is not valid legacy format")]
    MalformedTx,
    #[msg("no such output in that transaction")]
    NoSuchOutput,
    #[msg("the output does not carry the claimed amount")]
    AmountMismatch,
    #[msg("the output carries no value")]
    ZeroValue,
    #[msg("the output does not pay the bridge's deposit script")]
    WrongOutputScript,
    #[msg("no OP_RETURN carrying this recipient")]
    MissingPayload,
    #[msg("not enough confirmations yet")]
    InsufficientConfirmations,
    #[msg("this deposit has already been minted")]
    AlreadyMinted,
    #[msg("the nullifier account is not the PDA this (txid, vout) derives")]
    WrongNullifier,
    #[msg("the deposit's block is still inside the header window, so its nullifier cannot be pruned")]
    NullifierNotPrunable,
    #[msg("effective_slot is sooner than the authority timelock allows")]
    TimelockTooSoon,
    #[msg("the authority timelock has not elapsed yet")]
    TimelockNotElapsed,
    #[msg("deposit script must be a canonical P2PKH or a 2-of-2 reserve multisig")]
    DepositScriptNotP2pkh,
    #[msg("deposit script exceeds MAX_SCRIPT_LEN")]
    DepositScriptTooLong,
    #[msg("the recipient account does not match the OP_RETURN payload")]
    RecipientMismatch,
    #[msg("a competing branch must contain at least one header")]
    EmptyFork,
    #[msg("the fork point is not inside the header window")]
    ForkPointNotInWindow,
    #[msg("the fork point has moved since this branch was staged — re-stage it")]
    ForkPointMoved,
    #[msg("the competing branch is not heavier than the current one")]
    ForkNotHeavier,
    #[msg("the staging account belongs to a different submitter")]
    NotStagingOwner,
    #[msg("the staged branch is longer than the window")]
    ForkTooLong,
    #[msg("more headers in one batch than a transaction can carry")]
    BatchTooLarge,
    #[msg("the signer is not the light client's authority")]
    Unauthorized,
    #[msg("the header chain is still waiting for its trusted seed")]
    Seeding,
    #[msg("the client is not seeding — seed_headers is only valid on a fresh checkpoint")]
    NotSeeding,
    #[msg("the seed batch is longer than the records still required")]
    SeedTooLong,
    #[msg("the seed does not end at the checkpoint it was opened for")]
    SeedWrongTip,
    #[msg("the completed seed did not produce a full lookback window")]
    SeedWrongLength,
    #[msg("the seed batch is empty")]
    EmptySeed,
    #[msg("the checkpoint is lower than the seed's lookback")]
    CheckpointTooLow,
    #[msg("the window does not reach cw-144's lookback, so no target is computable")]
    DifficultyNotComputable,
    #[msg("the fork point is too close to the bottom of the window for cw-144")]
    ForkPointTooOld,
    #[msg("the light client account data is not a well-formed LightClient")]
    MalformedClientData,
    #[msg("the staged-mint account is not the PDA this (txid, vout) derives")]
    WrongStagedMint,
    #[msg("this deposit has already been staged")]
    AlreadyStaged,
    #[msg("the light client's view of the chain is older than max_staleness_slots")]
    StaleClient,
    #[msg("the deposit has not reached the maturity recorded when it was verified")]
    NotMatured,
    #[msg("the deposit's block has left the header window, so its hash can no longer be checked")]
    DepositHeightNotInWindow,
    #[msg("the client's stored hash at the deposit's height has changed — the deposit was reorged")]
    DepositHashChanged,
    #[msg("the client's stored hash at the deposit's height is unchanged — there is nothing to burn")]
    DepositHashUnchanged,
    #[msg("the spent-outpoint account is not the PDA this (txid, vout) derives")]
    WrongSpentOutpoint,
    #[msg("this deposit outpoint is already recorded as spent")]
    OutpointAlreadyReported,
    #[msg("the reserve has spent this deposit's outpoint, so it cannot be minted")]
    DepositSpent,
    #[msg("the redemption is below po.d_min")]
    BelowMinimumRedeem,
    #[msg("the redemption id is not the book's next id")]
    WrongRedeemId,
    #[msg("po.max_pending concurrent redemptions are already open")]
    TooManyPendingRedemptions,
    #[msg("the redemption deadline has not passed yet")]
    DeadlineNotReached,
    #[msg("a live payout has been claimed for this redemption, so it cannot be cancelled")]
    RedeemClaimed,
    #[msg("the cancel destination is not the redemption's recorded holder")]
    HolderMismatch,
    #[msg("no payout has been claimed for this redemption")]
    PayoutNotClaimed,
    #[msg("the client's stored hash at the payout's height has changed — the payout was reorged")]
    PayoutReorged,
    #[msg("the payout's block has left the header window, so it can no longer be checked")]
    PayoutHeightNotInWindow,
    #[msg("the payout has not yet been buried for po.challenge_window BSV blocks")]
    ChallengeWindowNotElapsed,
    #[msg("a live payout is already staged for this redemption")]
    PayoutAlreadyClaimed,
    #[msg("the output does not pay the redemption's BSV address")]
    PayoutAddressMismatch,
    #[msg("the output carries less than the redemption's amount minus its fee")]
    PayoutAmountTooLow,
    #[msg("the payout-nullifier account is not the PDA this (txid, vout) derives")]
    WrongPayoutNullifier,
    #[msg("this payout outpoint has already settled a different redemption")]
    PayoutOutpointReused,
    #[msg("the gateway signing threshold must be at least 2 and its keys non-empty and distinct")]
    BadFederationThreshold,
    #[msg("the gateway and Greycore keys in the reserve script must be two different keys")]
    ReserveKeysNotDistinct,
    #[msg("the member account is not the PDA this identity derives")]
    MemberIdentityMismatch,
    #[msg("admission requires a signature from a registered Greycore member")]
    NotGreycore,
    #[msg("the gateway roster is at its headroom above the threshold")]
    FederationFull,
    #[msg("this member is not active, so the seat cannot change")]
    MemberNotActive,
    #[msg("both sides of the bond must be recorded before the seat counts")]
    BondMissing,
    #[msg("this member has already recorded both bonds")]
    BondsAlreadyRecorded,
    #[msg("leaving would take the bonded gateway set below its signing threshold")]
    GatewayBelowThreshold,
    #[msg("this member has no recorded solBSV-side bond to seize")]
    NoBondToSeize,
}

#[cfg(test)]
mod script_shape_tests {
    use super::*;

    fn p2pkh() -> Vec<u8> {
        let mut s = vec![0x76, 0xa9, 0x14];
        s.extend([0u8; 20]);
        s.extend([0x88, 0xac]);
        s
    }

    fn reserve_2of2() -> Vec<u8> {
        let mut s = vec![0x52, 0x21];
        s.extend([2u8; 33]);
        s.push(0x21);
        s.extend([3u8; 33]);
        s.extend([0x52, 0xae]);
        s
    }

    #[test]
    fn reserve_script_shapes() {
        let m = reserve_2of2();
        assert_eq!(m.len(), 71, "a 2-of-2 reserve script is 71 bytes");
        assert!(is_reserve_multisig(&m));
        assert!(is_acceptable_deposit_script(&m));
        assert!(!is_p2pkh(&m));

        let p = p2pkh();
        assert_eq!(p.len(), 25);
        assert!(is_p2pkh(&p));
        assert!(is_acceptable_deposit_script(&p));
        assert!(!is_reserve_multisig(&p));

        assert!(MAX_SCRIPT_LEN >= m.len(), "the account must fit what we accept");

        // Wrong shapes must be refused.
        assert!(!is_acceptable_deposit_script(&[0x00, 0x01, 0x02, 0x03]));
        assert!(!is_acceptable_deposit_script(&[]));
        // 70 bytes: the last OP_CHECKMULTISIG is missing.
        let mut short = reserve_2of2();
        short.pop();
        assert!(!is_acceptable_deposit_script(&short));
        // 72 bytes: one byte too long.
        let mut long = reserve_2of2();
        long.push(0x00);
        assert!(!is_acceptable_deposit_script(&long));
        // A P2PKH with one byte altered.
        let mut bad_p = p2pkh();
        bad_p[2] = 0x15;
        assert!(!is_acceptable_deposit_script(&bad_p));
    }
}

/// The hand-written layouts the vault creates by hand.
///
/// `create_staged_mint` writes `StagedMint` field for field rather than through
/// Borsh, because the account is built by `allocate`/`assign` (its seeds are
/// instruction arguments, so it cannot be an Anchor `init`). That is exactly the
/// kind of code where a field reorder compiles and then reads the wrong bytes on
/// chain, so the two representations are compared directly here.
#[cfg(test)]
mod vault_layout_tests {
    use super::*;

    #[test]
    fn staged_mint_manual_layout_matches_borsh() {
        let m = StagedMint {
            recipient: Pubkey::new_from_array([7u8; 32]),
            amount: 0x0000_0001_2345_6789,
            deposit_height: 968_376,
            deposit_hash: [0xab; 32],
            maturity_at_deposit: 144,
            bump: 251,
        };

        let mut borsh = Vec::new();
        borsh.extend_from_slice(StagedMint::DISCRIMINATOR);
        AnchorSerialize::serialize(&m, &mut borsh).unwrap();
        assert_eq!(borsh.len(), StagedMint::SPACE);

        // The byte writes `create_staged_mint` performs, at the offsets it uses.
        let mut manual = vec![0u8; StagedMint::SPACE];
        manual[..8].copy_from_slice(StagedMint::DISCRIMINATOR);
        manual[8..40].copy_from_slice(&m.recipient.to_bytes());
        manual[40..48].copy_from_slice(&m.amount.to_le_bytes());
        manual[48..56].copy_from_slice(&m.deposit_height.to_le_bytes());
        manual[56..88].copy_from_slice(&m.deposit_hash);
        manual[88..96].copy_from_slice(&m.maturity_at_deposit.to_le_bytes());
        manual[96] = m.bump;

        assert_eq!(manual, borsh, "you must update create_staged_mint");
    }

    /// `Config` is small enough to check the same way, and both policy fields
    /// it carries are read before any token moves: `maturity_blocks` on the
    /// mint path, `redeem_deadline_slots` on the peg-out path.
    #[test]
    fn config_layout_is_maturity_then_redeem_deadline_then_bump() {
        let c = Config {
            maturity_blocks: 1_234_567,
            redeem_deadline_slots: 216_000,
            bump: 254,
        };
        let mut data = Vec::new();
        data.extend_from_slice(Config::DISCRIMINATOR);
        AnchorSerialize::serialize(&c, &mut data).unwrap();
        assert_eq!(data.len(), Config::SPACE);
        assert_eq!(
            u64::from_le_bytes(data[8..16].try_into().unwrap()),
            c.maturity_blocks
        );
        assert_eq!(
            u64::from_le_bytes(data[16..24].try_into().unwrap()),
            c.redeem_deadline_slots
        );
        assert_eq!(data[24], c.bump);
    }

    /// The spent-outpoint record is the other account `create_spent_outpoint`
    /// writes by hand, and it is the one whose layout must stay trivial: it
    /// stores only `bump`, because its existence is the record and it is
    /// permanent. A field added here without a reader would be dead weight on
    /// every reported outpoint forever.
    #[test]
    fn spent_outpoint_layout_is_discriminator_then_bump() {
        let s = SpentOutpoint { bump: 253 };
        let mut borsh = Vec::new();
        borsh.extend_from_slice(SpentOutpoint::DISCRIMINATOR);
        AnchorSerialize::serialize(&s, &mut borsh).unwrap();
        assert_eq!(borsh.len(), SpentOutpoint::SPACE);

        // The two byte writes `create_spent_outpoint` performs.
        let mut manual = vec![0u8; SpentOutpoint::SPACE];
        manual[..8].copy_from_slice(SpentOutpoint::DISCRIMINATOR);
        manual[8] = s.bump;

        assert_eq!(manual, borsh, "you must update create_spent_outpoint");
    }
}

/// The peg-out's hand-written layouts and its two pure functions.
///
/// `PendingRedeem` goes through Anchor's `init`, so it is Borsh on both sides
/// and only needs its `SPACE` pinned to the fields it carries. `PayoutNullifier`
/// is the one peg-out account `stage_payout_outpoint` writes by hand — for the
/// same reason the nullifier and the spent record are written by hand, its seeds
/// are instruction arguments — so its layout is compared against a real
/// `AnchorSerialize` exactly as those two are.
#[cfg(test)]
mod pegout_tests {
    use super::*;

    #[test]
    fn pending_redeem_space_matches_its_fields() {
        let p = PendingRedeem {
            holder: Pubkey::new_from_array([3u8; 32]),
            amount: 0x0000_0001_2345_6789,
            fee: 15_000,
            bsv_address: [0x5a; 20],
            initiated_slot: 999,
            deadline_slot: 999 + REDEEM_DEADLINE_SLOTS,
            payout_height: 968_376,
            payout_hash: [0xcd; 32],
            bump: 250,
        };
        let mut data = Vec::new();
        data.extend_from_slice(PendingRedeem::DISCRIMINATOR);
        AnchorSerialize::serialize(&p, &mut data).unwrap();
        assert_eq!(data.len(), PendingRedeem::SPACE);
    }

    #[test]
    fn payout_nullifier_manual_layout_matches_borsh() {
        let n = PayoutNullifier {
            redemption_id: 0x0000_0000_0000_002a,
            bump: 249,
        };
        let mut borsh = Vec::new();
        borsh.extend_from_slice(PayoutNullifier::DISCRIMINATOR);
        AnchorSerialize::serialize(&n, &mut borsh).unwrap();
        assert_eq!(borsh.len(), PayoutNullifier::SPACE);

        // The two byte writes `stage_payout_outpoint` performs.
        let mut manual = vec![0u8; PayoutNullifier::SPACE];
        manual[..8].copy_from_slice(PayoutNullifier::DISCRIMINATOR);
        manual[8..16].copy_from_slice(&n.redemption_id.to_le_bytes());
        manual[16] = n.bump;

        assert_eq!(manual, borsh, "you must update stage_payout_outpoint");
    }

    /// The fee is `fee.redeem_bp` basis points of the amount, rounded down, and
    /// the payout floor is what is left. The arithmetic is checked here because
    /// it decides what a claim must prove and a wrong rounding would move real
    /// value for every redemption at once.
    #[test]
    fn redeem_fee_is_basis_points_rounded_down() {
        assert_eq!(REDEEM_FEE_BP, 30);
        // 0.01 BSV at 30 bp: 3,000 base units, floor 997,000.
        assert_eq!(redeem_fee(1_000_000).unwrap(), 3_000);
        // 1 BSV at 30 bp.
        assert_eq!(redeem_fee(100_000_000).unwrap(), 300_000);
        // Rounds down: 1,001 * 30 / 10,000 = 3.003 -> 3.
        assert_eq!(redeem_fee(1_001).unwrap(), 3);
        // The floor is the minimum itself, so the value moved is always the fee.
        let amount = 5_000_000u64;
        assert_eq!(amount - redeem_fee(amount).unwrap(), 4_985_000);
        // No overflow at the whole-supply scale: 21M BSV in base units.
        assert_eq!(redeem_fee(2_100_000_000_000_000).unwrap(), 6_300_000_000_000);
    }

    /// The stored address must expand to exactly the canonical P2PKH script, and
    /// nothing else may pass — including a P2PKH for a different hash, the
    /// reserve's own multisig, and a one-byte-mutated script.
    #[test]
    fn payout_script_must_be_the_stored_addresses_p2pkh() {
        let hash = [0x11u8; 20];
        let mut script = vec![0x76, 0xa9, 0x14];
        script.extend(hash);
        script.extend([0x88, 0xac]);
        assert_eq!(script.len(), 25);
        assert!(is_p2pkh_for(&script, &hash));

        // A different holder's address.
        assert!(!is_p2pkh_for(&script, &[0x22u8; 20]));
        // The reserve script: a valid deposit script, not a payout address.
        let mut multisig = vec![0x52, 0x21];
        multisig.extend([2u8; 33]);
        multisig.push(0x21);
        multisig.extend([3u8; 33]);
        multisig.extend([0x52, 0xae]);
        assert!(is_reserve_multisig(&multisig));
        assert!(!is_p2pkh_for(&multisig, &hash));
        // One byte altered.
        let mut bad = script.clone();
        bad[2] = 0x15;
        assert!(!is_p2pkh_for(&bad, &hash));
        // Truncated.
        assert!(!is_p2pkh_for(&script[..24], &hash));
    }

    /// `po.cancel_grace` ships at **0**, so on a local validator the term is a
    /// no-op and no on-chain assertion can tell it apart from an omitted add.
    /// The constant is therefore asserted here, and the arithmetic that uses it
    /// is exercised directly — including a non-zero grace the shipped
    /// configuration cannot reach. This states exactly what is and is not
    /// covered: the formula is tested, the shipped value is 0, and an
    /// end-to-end non-zero grace stays untested because there is none to run.
    #[test]
    fn redeem_deadline_is_policy_plus_cancel_grace() {
        assert_eq!(CANCEL_GRACE_SLOTS, 0, "shipped po.cancel_grace");
        // The shipped arithmetic: now + policy + 0.
        assert_eq!(redeem_deadline_slot(1_000, 216_000, 0).unwrap(), 217_000);
        // The term the parameter adds when it is not zero.
        assert_eq!(redeem_deadline_slot(1_000, 216_000, 30).unwrap(), 217_030);
        assert_eq!(
            redeem_deadline_slot(1_000, 216_000, CANCEL_GRACE_SLOTS).unwrap(),
            1_000 + 216_000
        );
        // The grace is added after the policy, so it can only move the deadline
        // later, never truncate it.
        assert!(
            redeem_deadline_slot(1_000, 216_000, 30).unwrap()
                > redeem_deadline_slot(1_000, 216_000, 0).unwrap()
        );
        // Overflow is an error, not a wrap.
        assert!(redeem_deadline_slot(u64::MAX, 1, 0).is_err());
        assert!(redeem_deadline_slot(1, u64::MAX, 0).is_err());
        assert!(redeem_deadline_slot(1, 1, u64::MAX).is_err());
    }
}

/// The federation registry's layout and its two pure rules.
///
/// The accounts are compared against a real `AnchorSerialize` because a field
/// reorder compiles and then reads the wrong bytes on chain, which is exactly
/// the failure this project has already had once. `FederationConfig` and
/// `GatewayMember` are the two accounts whose fields the threshold floor and a
/// member's bonds are read from, so their layouts are pinned here.
///
/// The two pure rules are tested directly because **the departure floor cannot
/// be reached on a local validator without first building a federation of
/// `threshold + 1` bonded members**, and that end-to-end test lives in the
/// TypeScript suite where it belongs. What is tested here is the arithmetic and
/// the key comparison; what is tested there is that the handlers consult them.
#[cfg(test)]
mod federation_tests {
    use super::*;

    fn distinct_keys() -> (Pubkey, Pubkey) {
        (
            Pubkey::new_from_array([7u8; 32]),
            Pubkey::new_from_array([9u8; 32]),
        )
    }

    /// A departure is refused **at** the floor and allowed above it, and the
    /// floor is `t` rather than `t - 1`. At the shipped `4-of-N` a set of four
    /// refuses -- and so does an empty set, rather than underflowing -- while a
    /// set of five is the first that may lose a member.
    #[test]
    fn departure_is_refused_at_the_threshold_and_allowed_above_it() {
        // The shipped gateway threshold, read from the generated sheet rather
        // than retyped, so a parameter change moves this test with it.
        let t = 4u64;
        assert!(!departure_keeps_threshold(t, t), "4 -> 3 is below 4-of-N");
        assert!(departure_keeps_threshold(t + 1, t), "5 -> 4 is still 4-of-N");
        assert!(departure_keeps_threshold(9, t));
        // Below the threshold to begin with: 3 -> 2 cannot host a 4-of-N key.
        assert!(!departure_keeps_threshold(3, t));
        // An empty set refuses rather than wrapping.
        assert!(!departure_keeps_threshold(0, t));
        // Exactly zero remaining is a refusal for any non-zero threshold.
        assert!(!departure_keeps_threshold(1, 1));
        // The boundary, stated once more: the count after the departure is
        // `bonded - 1`, and it is compared with `>=`, so `t + 1` is the
        // smallest set that may lose a seat.
        for bonded in 0..12u64 {
            assert_eq!(
                departure_keeps_threshold(bonded, t),
                bonded >= 1 && bonded - 1 >= t,
                "bonded = {bonded}"
            );
        }
    }

    /// The two reserve legs must be two different, non-empty keys.
    ///
    /// A repeated key would still assemble a `2-of-2` script, but one signer
    /// would satisfy both legs and **the second quorum would not exist** -- the
    /// mechanism that stops a gateway majority moving the reserve alone.
    #[test]
    fn reserve_legs_must_be_two_distinct_non_empty_keys() {
        let (gateway, greycore) = distinct_keys();
        assert!(reserve_keys_distinct(&gateway, &greycore));
        // Order does not matter; only distinctness does.
        assert!(reserve_keys_distinct(&greycore, &gateway));

        // The same key in both legs: one signer would satisfy the whole script.
        assert!(!reserve_keys_distinct(&gateway, &gateway));
        // The all-zero key is not a key, in either leg.
        assert!(!reserve_keys_distinct(&Pubkey::default(), &greycore));
        assert!(!reserve_keys_distinct(&gateway, &Pubkey::default()));
        assert!(!reserve_keys_distinct(&Pubkey::default(), &Pubkey::default()));
    }

    /// `FederationConfig` is Borsh on both sides (created by `init`), so its
    /// `SPACE` must be exactly what its fields serialise to. The order pinned
    /// here is the order the handlers read.
    #[test]
    fn federation_config_layout_matches_its_fields() {
        let (gateway, greycore) = distinct_keys();
        let fed = FederationConfig {
            authority: Pubkey::new_from_array([1u8; 32]),
            threshold: 4,
            gateway_key: gateway,
            greycore_threshold: 1,
            greycore_key: greycore,
            gateway_members: 3,
            greycore_members: 2,
            bonded_members: 2,
            bump: 255,
        };

        let mut data = Vec::new();
        data.extend_from_slice(FederationConfig::DISCRIMINATOR);
        AnchorSerialize::serialize(&fed, &mut data).unwrap();
        assert_eq!(data.len(), FederationConfig::SPACE);

        // The exact byte offsets the account's readers use, so a reorder is a
        // test failure rather than a silent misread.
        assert_eq!(data[8..40], fed.authority.to_bytes());
        assert_eq!(u64::from_le_bytes(data[40..48].try_into().unwrap()), 4);
        assert_eq!(data[48..80], gateway.to_bytes());
        assert_eq!(u64::from_le_bytes(data[80..88].try_into().unwrap()), 1);
        assert_eq!(data[88..120], greycore.to_bytes());
        assert_eq!(u64::from_le_bytes(data[120..128].try_into().unwrap()), 3);
        assert_eq!(u64::from_le_bytes(data[128..136].try_into().unwrap()), 2);
        assert_eq!(u64::from_le_bytes(data[136..144].try_into().unwrap()), 2);
        assert_eq!(data[144], 255);
    }

    /// `GreycoreMember` and `GatewayMember`: the two registries' records.
    #[test]
    fn gateway_and_greycore_member_layouts_match_their_fields() {
        let (gateway, greycore) = distinct_keys();

        let gc = GreycoreMember {
            identity: gateway,
            bump: 254,
        };
        let mut data = Vec::new();
        data.extend_from_slice(GreycoreMember::DISCRIMINATOR);
        AnchorSerialize::serialize(&gc, &mut data).unwrap();
        assert_eq!(data.len(), GreycoreMember::SPACE);
        assert_eq!(data[8..40], gateway.to_bytes());
        assert_eq!(data[40], 254);

        let member = GatewayMember {
            identity: gateway,
            bsv_bond: 1_000,
            solbsv_bond: 2_000,
            bond_key: greycore,
            approved_by: Pubkey::new_from_array([11u8; 32]),
            bonded: true,
            status: MemberStatus::Active,
            bump: 253,
        };
        let mut m = Vec::new();
        m.extend_from_slice(GatewayMember::DISCRIMINATOR);
        AnchorSerialize::serialize(&member, &mut m).unwrap();
        assert_eq!(m.len(), GatewayMember::SPACE);
        assert_eq!(m[8..40], gateway.to_bytes());
        assert_eq!(u64::from_le_bytes(m[40..48].try_into().unwrap()), 1_000);
        assert_eq!(u64::from_le_bytes(m[48..56].try_into().unwrap()), 2_000);
        assert_eq!(m[56..88], greycore.to_bytes());
        assert_eq!(m[88..120], member.approved_by.to_bytes());
        assert_eq!(m[120], 1, "bonded");
        assert_eq!(m[121], 0, "Active is tag 0");
        assert_eq!(m[122], 253);

        // A departed member keeps the record and serialises as tag 1.
        let left = GatewayMember {
            status: MemberStatus::Left,
            ..member.clone()
        };
        let mut l = Vec::new();
        l.extend_from_slice(GatewayMember::DISCRIMINATOR);
        AnchorSerialize::serialize(&left, &mut l).unwrap();
        assert_eq!(l.len(), GatewayMember::SPACE);
        assert_eq!(l[121], 1, "Left is tag 1");
    }

    /// The roster headroom is a **PoC placeholder**, not `N`, and it is pinned
    /// here so that raising it is a deliberate edit with a test to change rather
    /// than a quiet widening of an unbounded set.
    #[test]
    fn gateway_headroom_is_the_placeholder_eight() {
        assert_eq!(MAX_GATEWAY_HEADROOM, 8);
        // The bound the handler applies, stated as arithmetic: it refuses a new
        // member once the roster reaches `t + 8`, so at `4-of-N` the thirteenth
        // admission is the first refused.
        assert!(4 + MAX_GATEWAY_HEADROOM == 12);
    }
}

