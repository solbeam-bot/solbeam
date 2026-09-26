# Phase 2 — the BSV light client on Solana

**First increment: the light client only.** A checkpoint, a rolling window of
headers, and the two checks that make a header chain meaningful — linkage and
proof of work. The mint and the token come next; see *Known gaps* below.

The reason to build this part first: it is the half that must be **identical**
to the Python reference in `poc/checks/`. If the two disagree about a header
hash or a Merkle fold, everything above them is worthless. Proving agreement on
the same fixture is the most valuable test in the PoC, and it needs nothing
else built.

## Running it

On the x86_64 box (the droplet that already has Anchor 1.2.0):

```bash
cd poc/solana

anchor keys sync            # only if target/ is empty — see below
npm install                 # the build box has npm; it has no yarn
anchor build
anchor test --validator legacy
```

**The program id is **committed** (`EYsckW3596zBL1pxfxGev44z6LH4hEpoHff7tSisvjCW`), so a box that
already has `target/deploy/solbeam-keypair.json` needs no sync. A fresh clone
with an empty `target/` does need `anchor keys sync`, which regenerates the
keypair and rewrites the id — and, note, **strips the comments out of
`Anchor.toml`** while doing it.

`--validator legacy` matters.** Anchor 1.x uses **Surfpool** as the default
backend for `anchor test` and `anchor localnet`. If Surfpool is not installed,
pass `--validator legacy` to keep using `solana-test-validator`, which the
droplet already has.

## What the tests assert

Four tests, all against `poc/fixtures/deposit_1.json` read **unmodified**:

1. **The checkpoint agrees.** The program rebuilds the checkpoint header from
   its fields and derives its hash; the test compares that against the hash
   computed independently in TypeScript from the same raw bytes.
2. **Every header is accepted and the tip is reached.** All 16 fixture headers
   are pushed, and the tip height and hash must match the fixture's last header.
3. **Broken linkage is rejected.** Re-submitting the current tip has valid proof
   of work but the wrong parent, so it must fail on `BrokenLinkage`.
4. **Unmet proof of work is rejected.** A constructed header whose parent *is*
   the tip, so linkage passes and the target is the only thing that can fail.

If test 2 passes, the on-chain light client and the Python reference agree on
the same bytes. That is the cross-implementation agreement the test plan calls
the most valuable test in the PoC.

## Anchor 1.x notes

Anchor 1.0.0 was a large breaking release. Three changes shaped this workspace,
and each would have been a wrong guess without checking:

| Change | What it means here |
|---|---|
| TS package renamed `@coral-xyz/anchor` → `@anchor-lang/core` | `package.json` and both imports in the test use the new name |
| `anchor test` defaults to **Surfpool** | hence `--validator legacy` |
| `CpiContext::new` no longer takes the program `AccountInfo`; duplicate mutable accounts are rejected by default | not yet hit, because there is no CPI in this increment — it will matter for the token mint |
| `[registry]` removed from `Anchor.toml` | deliberately absent |

`solana_program` is used **through** `anchor_lang::solana_program`, with no
direct dependency in `Cargo.toml`. Two copies of it in one build is the usual
cause of a version-mismatch build failure.

## Status — read this before trusting it

**This code has never been compiled.** It was written against the Anchor 1.x
API from the release notes, on a machine with no Rust toolchain
(`poc/VERSIONS.md` explains why). Expect a fix pass on the first `anchor build`
— most likely small things: a lifetime, a `Result` signature, a trait import.

That is deliberate and cheap: one build reveals all of them, and it is the same
pattern that worked for Phase 1B, where four wrong assumptions about one RPC
were each settled by one live run.

## Known gaps in this increment

Each is a decision, not an oversight:

- **No chainwork.** Reorg resolution needs accumulated work to reject a *valid
  but lower-work* competing chain — linkage alone is not enough. Getting it
  right means 256-bit arithmetic, so it gets its own increment rather than an
  approximation.
- **No DAA.** Regtest fixes the target. `check_daa()` exists, is code-pathed and
  runs on every header, so enabling retargeting for testnet is a comparison
  rather than a rewrite.
- **No mint and no token.** Deliberately absent, which also keeps this
  increment clear of the Anchor 1.x CPI changes.

## Next

1. `anchor build` on the droplet, then a fix pass.
2. Chainwork, so a lower-work competing chain is rejected.
3. The Merkle fold and `verify_and_mint`, consuming the fixture's **instruction**
   — not just its headers.
4. `solBSV`: classic SPL, 8 decimals, no freeze authority, mint authority = the
   bridge PDA, with the ATA created so a first-time user needs no SOL.
5. The hostile advancer, which is the Phase 2 test that matters most.
