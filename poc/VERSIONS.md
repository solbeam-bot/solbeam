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

Read from the pinned tag's own source (`src/rpc/rawtransaction.cpp`). Neither is guessable, and both cost a round trip to discover.

| Fact | Value |
|---|---|
| **`getmerkleproof2` signature** | `getmerkleproof2 "blockhash" "txid" ( includeFullTx targetType format )` — **block hash first**. Passing only the txid makes the node read it as a block hash and answer HTTP 500 |
| **`getmerkleproof2` result** | The TSC Merkle-proof form: `{ "flags": 2, "index": n, "txOrId": "<txid>", "target": {block header}, "nodes": [...] }` |
| The branch key is **`nodes`**, not `proof` | A parser looking for `proof` finds nothing |
| A node may be the string **`"*"`** | "A copy of the node being calculated" — the odd-level duplication case. Our `merkle_branch()` emits the real duplicated *hash* instead, so the two encodings differ while proving the same thing. The harness resolves `*` into concrete hashes before handing the branch to `verify_deposit()`, which takes real values because those are what fold on-chain |
| `getmerkleproof` is **deprecated** | Its help text says "use getmerkleproof2 instead". Same result shape |

**The lesson worth keeping.** The first live run passed everything except this: txid, transaction codec, header serialisation and Merkle root all matched the node byte for byte — then failed on an argument *order*. `getmerkleproof2` was the only piece no amount of offline work could have pinned, which is the entire reason Phase 1B exists as a separate step rather than being folded into Phase 1A.

## Regtest genesis

SV Node's regtest genesis is **not** Bitcoin's — different header, different target, different message. Any hard-coded assumption about the genesis hash silently breaks.

`bsvchain.py` uses a **synthetic** genesis, clearly marked as such. Phase 1B replaces it with the real regtest genesis and asserts that the mint instruction is byte-identical either way; if the two disagree, the synthetic chain has an assumption wrong and the fix goes in Phase 1A.
