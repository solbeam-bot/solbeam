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

// 471 contiguous real BSV mainnet headers, heights 968,230–968,700, each with
// its serialised 80 bytes. The `raw` column was added for the on-chain
// instruction-path test: `push_header` hashes bytes and checks linkage by hash,
// so a summary of `hash`/`bits`/`time` cannot be pushed. See
// `workstreams/data/fetch_raw_headers.py`, which rebuilds each header and
// verifies it against the fixture's recorded hash before writing.
const MAINNET_FIXTURE = path.resolve(
  __dirname, "../../../workstreams/data/headers_mainnet.json");

function doubleSha256(buf: Buffer): Buffer {
  return createHash("sha256")
    .update(createHash("sha256").update(buf).digest())
    .digest();
}

/** The fixture stores display-order hex; the program compares internal order. */
function displayToInternal(hex: string): Buffer {
  return Buffer.from(hex, "hex").reverse();
}

// -- regtest proof-of-work helpers -------------------------------------------
//
// The fixture is a synthetic regtest chain: every header declares bits
// 0x207fffff, so every header a test fabricates has to be mined against that
// target. Getting the target's BYTE POSITION wrong fails silently and
// expensively. The program's `bits_to_target_be` places the mantissa at
// `32 - exponent`, so for exponent 0x20 the three mantissa bytes land at
// [0..3] and the target is ~2^255 — a coin flip per nonce. An earlier version
// of this file put them at [3..6], a target of ~2^231 that a random digest
// meets about once in 2^25 tries; the A7 loop ground through 10M nonces and
// found nothing, which is how that mistake surfaced.

/** Regtest's compact target: the `bits` every fixture header carries. */
const REGTEST_BITS = 0x207fffff;

/** `bits_to_target_be(0x207fffff)` — mantissa 0x7fffff at bytes [0..2]. */
const REGTEST_TARGET = (() => {
  const target = Buffer.alloc(32);
  target[0] = 0x7f;
  target[1] = 0xff;
  target[2] = 0xff;
  return target;
})();

/** The digest in the order `meets_target` compares it: reversed. */
function hashBigEndian(header: Buffer): Buffer {
  return Buffer.from(doubleSha256(header)).reverse();
}

/**
 * Grind `header`'s nonce until it meets regtest's target. Mutates in place and
 * returns the header. Throws rather than returning an unmined header: a caller
 * that ignored the failure would just push another invalid block.
 */
function mineRegtest(header: Buffer, limit = 10_000_000): Buffer {
  for (let nonce = 0; nonce < limit; nonce++) {
    header.writeUInt32LE(nonce, 76);
    if (hashBigEndian(header).compare(REGTEST_TARGET) <= 0) return header;
  }
  throw new Error(`no regtest solution in ${limit} nonces`);
}

/**
 * A block in the shape of `template` that links to `prev` and meets the regtest
 * target, but is a genuinely different block from `template`.
 *
 * Rewriting `prev` alone is not enough. The fixture is a contiguous chain, so
 * for a header at height H the fixture's own `prev` already IS hash(H-1), and
 * `forkFrom(rawAt(H), rawAt(H-1))` is a byte-for-byte no-op: the "new" branch
 * would carry the canonical block's own hash and a test comparing the two would
 * be comparing a value with itself. The timestamp bump makes the block
 * distinct; re-mining the nonce keeps it well formed, because linkage, the
 * chain's target and proof of work all still have to pass.
 */
function forkFrom(template: Buffer, prev: Buffer, salt = 0): Buffer {
  const raw = Buffer.from(template);
  doubleSha256(prev).copy(raw, 4);
  raw.writeUInt32LE(raw.readUInt32LE(68) + 1 + salt, 68);
  raw.writeUInt32LE(REGTEST_BITS, 72);
  return mineRegtest(raw);
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

  // The BPF upgradeable loader's `ProgramData` account for this program. Its
  // `upgrade_authority_address` is the ONLY key allowed to call `initialize`
  // and `initialize_bridge`; Anchor's localnet deploy sets it to the provider
  // wallet. Derived rather than hardcoded, so a re-keyed deploy still tests.
  const BPF_LOADER_UPGRADEABLE = new anchor.web3.PublicKey(
    "BPFLoaderUpgradeab1e11111111111111111111111");
  const [programData] = anchor.web3.PublicKey.findProgramAddressSync(
    [program.programId.toBuffer()], BPF_LOADER_UPGRADEABLE);

  // The bridge PDAs, needed by the initialiser test below before the second
  // describe block creates them.
  const [usedDeposits] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("used_deposits")], program.programId);
  const [depositScript] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("deposit_script")], program.programId);

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

  /** A funded key that is NOT this program's upgrade authority. */
  const stranger = async (): Promise<anchor.web3.Keypair> => {
    const key = anchor.web3.Keypair.generate();
    const sig = await provider.connection.requestAirdrop(
      key.publicKey, anchor.web3.LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sig);
    return key;
  };

  // The vulnerability this closes: `initialize` was unpermissioned, so the first
  // caller became `authority` — and with it, the key that can rewrite the
  // checkpoint and mint. This test must run BEFORE the authority's own
  // `initialize` below, so the PDA is genuinely unclaimed when it is attempted.
  it("refuses to initialize for a payer that is not the upgrade authority", async () => {
    const attacker = await stranger();

    try {
      await program.methods
        .initialize(new anchor.BN(fixture.checkpoint.height), Array.from(raws[0]))
        .accounts({ lightClient, programData, payer: attacker.publicKey })
        .signers([attacker])
        .rpc();
      expect.fail("a non-authority payer must not be able to initialize");
    } catch (e: any) {
      expect(String(e)).to.contain("Unauthorized");
    }

    // Solana rolls the failed instruction back, so the attacker took nothing:
    // the PDA is still unclaimed and the real authority can still initialise.
    expect(await provider.connection.getAccountInfo(lightClient)).to.equal(null);
  });

  it("refuses to initialize the bridge for a payer that is not the upgrade authority", async () => {
    const attacker = await stranger();

    try {
      await program.methods
        .initializeBridge(Buffer.from(fixture.deposit_script, "hex"))
        .accounts({ usedDeposits, depositScript, programData, payer: attacker.publicKey })
        .signers([attacker])
        .rpc();
      expect.fail("a non-authority payer must not be able to initialize the bridge");
    } catch (e: any) {
      expect(String(e)).to.contain("Unauthorized");
    }

    expect(await provider.connection.getAccountInfo(usedDeposits)).to.equal(null);
    expect(await provider.connection.getAccountInfo(depositScript)).to.equal(null);
  });

  it("agrees with the fixture's checkpoint", async () => {
    const cp = raws[0];
    await program.methods
      // The whole 80-byte header, not its fields: nothing can be dropped or
      // mis-ordered this way, which is exactly how the version field got lost.
      .initialize(new anchor.BN(fixture.checkpoint.height), Array.from(cp))
      .accounts({ lightClient, programData, payer: provider.wallet.publicKey })
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
  // The loader's ProgramData PDA — see the first describe block. The provider
  // wallet is the localnet upgrade authority, so it is the only key that can
  // call `initialize_bridge`.
  const BPF_LOADER_UPGRADEABLE = new anchor.web3.PublicKey(
    "BPFLoaderUpgradeab1e11111111111111111111111");
  const [programData] = anchor.web3.PublicKey.findProgramAddressSync(
    [program.programId.toBuffer()], BPF_LOADER_UPGRADEABLE);

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
        .accounts({ lightClient, programData, payer: provider.wallet.publicKey })
        .rpc();
      for (const raw of raws.slice(1)) {
        await program.methods.pushHeader(Array.from(raw))
          .accounts({ lightClient, advancer: provider.wallet.publicKey }).rpc();
      }
    }
    if (!(await provider.connection.getAccountInfo(mint))) {
      await program.methods
        .initializeToken()
        .accounts({ mint, lightClient, payer: provider.wallet.publicKey })
        .rpc();
    }
  });

  // The legitimate half of the initialiser gate. It is a test in its own right
  // rather than setup, and it must run before the deposit tests because they
  // need the PDAs it creates. Its opposite number — the stranger who is refused
  // — is the second test of the first describe block.
  it("initialises the bridge when the payer IS the upgrade authority", async () => {
    await program.methods
      // Vec<u8> must be a Buffer, not an Array: borsh encodes it as
      // `bytes` and calls .copy() on it.
      .initializeBridge(Buffer.from(fixture.deposit_script, "hex"))
      .accounts({ usedDeposits, depositScript, programData, payer: provider.wallet.publicKey })
      .rpc();

    const ds = await program.account.depositScript.fetch(depositScript);
    expect(Buffer.from(ds.script).toString("hex"))
      .to.equal(Buffer.from(fixture.deposit_script, "hex").toString("hex"));
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

  // The loader's ProgramData PDA is required by `initialize`; see the first
  // describe block.
  const BPF_LOADER_UPGRADEABLE = new anchor.web3.PublicKey(
    "BPFLoaderUpgradeab1e11111111111111111111111");
  const [programData] = anchor.web3.PublicKey.findProgramAddressSync(
    [program.programId.toBuffer()], BPF_LOADER_UPGRADEABLE);

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
        .accounts({ lightClient, programData, payer: provider.wallet.publicKey })
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

  // The loader's ProgramData PDA is required by `initialize`; see the first
  // describe block.
  const BPF_LOADER_UPGRADEABLE = new anchor.web3.PublicKey(
    "BPFLoaderUpgradeab1e11111111111111111111111");
  const [programData] = anchor.web3.PublicKey.findProgramAddressSync(
    [program.programId.toBuffer()], BPF_LOADER_UPGRADEABLE);

  before(async () => {
    if (!(await provider.connection.getAccountInfo(lightClient))) {
      const cp = raws[0];
      await program.methods
        .initialize(new anchor.BN(fixture.checkpoint.height), Array.from(cp))
        .accounts({ lightClient, programData, payer: provider.wallet.publicKey }).rpc();
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
 * This runs last and re-anchors the checkpoint, because it needs block 117 in
 * the window: a re-inclusion that moves the deposit *forward* is the direction a
 * reorg produces, and the earlier blocks have long since left a 192-record
 * window by the time the reorg suite has finished. The re-inclusion at 118 is
 * then buried twelve blocks deep, because the replay check is not reached at
 * all until MIN_CONFIRMATIONS is satisfied.
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

  /**
   * An 80-byte regtest header to use as a template at `height`. The main chain
   * only runs 113–128; the competing branch covers 119–190 and is a valid
   * regtest chain of the same shape, so it supplies the deeper heights. Only the
   * template's size and `bits` matter — `forkFrom` rewrites prev, time and nonce.
   */
  const templateAt = (height: number): Buffer => {
    const h = fixture.headers.find((x: any) => x.height === height)
      ?? fixture.fork.headers.find((x: any) => x.height === height);
    return Buffer.from(h.raw, "hex");
  };

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
    // It takes the raw 80-byte header (F2): the difficulty state — bits,
    // no_retargeting, pow_limit — is re-derived from it, and a bare hash cannot
    // carry `bits`.
    await program.methods
      .setCheckpoint(new anchor.BN(116), Array.from(raw116))
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
    reIncluded.writeUInt32LE(REGTEST_BITS, 72);           // regtest target
    mineRegtest(reIncluded);                              // ~2 tries expected
    expect(doubleSha256(reIncluded).toString("hex"))
      .to.not.equal(doubleSha256(raw118).toString("hex"));

    await program.methods.pushHeader(Array.from(reIncluded))
      .accounts({ lightClient, advancer: provider.wallet.publicKey }).rpc();

    // Bury it. `verify_deposit` refuses a claim with fewer than
    // MIN_CONFIRMATIONS (12) confirmations BEFORE it reaches the replay check,
    // so a re-inclusion left at the tip would be refused as
    // InsufficientConfirmations and this test would prove nothing about replay.
    // Eleven successors put block 118 twelve deep; they are the fixture's own
    // next blocks re-mined onto the re-inclusion, which is the shape of a
    // forward reorg.
    let parent: Buffer = reIncluded;
    for (let h = 119; h <= 129; h++) {
      parent = forkFrom(templateAt(h), parent);
      await program.methods.pushHeader(Array.from(parent))
        .accounts({ lightClient, advancer: provider.wallet.publicKey }).rpc();
    }

    const afterPush = await program.account.lightClient.fetch(lightClient);
    expect(afterPush.tipHeight.toNumber()).to.equal(129);
    expect(Buffer.from(afterPush.tipHash).toString("hex"))
      .to.equal(doubleSha256(parent).toString("hex"));

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
 * `commit_fork` used to splice `headers[..=fork_idx] ++ staging.records` while
 * looking the fork point up from *current* chain state, and `push_fork_header`
 * linked a branch's first header the same way. A branch staged on block X at
 * height H could therefore survive a competing commit forked BELOW H: that
 * commit replaced X with X', and the stale branch — whose first header links to
 * X — was spliced onto X' regardless. The window then held `headers[..=H]` from
 * one chain stapled to a branch from another at a broken link, which is the
 * client's only invariant. A header that is in no chain could then be made
 * canonical, which is a mint forgery, not a nuisance.
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

  /**
   * Re-anchor the trusted checkpoint, which resets the window.
   *
   * Note what the reset leaves behind: `set_checkpoint` empties `headers`, so
   * the checkpoint block itself is NOT a record. The first `push_header`
   * afterwards sets `window_start` to *that* block's height. So the shallowest
   * height a branch can name as a fork point is one above the checkpoint, and
   * any test that wants a fork point at all has to push a header first.
   */
  const reanchor = async (height: number) => {
    await program.methods
      // The raw header, not its hash: set_checkpoint re-derives the difficulty
      // state from the header itself (F2), and on a mainnet header that opens
      // the trusted seed instead of going live immediately.
      .setCheckpoint(new anchor.BN(height), Array.from(rawAt(height)))
      .accounts({ lightClient, authority: provider.wallet.publicKey })
      .rpc();
  };

  /** Extend the canonical chain with the fixture's own header at that height. */
  const push = async (raw: Buffer) => {
    await program.methods.pushHeader(Array.from(raw))
      .accounts({ lightClient, advancer: provider.wallet.publicKey }).rpc();
  };

  const commit = async (who: { key: anchor.web3.Keypair; addr: anchor.web3.PublicKey }) => {
    await program.methods.commitFork()
      .accounts({ lightClient, staging: who.addr, submitter: who.key.publicKey })
      .signers([who.key]).rpc();
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
    // Re-anchor one block below the fork point the stale branch will name, and
    // push two real headers, so that both the fork point (121) and the block
    // that will replace it are genuine records in the window.
    await reanchor(119);
    expect((await program.account.lightClient.fetch(lightClient)).tipHeight.toNumber())
      .to.equal(119);
    await push(rawAt(120));
    await push(rawAt(121));

    // Branch B's replacement blocks. `forkFrom` re-mines the fixture's own 121
    // and 122 onto the chain, so B carries the fixture's real proof of work but
    // is a different chain above 120 — and, importantly, the block it puts at
    // 121 is NOT the block the fixture has there.
    const b121 = forkFrom(rawAt(121), rawAt(120));
    const b122 = forkFrom(rawAt(122), b121);
    expect(doubleSha256(b121).toString("hex"))
      .to.not.equal(doubleSha256(rawAt(121)).toString("hex"));

    // Branch A: two headers off 121, staged while the chain holds the fixture's
    // block at 121. A records that block as its parent. It is deliberately
    // STRICTLY HEAVIER than anything the chain will hold — four blocks' work
    // from the re-anchor against B's three — so the re-anchor check is provably
    // the only thing that can refuse it. A tie or a lighter branch would be
    // refused as ForkNotHeavier too, and the test would not distinguish the fix
    // from its absence.
    const a1 = forkFrom(rawAt(122), rawAt(121));
    const a2 = forkFrom(rawAt(123), a1);
    const a = await submitter();
    await stage(a, 121, [a1, a2]);

    // Now move the fork point out from under A. B forks one block BELOW A, at
    // 120, and is heavier, so its commit replaces the block at 121 with b121.
    // The chain still has *a* block at 121 when A commits — just not A's.
    const b = await submitter();
    await stage(b, 120, [b121, b122]);
    await commit(b);

    const moved = await program.account.lightClient.fetch(lightClient);
    expect(moved.tipHeight.toNumber()).to.equal(122);
    expect(Buffer.from(moved.tipHash).toString("hex"))
      .to.equal(doubleSha256(b122).toString("hex"));

    // A's staged branch outweighs the incumbent, so a commit that did not
    // re-check the parent would splice it in and the window would hold a branch
    // whose parent hash is not in the chain. State the inequality rather than
    // leaving it to the error code. Compared as BigInt because chainwork is a
    // u128 and `BN` in the hand-written type stand-in exposes only toString.
    const aStaged = await program.account.forkStaging.fetch(a.addr);
    const work = (r: { chainwork: { toString(): string } }) =>
      BigInt(r.chainwork.toString());
    const aHeaviest = aStaged.records[aStaged.records.length - 1];
    const incumbentTip = moved.headers[moved.headers.length - 1];
    expect(work(aHeaviest) > work(incumbentTip),
      "A's branch must be strictly heavier, or ForkNotHeavier refuses it too")
      .to.equal(true);

    try {
      await commit(a);
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
    // The same shape without the interleaving. `set_checkpoint(124)` leaves no
    // record at 124, so push the fixture's 125 to give the branch a fork point
    // that is genuinely in the window. If the re-anchor check were too strict —
    // comparing against the wrong block, or re-reading the parent after the
    // window had already been rebuilt — this fails while the test above still
    // passes.
    await reanchor(124);
    await push(rawAt(125));
    const b126 = forkFrom(rawAt(126), rawAt(125));

    const c = await submitter();
    await stage(c, 125, [b126]);
    await commit(c);

    const after = await program.account.lightClient.fetch(lightClient);
    expect(after.tipHeight.toNumber()).to.equal(126);
    expect(Buffer.from(after.tipHash).toString("hex"))
      .to.equal(doubleSha256(b126).toString("hex"));
    expect(await provider.connection.getAccountInfo(c.addr)).to.be.null;
  });
});

// ---------------------------------------------------------------------------
// Independent cw-144, for the mainnet instruction-path test
// ---------------------------------------------------------------------------
//
// The on-chain test must assert "the resulting bits matches", not only "the
// transaction did not throw". Rather than scrape the program's log, this
// recomputes the target from the CLIENT'S OWN window — fetched before the push
// — and asserts the fixture header declares exactly that value. It is a second
// implementation on purpose, and it mirrors `programs/solbeam/src/difficulty.rs`
// operand for operand: the vector suite proves the Rust function is right, and
// this proves the *instruction path* fed it the state the function expects.

const MAINNET_POW_LIMIT_BITS = 0x1d00ffff;
const U256_MASK = (1n << 256n) - 1n;

/** `GetSuitableBlock` — median of the three records ending at `index`, by time. */
function suitableIndex(records: { time: number }[], index: number): number {
  const lo = Math.max(0, index - 2);
  const candidates: number[] = [];
  for (let i = lo; i <= index; i++) candidates.push(i);
  for (let i = 1; i < candidates.length; i++) {
    let j = i;
    while (j > 0 && records[candidates[j]].time < records[candidates[j - 1]].time) {
      const tmp = candidates[j];
      candidates[j] = candidates[j - 1];
      candidates[j - 1] = tmp;
      j--;
    }
  }
  return candidates[Math.floor(candidates.length / 2)];
}

/** `SetCompact` — compact `bits` to a 256-bit target. */
function compactToTarget(bits: number): bigint {
  const exponent = bits >>> 24;
  const mantissa = bits & 0x007fffff;
  if (exponent <= 3) return BigInt(mantissa >>> (8 * (3 - exponent)));
  if (exponent <= 32) return BigInt(mantissa) << BigInt(8 * (exponent - 3));
  return 0n;
}

/** `GetCompact` — the node's compact encoding, sign-bit special case included. */
function targetToCompact(target: bigint): number {
  const bytes = new Uint8Array(32);
  let t = target;
  for (let i = 31; i >= 0; i--) {
    bytes[i] = Number(t & 0xffn);
    t >>= 8n;
  }
  let size = 0;
  while (size < 32 && bytes[size] === 0) size++;
  if (size === 32) return 0;
  let compact = 0;
  for (let i = 0; i < 3; i++) {
    if (size + i < 32) compact |= bytes[size + i] << (8 * (2 - i));
  }
  size = 32 - size;
  if (compact & 0x00800000) {
    compact >>>= 8;
    size += 1;
  }
  return (compact | (size << 24)) >>> 0;
}

function computeTarget(
  firstWork: bigint, firstTime: number, lastWork: bigint, lastTime: number,
): bigint {
  let workDelta = lastWork - firstWork;
  if (workDelta < 0n) workDelta = 0n;
  const work = workDelta * 600n;
  let actual = lastTime - firstTime;
  if (actual > 172800) actual = 172800;
  else if (actual < 43200) actual = 43200;
  const scaled = work / BigInt(actual);
  if (scaled === 0n) return U256_MASK;
  return ((~scaled) & U256_MASK) / scaled;
}

/** `next_target`, on the window the client actually holds. `null` below lookback. */
function requiredBits(
  records: { time: number; chainwork: bigint }[],
  oldestHeight: number,
): number | null {
  if (records.length < 147) return null;
  const last = suitableIndex(records, records.length - 1);
  const parentHeight = oldestHeight + records.length - 1;
  const ancestorIndex = parentHeight - 144 - oldestHeight;
  if (ancestorIndex < 0) return null;
  const first = suitableIndex(records, ancestorIndex);
  let target = computeTarget(
    records[first].chainwork, records[first].time,
    records[last].chainwork, records[last].time,
  );
  const powLimit = compactToTarget(MAINNET_POW_LIMIT_BITS);
  if (target > powLimit) target = powLimit;
  return targetToCompact(target);
}

/**
 * The tests that would have caught F1, F2 and F3.
 *
 * The 20 tests above run on a regtest chain, where `no_retargeting` makes the
 * target constant. They validate linkage, the window, the reorg machinery and
 * the mint — everything except the one rule a real chain exercises. The
 * 324/324 fixture result validated `difficulty.rs` as a *function*, not the
 * instruction path that feeds it. These tests push **real BSV mainnet headers**
 * through `push_header` and `push_fork_header`, which is the gap.
 */
describe("solbeam — real mainnet headers (F1, F2, F3)", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.Solbeam as unknown as Solbeam;

  const regtest = JSON.parse(fs.readFileSync(FIXTURE, "utf8"));
  const regRaw = (height: number): Buffer =>
    Buffer.from(regtest.headers.find((h: any) => h.height === height)!.raw, "hex");

  const MAINNET = JSON.parse(fs.readFileSync(MAINNET_FIXTURE, "utf8"));
  const mraw: Buffer[] = MAINNET.map((h: any) => Buffer.from(h.raw, "hex"));

  const [lightClient] = anchor.web3.PublicKey.findProgramAddressSync(
    [Buffer.from("light_client")], program.programId);

  // The checkpoint is the fixture's 147th header (index 146), so the first 147
  // headers ARE the seed — 146 ancestors plus the checkpoint itself — and the
  // first live push is index 147. The 160 headers pushed below are the first
  // 160 of the 324 the vector suite predicts exactly.
  const CP_INDEX = 146;
  const CP_HEIGHT = MAINNET[CP_INDEX].height; // 968,376
  const SEED: Buffer[] = mraw.slice(0, CP_INDEX + 1); // 147 records
  const LIVE_FROM = CP_INDEX + 1; // index 147, height 968,377
  const PUSH_COUNT = 160;
  const MAX_FORK_BATCH = 12;
  // A branch batch is smaller than the program's limit because this submitter is
  // a keypair of its own: the provider wallet pays the fee, so the transaction
  // carries TWO signatures (128 bytes) rather than one, and 12 x 80 does not
  // fit. The program's ceiling is still 12; this is only what one transaction
  // can carry here.
  const BRANCH_BATCH = 11;

  const bitsOf = (raw: Buffer): number => raw.readUInt32LE(72);
  const internalHash = (raw: Buffer): string => doubleSha256(raw).toString("hex");

  const setCheckpoint = async (height: number, header: Buffer) => {
    await program.methods.setCheckpoint(new anchor.BN(height), Array.from(header))
      .accounts({ lightClient, authority: provider.wallet.publicKey }).rpc();
  };

  const attemptPush = async (raw: Buffer): Promise<string> => {
    try {
      await program.methods.pushHeader(Array.from(raw))
        .accounts({ lightClient, advancer: provider.wallet.publicKey }).rpc();
      return "ACCEPTED";
    } catch (e: any) {
      const m = String(e).match(/Error Code: (\w+)/);
      return m ? m[1] : String(e).slice(0, 80);
    }
  };

  const seedAll = async () => {
    for (let i = 0; i < SEED.length; i += MAX_FORK_BATCH) {
      await program.methods.seedHeaders(
        Buffer.concat(SEED.slice(i, i + MAX_FORK_BATCH)))
        .accounts({ lightClient, authority: provider.wallet.publicKey }).rpc();
    }
  };

  const fundedSubmitter = async (): Promise<anchor.web3.Keypair> => {
    const key = anchor.web3.Keypair.generate();
    const sig = await provider.connection.requestAirdrop(
      key.publicKey, 2 * anchor.web3.LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sig);
    return key;
  };

  const stagingFor = (key: anchor.web3.Keypair): anchor.web3.PublicKey =>
    anchor.web3.PublicKey.findProgramAddressSync(
      [Buffer.from("staging"), key.publicKey.toBuffer()], program.programId)[0];

  const initStaging = async (key: anchor.web3.Keypair, staging: anchor.web3.PublicKey, forkHeight: number) => {
    await program.methods.initStaging(new anchor.BN(forkHeight))
      .accounts({ lightClient, staging, submitter: key.publicKey,
                  systemProgram: anchor.web3.SystemProgram.programId })
      .signers([key]).rpc();
  };

  const pushFork = async (key: anchor.web3.Keypair, staging: anchor.web3.PublicKey, batch: Buffer[]) => {
    await program.methods.pushForkHeader(Buffer.concat(batch))
      .accounts({ lightClient, staging, submitter: key.publicKey })
      .signers([key]).rpc();
  };

  const commitFork = async (key: anchor.web3.Keypair, staging: anchor.web3.PublicKey) => {
    await program.methods.commitFork()
      .accounts({ lightClient, staging, submitter: key.publicKey })
      .signers([key]).rpc();
  };

  const abandon = async (key: anchor.web3.Keypair, staging: anchor.web3.PublicKey) => {
    await program.methods.abandonStaging()
      .accounts({ staging, submitter: key.publicKey }).signers([key]).rpc();
  };

  before(async () => {
    try {
      const sig = await provider.connection.requestAirdrop(
        provider.wallet.publicKey, 2 * anchor.web3.LAMPORTS_PER_SOL);
      await provider.connection.confirmTransaction(sig);
    } catch {
      /* already funded — fine */
    }
  });

  it("F2: set_checkpoint re-derives the difficulty state instead of keeping regtest's", async () => {
    // The client is where the P2 suite left it: anchored on a regtest header,
    // so `no_retargeting` is true and cw-144 is not consulted at all.
    const start = await program.account.lightClient.fetch(lightClient);
    expect(start.noRetargeting).to.equal(true);

    await setCheckpoint(CP_HEIGHT, mraw[CP_INDEX]);

    const lc = await program.account.lightClient.fetch(lightClient);
    // The stale flag is gone: this is the whole of F2. On the old code it stayed
    // true and every header after the reset was accepted at regtest's target.
    expect(lc.noRetargeting).to.equal(false);
    expect(lc.expectedBits).to.equal(bitsOf(mraw[CP_INDEX]));
    expect(lc.powLimitBits).to.equal(MAINNET_POW_LIMIT_BITS);
    expect(lc.seedRemaining).to.equal(147);
    expect(lc.headers.length).to.equal(0);
    expect(lc.windowStart.toNumber()).to.equal(CP_HEIGHT - 146);

    // And the client will not advance until it is seeded: with the stale regtest
    // state the old code accepted this header outright.
    expect(await attemptPush(mraw[LIVE_FROM])).to.equal("Seeding");

    // Re-anchoring back to regtest must go the other way, so the derivation is
    // genuinely per-checkpoint rather than a one-way flag clear.
    await setCheckpoint(124, regRaw(124));
    const back = await program.account.lightClient.fetch(lightClient);
    expect(back.noRetargeting).to.equal(true);
    expect(back.seedRemaining).to.equal(0);
    await program.methods.pushHeader(Array.from(regRaw(125)))
      .accounts({ lightClient, advancer: provider.wallet.publicKey }).rpc();
    const live = await program.account.lightClient.fetch(lightClient);
    expect(live.tipHeight.toNumber()).to.equal(125);
  });

  it("the seed rejects a broken linkage and cannot be completed past a fabricated ancestor", async () => {
    await setCheckpoint(CP_HEIGHT, mraw[CP_INDEX]);

    // (a) A batch whose second header does not link to the first. The whole
    // instruction reverts, so the seed is untouched.
    const corrupt = Buffer.from(mraw[1]);
    corrupt[4] ^= 0xff; // break `prev`
    try {
      await program.methods.seedHeaders(Buffer.concat([mraw[0], corrupt]))
        .accounts({ lightClient, authority: provider.wallet.publicKey }).rpc();
      expect.fail("the seed should have rejected a broken linkage");
    } catch (e: any) {
      expect(String(e)).to.contain("BrokenLinkage");
    }
    let lc = await program.account.lightClient.fetch(lightClient);
    expect(lc.seedRemaining).to.equal(147, "a refused batch must not consume the seed");

    // (b) A fabricated ancestor. The first seed record has nothing to link to,
    // so it is accepted — and that is exactly why the *terminal* hash is pinned
    // to the checkpoint. The real chain cannot link to a fabricated root, so the
    // seed can never complete: the fabricated record is detectable even though
    // its difficulty is taken on trust.
    const fabricated = Buffer.alloc(80, 0x5a);
    fabricated.writeUInt32LE(0x20000000, 0);
    fabricated.writeUInt32LE(bitsOf(mraw[0]), 72);
    await program.methods.seedHeaders(fabricated)
      .accounts({ lightClient, authority: provider.wallet.publicKey }).rpc();
    lc = await program.account.lightClient.fetch(lightClient);
    expect(lc.seedRemaining).to.equal(146);
    expect(lc.headers.length).to.equal(1);

    try {
      await program.methods.seedHeaders(mraw[1])
        .accounts({ lightClient, authority: provider.wallet.publicKey }).rpc();
      expect.fail("the real header must not link to a fabricated root");
    } catch (e: any) {
      expect(String(e)).to.contain("BrokenLinkage");
    }
    lc = await program.account.lightClient.fetch(lightClient);
    expect(lc.seedRemaining).to.equal(146);
    expect(lc.headers.length).to.equal(1);
  });

  it("F1: seeds 147 trusted ancestors, then accepts real mainnet headers through push_header", async () => {
    await setCheckpoint(CP_HEIGHT, mraw[CP_INDEX]);
    await seedAll();

    const seeded = await program.account.lightClient.fetch(lightClient);
    expect(seeded.headers.length).to.equal(147);
    expect(seeded.seedRemaining).to.equal(0);
    expect(seeded.tipHeight.toNumber()).to.equal(CP_HEIGHT);
    expect(Buffer.from(seeded.tipHash).toString("hex"))
      .to.equal(internalHash(mraw[CP_INDEX]));

    let accepted = 0;
    for (let i = LIVE_FROM; i < LIVE_FROM + PUSH_COUNT; i++) {
      const raw = mraw[i];

      // The window the program will read, and the target cw-144 derives from it.
      // Asserting this before the push is the literal "the resulting bits
      // matches": the real mainnet header must carry exactly the value the
      // client is about to require.
      const lc = await program.account.lightClient.fetch(lightClient);
      const records = lc.headers.map((h: any) => ({
        time: h.time,
        chainwork: BigInt(h.chainwork.toString()),
      }));
      const required = requiredBits(records, lc.windowStart.toNumber());
      expect(required, `height ${MAINNET[i].height}: cw-144 on the client's own window`)
        .to.equal(bitsOf(raw));

      // And the instruction accepts it. On the old code this threw
      // UnexpectedRetarget at the first header after the checkpoint — the F1
      // deadlock — because `bits` changes every block on mainnet.
      await program.methods.pushHeader(Array.from(raw))
        .accounts({ lightClient, advancer: provider.wallet.publicKey }).rpc();
      accepted++;
    }

    expect(accepted).to.be.at.least(150);
    console.log(`  pushed ${accepted} real mainnet headers through push_header`);

    const lc = await program.account.lightClient.fetch(lightClient);
    const tip = LIVE_FROM + PUSH_COUNT - 1;
    expect(lc.tipHeight.toNumber()).to.equal(MAINNET[tip].height);
    expect(Buffer.from(lc.tipHash).toString("hex")).to.equal(internalHash(mraw[tip]));
    // The seed has aged out of the bounded window: 147 seed + 45 live = 192,
    // and every push after the 45th evicts one of the seed records.
    expect(lc.headers.length).to.equal(192);
    expect(lc.windowStart.toNumber()).to.be.greaterThan(CP_HEIGHT - 146);
  });

  it("F3: a branch header is checked at its own height, where bits changes every block", async () => {
    // The F1 test left the tip at index 306 of the fixture with a full window.
    const tipIdx = LIVE_FROM + PUSH_COUNT - 1; // 306, height 968,536
    const before = await program.account.lightClient.fetch(lightClient);
    expect(before.tipHeight.toNumber()).to.equal(MAINNET[tipIdx].height);

    // Fork below the tip and branch past it, so the branch is strictly heavier
    // and can actually commit — the fork point itself is what F3 was about.
    const forkIdx = tipIdx - 36; // 270, height 968,500
    const forkHeight = MAINNET[forkIdx].height;
    const branch = mraw.slice(forkIdx + 1, tipIdx + 10); // indices 271..315

    // The property under test is only meaningful if bits moves inside the
    // branch: at least two changes. On mainnet it changes every header.
    let changes = 0;
    for (let i = 1; i < branch.length; i++) {
      if (bitsOf(branch[i]) !== bitsOf(branch[i - 1])) changes++;
    }
    expect(changes, "bits must change at least twice inside the branch")
      .to.be.at.least(2);

    // Negative control: the same header with the incumbent tip's *next* target,
    // which is what the old code demanded of every branch header. The new code
    // must reject it, because that is not the target for height 968,501.
    const wrong = Buffer.from(branch[0]);
    const tipNextBits = bitsOf(mraw[tipIdx + 1]);
    expect(tipNextBits).to.not.equal(bitsOf(branch[0]));
    wrong.writeUInt32LE(tipNextBits, 72);
    const keyA = await fundedSubmitter();
    const stagingA = stagingFor(keyA);
    await initStaging(keyA, stagingA, forkHeight);
    try {
      await pushFork(keyA, stagingA, [wrong]);
      expect.fail("a branch header carrying the tip's next target must be rejected");
    } catch (e: any) {
      expect(String(e)).to.contain("UnexpectedRetarget");
    }
    await abandon(keyA, stagingA);

    // Positive: the fixture's real headers, judged at their own heights from the
    // branch's own ancestry. The old code rejected this at the first header.
    const key = await fundedSubmitter();
    const staging = stagingFor(key);
    await initStaging(key, staging, forkHeight);
    for (let i = 0; i < branch.length; i += BRANCH_BATCH) {
      await pushFork(key, staging, branch.slice(i, i + BRANCH_BATCH));
    }
    await commitFork(key, staging);

    const after = await program.account.lightClient.fetch(lightClient);
    expect(after.tipHeight.toNumber()).to.equal(MAINNET[tipIdx + 9].height);
    expect(Buffer.from(after.tipHash).toString("hex"))
      .to.equal(internalHash(branch[branch.length - 1]));
    // A commit closes the staging account and returns the rent.
    const gone = await provider.connection.getAccountInfo(staging);
    expect(gone).to.be.null;
  });
});
