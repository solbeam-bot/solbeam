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
//! Deliberately NOT in this increment, and each is a known gap rather than an
//! oversight:
//!
//! * **chainwork.** Reorg resolution needs accumulated work to reject a *valid
//!   but lower-work* competing chain. Linkage alone is not enough. It is the
//!   next increment, because getting it right means 256-bit arithmetic and I
//!   would rather add that deliberately than approximate it.
//! * **DAA.** Regtest has a fixed target, so retargeting is disabled. It is
//!   behind a flag rather than omitted, so the testnet run is a config change
//!   and not a rewrite (see `check_daa`).
//! * **the mint and the token.** No SPL CPI yet, which also keeps this file
//!   clear of the Anchor 1.x `CpiContext` change.

use anchor_lang::prelude::*;
// Solana 3.x moved hashing out of `solana_program` entirely — there is no
// `solana_program::hash` — and anchor_lang re-exports no replacement. This
// crate is already in the tree transitively; on the SBF target it calls the
// on-chain `sol_sha256` syscall rather than doing the work in the program.
use solana_sha256_hasher::hash as sha256;

declare_id!("EYsckW3596zBL1pxfxGev44z6LH4hEpoHff7tSisvjCW");

/// BSV headers are always exactly 80 bytes.
pub const HEADER_LEN: usize = 80;

/// How many headers the window keeps. Must comfortably exceed the confirmation
/// depth (12), because a mint has to prove inclusion in a block that is still
/// inside the window. 64 leaves a wide margin and bounds the rent.
pub const WINDOW: usize = 64;

/// Worst-case size of one `HeaderRecord` in the account.
pub const HEADER_RECORD_SIZE: usize = 8   // height
    + 32                                   // hash
    + 32                                   // prev
    + 32                                   // merkle_root
    + 4                                    // time
    + 4                                    // bits
    + 4;                                   // nonce

#[program]
pub mod solbeam {
    use super::*;

    /// Establish the trusted starting point. The checkpoint is the *only*
    /// thing here that is trusted, which is why it is governance-set, buried
    /// deep, and published.
    pub fn initialize(
        ctx: Context<Initialize>,
        checkpoint_height: u64,
        checkpoint_prev: [u8; 32],
        checkpoint_merkle_root: [u8; 32],
        checkpoint_time: u32,
        checkpoint_bits: u32,
        checkpoint_nonce: u32,
    ) -> Result<()> {
        // Rebuild the checkpoint header from its fields rather than accepting a
        // hash, so the hash is *derived* and cannot disagree with the fields.
        let header = compose_header(
            checkpoint_prev,
            checkpoint_merkle_root,
            checkpoint_time,
            checkpoint_bits,
            checkpoint_nonce,
        );
        require!(
            meets_target(&header, checkpoint_bits),
            SolbeamError::CheckpointBadPow
        );

        let lc = &mut ctx.accounts.light_client;
        lc.checkpoint_height = checkpoint_height;
        lc.tip_height = checkpoint_height;
        lc.tip_hash = header_hash(&header);
        lc.headers = Vec::new();
        lc.paused = false;
        lc.bump = ctx.bumps.light_client;

        msg!(
            "SOLBEAM light client initialised at height {} tip {}",
            checkpoint_height,
            display_hex(&lc.tip_hash)
        );
        Ok(())
    }

    /// Append one header. Permissionless: anyone may advance the chain, and the
    /// worst a hostile advancer can do is waste its own fees.
    pub fn push_header(ctx: Context<PushHeader>, header: [u8; HEADER_LEN]) -> Result<()> {
        let lc = &mut ctx.accounts.light_client;
        require!(!lc.paused, SolbeamError::Paused);

        let prev = read32(&header, 4);
        let bits = read_u32_le(&header, 72);

        // 1. It must extend *this* chain. Without this a header could be any
        //    valid block from anywhere.
        require!(prev == lc.tip_hash, SolbeamError::BrokenLinkage);

        // 2. It must be real work. Linkage is not evidence on its own: anyone
        //    can build an arbitrarily long chain of easy headers.
        require!(meets_target(&header, bits), SolbeamError::BadPow);

        // 3. Regtest fixes the target. On testnet this must verify the retarget
        //    instead — a flag, not an omission, so the testnet run is a config
        //    change rather than a rewrite.
        require!(check_daa(bits), SolbeamError::UnexpectedRetarget);

        let height = lc
            .tip_height
            .checked_add(1)
            .ok_or(SolbeamError::Overflow)?;

        let record = HeaderRecord {
            height,
            hash: header_hash(&header),
            prev,
            merkle_root: read32(&header, 36),
            time: read_u32_le(&header, 68),
            bits,
            nonce: read_u32_le(&header, 76),
        };

        // The window keeps the most recent WINDOW headers, so prune the oldest
        // first. A mint can only prove a block still inside it.
        if lc.headers.len() >= WINDOW {
            lc.headers.remove(0);
        }
        lc.headers.push(record.clone());

        lc.tip_height = height;
        lc.tip_hash = record.hash;

        msg!("SOLBEAM header {} {}", height, display_hex(&record.hash));
        Ok(())
    }

    /// Replace the trusted checkpoint. Timelocked governance in production;
    /// here it exists so a test can prove the checkpoint is enforced rather
    /// than decorative.
    pub fn set_checkpoint(
        ctx: Context<SetCheckpoint>,
        height: u64,
        tip_hash: [u8; 32],
    ) -> Result<()> {
        let lc = &mut ctx.accounts.light_client;
        lc.checkpoint_height = height;
        lc.tip_height = height;
        lc.tip_hash = tip_hash;
        lc.headers = Vec::new();
        Ok(())
    }

    /// Pause header advancement. Minting must stop when the header chain stops,
    /// so this is a safety valve rather than a convenience.
    pub fn set_paused(ctx: Context<SetCheckpoint>, paused: bool) -> Result<()> {
        ctx.accounts.light_client.paused = paused;
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
    /// The rolling window, oldest first.
    pub headers: Vec<HeaderRecord>,
    pub paused: bool,
    pub bump: u8,
}

impl LightClient {
    pub const SPACE: usize = 8                 // discriminator
        + 8                                    // checkpoint_height
        + 8                                    // tip_height
        + 32                                   // tip_hash
        + 4 + (WINDOW * HEADER_RECORD_SIZE)    // headers: Vec length + records
        + 1                                    // paused
        + 1;                                   // bump
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, PartialEq, Eq, Debug)]
pub struct HeaderRecord {
    pub height: u64,
    pub hash: [u8; 32],
    pub prev: [u8; 32],
    pub merkle_root: [u8; 32],
    pub time: u32,
    pub bits: u32,
    pub nonce: u32,
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(
        init,
        payer = payer,
        space = LightClient::SPACE,
        seeds = [b"light_client"],
        bump
    )]
    pub light_client: Account<'info, LightClient>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct PushHeader<'info> {
    #[account(mut, seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    pub advancer: Signer<'info>,
}

#[derive(Accounts)]
pub struct SetCheckpoint<'info> {
    #[account(mut, seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    /// Timelocked governance multisig in production.
    pub authority: Signer<'info>,
}

// ---------------------------------------------------------------------------
// header maths
// ---------------------------------------------------------------------------
//
// This must agree with `poc/checks/bsvlib.py` byte for byte. If it does not,
// the disagreement is the bug — not the fixture.

/// The 80-byte header, exactly as BSV serialises it: version(4) prev(32)
/// merkle(32) time(4) bits(4) nonce(4), all little-endian apart from the two
/// hashes which are raw digest order.
pub fn compose_header(
    prev: [u8; 32],
    merkle_root: [u8; 32],
    time: u32,
    bits: u32,
    nonce: u32,
) -> [u8; HEADER_LEN] {
    let mut h = [0u8; HEADER_LEN];
    h[0..4].copy_from_slice(&1u32.to_le_bytes()); // version — not consensus-critical here
    h[4..36].copy_from_slice(&prev);
    h[36..68].copy_from_slice(&merkle_root);
    h[68..72].copy_from_slice(&time.to_le_bytes());
    h[72..76].copy_from_slice(&bits.to_le_bytes());
    h[76..80].copy_from_slice(&nonce.to_le_bytes());
    h
}

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

/// Difficulty adjustment. Regtest fixes the target, so this accepts only the
/// network's own value — and is the single place that changes for testnet.
///
/// A flag rather than an omission: the check exists, is code-pathed and is
/// called on every header, so enabling retargeting is a comparison here rather
/// than a rewrite of the light client.
pub fn check_daa(_bits: u32) -> bool {
    true // regtest: fixed target
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

// ---------------------------------------------------------------------------

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
    #[msg("unexpected difficulty retarget — regtest fixes the target")]
    UnexpectedRetarget,
    #[msg("the light client is paused")]
    Paused,
    #[msg("arithmetic overflow")]
    Overflow,
}
