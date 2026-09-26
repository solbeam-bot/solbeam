# Running the PoC — setup and replication

From an empty machine to a passing test suite. Written from a box that was built this way, so the steps and the failure modes are both real.

---

## 1. What you need

**An x86_64 Linux box**, Ubuntu 22.04 or 24.04. Not a preference:

| Dependency | Ships for arm64 Linux? |
|---|---|
| Solana / Agave CLI | **No, and never has.** Every release is `x86_64-unknown-linux-gnu` or `aarch64-apple-darwin` |
| SV Node (`bitcoin-sv`) | **No.** Only `bitcoin-sv-1.1.1-x86_64-linux-gnu.tar.gz` exists; v1.2.x publishes no binaries |

4 cores, 8 GB RAM, ~40 GB disk is comfortable. Nothing needs installing beforehand.

> **On multipass:** it can only launch guests matching the **host** architecture, so an arm64 host gives an arm64 guest and nothing here will run. Cross-architecture support is an open multipass feature request, not a flag you missed. Use QEMU/UTM with full emulation, or a cloud x86_64 VM.

---

## 2. One-command setup

```bash
git clone https://github.com/solbeam-bot/solbeam.git
cd solbeam
poc/scripts/bootstrap.sh          # ~10 minutes
source ~/.profile                 # bootstrap appends the toolchain paths here
poc/scripts/doctor.sh             # must exit 0
```

`bootstrap.sh` is re-runnable and writes `VERSIONS.lock` recording what it resolved. It installs:

| | Why this one |
|---|---|
| **SV Node 1.1.1** | The last release that publishes a binary *at all* |
| **Solana CLI `stable`** (4.1.2) | Ships `solana-test-validator` |
| **Anchor 1.2.0** via `avm` | |
| **Node 22 from NodeSource** | Ubuntu's `nodejs` is 18, and Anchor 1.x needs **≥ 20.18** |
| **Rust** via rustup | |
| **A Solana keypair** | `anchor test` deploys with it and stops dead without it |

**`source ~/.profile` is not optional in the same shell.** The installers put their binaries on a PATH only a *new* login shell knows, so bootstrap exports them for itself and persists them for later. An already-open shell has not read the new file — and that looks exactly like "the install did nothing".

`doctor.sh` is the acceptance test, and it is phase-selective:

```bash
poc/scripts/doctor.sh 1a    # only Phase 1A — runs anywhere, even on arm64
poc/scripts/doctor.sh       # everything; non-zero if anything is missing
```

---

## 3. Phase 1A — the BSV primitives and a mint instruction

Needs only Python 3.11+, so it runs on **any** machine:

```bash
bash poc/checks/run_all.sh
```

Expected: `20/20`, `51/51`, `17/17`, `48/48`, `20/20`, `13/13`, then **`ALL CHECKERS PASSED`**.

Two of the six reach the network to validate against live chain data (mainnet block 800000, real testnet blocks and signatures). The rest are offline, and without network the two **fail** rather than silently passing — deliberately.

The deliverable is `poc/fixtures/deposit_1.json`: a byte-deterministic, self-verifying **202-byte mint instruction**.

---

## 4. Phase 1B — pin the formats against a real SV Node

```bash
poc/scripts/regtest-up.sh                 # SV Node in regtest, mines 101 blocks

export SOLBEAM_RPC=http://127.0.0.1:18443
export SOLBEAM_RPC_USER=solbeam
export SOLBEAM_RPC_PASS=solbeam
export SOLBEAM_REQUIRE_NODE=1             # makes a skipped pin a failure

bash poc/checks/run_all.sh                # the live pin runs instead of SKIPPING
```

The pin rebuilds a deposit on the real chain and asserts every byte format matches: our txid, our codec against `decoderawtransaction`, our 80-byte header against the node's raw header, our Merkle root, and our branch against `getmerkleproof2`. **21 checks.**

It writes `poc/fixtures/node_merkleproof_raw.json` — the node's actual response, so the format is a fact in the repo rather than something someone remembers. Commit it.

**The node does not restart itself** (it runs with `-daemon`, not as a service):

```bash
poc/scripts/regtest-up.sh status     # chain, height, hash rate
poc/scripts/regtest-up.sh            # idempotent: starts it, keeps the chain
```

---

## 5. Phase 2 — the Solana program

```bash
poc/scripts/solana-test.sh              # pull, sync keys, install, build, test
poc/scripts/solana-test.sh --build-only
poc/scripts/solana-test.sh --skip-pull  # build from what is on disk
```

It exists because three of its steps have a non-obvious failure mode:

- **`git pull` can refuse to run.** `check_bsv_node.py` *writes* `poc/fixtures/node_merkleproof_raw.json` and the repo also *tracks* it, so on a machine that has run the live pin there is an untracked copy in the way and git aborts with *"untracked working tree files would be overwritten by merge"*. The script moves it to `*.local-backup` and says so, rather than deleting a file someone might be reading.
- **`anchor keys sync` rewrites tracked files** — `declare_id!` and the program id. Intended; commit the result.
- **`anchor test` needs `--validator legacy`**, because Anchor 1.x defaults to Surfpool and this box has `solana-test-validator`.

By hand:

```bash
cd poc/solana
anchor keys sync          # only if target/ is empty
npm install
anchor build
anchor test --validator legacy
```

**Expect the first `anchor build` to fail.** The program was written against the Anchor 1.x API from its release notes, on a machine with no Rust toolchain. The errors are the point of that step.

---

## 6. A box that configures itself

Paste [`scripts/cloud-init.sh`](scripts/cloud-init.sh) into the provider's **Startup scripts** field — on DigitalOcean that is *Additional Options → Startup scripts*, and that field **is** the user-data field. It re-execs as the login user, waits for apt and the network, clones over HTTPS, and runs everything.

```bash
tail -f /var/log/solbeam-startup.log     # everything goes here
ls -l /home/ubuntu/SOLBEAM_*             # READY or FAILED
```

cloud-init's own `/var/log/cloud-init-output.log` looks quiet after the first line, because the script redirects its output. Expected, not a silent failure.

---

## 7. When it goes wrong

All of these were hit for real. The error message rarely names the cause.

| Symptom | Cause | Fix |
|---|---|---|
| `anchor: command not found` after bootstrap | Shell has not re-read `~/.profile` | `source ~/.profile`, or a new session |
| `doctor` reports missing tools bootstrap just installed | Same | Same |
| `git pull` refuses: *untracked working tree files would be overwritten* | The pin's fixture is untracked but now tracked | `solana-test.sh` handles it; by hand, move the file aside |
| `Unknown file extension ".ts"` from mocha | Node too old. The real cause is an ESM/CJS failure in a transitive dependency | Node 22 from NodeSource |
| `ERR_REQUIRE_ESM` in `rpc-websockets` | Same | Same |
| `command not found: mocha` | Anchor does not put `node_modules/.bin` on PATH | The script uses `npx` |
| `Unable to read keypair file` | No Solana keypair | `solana-keygen new --no-bip39-passphrase -o ~/.config/solana/id.json` |
| `anchor.web3.getAssociatedTokenAddressSync is not a function` | `anchor.web3` is a partial re-export in Anchor 1.x | Derive the ATA with `findProgramAddressSync` |
| `Account already in use` in test setup | Two suites, one validator, both initialising | Keep setup idempotent |
| `src.copy is not a function` | A `Vec<u8>` argument was a plain Array | Pass a `Buffer` |
| `no create_type / DISCRIMINATOR for anchor_spl::token::Mint` | IDL build needs `anchor-spl/idl-build` | Enable that feature |
| `cannot find hash in solana_program` | Solana 3.x removed it | Use `solana-sha256-hasher` |
| `this bitcoind is not Bitcoin SV` | Bitcoin Core cannot pin BSV formats | Use an SV Node |
| `bootstrap.sh` exits 1 immediately | Not an x86_64 host | See §1 |

The full list of toolchain facts, with reasoning, is in [`VERSIONS.md`](VERSIONS.md).

---

## 8. How you know it worked

| | |
|---|---|
| `poc/scripts/doctor.sh` | exits 0 — **19 ok, 0 failures** |
| `bash poc/checks/run_all.sh` | **ALL CHECKERS PASSED** — 156 offline, 157 with a node, plus 13 plays |
| `poc/scripts/regtest-up.sh status` | chain `regtest`, a block height, a hash rate |
| `anchor test --validator legacy` | **9 passing** |
| `poc/fixtures/` | `deposit_1.json` and `node_merkleproof_raw.json` |

---

## 9. Where to go next

| You want to | Read |
|---|---|
| What is proven, and what is not | [`TEST_PLAN.md`](TEST_PLAN.md) §1 |
| Attack it yourself | [`ADVERSARY_PLAYBOOK.md`](ADVERSARY_PLAYBOOK.md) |
| What is pinned, and why | [`VERSIONS.md`](VERSIONS.md) |
| The Solana program | [`solana/README.md`](solana/README.md) |
| The trust model | [`../docs/04-trust-model.md`](../docs/04-trust-model.md) |
| What the site will publish once there is data | [`PHASE5_MONITORING.md`](PHASE5_MONITORING.md) |
