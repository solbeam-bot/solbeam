import * as anchor from "@anchor-lang/core";
import { Program } from "@anchor-lang/core";
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
  const program = anchor.workspace.Solbeam as Program<Solbeam>;

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
    // Constructed so linkage PASSES and the proof of work is the sole failure:
    // parent set to the current tip, everything else arbitrary, and a target
    // from mainnet difficulty, which no regtest nonce will meet.
    const lc = await program.account.lightClient.fetch(lightClient);
    const bad = Buffer.alloc(80);
    bad.writeUInt32LE(1, 0);
    Buffer.from(lc.tipHash).copy(bad, 4);
    Buffer.alloc(32, 0xab).copy(bad, 36);
    bad.writeUInt32LE(0, 68);
    bad.writeUInt32LE(0x1d00ffff, 72);
    bad.writeUInt32LE(0, 76);

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
  const program = anchor.workspace.Solbeam as Program<Solbeam>;

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
  const program = anchor.workspace.Solbeam as Program<Solbeam>;

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

  it("cannot follow a fork, which is the gap this test exists to expose", async () => {
    // A header built on an *older* block: exactly what a legitimate reorg
    // presents. It is rejected, correctly, because it does not extend the tip —
    // but the consequence is that the client cannot FOLLOW a reorg either. It
    // stalls and stays on the abandoned branch.
    //
    // That is the honest state: safe against a hostile advancer, but blind to a
    // real reorg. Recorded in TEST_PLAN.md as a known gap with a plan.
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
  const program = anchor.workspace.Solbeam as Program<Solbeam>;

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

    for (const raw of branch) {
      await program.methods.pushForkHeader(Array.from(raw))
        .accounts({ lightClient, staging, submitter: provider.wallet.publicKey }).rpc();
    }

    await program.methods.commitFork()
      .accounts({ lightClient, staging, submitter: provider.wallet.publicKey }).rpc();

    const after = await program.account.lightClient.fetch(lightClient);
    const last = branch[branch.length - 1];

    expect(after.tipHeight.toNumber()).to.equal(fork.tip_height);
    expect(Buffer.from(after.tipHash).toString("hex"))
      .to.equal(doubleSha256(last).toString("hex"));
    // The window is bounded, so the deepest headers fall out of it. 288 is the
    // 48-hour window, sized by time rather than picked — see TEST_PLAN 4.7.
    expect(after.headers.length).to.be.at.most(288);
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
    for (const h of fork.headers) {
      await program.methods.pushForkHeader(Array.from(Buffer.from(h.raw, "hex")))
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
  });
});
