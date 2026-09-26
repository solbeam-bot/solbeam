# Version pins and toolchain facts

**Policy: never guess a version.** Pin only what has been verified to exist, and let the machine record the rest.

`poc/scripts/bootstrap.sh` writes **`VERSIONS.lock`** after a successful run, capturing the version of every tool it found. That file — not this document — is the source of truth for exact versions, and it is what makes a build reproducible.

---

## Pins that are verified, with the evidence

| Thing | Pin | Why, and how it was verified |
|---|---|---|
| **SV Node** | `bitcoin-sv 1.1.1`, `x86_64-linux-gnu` | **The last release that publishes a binary at all.** v1.2.0–v1.2.2 publish zero assets. Verified by listing releases via the GitHub API, and the tarball URL was confirmed to return HTTP 200 |
| **Solana CLI** | `stable` channel, resolved at bootstrap | No aarch64 Linux build exists, so the channel is resolved on the x86_64 host and pinned afterwards into `VERSIONS.lock` |
| **Rust** | rustup default | Pinned via `RUST_TOOLCHAIN` once a build is known-good |
| **Anchor** | `avm install latest` | Pinned into the lock file after the first successful program build |
| **Python** | **3.11+** | `doctor.sh` enforces it. The checker suite uses only the standard library — no pip installs, no virtualenv |

## Regtest facts

| Item | Value | Note |
|---|---|---|
| Regtest compact target | `0x207fffff` | `bsvchain.REGTEST_BITS`. Roughly half of all nonces satisfy it |
| Coinbase maturity | 100 blocks | Standard. Enforced by `bsvchain.COINBASE_MATURITY` |
| Required SV Node parameters | `-excessiveblocksize`, `-maxstackmemoryusageconsensus` | **Required on modern SV Node.** Without them large scripts are rejected consensus-side, and the failure looks like a covenant bug |
| `-txindex=1` | required | Needed to fetch arbitrary transactions when building proofs |
| Time scale | `TIMESCALE = 3600` → 1 hour is 1 second | `bsvchain.TIMESCALE`. One value, so production is `TIMESCALE = 1` |

## Architecture facts — the expensive ones

These cost real time to establish, so they are written down.

| Fact | Consequence |
|---|---|
| **Agave/Solana has never published an aarch64 Linux build.** Every release lists `x86_64-unknown-linux-gnu` only, plus `aarch64-apple-darwin` | An arm64 host cannot install the Solana CLI natively. Verified across ten Agave releases and the older `solana-labs/solana` repo |
| **SV Node's only published binary is `x86_64`** | Same consequence |
| The box this PoC was written on is **aarch64, 4 cores, 3.8 GiB RAM**, with no compilers | Hence: Phase 1A is pure Python and runs there; Phases 1B–3 need an x86_64 host |

`doctor.sh` encodes all of this and refuses to report a machine as ready when it is not.

## SV Node RPC facts

**Confirmed against a live SV Node v1.1.1 in regtest on 2026-09-26**, and read from the pinned tag's own source (`src/rpc/rawtransaction.cpp`). Phase 1B passed on these; the raw response is committed at [`fixtures/node_merkleproof_raw.json`](fixtures/node_merkleproof_raw.json).

| Fact | Value |
|---|---|
| **`getmerkleproof2` signature** | `getmerkleproof2 "blockhash" "txid" ( includeFullTx targetType format )` — **block hash first**. Passing only the txid makes the node read it as a block hash and answer HTTP 500 |
| **Result keys** | `{ index, nodes, target, txOrId }` — and **no `flags`**, contrary to what `getmerkleproof`'s help text implies |
| The branch key is **`nodes`** | Not `proof`. A parser looking for `proof` finds nothing |
| **`nodes` are display-order hex** | Byte-reversed relative to the internal order we compute with. Taking them at face value folds to the wrong root — this was the second live failure. The harness now *determines* the encoding rather than assuming it (`interpret_nodes`) |
| **`target` is a string**, not a header object | A 64-char block hash here. The help text describes a header object; the real response is not one. Handled as dict / 64-char hash / 160-char raw header, and anything else is logged with its type |
| **`nodes` may contain the string `"*"`** | "A copy of the node being calculated" — the odd-level duplication case. Not seen in the 2-transaction block above, but handled and exercised offline, because our `merkle_branch()` emits the real duplicated *hash* where the TSC format uses `*` |
| `getmerkleproof` is **deprecated** | Its help text says "use getmerkleproof2 instead". Same result shape |

### What this cost, and why the step exists

Four wrong assumptions about this one RPC, each found by a live run and none by offline work:

1. the argument order,
2. that the branch was under `proof` rather than `nodes`,
3. that the hashes were in internal order,
4. that `target` was a header object and `flags` was present.

Every other part of the pin passed on its first attempt — txid, the transaction codec against `decoderawtransaction`, the 80-byte header against the node's raw header, and the Merkle root against the block's. That asymmetry is the whole argument for Phase 1B being a separate step: **the primitives could be validated offline; the node's wire format could not.**

## Solana toolchain facts

Each of these cost a round trip to find, and none is guessable from the error it produces.

| Fact | Value |
|---|---|
| **`@anchor-lang/core` needs Node >= 20.18** | It declares `engines: {"node": ">=20.18"}`. Ubuntu 24.04's `nodejs` is 18. The symptom is nothing like the cause: `ERR_REQUIRE_ESM` from `rpc-websockets` requiring an ESM-only `uuid`. Node 20.19+/22 fix that too, via `require(esm)`. Install Node 22 from NodeSource |
| **The TS package is `@anchor-lang/core`** | Renamed from `@coral-xyz/anchor` in Anchor 1.0.0. Both exist on npm; the old one stops at 0.32.1 |
| **`ts-mocha` pins `ts-node` 7.0.1 exactly** | Not a range. npm nests it, so a top-level `ts-node@^10` does not reliably win. Use `mocha --require ts-node/register` directly and drop ts-mocha |
| **Anchor does not put `node_modules/.bin` on PATH** for the `[scripts] test` command | A bare `mocha` gives `command not found` (exit 127). Use `npx mocha` |
| **`anchor test` needs `~/.config/solana/id.json`** | Otherwise it stops with `Unable to read keypair file`. Any throwaway localnet key will do |
| **`anchor test` defaults to Surfpool** | Pass `--validator legacy` to keep `solana-test-validator` |
| **`anchor keys sync` rewrites `Anchor.toml` and strips its comments** | Expect the header to vanish. Fold the synced program id back into the repo |
| **Solana 3.x removed `solana_program::hash`** | `anchor_lang::solana_program` re-exports no hasher either. Use `solana-sha256-hasher` — already in the tree, calls the on-chain `sol_sha256` syscall, and its `sha2` feature (not default) is what makes a host build work |
| **`[registry]` is gone from `Anchor.toml`** | Removed in Anchor 1.0.0 |

## Regtest genesis

SV Node's regtest genesis is **not** Bitcoin's — different header, different target, different message. Any hard-coded assumption about the genesis hash silently breaks.

`bsvchain.py` uses a **synthetic** genesis, clearly marked as such. Phase 1B replaces it with the real regtest genesis and asserts that the mint instruction is byte-identical either way; if the two disagree, the synthetic chain has an assumption wrong and the fix goes in Phase 1A.
