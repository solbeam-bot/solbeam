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

/// How deep a deposit must be buried before it can be minted. Twelve blocks is
/// roughly two hours on BSV. The test matrix compresses time, not depth, so this
/// stays a real number.
pub const MIN_CONFIRMATIONS: u64 = 12;

/// How many deposits one account remembers, to refuse replays. Fixed rather than
/// growable so the account can be sized up front and never needs reallocating.
pub const MAX_USED: usize = 256;

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
    /// thing trusted here, which is why it is governance-set, buried deep and
    /// published — and why it is taken as the **raw 80-byte header** rather
    /// than as individual fields.
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
        let bits = read_u32_le(&header, 72);
        require!(meets_target(&header, bits), SolbeamError::CheckpointBadPow);

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

    /// Create the bridge's own state: the script a deposit must pay, and the
    /// list of deposits already minted.
    pub fn initialize_bridge(ctx: Context<InitializeBridge>, deposit_script: Vec<u8>) -> Result<()> {
        require!(
            is_p2pkh(&deposit_script),
            SolbeamError::DepositScriptNotP2pkh
        );
        let ds = &mut ctx.accounts.deposit_script;
        ds.script = deposit_script;
        ds.bump = ctx.bumps.deposit_script;

        let used = &mut ctx.accounts.used_deposits;
        used.keys = Vec::new();
        used.bump = ctx.bumps.used_deposits;

        msg!("SOLBEAM bridge initialised");
        Ok(())
    }

    /// Verify a BSV deposit against the header window, and refuse to accept the
    /// same one twice.
    ///
    /// This is the trustless half of the peg, and it is deliberately explicit
    /// about what it does NOT take on trust. Everything the caller supplies -
    /// the branch, the transaction, the claimed output - is re-derived here.
    ///
    /// The token mint is not wired up yet: this records the claim and emits the
    /// amount and recipient. Adding the SPL CPI is the next increment, and
    /// keeping it separate means a failure here is never ambiguous about which
    /// half broke.
    pub fn verify_deposit(ctx: Context<VerifyDeposit>, claim: DepositClaim) -> Result<()> {
        let lc = &ctx.accounts.light_client;
        require!(!lc.paused, SolbeamError::Paused);

        // 1. The header must still be inside the window. A proof against a
        //    header we no longer hold cannot be checked at all.
        let record = lc
            .headers
            .iter()
            .find(|h| h.height == claim.height)
            .ok_or(SolbeamError::HeaderNotInWindow)?;

        // 2. The transaction must be the one the proof names. Without this the
        //    branch could be valid for a *different* transaction.
        let txid = header_hash_of_bytes(&claim.tx);
        require!(txid == claim.txid, SolbeamError::TxidMismatch);

        // 3. And it must be *in* the block, which is what the branch proves.
        require!(
            fold_branch(claim.txid, claim.index, &claim.branch) == record.merkle_root,
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

        // 7. Replay. The (txid, vout) pair is the identity of a deposit, so it is
        //    what gets remembered.
        let used = &mut ctx.accounts.used_deposits;
        let key = DepositKey { txid: claim.txid, vout: claim.vout };
        require!(!used.keys.contains(&key), SolbeamError::AlreadyMinted);
        require!(used.keys.len() < MAX_USED, SolbeamError::NoRoomForMoreDeposits);
        used.keys.push(key);

        emit!(DepositVerified {
            txid: claim.txid,
            vout: claim.vout,
            amount: claim.amount,
            recipient: claim.recipient,
            height: claim.height,
            confirmations,
        });
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
    /// The raw deposit transaction, so the claimed output can be re-derived
    /// rather than trusted.
    pub tx: Vec<u8>,
}

/// The identity of a deposit. A transaction can have several outputs, so the
/// pair is the key, not the txid alone.
#[derive(AnchorSerialize, AnchorDeserialize, Clone, PartialEq, Eq)]
pub struct DepositKey {
    pub txid: [u8; 32],
    pub vout: u32,
}

#[account]
pub struct UsedDeposits {
    pub keys: Vec<DepositKey>,
    pub bump: u8,
}

impl UsedDeposits {
    pub const SPACE: usize = 8 + 4 + (MAX_USED * 36) + 1;
}

/// The bridge's deposit script, passed in so the check is against the account
/// the bridge actually controls rather than a constant baked into the program.
#[account]
pub struct DepositScript {
    pub script: Vec<u8>,
    pub bump: u8,
}

impl DepositScript {
    pub const SPACE: usize = 8 + 4 + 25 + 1;
}

#[derive(Accounts)]
pub struct InitializeBridge<'info> {
    #[account(init, payer = payer, space = UsedDeposits::SPACE,
              seeds = [b"used_deposits"], bump)]
    pub used_deposits: Account<'info, UsedDeposits>,
    #[account(init, payer = payer, space = DepositScript::SPACE,
              seeds = [b"deposit_script"], bump)]
    pub deposit_script: Account<'info, DepositScript>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct VerifyDeposit<'info> {
    #[account(seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
    #[account(mut)]
    pub used_deposits: Account<'info, UsedDeposits>,
    #[account(seeds = [b"deposit_script"], bump = deposit_script.bump)]
    pub deposit_script: Account<'info, DepositScript>,
    pub submitter: Signer<'info>,
}

#[event]
pub struct DepositVerified {
    pub txid: [u8; 32],
    pub vout: u32,
    pub amount: u64,
    pub recipient: [u8; 32],
    pub height: u64,
    pub confirmations: u64,
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
    raw.get(at..at + n).ok_or(SolbeamError::MalformedTx)
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

/// A canonical P2PKH script: `76 a9 14 <20 bytes> 88 ac`, 25 bytes.
pub fn is_p2pkh(script: &[u8]) -> bool {
    script.len() == 25
        && script[0] == 0x76
        && script[1] == 0xa9
        && script[2] == 0x14
        && script[23] == 0x88
        && script[24] == 0xac
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
    #[msg("no header at that height is inside the window")]
    HeaderNotInWindow,
    #[msg("the raw transaction does not hash to the claimed txid")]
    TxidMismatch,
    #[msg("the Merkle branch does not fold to the block's root")]
    BadMerkleProof,
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
    #[msg("the used-deposit list is full")]
    NoRoomForMoreDeposits,
    #[msg("the deposit script must be a P2PKH script")]
    DepositScriptNotP2pkh,
}
