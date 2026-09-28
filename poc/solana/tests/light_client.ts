import * as anchor from "@anchor-lang/core";
// `Solbeam` is the generated IDL type (see target/types/solbeam.ts). The
// tests treat the program namespace structurally rather than through
// Anchor's Idl generic, which needs the real generated IDL to instantiate.
import { Solbeam } from "../target/types/solbeam";
import { expect } from "chai";
import { createHash } from "crypto";
import * as fs from "fs";
import * as path from "path";

// The Phase 1A fixture, consumed UNMODIFIED. If this needs changing, the layout
// is the thing that is wrong and the fix belongs in Phase 1A — not here.
const FIXTURE = path.resolve(__dirname, "../../fixtures/deposit_1.json");

function doubleSha256(buf: Buffer): Buffer {
  return createHash("sha256")
    .update(createHash("sha256").update(buf).digest())
    .digest();
}

/** The fixture stores display-order hex; the program compares internal order. */
function displayToInternal(hex: string): Buffer {
  return Buffer.from(hex, "hex").reverse();
}

describe("solbeam — BSV light client", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Solbeam as unknown as Solbeam;

  const fixture = JSON.parse(fs.readFileSync(FIXTURE, "utf8"));
  const raws: Buffer[] = fixture.headers.map((h: any) => Buffer.from(h.raw, "hex"));

  const [lightClient] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("light_client")],
    program.programId,
  );

  before(async () => {
    // Anchor's localnet wallet is normally funded. Top up defensively, and do
    // not fail the suite if the airdrop is rate-limited.
    try {
      const sig = await provider.connection.requestAirdrop(
        provider.wallet.publicKey,
        2 * anchor.web3.LAMPORTS_PER_SOL,
      );
      await provider.connection.confirmTransaction(sig);
    } catch {
      /* already funded, or the validator refuses airdrops — fine */
    }
  });

  it("agrees with the fixture's checkpoint", async () => {
    const cp = raws[0];
    await program.methods
      // The whole 80-byte header, not its fields: nothing can be dropped or
      // mis-ordered this way, which is exactly how the version field got lost.
      .initialize(new anchor.BN(fixture.checkpoint.height), Array.from(cp))
      .accounts({ lightClient, payer: provider.wallet.publicKey })
      .rpc();

    const lc = await program.account.lightClient.fetch(lightClient);
    expect(lc.tipHeight.toNumber()).to.equal(fixture.checkpoint.height);

    // tip_hash is stored in INTERNAL byte order — the same order the Python
    // reference uses and the order the Merkle fold consumes. So compare like
    // for like: reversing here compares the internal value against the display
    // form, which are byte-reverses of each other and will never match.
    const internal = doubleSha256(cp).toString("hex");
    expect(Buffer.from(lc.tipHash).toString("hex")).to.equal(internal);

    // And the display form must be what the fixture recorded, which ties the
    // on-chain value back to Phase 1A rather than only to this file.
    expect(Buffer.from(internal, "hex").reverse().toString("hex"))
      .to.equal(fixture.checkpoint.hash);
  });

  it("accepts every remaining header and reaches the fixture's tip", async () => {
    for (const raw of raws.slice(1)) {
      await program.methods
        .pushHeader(Array.from(raw))
        .accounts({ lightClient, advancer: provider.wallet.publicKey })
        .rpc();
    }

    const lc = await program.account.lightClient.fetch(lightClient);
    const last = raws[raws.length - 1];
    const lastHeight = fixture.headers[fixture.headers.length - 1].height;

    expect(lc.tipHeight.toNumber()).to.equal(lastHeight);
    expect(Buffer.from(lc.tipHash).toString("hex"))
      .to.equal(doubleSha256(last).toString("hex"));
    expect(lc.headers.length).to.equal(raws.length - 1);
    expect(lc.paused).to.equal(false);
  });

  it("rejects a header that does not link to the tip", async () => {
    // Re-submitting the current tip: valid proof of work, valid header, but its
    // parent is the block before it, not the tip. Linkage is checked first.
    const again = raws[raws.length - 1];
    try {
      await program.methods
        .pushHeader(Array.from(again))
        .accounts({ lightClient, advancer: provider.wallet.publicKey })
        .rpc();
      expect.fail("should have rejected a broken linkage");
    } catch (e: any) {
      expect(String(e)).to.contain("BrokenLinkage");
    }
  });

  it("rejects a header whose proof of work misses its target", async () => {
    // Linkage passes AND the target is the chain's own, so proof of work is the
    // sole failure. This used to declare a mainnet target and rely on the
    // resulting hash missing it; that is now caught earlier as
    // UnexpectedRetarget, because the target is validated before it is used.
    // So grind a nonce that genuinely misses the real target: its first
    // big-endian byte is 0x7f, so a digest whose last byte (which becomes the
    // first when reversed) is >= 0x80 sits above it.
    const lc = await program.account.lightClient.fetch(lightClient);
    const bad = Buffer.alloc(80);
    bad.writeUInt32LE(1, 0);
    Buffer.from(lc.tipHash).copy(bad, 4);
    Buffer.alloc(32, 0xab).copy(bad, 36);
    bad.writeUInt32LE(0, 68);
    bad.writeUInt32LE(0x207fffff, 72);

    let nonce = 0;
    for (;;) {
      bad.writeUInt32LE(nonce, 76);
      if (doubleSha256(bad)[31] >= 0x80) break; // above the target
      nonce++;
    }

    try {
      await program.methods
        .pushHeader(Array.from(bad))
        .accounts({ lightClient, advancer: provider.wallet.publicKey })
        .rpc();
      expect.fail("should have rejected unmet proof of work");
    } catch (e: any) {
      expect(String(e)).to.contain("BadPow");
    }
  });
});

describe("solbeam — verify a deposit against the window", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Solbeam as unknown as Solbeam;

  const fixture = JSON.parse(fs.readFileSync(FIXTURE, "utf8"));
  const raws: Buffer[] = fixture.headers.map((h: any) => Buffer.from(h.raw, "hex"));

  const [lightClient] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("light_client")], program.programId);
  const [usedDeposits] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("used_deposits")], program.programId);
  const [depositScript] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("deposit_script")], program.programId);
  const [mint] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("mint")], program.programId);

  // The Solana address named in the deposit's OP_RETURN. Its key is what the
  // program checks against the payload, and its ATA is where tokens land.
  const recipientOwner = new anchor.web3.PublicKey(
    Buffer.from(fixture.proof.recipient, "hex"));

  // Derived by hand rather than with a helper. anchor.web3 is a limited
  // re-export in Anchor 1.x — PublicKey is there, getAssociatedTokenAddressSync
  // is not — and deriving it is two lines with no extra dependency.
  const TOKEN_PROGRAM = new anchor.web3.PublicKey(
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
  const ASSOCIATED_TOKEN_PROGRAM = new anchor.web3.PublicKey(
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
  const [recipientAta] = anchor.web3.PublicKey.findProgramAddressSync(
    [recipientOwner.toBuffer(), TOKEN_PROGRAM.toBuffer(), mint.toBuffer()],
    ASSOCIATED_TOKEN_PROGRAM);

  /** The fixture stores the txid in display order; the program compares internal. */
  const proof = () => ({
    height: new anchor.BN(fixture.proof.height),
    // display -> internal, because the program hashes the raw bytes itself
    txid: Array.from(displayToInternal(fixture.proof.txid)),
    vout: fixture.proof.vout,
    amount: new anchor.BN(fixture.proof.amount),
    recipient: Array.from(Buffer.from(fixture.proof.recipient, "hex")),
    index: fixture.proof.index,
    // branch elements are already internal order in the fixture
    branch: fixture.proof.branch.map((h: string) => Array.from(Buffer.from(h, "hex"))),
    // The window stores block hashes only. The claim supplies the raw 80-byte
    // header of the block at `height`; the program hashes it and checks that
    // against the stored hash, then reads the Merkle root out of it. That check
    // is what makes storing the root separately redundant.
    header: Array.from(Buffer.from(
      fixture.headers.find((h: any) => h.height === fixture.proof.height)!.raw, "hex")),
    // Vec<u8>, so a Buffer rather than an Array — see above.
    tx: Buffer.from(fixture.deposit_tx_raw, "hex"),
  });

  before(async () => {
    // Idempotent on purpose. Both describe blocks share one validator, and the
    // first block has already created these PDAs — so re-initialising would
    // fail with "account already in use", which says nothing useful about the
    // code under test. Create only what is missing.
    if (!(await provider.connection.getAccountInfo(lightClient))) {
      const cp = raws[0];
      await program.methods
        .initialize(new anchor.BN(fixture.checkpoint.height), Array.from(cp))
        .accounts({ lightClient, payer: provider.wallet.publicKey })
        .rpc();
      for (const raw of raws.slice(1)) {
        await program.methods.pushHeader(Array.from(raw))
          .accounts({ lightClient, advancer: provider.wallet.publicKey }).rpc();
      }
    }
    if (!(await provider.connection.getAccountInfo(usedDeposits))) {
      await program.methods
        // Vec<u8> must be a Buffer, not an Array: borsh encodes it as
        // `bytes` and calls .copy() on it.
        .initializeBridge(Buffer.from(fixture.deposit_script, "hex"))
        .accounts({ usedDeposits, depositScript, payer: provider.wallet.publicKey })
        .rpc();
    }
    if (!(await provider.connection.getAccountInfo(mint))) {
      await program.methods
        .initializeToken()
        .accounts({ mint, lightClient, payer: provider.wallet.publicKey })
        .rpc();
    }
  });

  it("accepts the fixture's deposit and records it as minted", async () => {
    await program.methods
      .verifyDeposit(proof())
      .accounts({
        lightClient, usedDeposits, depositScript, mint,
        recipientTokenAccount: recipientAta, recipientOwner,
        submitter: provider.wallet.publicKey,
      })
      .rpc();

    // Assert on STATE rather than scraping logs. Scraping is fragile — the
    // transaction is not always retrievable immediately as confirmed, and an
    // empty log list then looks like a failed event rather than a slow RPC.
    // The account is the stronger claim anyway: it proves the deposit was
    // accepted AND that it can never be accepted again.
    const used = await program.account.usedDeposits.fetch(usedDeposits);
    expect(used.keys.length).to.equal(1);
    expect(Buffer.from(used.keys[0].txid).toString("hex"))
      .to.equal(displayToInternal(fixture.proof.txid).toString("hex"));
    expect(used.keys[0].vout).to.equal(fixture.proof.vout);
    // The height is stored so entries whose block leaves the window can be
    // pruned. Without it the list could never be trimmed, and would cap lifetime
    // usage rather than window usage -- the bug this field exists to fix.
    expect(used.keys[0].height.toNumber()).to.equal(fixture.proof.height);

    // And the tokens exist. Eight decimals, one base unit per satoshi, so the
    // minted amount must equal the deposit exactly — no scaling anywhere.
    // Read the amount straight out of the token account rather than through a
    // helper. An SPL token account is mint(32) owner(32) amount(8), so the
    // balance is a u64 at offset 64 — no library needed and no API to be
    // missing.
    const ataInfo = await provider.connection.getAccountInfo(recipientAta);
    expect(ataInfo, "the recipient's token account should have been created")
      .to.not.equal(null);
    expect(Number(ataInfo!.data.readBigUInt64LE(64))).to.equal(fixture.proof.amount);
  });

  it("refuses the same deposit twice", async () => {
    try {
      await program.methods
        .verifyDeposit(proof())
        .accounts({
        lightClient, usedDeposits, depositScript, mint,
        recipientTokenAccount: recipientAta, recipientOwner,
        submitter: provider.wallet.publicKey,
      })
        .rpc();
      expect.fail("should have refused a replay");
    } catch (e: any) {
      expect(String(e)).to.contain("AlreadyMinted");
    }
  });

  it("refuses a tampered Merkle branch", async () => {
    const bad = proof();
    bad.branch[0] = Array.from(Buffer.alloc(32, 0x11));
    try {
      await program.methods
        .verifyDeposit(bad)
        .accounts({
        lightClient, usedDeposits, depositScript, mint,
        recipientTokenAccount: recipientAta, recipientOwner,
        submitter: provider.wallet.publicKey,
      })
        .rpc();
      expect.fail("should have refused a bad branch");
    } catch (e: any) {
      expect(String(e)).to.contain("BadMerkleProof");
    }
  });

  it("refuses a claim whose header is not the canonical block", async () => {
    // The window stores hashes only, so the claim supplies the header and the
    // program checks it against the stored hash. This is the check that makes
    // storing the Merkle root unnecessary — without it, a claimant could supply
    // a header of its own choosing and prove anything it liked.
    const bad = proof();
    const h = Buffer.from(bad.header);
    h[36] ^= 0xff; // perturb the Merkle root inside the header
    bad.header = Array.from(h);
    try {
      await program.methods
        .verifyDeposit(bad)
        .accounts({
        lightClient, usedDeposits, depositScript, mint,
        recipientTokenAccount: recipientAta, recipientOwner,
        submitter: provider.wallet.publicKey,
      })
        .rpc();
      expect.fail("should have refused a substituted header");
    } catch (e: any) {
      expect(String(e)).to.contain("HeaderMismatch");
    }
  });

  it("refuses an inflated amount", async () => {
    const bad = proof();
    bad.amount = new anchor.BN(fixture.proof.amount * 100);
    try {
      await program.methods
        .verifyDeposit(bad)
        .accounts({
        lightClient, usedDeposits, depositScript, mint,
        recipientTokenAccount: recipientAta, recipientOwner,
        submitter: provider.wallet.publicKey,
      })
        .rpc();
      expect.fail("should have refused an inflated amount");
    } catch (e: any) {
      expect(String(e)).to.contain("AmountMismatch");
    }
  });

  it("refuses a recipient that is not in the transaction", async () => {
    const bad = proof();
    bad.recipient = Array.from(Buffer.alloc(32, 0x22));
    try {
      await program.methods
        .verifyDeposit(bad)
        .accounts({
        lightClient, usedDeposits, depositScript, mint,
        recipientTokenAccount: recipientAta, recipientOwner,
        submitter: provider.wallet.publicKey,
      })
        .rpc();
      expect.fail("should have refused a missing payload");
    } catch (e: any) {
      expect(String(e)).to.contain("MissingPayload");
    }
  });
});

/**
 * The advancer is permissionless and untrusted. This is where that claim is
 * tested rather than asserted: the worst it should be able to do is waste its
 * own fees, never forge, reorder, or rewind its way to a false proof.
 */
describe("solbeam — a hostile advancer", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Solbeam as unknown as Solbeam;

  const fixture = JSON.parse(fs.readFileSync(FIXTURE, "utf8"));
  const raws: Buffer[] = fixture.headers.map((h: any) => Buffer.from(h.raw, "hex"));

  const [lightClient] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("light_client")], program.programId);

  const tipHash = async (): Promise<Buffer> => {
    const lc = await program.account.lightClient.fetch(lightClient);
    return Buffer.from(lc.tipHash);
  };

  /** Attempt a push, and report the error code the program chose. */
  const attempt = async (header: Buffer): Promise<string> => {
    try {
      await program.methods.pushHeader(Array.from(header))
        .accounts({ lightClient, advancer: provider.wallet.publicKey }).rpc();
      return "ACCEPTED";
    } catch (e: any) {
      const m = String(e).match(/Error Code: (\w+)/);
      return m ? m[1] : String(e).slice(0, 60);
    }
  };

  before(async () => {
    if (!(await provider.connection.getAccountInfo(lightClient))) {
      const cp = raws[0];
      await program.methods
        .initialize(new anchor.BN(fixture.checkpoint.height), Array.from(cp))
        .accounts({ lightClient, payer: provider.wallet.publicKey })
        .rpc();
      for (const raw of raws.slice(1)) {
        await program.methods.pushHeader(Array.from(raw))
          .accounts({ lightClient, advancer: provider.wallet.publicKey }).rpc();
      }
    }
  });

  it("cannot push a fabricated header", async () => {
    // Random 80 bytes with a plausible target. Its parent is nothing, so it
    // cannot pass the linkage check however good its proof of work looks —
    // which is the point: a valid hash alone proves nothing.
    const fabricated = Buffer.alloc(80, 0x5a);
    fabricated.writeUInt32LE(0x207fffff, 72);
    expect(await attempt(fabricated)).to.equal("BrokenLinkage");
  });

  it("cannot replay or reorder an accepted header", async () => {
    // A real header, real proof of work, already in the window. Its parent is
    // the block before it, not the tip.
    expect(await attempt(raws[5])).to.equal("BrokenLinkage");
    expect(await attempt(raws[raws.length - 1])).to.equal("BrokenLinkage");
  });

  it("cannot rewind the tip", async () => {
    const before = await tipHash();
    await attempt(raws[1]);
    expect((await tipHash()).toString("hex")).to.equal(before.toString("hex"));
  });

  it("rejects a header that declares its own target", async () => {
    // The vulnerability this closes: the difficulty target used to be read from
    // the submitted header's own `bits` field, so an attacker declared an easy
    // target, ground a single hash, and the proof-of-work check passed. Twelve
    // such headers and a fabricated deposit would mint. The target now comes
    // from the chain, established at `initialize`.
    const forged = Buffer.alloc(80, 0x33);
    forged.writeUInt32LE(0x20000000, 0);              // version
    doubleSha256(raws[raws.length - 1]).copy(forged, 4); // links to the real tip
    forged.writeUInt32LE(0x2100ffff, 72);             // easier than regtest's target
    expect(await attempt(forged)).to.equal("UnexpectedRetarget");
  });

  it("cannot smuggle a fork through push_header", async () => {
    // A header built on an *older* block. push_header refuses it because it does
    // not extend the tip, and that refusal is load-bearing: it is what stops an
    // advancer reordering, replaying, rewinding or substituting a branch.
    //
    // Following a legitimate reorg is a *separate*, explicit path (init_staging /
    // push_fork_header / commit_fork), where the branch is validated header by
    // header and only replaces the tip if it is strictly heavier. The two must
    // stay separate: the cheap permissionless path must never be able to rewind
    // the chain, or the hostile-advancer guarantee above is gone. See TEST_PLAN
    // 4.5 and 4.6.
    const forked = Buffer.alloc(80, 0x33);
    forked.writeUInt32LE(0x20000000, 0);          // version
    raws[3].copy(forked, 4, 4, 36);               // parent = an OLD header
    forked.writeUInt32LE(0x207fffff, 72);
    expect(await attempt(forked)).to.equal("BrokenLinkage");
  });
});

describe("solbeam — following a reorg", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Solbeam as unknown as Solbeam;

  const fixture = JSON.parse(fs.readFileSync(FIXTURE, "utf8"));
  const raws: Buffer[] = fixture.headers.map((h: any) => Buffer.from(h.raw, "hex"));

  const [lightClient] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("light_client")], program.programId);

  before(async () => {
    if (!(await provider.connection.getAccountInfo(lightClient))) {
      const cp = raws[0];
      await program.methods
        .initialize(new anchor.BN(fixture.checkpoint.height), Array.from(cp))
        .accounts({ lightClient, payer: provider.wallet.publicKey }).rpc();
      for (const raw of raws.slice(1)) {
        await program.methods.pushHeader(Array.from(raw))
          .accounts({ lightClient, advancer: provider.wallet.publicKey }).rpc();
      }
    }
  });

  // A branch is staged one header at a time because a Solana transaction caps at
  // 1232 bytes, and this branch is 72 headers. See TEST_PLAN 4.6.
  it("follows a strictly heavier competing branch", async () => {
    const fork = fixture.fork;
    const branch: Buffer[] = fork.headers.map((h: any) => Buffer.from(h.raw, "hex"));

    const before = await program.account.lightClient.fetch(lightClient);
    expect(before.tipHeight.toNumber()).to.equal(fixture.tip_height);

    const [staging] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("staging"), provider.wallet.publicKey.toBuffer()], program.programId);

    // fork.from_height is the first *branch* block; initStaging wants the last
    // *shared* block, which is one below it.
    await program.methods
      .initStaging(new anchor.BN(fork.from_height - 1))
      .accounts({ lightClient, staging, submitter: provider.wallet.publicKey,
                  systemProgram: anchor.web3.SystemProgram.programId }).rpc();

    // Twelve headers is what a 1232-byte transaction carries, so the branch goes
    // up in six transactions rather than seventy-two.
    for (let i = 0; i < branch.length; i += 12) {
      await program.methods.pushForkHeader(Buffer.concat(branch.slice(i, i + 12)))
        .accounts({ lightClient, staging, submitter: provider.wallet.publicKey }).rpc();
    }

    await program.methods.commitFork()
      .accounts({ lightClient, staging, submitter: provider.wallet.publicKey }).rpc();

    const after = await program.account.lightClient.fetch(lightClient);
    const last = branch[branch.length - 1];

    expect(after.tipHeight.toNumber()).to.equal(fork.tip_height);
    expect(Buffer.from(after.tipHash).toString("hex"))
      .to.equal(doubleSha256(last).toString("hex"));
    // The window is bounded, so the deepest headers fall out of it. 192 is the
    // 32-hour window, and it is fixed by what cw-144 needs to store per header
    // (hash + chainwork + time = 52 bytes) against the 10,240-byte account cap,
    // not by a duration anyone picked — see TEST_PLAN 4.7 and §0.
    expect(after.headers.length).to.be.at.most(192);
    // Committing closes the staging account and hands the rent back, so a
    // successful reorg does not strand a deposit.
    expect(await provider.connection.getAccountInfo(staging)).to.be.null;
  });

  it("refuses a branch that is not heavier", async () => {
    // The previous test already moved the tip onto this branch, so replaying it
    // lands on the same height. A tie must keep the incumbent rather than churn
    // the tip, so the commit has to be refused.
    const fork = fixture.fork;
    const [staging] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("staging"), provider.wallet.publicKey.toBuffer()], program.programId);

    // fork.from_height is the first *branch* block; initStaging wants the last
    // *shared* block, which is one below it.
    await program.methods
      .initStaging(new anchor.BN(fork.from_height - 1))
      .accounts({ lightClient, staging, submitter: provider.wallet.publicKey,
                  systemProgram: anchor.web3.SystemProgram.programId }).rpc();
    const headers = fork.headers.map((h: any) => Buffer.from(h.raw, "hex"));
    for (let i = 0; i < headers.length; i += 12) {
      await program.methods.pushForkHeader(Buffer.concat(headers.slice(i, i + 12)))
        .accounts({ lightClient, staging, submitter: provider.wallet.publicKey }).rpc();
    }

    try {
      await program.methods.commitFork()
        .accounts({ lightClient, staging, submitter: provider.wallet.publicKey }).rpc();
      expect.fail("should have refused a branch that is not heavier");
    } catch (e: any) {
      const m = String(e).match(/Error Code: (\w+)/);
      expect(m ? m[1] : String(e)).to.equal("ForkNotHeavier");
    }

    // A refused commit reverts, so its `close` constraint never runs. Without
    // abandon_staging this account and its rent would be stranded permanently —
    // which is the entire reason that instruction exists.
    expect(await provider.connection.getAccountInfo(staging)).to.not.be.null;
    await program.methods.abandonStaging()
      .accounts({ staging, submitter: provider.wallet.publicKey }).rpc();
    expect(await provider.connection.getAccountInfo(staging)).to.be.null;
  });
});

/**
 * The A7 case the old test did NOT cover.
 *
 * The replay key used to be `(txid, vout, height)`. Re-submitting the same proof
 * at the same height therefore fails under either key and proves nothing about
 * the bug — which is what "refuses the same deposit twice" above does. The bug
 * only appears when the SAME transaction is included at a DIFFERENT height,
 * which is what a reorg produces when it re-mines a block.
 *
 * The re-inclusion is built for real rather than approximated: block 117's
 * Merkle root is put into a new header at 118 that links to 117. A block that
 * carries 117's root contains exactly 117's transactions, so the deposit really
 * is in the canonical block at 118 — which is the consensus-relevant part of a
 * reorg, and a valid header by every check the client makes (linkage, the
 * chain's own target, real proof of work against the regtest target).
 *
 * This runs last and re-anchors the checkpoint, because it needs block 117 to be
 * the TIP: a re-inclusion that moves the deposit *forward* is the direction a
 * reorg produces, and the earlier blocks have long since left a 192-record
 * window by the time the reorg suite has finished.
 */
describe("solbeam — replay across a re-inclusion (A7)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Solbeam as unknown as Solbeam;

  const fixture = JSON.parse(fs.readFileSync(FIXTURE, "utf8"));

  const [lightClient] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("light_client")], program.programId);
  const [usedDeposits] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("used_deposits")], program.programId);
  const [depositScript] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("deposit_script")], program.programId);
  const [mint] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("mint")], program.programId);
  const recipientOwner = new anchor.web3.PublicKey(
    Buffer.from(fixture.proof.recipient, "hex"));
  const TOKEN_PROGRAM = new anchor.web3.PublicKey(
    "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
  const ASSOCIATED_TOKEN_PROGRAM = new anchor.web3.PublicKey(
    "ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
  const [recipientAta] = anchor.web3.PublicKey.findProgramAddressSync(
    [recipientOwner.toBuffer(), TOKEN_PROGRAM.toBuffer(), mint.toBuffer()],
    ASSOCIATED_TOKEN_PROGRAM);

  const rawAt = (height: number): Buffer =>
    Buffer.from(fixture.headers.find((h: any) => h.height === height)!.raw, "hex");

  /** The fixture's deposit proof, optionally re-anchored to another height. */
  const proof = (height: number, header: Buffer) => ({
    height: new anchor.BN(height),
    txid: Array.from(displayToInternal(fixture.proof.txid)),
    vout: fixture.proof.vout,
    amount: new anchor.BN(fixture.proof.amount),
    recipient: Array.from(Buffer.from(fixture.proof.recipient, "hex")),
    index: fixture.proof.index,
    branch: fixture.proof.branch.map((h: string) => Array.from(Buffer.from(h, "hex"))),
    header: Array.from(header),
    tx: Buffer.from(fixture.deposit_tx_raw, "hex"),
  });

  const accounts = () => ({
    lightClient, usedDeposits, depositScript, mint,
    recipientTokenAccount: recipientAta, recipientOwner,
    submitter: provider.wallet.publicKey,
  });

  it("refuses the same (txid, vout) re-included at a different height", async () => {
    const raw116 = rawAt(116);
    const raw117 = rawAt(117);
    const raw118 = rawAt(118);

    // Reach a state where 117 is the tip, by moving the checkpoint to 116 and
    // extending. `setCheckpoint` resets the window, so this is the honest way to
    // get a small freshly-anchored chain rather than by rewinding anything.
    await program.methods
      .setCheckpoint(new anchor.BN(116), Array.from(doubleSha256(raw116)))
      .accounts({ lightClient, authority: provider.wallet.publicKey })
      .rpc();
    await program.methods.pushHeader(Array.from(raw117))
      .accounts({ lightClient, advancer: provider.wallet.publicKey }).rpc();

    const lc = await program.account.lightClient.fetch(lightClient);
    expect(lc.tipHeight.toNumber()).to.equal(117);
    // One record plus the 116 tip is two, well inside the window, so nothing
    // here is exercising window eviction.
    expect(lc.headers.length).to.equal(1);

    // The re-inclusion: 117's transactions, in a block at 118.
    const reIncluded = Buffer.alloc(80);
    reIncluded.writeUInt32LE(0x20000000, 0);              // version
    doubleSha256(raw117).copy(reIncluded, 4);             // links to 117
    raw117.copy(reIncluded, 36, 36, 68);                  // SAME merkle root
    reIncluded.writeUInt32LE(1780000000, 68);             // time
    reIncluded.writeUInt32LE(0x207fffff, 72);             // regtest target
    // Regtest's target, big-endian: mantissa 0x7fffff, exponent 0x20, so the
    // three mantissa bytes land at [3..6]. The program compares the digest
    // reversed against exactly this.
    const target = Buffer.alloc(32);
    target[3] = 0x7f; target[4] = 0xff; target[5] = 0xff;
    let nonce = 0;
    for (;;) {
      expect(nonce).to.be.below(10_000_000); // ~2 tries expected at regtest's target
      reIncluded.writeUInt32LE(nonce, 76);
      if (Buffer.from(doubleSha256(reIncluded)).reverse().compare(target) <= 0) break;
      nonce++;
    }
    expect(doubleSha256(reIncluded).toString("hex"))
      .to.not.equal(doubleSha256(raw118).toString("hex"));

    await program.methods.pushHeader(Array.from(reIncluded))
      .accounts({ lightClient, advancer: provider.wallet.publicKey }).rpc();
    const afterPush = await program.account.lightClient.fetch(lightClient);
    expect(afterPush.tipHeight.toNumber()).to.equal(118);
    expect(Buffer.from(afterPush.tipHash).toString("hex"))
      .to.equal(doubleSha256(reIncluded).toString("hex"));

    // The probe: the same transaction, the same output, a different height, and
    // the claimed header is now canonical at that height.
    try {
      await program.methods.verifyDeposit(proof(118, reIncluded)).accounts(accounts()).rpc();
      expect.fail("should have refused a re-inclusion at a different height");
    } catch (e: any) {
      // AlreadyMinted specifically — not HeaderMismatch or BadMerkleProof, which
      // would mean the proof never reached the replay check and the test would be
      // passing for the wrong reason.
      expect(String(e)).to.contain("AlreadyMinted");
    }

    // The control: at the height the deposit was ACTUALLY minted at, the same
    // proof is refused for the same reason. If the two error differently, the
    // key is height-sensitive somewhere.
    try {
      await program.methods.verifyDeposit(proof(117, raw117)).accounts(accounts()).rpc();
      expect.fail("should have refused the original replay too");
    } catch (e: any) {
      expect(String(e)).to.contain("AlreadyMinted");
    }

    // Still exactly one entry, still at its original height. A height-keyed
    // implementation reaches this point with two.
    const used = await program.account.usedDeposits.fetch(usedDeposits);
    expect(used.keys.length).to.equal(1);
    expect(used.keys[0].height.toNumber()).to.equal(fixture.proof.height);
  });
});

/**
 * P2 — the fork re-anchoring forgery vector.
 *
 * `commit_fork` used to splice `headers[..=fork_idx] ++ staging.hashes` while
 * looking the fork point up from *current* chain state, and `push_fork_header`
 * linked a branch's first header the same way. Two branches could therefore be
 * staged against the same height, commit one after the other, and produce a
 * window holding the prefix of one chain and the branch of another with no
 * linkage between them — the client's only invariant. A header that is in no
 * chain could then be made canonical, which is a mint forgery, not a nuisance.
 *
 * The fix records the parent hash when the branch is staged and re-checks it at
 * commit. These two tests pin both halves: that the check fires when the parent
 * moves, and that it does not fire when it has not.
 */
describe("solbeam — a stale staged fork (P2)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Solbeam as unknown as Solbeam;

  const fixture = JSON.parse(fs.readFileSync(FIXTURE, "utf8"));
  const rawAt = (height: number): Buffer =>
    Buffer.from(fixture.headers.find((h: any) => h.height === height)!.raw, "hex");

  const [lightClient] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("light_client")], program.programId);

  /**
   * A funded submitter. The staging account is ~10 KB, so `init` needs rent —
   * an unfunded keypair fails with a system-program error that reads like a
   * logic failure and is not one. Each test gets its own keypair because the
   * staging PDA is seeded on the submitter: two branches by one submitter would
   * be the same account.
   */
  const submitter = async (): Promise<{ key: anchor.web3.Keypair; addr: anchor.web3.PublicKey }> => {
    const key = anchor.web3.Keypair.generate();
    const sig = await provider.connection.requestAirdrop(
      key.publicKey, 2 * anchor.web3.LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sig);
    const [addr] = anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("staging"), key.publicKey.toBuffer()], program.programId);
    return { key, addr };
  };

  /** Re-anchor the trusted checkpoint, which resets the window. */
  const reanchor = async (height: number) => {
    await program.methods
      .setCheckpoint(new anchor.BN(height), Array.from(doubleSha256(rawAt(height))))
      .accounts({ lightClient, authority: provider.wallet.publicKey })
      .rpc();
  };

  /**
   * A block that links to `prev` but is otherwise the fixture's block at
   * `height`. Only `prev` is rewritten, so the header is well-formed, carries
   * the fixture's real proof of work against the regtest target, and its hash is
   * a new one.
   */
  const extends_ = (height: number, prev: Buffer): Buffer => {
    const raw = rawAt(height);
    doubleSha256(prev).copy(raw, 4);
    return raw;
  };

  const stage = async (
    who: { key: anchor.web3.Keypair; addr: anchor.web3.PublicKey },
    forkHeight: number,
    branch: Buffer[],
  ) => {
    await program.methods.initStaging(new anchor.BN(forkHeight))
      .accounts({ lightClient, staging: who.addr, submitter: who.key.publicKey,
                  systemProgram: anchor.web3.SystemProgram.programId })
      .signers([who.key]).rpc();
    for (let i = 0; i < branch.length; i += 12) {
      await program.methods.pushForkHeader(Buffer.concat(branch.slice(i, i + 12)))
        .accounts({ lightClient, staging: who.addr, submitter: who.key.publicKey })
        .signers([who.key]).rpc();
    }
  };

  it("rejects a commit whose fork point has moved (the stale branch)", async () => {
    await reanchor(120);
    expect((await program.account.lightClient.fetch(lightClient)).tipHeight.toNumber())
      .to.equal(120);

    const base120 = rawAt(120);
    const b121 = extends_(121, base120);
    const b122 = extends_(122, b121);
    const b123 = extends_(123, b122);
    expect(doubleSha256(b121).toString("hex")).to.not.equal(doubleSha256(rawAt(121)).toString("hex"));

    // Branch A: a single header off 120, staged while the fork point is 120.
    const a = await submitter();
    await stage(a, 120, [b121]);

    // Now move the fork point: a DIFFERENT, heavier branch off the same height
    // commits, so the chain no longer holds A's parent at 120.
    const b = await submitter();
    await stage(b, 120, [b121, b122, b123]);
    await program.methods.commitFork()
      .accounts({ lightClient, staging: b.addr, submitter: b.key.publicKey })
      .signers([b.key]).rpc();

    const moved = await program.account.lightClient.fetch(lightClient);
    expect(moved.tipHeight.toNumber()).to.equal(123);
    expect(Buffer.from(moved.tipHash).toString("hex"))
      .to.equal(doubleSha256(b123).toString("hex"));

    try {
      await program.methods.commitFork()
        .accounts({ lightClient, staging: a.addr, submitter: a.key.publicKey })
        .signers([a.key]).rpc();
      expect.fail("should have refused a branch whose fork point moved");
    } catch (e: any) {
      const m = String(e).match(/Error Code: (\w+)/);
      // ForkPointMoved specifically. ForkNotHeavier would mean the re-anchor
      // check never ran, which is the bug this test exists for.
      expect(m ? m[1] : String(e)).to.equal("ForkPointMoved");
    }

    // A refused commit reverts, so its `close` constraint never runs. Without
    // abandon_staging the rent would be stranded — which is that instruction's
    // entire purpose.
    expect(await provider.connection.getAccountInfo(a.addr)).to.not.be.null;
    await program.methods.abandonStaging()
      .accounts({ staging: a.addr, submitter: a.key.publicKey })
      .signers([a.key]).rpc();
    expect(await provider.connection.getAccountInfo(a.addr)).to.be.null;
  });

  it("still commits when the fork point has NOT moved (the control)", async () => {
    // The same shape without the interleaving. If the re-anchor check were too
    // strict — comparing against the wrong block, or re-reading the parent after
    // the window had already been rebuilt — this fails while the test above
    // still passes.
    await reanchor(124);
    const b125 = extends_(125, rawAt(124));

    const c = await submitter();
    await stage(c, 124, [b125]);
    await program.methods.commitFork()
      .accounts({ lightClient, staging: c.addr, submitter: c.key.publicKey })
      .signers([c.key]).rpc();

    const after = await program.account.lightClient.fetch(lightClient);
    expect(after.tipHeight.toNumber()).to.equal(125);
    expect(Buffer.from(after.tipHash).toString("hex"))
      .to.equal(doubleSha256(b125).toString("hex"));
    expect(await provider.connection.getAccountInfo(c.addr)).to.be.null;
  });
});
