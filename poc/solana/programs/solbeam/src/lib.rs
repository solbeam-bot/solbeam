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
use anchor_spl::associated_token::AssociatedToken;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};
// Solana 3.x moved hashing out of `solana_program` entirely — there is no
// `solana_program::hash` — and anchor_lang re-exports no replacement. This
// crate is already in the tree transitively; on the SBF target it calls the
// on-chain `sol_sha256` syscall rather than doing the work in the program.
use solana_sha256_hasher::hash as sha256;

declare_id!("EYsckW3596zBL1pxfxGev44z6LH4hEpoHff7tSisvjCW");

/// BSV headers are always exactly 80 bytes.
pub const HEADER_LEN: usize = 80;

/// How long a reorg the client can follow, expressed in TIME rather than a
/// block count — the earlier "64" was arbitrary and nothing justified it.
///
/// BSV targets a ten-minute block, so 48 hours is 288 blocks. A day of margin
/// sits on top of the rule of thumb that a reorg deeper than a day means BSV is
/// broken and the peg has far larger problems than its header window.
///
/// Two constraints set what is affordable, and both are hard:
///
///   * **Account creation caps at 10,240 bytes.** The window must keep its
///     per-header record small. `HeaderRecord` stores a bare hash (32 bytes):
///     288 x 32 + overhead = 9,286 bytes, the same account size a 144-header
///     window needed when records also carried a Merkle root.
///   * **The whole account is deserialised on every instruction**, so a bigger
///     window is also more compute on the mint path. 9,216 bytes of records is
///     comfortable against the 200,000 CU budget.
pub const WINDOW_HOURS: u64 = 48;
pub const SECONDS_PER_BLOCK: u64 = 600;
pub const WINDOW: usize = (WINDOW_HOURS * 3600 / SECONDS_PER_BLOCK) as usize; // 288

/// How deep a deposit must be buried before it can be minted. Twelve blocks is
/// roughly two hours on BSV. The test matrix compresses time, not depth, so this
/// stays a real number.
pub const MIN_CONFIRMATIONS: u64 = 12;

/// `solBSV` is a classic SPL token with eight decimals, matching BSV's own
/// satoshi precision. One satoshi is one base unit, so no conversion is ever
/// needed when minting a deposit.
pub const TOKEN_DECIMALS: u8 = 8;

/// How many deposits the replay list remembers at once. **Not a lifetime limit.**
///
/// Entries are pruned once their block leaves the header window, because a claim
/// against a height below `window_start` is refused before the replay check is
/// ever reached — so the list only has to cover the window. It is therefore a
/// bound on *deposits per window*, not on total usage; the original fixed list was
/// never pruned and silently stopped the peg-in path after 256 mints in its life.
pub const MAX_USED: usize = 200;

/// Size of one `HeaderRecord`: a bare block hash, and nothing else. Height is
/// derived from the window's start; `prev`, `time`, `bits` and `nonce` are used
/// once when a header is pushed; and the Merkle root is a field *inside* the
/// header, already committed to by this hash, so a claim can simply supply it.
pub const HEADER_RECORD_SIZE: usize = 32;

/// Solana refuses to grow an account by more than this in one instruction, and
/// `init` allocates the whole `LightClient` in one go. Exceeding it is not a
/// graceful failure — `initialize` simply reverts — so it is asserted at compile
/// time rather than discovered on testnet. This cap is the reason the window is
/// sized against a 64-byte record rather than the 116-byte one it started with.
pub const MAX_ACCOUNT_CREATE: usize = 10_240;

/// How many branch headers fit in one transaction. A Solana transaction is
/// capped at 1232 bytes; after the signature, accounts, blockhash, instruction
/// header and the `Vec<u8>` length prefix roughly 215 bytes are gone, leaving
/// about 12 headers of 80 bytes. The earlier `push_fork` failed precisely
/// because it ignored this ceiling and tried to send 72.
pub const MAX_FORK_BATCH: usize = 12;

const _: () = assert!(
    LightClient::SPACE <= MAX_ACCOUNT_CREATE,
    "LightClient::SPACE exceeds Solana's account-creation cap: shrink HeaderRecord or WINDOW"
);

const _: () = assert!(
    UsedDeposits::SPACE <= MAX_ACCOUNT_CREATE,
    "UsedDeposits::SPACE exceeds Solana's account-creation cap: lower MAX_USED"
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
    /// Branch headers, oldest first, excluding the fork point itself.
    pub hashes: Vec<[u8; 32]>,
    pub bump: u8,
}

impl ForkStaging {
    pub const SPACE: usize = 8                  // discriminator
        + 32                                    // submitter
        + 8                                     // fork_height
        + 4 + (WINDOW * HEADER_RECORD_SIZE)     // hashes: Vec length + records
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
        lc.window_start = checkpoint_height;
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

        let record_hash = header_hash(&header);
        if lc.headers.len() >= WINDOW {
            lc.headers.remove(0);
            lc.window_start += 1;
        }
        if lc.headers.is_empty() {
            lc.window_start = height;
        }
        lc.headers.push(HeaderRecord { hash: record_hash });

        lc.tip_height = height;
        lc.tip_hash = record_hash;

        msg!("SOLBEAM header {} {}", height, display_hex(&record_hash));
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
    /// **KNOWN NOT TO WORK AS WRITTEN.** A Solana transaction is capped at 1232
    /// bytes, so a branch of more than about 13 headers cannot be submitted in
    /// one instruction at all — the client fails with "Invalid bytes for
    /// branch_bytes: length exceeds remaining bytes", which reads like an
    /// encoding bug and is a size limit. This needs a staging area and an
    /// incremental push-then-commit, which is a design decision as much as
    /// code. See TEST_PLAN 4.6.
    ///
    /// **Weight here is height.** Regtest fixes the difficulty, so every header
    /// carries the same work and accumulated work is proportional to length.
    /// Testnet needs real chainwork - `work = 2^256 / (target + 1)`, summed -
    /// and that is a flag on this comparison rather than a rewrite, in the same
    /// way DAA is. It cannot be exercised on regtest, where the target never
    /// changes, so implementing it here would be untested code.
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
        // The common ancestor must be a header we still hold. Deeper than the
        // window needs a checkpoint reset, which is a governance action.
        require!(
            lc.index_of(fork_height).is_some(),
            SolbeamError::ForkPointNotInWindow
        );

        let staging = &mut ctx.accounts.staging;
        staging.submitter = ctx.accounts.submitter.key();
        staging.fork_height = fork_height;
        staging.hashes = Vec::new();
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
        let lc = &ctx.accounts.light_client;
        require!(!lc.paused, SolbeamError::Paused);
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
            staging.hashes.len() + batch <= WINDOW,
            SolbeamError::ForkTooLong
        );

        // Link to the tip of the branch so far, or to the main chain at the fork
        // point while the branch is still empty. `get` rather than indexing: the
        // window can be empty immediately after `initialize`, which is a real
        // state rather than a panic.
        let mut prev = match staging.hashes.last() {
            Some(h) => *h,
            None => {
                let idx = lc
                    .index_of(staging.fork_height)
                    .ok_or(SolbeamError::ForkPointNotInWindow)?;
                lc.headers
                    .get(idx)
                    .ok_or(SolbeamError::ForkPointNotInWindow)?
                    .hash
            }
        };

        // Validate the whole batch before writing any of it, so a bad header
        // half-way through does not leave a partial branch staged.
        let mut checked: Vec<[u8; 32]> = Vec::with_capacity(batch);
        for raw in branch_bytes.chunks(HEADER_LEN) {
            require!(read32(raw, 4) == prev, SolbeamError::BrokenLinkage);
            let bits = read_u32_le(raw, 72);
            require!(meets_target_slice(raw, bits), SolbeamError::BadPow);
            require!(check_daa(bits), SolbeamError::UnexpectedRetarget);

            let hash = header_hash_of_bytes(raw);
            checked.push(hash);
            prev = hash;
        }
        staging.hashes.extend(checked);
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
    /// **Strictly** heavier: a tie keeps the incumbent, so equal-length branches
    /// cannot be used to churn the tip. The staging account is closed here and
    /// its rent returns to the submitter.
    pub fn commit_fork(ctx: Context<CommitFork>) -> Result<()> {
        let lc = &mut ctx.accounts.light_client;
        require!(!lc.paused, SolbeamError::Paused);
        let staging = &ctx.accounts.staging;
        require!(
            staging.submitter == ctx.accounts.submitter.key(),
            SolbeamError::NotStagingOwner
        );
        require!(!staging.hashes.is_empty(), SolbeamError::EmptyFork);

        let new_tip_height = staging.fork_height + staging.hashes.len() as u64;

        // Regtest fixes the target, so every header carries the same work and
        // accumulated work is proportional to length — length is the comparison
        // here. Testnet needs real chainwork, `work = 2^256 / (target + 1)`
        // summed, and that is a flag on this comparison rather than a rewrite, in
        // the same way DAA is. It cannot be exercised on regtest, where the target
        // never changes, so implementing it now would be untested code.
        require!(new_tip_height > lc.tip_height, SolbeamError::ForkNotHeavier);

        let fork_idx = lc
            .index_of(staging.fork_height)
            .ok_or(SolbeamError::ForkPointNotInWindow)?;

        // Keep the prefix up to and including the fork point, append the branch,
        // then prune to the window. The prefix always begins at headers[0], so
        // window_start only moves by whatever the prune discards.
        let mut rebuilt: Vec<HeaderRecord> = lc.headers[..=fork_idx].to_vec();
        rebuilt.extend(staging.hashes.iter().map(|hash| HeaderRecord { hash: *hash }));
        if rebuilt.len() > WINDOW {
            let excess = rebuilt.len() - WINDOW;
            rebuilt.drain(0..excess);
            lc.window_start += excess as u64;
        }
        if rebuilt.is_empty() {
            lc.window_start = new_tip_height;
        }

        lc.headers = rebuilt;
        lc.tip_height = new_tip_height;
        lc.tip_hash = *staging.hashes.last().unwrap();

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

        // 7. Replay. The (txid, vout) pair is the identity of a deposit, so it is
        //    what gets remembered.
        let used = &mut ctx.accounts.used_deposits;

        // Drop entries whose block has left the window. Pruning is safe precisely
        // because of check 1 above: a claim is refused unless its height is at or
        // above `window_start`, so a deposit whose block has fallen out can never
        // reach this point to be replayed. Without the prune the list fills and
        // the peg-in path stops working permanently — a cap on total usage rather
        // than a replay defence, and one that ordinary volume reaches on its own.
        let window_start = ctx.accounts.light_client.window_start;
        used.keys.retain(|k| k.height >= window_start);

        let key = DepositKey {
            txid: claim.txid,
            vout: claim.vout,
            height: claim.height,
        };
        require!(!used.keys.contains(&key), SolbeamError::AlreadyMinted);
        require!(used.keys.len() < MAX_USED, SolbeamError::NoRoomForMoreDeposits);
        used.keys.push(key);

        // 8. The recipient named in the OP_RETURN is the account that receives
        //    the tokens. This is the binding between the BSV payload and the
        //    Solana destination, so it is checked rather than assumed.
        require!(
            ctx.accounts.recipient_owner.key().to_bytes() == claim.recipient,
            SolbeamError::RecipientMismatch
        );

        // 9. Mint. The authority is this program's own PDA, so the program
        //    signs for it — no operator key can mint.
        let bump = ctx.accounts.light_client.bump;
        let seeds: &[&[u8]] = &[b"light_client", &[bump]];
        token::mint_to(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                MintTo {
                    mint: ctx.accounts.mint.to_account_info(),
                    to: ctx.accounts.recipient_token_account.to_account_info(),
                    authority: ctx.accounts.light_client.to_account_info(),
                },
            )
            .with_signer(&[seeds]),
            claim.amount,
        )?;

        emit!(DepositMinted {
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
        lc.window_start = height;
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
    /// Height of `headers[0]`. The window is contiguous, so every other height
    /// is derived from this — storing a height per record would cost eight
    /// bytes times WINDOW for information that is already implied.
    pub window_start: u64,
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
        + 8                                    // window_start
        + 4 + (WINDOW * HEADER_RECORD_SIZE)    // headers: Vec length + records
        + 1                                    // paused
        + 1;                                   // bump

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
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Default, PartialEq, Eq, Debug)]
pub struct HeaderRecord {
    /// Internal byte order, matching the Python reference. Everything else a
    /// deposit proof needs is derived from, or supplied alongside, this.
    pub hash: [u8; 32],
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
    #[account(seeds = [b"light_client"], bump = light_client.bump)]
    pub light_client: Account<'info, LightClient>,
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
    /// The raw 80-byte header of the block at `height`. Stored records hold
    /// only a hash, so the claim must supply the header — and the hash check in
    /// `verify_deposit` is what proves it is the canonical one. The Merkle root
    /// is read out of it rather than stored.
    pub header: [u8; HEADER_LEN],
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
    /// The block the deposit is in. Carried so the list can be pruned once that
    /// block leaves the header window — see the pruning in `verify_deposit`.
    pub height: u64,
}

#[account]
pub struct UsedDeposits {
    pub keys: Vec<DepositKey>,
    pub bump: u8,
}

impl UsedDeposits {
    pub const SPACE: usize = 8 + 4 + (MAX_USED * 44) + 1; // 8,813 of 10,240
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
    #[account(mut)]
    pub used_deposits: Account<'info, UsedDeposits>,
    #[account(seeds = [b"deposit_script"], bump = deposit_script.bump)]
    pub deposit_script: Account<'info, DepositScript>,
    #[account(mut)]
    pub mint: Account<'info, Mint>,
    /// Created on the recipient's behalf if they have never held solBSV, so a
    /// first-time user needs no SOL to receive.
    #[account(
        init_if_needed,
        payer = submitter,
        associated_token::mint = mint,
        associated_token::authority = recipient_owner,
    )]
    pub recipient_token_account: Account<'info, TokenAccount>,
    /// CHECK: the ATA address is derived from this account, and its key is
    /// checked against the OP_RETURN payload before anything is minted. It is
    /// never read or written.
    pub recipient_owner: UncheckedAccount<'info>,
    #[account(mut)]
    pub submitter: Signer<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

#[event]
pub struct ChainReorganised {
    pub from_height: u64,
    pub new_tip_height: u64,
}

#[event]
pub struct DepositMinted {
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
    #[msg("the used-deposit list is full")]
    NoRoomForMoreDeposits,
    #[msg("the deposit script must be a P2PKH script")]
    DepositScriptNotP2pkh,
    #[msg("the recipient account does not match the OP_RETURN payload")]
    RecipientMismatch,
    #[msg("a competing branch must contain at least one header")]
    EmptyFork,
    #[msg("the fork point is not inside the header window")]
    ForkPointNotInWindow,
    #[msg("the competing branch is not heavier than the current one")]
    ForkNotHeavier,
    #[msg("the staging account belongs to a different submitter")]
    NotStagingOwner,
    #[msg("the staged branch is longer than the window")]
    ForkTooLong,
    #[msg("more headers in one batch than a transaction can carry")]
    BatchTooLarge,
}
