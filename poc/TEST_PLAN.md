# SOLBEAM PoC — phased test scope and work plan

Status: **draft for review.** This supersedes the `P0`–`P5` list in `README.md` §5 and reorganises the work into four phases, ordered so that the parts that can be proven without a toolchain are proven first.

The intent is unchanged: **get the design in front of reviewers as running code.** What changes is the sequencing — and, as §2 shows, the machine it runs on.

---

## 1. Where we are today

### 1.1 Done — the BSV primitives, validated against live chain data

The Python checker suite passes **156/156** offline, and **157/157** with a live SV Node (the live pin adds one check that needs a real node). It runs anywhere Python runs. These are the regression vectors every later implementation must match.

| Checker | Proves | Result |
|---|---|---|
| `checks/check_bsv_core.py` | Header serialisation + double-SHA256, PoW against the compact `bits` target, parent linkage, Merkle root rebuilt from real blocks, branch build/fold, odd-level duplication, tamper rejection | **20/20** — mainnet block 800000; testnet blocks with 5, 8 and 13 txs; synthetic 4- and 5-leaf trees |
| `checks/check_bsv_tx.py` | Legacy tx codec (byte-exact round-trip, txid), P2PKH parsing, `SIGHASH_FORKID` preimage + digest, **real network signatures verified against digests computed from scratch** | **51/51** — 3 real testnet txs, 12 inputs |
| `checks/check_bsv_deposit.py` | P2PKH address encoding vs real addresses, `OP_RETURN` carrying a Solana recipient, deposit tx shape, RFC-6979 signing, redemption tx shape | **17/17** |
| `checks/check_bsv_pegin.py` | **Phase 1A — complete.** A synthetic regtest chain (mining, coinbase maturity, reorg), deposit construction, the proof builder and the verifier: confirmation depth, tampering, malformed deposits, replay, odd Merkle counts, orphaned branches. Emits `fixtures/deposit_1.json` | **48/48** — the fixture is byte-deterministic across runs |
| `checks/check_bsv_node.py` | **Phase 1B — complete.** Pins our byte formats against a real SV Node: txid, the codec vs `decoderawtransaction`, the 80-byte header vs the node's raw header, the Merkle root, and the branch vs `getmerkleproof2` | **21/21 live** — passed against SV Node v1.1.1, 2026-09-26. Raw response committed as `fixtures/node_merkleproof_raw.json` |
| `adversary/attack.py` | 13 attacks against a fresh synthetic chain each, plus an honest control | **13/13 as documented** |

`checks/bsvchain.py` builds the synthetic chain; `checks/bsvlib.py` holds the single implementation of headers, PoW and Merkle folding used by the core checker, the chain builder and the node pin.

`bash checks/run_all.sh` runs everything. Without `SOLBEAM_RPC` the live pin prints `SKIPPED`; with it, and `SOLBEAM_REQUIRE_NODE=1`, a skip becomes a failure.

### 1.2 Phase 0 and Phase 1B: done

- **Phase 0 — executed.** `bootstrap.sh` ran end to end on a clean x86_64 Ubuntu droplet and installed SV Node v1.1.1, Rust 1.98.1, Solana CLI 4.1.2, Anchor 1.2.0, node and `solana-test-validator`. `doctor.sh` reports **19 ok, 1 warning, 0 failures** and exits 0.
- **Phase 1B — passed.** `check_bsv_node.py` pinned every byte format against a live SV Node in regtest. It took four corrections to get there, all of them about one RPC's wire format rather than about our own logic — see [`VERSIONS.md`](VERSIONS.md#sv-node-rpc-facts).

### 1.3 Not started

- **Phase 2** — the Anchor program and the BSV light client on `solana-test-validator`.
- **Phase 3** — the off-chain services (advancer / watcher / relayer); bond accounting, deadlines, refunds, the unbonding period.
- The user-facing surface.
- **Phase 5** — monitoring. Plan only; see §11.

### 1.4 The open technical unknown — **resolved**

Web APIs could not answer the questions that mattered, so a **real SV Node in regtest** did. All three are now pinned, and the raw response is committed:

- the exact output shape of `getmerkleproof2` — keys `{index, nodes, target, txOrId}`, branch under **`nodes`**, hashes in **display order**, `target` a **block-hash string**, and **no `flags`**,
- `decoderawtransaction` / `getblock` field names and types,
- that our serialisation matches the node's byte-for-byte.

`fixtures/node_merkleproof_raw.json` is the recorded response. It corrects four wrong assumptions, all of them about this one RPC's wire format and none about our own logic — the full account is in [`VERSIONS.md`](VERSIONS.md#sv-node-rpc-facts). §3.2's circularity risk is now closed by measurement rather than by argument.

### 1.5 Environment finding — why the PoC runs on an x86_64 VM

This machine is **aarch64 Linux, 4 cores, 3.8 GiB RAM**, with no compilers installed.

| Dependency | Does it ship for this box? |
|---|---|
| Solana / Agave CLI + `solana-test-validator` | **No — and never has.** Every Agave and `solana-labs/solana` release ships `x86_64-unknown-linux-gnu` only (plus `aarch64-apple-darwin`). There is no aarch64 Linux build to download |
| SV Node (`bitcoin-sv`) | **No.** Only `bitcoin-sv-1.1.1-x86_64-linux-gnu.tar.gz` exists; v1.2.x publishes no binaries at all |
| Rust, Go | Source-only, fine on arm64 |
| Compilers (gcc/g++/make/cmake) | Not installed, but `apt` and `sudo` are available |

So neither of the two heavyweight dependencies can be installed natively here. See §2.1 for the options.

---

### 1.6 Phase 1A — what is proven so far

**Passing now (48 checks, offline, no node, ~0.8 s):**

- A synthetic regtest chain: mining to the regtest target, coinbase maturity at 100 blocks, and a reorg that discards a branch.
- The deposit transaction — a user's payment to the bridge deposit address with the Solana recipient in an `OP_RETURN`, signed and independently verified.
- The **verifier**, which is the code the Solana program must reproduce, with a distinct rejection code for each failure: `BAD_POW`, `BROKEN_LINKAGE`, `BAD_CHECKPOINT`, `BAD_MERKLE_PROOF`, `TXID_MISMATCH`, `COINBASE_DEPOSIT`, `NO_SUCH_OUTPUT`, `AMOUNT_MISMATCH`, `ZERO_VALUE`, `WRONG_OUTPUT_SCRIPT`, `MISSING_PAYLOAD`, `INSUFFICIENT_CONFIRMATIONS`, `ALREADY_MINTED`.
- Confirmation depth as a **parameter**, not a constant: rejected at 11, accepted at 12.
- **Replay protection** through a used-`(txid, vout)` registry.
- Two deposits in one block, which exercises **odd-level Merkle duplication** — the rule most likely to be got wrong independently on-chain.
- Reorg: the deposit is mintable on the branch that holds it, and **not** mintable once that branch is discarded.

**The artefact:** `fixtures/deposit_1.json` — 16 headers, the deposit transaction, and a **202-byte mint instruction** with a committed `sha256d` hash. It is byte-deterministic across runs, and the checker re-verifies it **from the file alone**, so the fixture stands on its own rather than depending on the process that produced it. That is what Phase 1B compares against, and what Phase 2 consumes.

**The trust shape the code enforces:** the *verifier* owns the headers (they arrive from the advancer and are checked for PoW and linkage); the *producer* supplies the raw transaction, the Merkle branch and the claimed output, and none of it is believed. A malicious producer has nothing to gain, because every field it supplies is re-derived.

**What Phase 1B must still establish:** that our header and Merkle parsing agrees with the reference implementation — in particular the exact output shape of `getmerkleproof2`. No amount of synthetic testing can settle that, which is the entire reason 1B exists as a separate, byte-equality assertion.

---

## 2. Phase 0 — dependencies and precursors

Phase 0 is not "setup". It is a gate: nothing in Phases 1–3 should start until the environment decision is made and the bootstrap script can assert the environment on demand.

### 2.1 Where the PoC runs — **decided: x86_64 VM**

| Option | What it means | Verdict |
|---|---|---|
| **A. x86_64 Linux host** (4–8 vCPU, 16 GB, 100 GB) | Both dependencies have prebuilt binaries. Nothing to compile | ✅ **CHOSEN.** One cheap VM removes every remaining environment risk |
| **B. Docker + `--platform linux/amd64` on this box** | `docker.io` installs via apt; QEMU emulation runs the x86_64 images | Rejected — slow, and **3.8 GiB RAM is tight for a Solana validator** |
| **C. Build both from source on arm64** | Solana from source is a multi-hour, memory-hungry Rust build (very likely to OOM at 3.8 GiB); SV Node needs a C++20 toolchain that is not installed | Rejected |

**Still use the arm64 box for Phase 1A.** It needs no toolchain at all — see below — so the VM is only required from Phase 1B onward. Bring the VM up in parallel with 1A rather than before it.

**The mitigation that makes this non-blocking:** §3.2 shows that **Phase 1 needs no node at all.** We already own header serialisation, PoW, Merkle folding and tx signing in Python, so we can generate a valid regtest-shaped chain offline and test the whole deposit path against it. The real SV Node is then needed for *format pinning*, not for development.

That means work can start immediately on Phase 1 while the VM is arranged, and the Phase 2 toolchain only has to exist by the time the on-chain verifier is ready to deploy.

### 2.2 Dependency table

| Dependency | Needed by | Install route | On this box |
|---|---|---|---|
| **Python 3.11+** | All checkers, **and every off-chain service** (advancer, watcher, relayer, user-facing API) | present | ✅ 3.12.3 |
| `git`, `curl`, `jq` | Everything | present | ✅ |
| **SV Node** (`bitcoind`/`bitcoin-cli`) | 1B, 2, 3 | x86_64 release binary **or** Docker image **or** source build | ❌ needs 2.1 |
| **Solana CLI / Agave** | 2, 3 | `release.anza.xyz` install script (x86_64) | ❌ needs 2.1 |
| **`solana-test-validator`** | 2, 3 | ships with the CLI | ❌ needs 2.1 |
| **Rust + cargo** | 2, 3 | `rustup` (arm64 fine) | ❌ not installed |
| **Anchor + `avm`** | 2, 3 | `cargo install --git … avm` | ❌ not installed |
| **`spl-token` CLI** | 2, 3 | ships with the CLI | ❌ needs 2.1 |
| **Go / Rust** | **Not used in the PoC** — the service language is decided after the PoC, with the team, using the evidence the PoC produces | — | — |
| **Node 20+ / npm** | The minimal web page and a browser wallet adapter | present | ✅ 22.23.2 |
| C++20 toolchain, boost, libevent, openssl | only if building SV Node | `apt` | ❌ not installed (`sudo` available) |
| Docker | option B | `apt install docker.io` (29.1.3 available) | ❌ not installed |

**Pin every version in the repo once chosen** — especially the Solana CLI and Anchor versions, which drift fast and break builds.

### 2.3 Precursors — configuration, not packages

These are the things that are wrong-by-default and cost a day if discovered late.

**BSV / SV Node**

- `-regtest` with `-txindex=1` (needed to fetch arbitrary txs for proofs).
- `-excessiveblocksize=2GB` and `-maxstackmemoryusageconsensus=100MB` — **required** on modern SV Node; without them large scripts are rejected consensus-side.
- `-minminingtxfee` set low, so regtest txs are cheap.
- A regtest genesis that is **not** Bitcoin's — header, PoW target and message differ. Any hard-coded assumption here silently breaks.
- A funded regtest wallet: `generatetoaddress 101` for spendable coinbase.
- Regtest has a **fixed difficulty target**. DAA must therefore be *stubbed behind a flag*, not omitted — see §4.2.

**Solana**

- `solana-test-validator --reset` is the regtest equivalent: a single-node local cluster.
- A funded payer keypair (`solana airdrop` against localnet).
- An Anchor workspace with the program ID pinned, so the ID is stable across resets.

**Keys and secrets**

- A regtest BSV keypair for the deposit address and one for the relayer hot wallet.
- A Solana keypair for the payer, one per relayer, and the bridge PDA.
- `.env.example` committed, real keys never committed. The existing `.gitignore` already covers `*.key`, `*.pem`, `.env`.

### 2.3.1 Regtest time scale — **1 hour → 1 second**

Waiting is the enemy of a test matrix that has to run unattended. Every wall-clock deadline is compressed by one fixed factor, applied in a single place so it cannot drift between components.

| Production | Regtest | Mechanism |
|---|---|---|
| 12 BSV confirmations (~2 hours) | **12 blocks + 2 s** | The 12 blocks are mined on demand; the 2-second delay reproduces the *wait*, so the watcher's polling and the user's experience are exercised for real rather than skipped |
| Redemption deadline — 6 hours | **6 s** | The refund path fires on a 6-second timer |
| Payout settlement window — 6 hours | **6 s** | Drives the reorg-after-payout case |
| Challenge window — 24 hours | **24 s** | How long an unmatched-spend proof stays admissible |
| Unbonding period — 7 days | **30 s** | `≥ deadline + challenge window` = 6 + 24, which is the *production relationship*, merely scaled |

The factor is a single config value (`TIMESCALE = 3600`), not five independent numbers, so restoring production behaviour is `TIMESCALE = 1` and nothing else. A direct consequence for the tests: they must assert **relationships** (`unbonding > deadline + challenge`) rather than absolute seconds, or the whole matrix silently breaks the day the scale changes.

### 2.4 Deliverables

- `scripts/bootstrap.sh` — installs/pins every dependency for the chosen option.
- `scripts/doctor.sh` — asserts the environment and prints one line per requirement. **This is the acceptance test for Phase 0.**
- `.env.example`, `VERSIONS.md` (the pin list), and the decision recorded in this file.

**Phase 0 is done when** `./scripts/doctor.sh` exits 0 on a clean machine and `scripts/` can bring up both chains unattended.

---

## 3. Phase 1 — peg in, BSV side only (regtest)

**Goal: produce a *portable mint instruction* from a real BSV deposit, and prove it offline.**

This phase deliberately involves **no Solana**. The output is a self-contained artefact that Phase 2 will verify on-chain. Keeping the legs separate means a failure is never ambiguous about which half broke.

### 3.1 What gets built

1. A **chain builder** that generates a regtest-shaped BSV chain offline (heights, headers, coinbase, PoW) so tests are deterministic and need no node.
2. A **deposit builder**: the user's payment to the deposit address, with the Solana recipient in an `OP_RETURN`.
3. A **proof builder**: from a block and a tx, emit the header, the Merkle branch, the index, the height, and the claimed `(vout, amount, recipient)`.
4. A **proof verifier**: check PoW, linkage to a trusted checkpoint, Merkle inclusion, output value and script, `OP_RETURN` payload, and **n-of-12 confirmation depth** — then emit the mint instruction.
5. A **wallet-index**: map `(txid, vout)` → already-minted, for idempotency.

### 3.2 Two halves, and why the split matters

- **1A — synthetic chain (no node, runs anywhere).** Every case below runs against chains we generate. Fast, deterministic, zero dependencies. This is the bulk of the work and it can start today.
- **1B — real SV Node pin.** Re-run the identical cases against a real regtest node, and assert that our parser consumes the node's native `getmerkleproof2` / `getblock` output and produces a **byte-identical mint instruction** to 1A's.

1B is what converts "we are consistent with ourselves" into "we are consistent with the reference implementation." It is a **regression check, not a development blocker** — Phase 2 can begin before it lands.

### 3.3 Test scope

| # | Case | Expected |
|---|---|---|
| 1.1 | Deposit tx construction + `SIGHASH_FORKID` signing | Valid; signature verifies against a digest computed independently *(already 17/17)* |
| 1.2 | Header chain: serialisation, PoW vs target, linkage | Valid chain accepted; bad PoW and broken linkage rejected *(already 20/20)* |
| 1.3 | Merkle inclusion, incl. odd-level node duplication | Valid branch accepted; mutated leaf/branch rejected *(already covered)* |
| 1.4 | **Confirmation depth** | Rejected at 11 confirmations, accepted at 12 (depth is a parameter, not a constant) |
| 1.5 | **Reorg** — `invalidateblock` past a deposit | Deposit on the orphaned branch is **not** mintable; the replacement branch is used |
| 1.6 | **Idempotency** — same `(txid, vout)` twice | One mint instruction; the second is rejected |
| 1.7 | **Malformed deposits** — no `OP_RETURN`; truncated or oversized payload; non-32-byte recipient; underpaid/dust; output to an unexpected script | Each rejected with a distinct, named error |
| 1.8 | Coinbase / immature output claimed as a deposit | Rejected |
| 1.9 | **Multiple deposits in one block** | Each yields its own instruction; ordering is irrelevant |
| 1.10 | **Proof portability fixture** | The proof is serialised to the exact byte layout Phase 2 will submit, hashed, and committed as a fixture |

### 3.4 Acceptance and artefact

- `checks/run_all.sh` green, including the new cases.
- A committed fixture: `fixtures/deposit_1.json` — the chain, the deposit, and the *exact bytes* of the mint instruction.
- One command reproduces the fixture from scratch.

**Phase 1 is done when** a stranger can run one command, get a mint instruction from a deposit, and separately re-verify that instruction without trusting the producer.

---

## 4. Phase 2 — Solana: the token and the light client

**Goal: the Phase 1 fixture is verified *on-chain*, and the mint is authorised by the proof and nothing else.**

Solana's equivalent of regtest is **`solana-test-validator`** — a local single-node cluster. No faucet, no devnet, instant finality, `--reset` between runs.

### 4.1 What gets built

1. **`solBSV`** — a classic SPL mint: 8 decimals, **no freeze authority**, mint authority = the bridge PDA. Asserted programmatically, not by inspection.
2. **A BSV light client program** — a checkpoint, a rolling window of headers with chainwork, and instructions to push a header and to answer "is this tx in this block".
3. **The bridge program** — deposit registry, `verify_and_mint`, the used-`(txid,vout)` set, caps, and pause.
4. **The advancer** — an untrusted off-chain loop that reads headers from a BSV node and submits them.
5. **A hostile advancer** — the same loop, deliberately malformed. It exists only to drive the negative tests.

### 4.2 The DAA decision

Regtest has a fixed target, so DAA can be skipped for the PoC. **But it must be a flag, not an omission**: the check is implemented, code-pathed and unit-tested, and simply disabled on regtest. Otherwise the testnet run becomes a rewrite instead of a config change.

### 4.3 Test scope

| # | Case | Expected |
|---|---|---|
| 2.1 | Mint properties | 8 decimals; freeze authority absent; mint authority == the bridge PDA |
| 2.2 | Light client accepts a valid header sequence | Checkpoint + window stored; chainwork accumulates |
| 2.3 | Bad PoW / bad linkage / wrong checkpoint | Each rejected |
| 2.4 | **Rolling window bound** | A header outside the window is rejected; **rent cost measured and recorded** |
| 2.5 | **DAA check** (flag on) | Accepts a valid retarget, rejects an invalid one — even though regtest disables it |
| 2.6 | **Cross-implementation agreement** | The on-chain Merkle/header logic and the Python verifier accept and reject the *same* fixtures. This is the single most valuable test in the PoC — it is where two independent implementations are forced to agree |
| 2.7 | **Mint from the Phase 1 fixture** | Balance rises by exactly the deposited amount, at the right ATA, with 8 decimals |
| 2.8 | Replay the same proof | Rejected |
| 2.9 | Tampered branch / wrong `vout` / wrong amount / wrong recipient | Each rejected |
| 2.10 | **Hostile advancer** — fabricated header, out-of-order header, duplicated header | All rejected; it cannot mint, and it cannot corrupt the window |
| 2.11 | **Stalling advancer** | Chain stops advancing; **no funds are at risk**; a second advancer recovers the tip |
| 2.12 | Caps and `pause` | Mint above cap rejected; mint while paused rejected; **holders can still burn** |
| 2.13 | **Compute-unit budget** | The SPV-verify path's CU cost is measured and recorded. The design estimate is ~2,842 CU per deposit proof — **confirm or correct it** |
| 2.14 | User role, end to end | A browser wallet pointed at localnet sees and moves `solBSV` |

### 4.4 Acceptance and artefact

- `anchor test` green against a fresh `solana-test-validator`.
- The CU table committed alongside the program.
- The fixture from Phase 1 consumed **unmodified** — if the layout had to change, that is a Phase 1 bug and it must be fixed there.

**Phase 2 is done when** a deposit fixture produced by Phase 1 mints `solBSV` on a clean local validator, and a hostile advancer cannot forge one.

---

## 5. Phase 3 — peg out

**Goal: the enforcement path works — burn, pay, prove, settle — and cheating is bounded, punished, or both.**

This is where the trust-minimised machinery lives, and where the recent trust-model corrections ([`docs/04-trust-model.md`](../docs/04-trust-model.md#the-naked-option-attack)) turn into tests. The bond is **denominated in `solBSV`**, and the two invariants that matter are `bond ≥ k × (hot float + releasable tranche)` and `custodied BSV ≥ outstanding solBSV` at every step.

### 5.1 What gets built

1. `burn(amount, bsv_destination)` — burns `solBSV`, records a redemption with a deadline.
2. **A relayer binary** — watches burns, pays from the hot wallet, builds the payout proof, submits `fulfil`.
3. `fulfil(id, proof)` — verifies the payout on-chain against the requested destination and amount.
4. `refund(id)` — after the deadline, re-mints the holder and slashes the bond.
5. `challenge(...)` / `slash(...)` — the unmatched-spend path.
6. **Bond custody**: locked `solBSV`, and an **unbonding period** longer than the deadline plus the challenge window.
7. **A deliberately misbehaving relayer** — a mode that selects an attack. This is a *testability requirement*, not a nicety: it is the only way the negative cases can be driven deterministically.

### 5.2 Test scope

| # | Case | Expected |
|---|---|---|
| 3.1 | Happy path | Redemption closes; supply decrements; BSV lands at the destination |
| 3.2 | Relayer never pays | After the deadline the holder is **re-minted** and the bond is slashed |
| 3.3 | Wrong address / underpayment | Proof cannot match the request → not fulfilled → deadline refund |
| 3.4 | **Double-claim** — `fulfil` then `refund`, and `refund` then `fulfil` | Both orderings impossible; exactly one terminal state |
| 3.5 | **Naked spend** (no redemption outstanding) | Bounded by the float cap; **requires a challenger** to slash. Assert the bound is what the parameters claim |
| 3.6 | Naked spend, **nobody challenges** | The loss is bounded and the bond is *not* seized. **Record this as the honest residual**, with the no-idle-float rule as the mitigation |
| 3.7 | **Unbonding period** | A relayer cannot exit mid-commitment; exit succeeds only after commitments settle and the notice period elapses |
| 3.8 | **Bond denomination and deflationary slash** | A slashed theft removes supply as the reserve falls; assert backing per token does **not** fall — and rises when `bond > float` |
| 3.9 | Reorg after payout | The 6-hour window catches it; the closed redemption's behaviour is documented |
| 3.10 | Concurrent redemptions | All settle; no hot-wallet double spend |
| 3.11 | Relayer offline mid-flight | The deadline still protects the holder |
| 3.12 | **Round-trip invariant, after every case** | `custodied BSV ≥ outstanding solBSV` asserted as a hard failure, not a warning |

### 5.3 Acceptance and artefact

- The full matrix green, driven by one script.
- An invariant assertion that runs after **every** scenario and fails the run if it breaks.
- A second relayer instance, to prove one relayer cannot touch another's float or bond.

**Phase 3 is done when** the demo script runs the happy path, the liveness failure, the naked-spend bound and the reorg case unattended, printing the invariant at each step.

---

## 6. The actors, and what each one actually needs

The PoC has four roles. Three of them are software; one is a person.

### 6.1 The user — a browser and a wallet

Nothing else. No terminal, no account, no KYC.

| Need | Detail |
|---|---|
| **Peg-in: a BSV wallet** | It must pay to the deposit address **and attach the `OP_RETURN` payload** — decided, see §6.1.1 |
| **A Solana wallet** | To receive `solBSV`. The token account may not exist yet |
| **SOL for fees** | The burn costs a Solana fee. **If the user also needs SOL to create the token account, that is a bad first-run experience** — the bridge should create the ATA and pay its rent (`init_if_needed`), so a first-time user needs zero SOL to *receive* |
| **Browser wallet config** | Custom RPC = localnet (`http://localhost:8899`), cluster `localnet`, and the mint added manually to see the balance |
| **Peg-out** | The burn transaction plus a BSV destination address they control |
| **What we build for them** | A minimal web page (decision 4): generate the deposit address **and the exact `OP_RETURN` payload** for their wallet, show live status, and drive the burn. It is in PoC scope precisely so the acceptance test below is *runnable* rather than asserted |

**Acceptance test for this role:** a person who has never seen the system completes a peg-in and a peg-out using only a browser and their own wallets, with no terminal and no help.

#### 6.1.1 How the deposit carries the recipient — **decided: `OP_RETURN`**

| Model | How it works | Cost |
|---|---|---|
| **A. `OP_RETURN` payload** ✅ **chosen** | The user attaches their Solana address in an `OP_RETURN` when sending. **Single step** | Needs a wallet that can attach `OP_RETURN` data, and a UI that makes it unremarkable — many wallets cannot, and some discourage it |
| **B. Derived deposit address + registry** | Each deposit address is derived from a bridge xpub at an index; the user registers `(index → Solana address)` first. Any wallet works | Two steps, and unregistered deposits need a recovery path |

**Why A.** It collapses peg-in to one transaction, and the checkers already implement it (`check_bsv_deposit.py` builds and verifies exactly this shape), so Phase 1A builds on tested code rather than starting from zero.

**The risk this accepts, and what Phase 1A must therefore test.** The whole flow depends on the user's wallet being willing and able to attach an `OP_RETURN`. That is a *wallet* dependency, not a protocol one, and it is the single most likely cause of a failed first deposit. Phase 1A must:

- reject a deposit that arrives **without** the payload, with a distinct, user-legible error — not a generic "invalid deposit";
- and the minimal web page (§6.1, decision 4) must generate the exact payload and show the user what to attach.

Model **B** stays on the roadmap as the better *product* answer. The PoC's job is to price the difference in real deposits, not to assume it.

### 6.2 The relayer — software with capital at risk

| Need | Detail |
|---|---|
| **Software** | **A Python program** with a config file — one process, loops for watch / pay / prove / submit, plus unbonding (decision 3: the service language is chosen later, on the evidence) |
| **A BSV hot key** | Plus a way to broadcast — its own node or an untrusted broadcast API |
| **A Solana keypair + RPC** | To read burns and submit `fulfil` |
| **A bond in `solBSV`** | **Locked** on Solana, sized `≥ k × (hot float + releasable tranche)`. Not a balance it can move |
| **Proof data** | Headers and a Merkle branch for its own payout. Its own SPV or a public API — the *source need not be trusted*, because the program verifies the proof |
| **Monitoring** | Float level vs cap, bond level vs exposure, pending deadlines, missed fulfilments. A relayer that cannot see its own deadlines will be slashed by its own latency |
| **An exit path** | Announce, settle outstanding commitments, wait out the unbonding period |
| **A misbehaviour mode** | **Required for the PoC.** A flag that selects an attack — steal the float, vanish, underpay, pay the wrong address, refuse to unbond — so the negative tests are deterministic rather than hand-driven |
| **Two instances** | Run two relayers concurrently, to prove independence of float and bond, and that competition does not break settlement |

Note the capital consequence, which the PoC should make visible rather than hide: at `k = 5` a relayer locks **five times** the float it serves, and bonded `solBSV` **cannot be redeemed while bonded**. That locked capital, not gas, is what the fee has to cover.

### 6.3 The advancer — permissionless, untrusted, replaceable

| Need | Detail |
|---|---|
| **A BSV node RPC** | To read headers |
| **A Solana keypair + a little SOL** | For fees |
| **Nothing else** | No bond, no permission, no registration |

Its only power is *liveness*. The PoC must demonstrate this by running a **hostile** advancer (fabricated / out-of-order / duplicated headers → all rejected, no funds at risk) and a **second** honest advancer that recovers the tip when the first stalls.

Optionally run it as a third party for the demo, so reviewers can see the role is genuinely open rather than a story.

### 6.4 Shared: what the PoC does *not* ask of anyone

No trusted third party to operate, no oracle to run, no committee to convene. The only hosted component is a commodity BSV node, and any node will do.

---

## 7. Explicit non-goals

Recorded so reviewers know what they are *not* seeing.

| Deferred | Why |
|---|---|
| **Cold-reserve covenant** (cold → hot only, tranches, hot-balance cap) | Its job is to bound a stolen cold key, which only matters once there is a large reserve. The PoC keeps everything in one hot wallet covered by a bond |
| **Signerless peg-out** (ZK proof of a Solana burn verified in BSV Script) | A 2–4 year research programme |
| **FROST / threshold keys**, insurance, proof-of-reserves publication | Production hardening |
| **Relayer competition economics**, fee markets | The PoC runs two relayers to test independence, not to model a market |
| **Price oracle / price governor** | No longer needed at all: the bond is denominated in `solBSV`, so the invariant is price-invariant |
| **Mainnet hardening**, audits, upgrade-authority process | Out of scope for a PoC |

---

## 8. Sequencing and the critical path

```
Phase 0 ──┬─► 1A  (synthetic chain, no node)  ──┐
          │                                     ├─► Phase 2 ──► Phase 3
          └─► 1B  (real SV Node format pin) ────┘
```

- **The critical path is Phase 2** — specifically the light client. Everything else can run in parallel with it or ahead of it.
- **Phase 1A needs no environment at all** and can start today, on this machine.
- **Phase 1B is done** — every byte format is pinned against a live SV Node, so Phase 2's on-chain verifier now has a measured target rather than an assumed one. It cost four corrections, all to one RPC's wire format; see [`VERSIONS.md`](VERSIONS.md#sv-node-rpc-facts).
- **Phase 3 depends on Phase 2** and on the relayer, but its negative tests can be specified now.

The one ordering rule worth enforcing: **the fixture layout is owned by Phase 1.** If Phase 2 needs it changed, the fix goes in Phase 1 so that both halves keep consuming the same bytes.

---

## 9. Effort

Indicative, one focused developer. Note that Phase 1 is new work that the earlier `P0`–`P5` list folded into other phases, and that the negative tests are now inside each phase rather than a separate block.

| Phase | Duration | Notes |
|---|---|---|
| **Phase 0** — environment, bootstrap, doctor | 2–3 days | ✅ **Done.** `doctor.sh`: 19 ok, 0 failures on the x86_64 droplet |
| **Phase 1A** — synthetic chain + deposit proof | 3–4 days | ✅ **Done.** |
| **Phase 1B** — real SV Node format pin | 1–2 days | ✅ **Done.** Took four live iterations, all on `getmerkleproof2`'s wire format |
| **Phase 2** — token, light client, mint, hostile advancer | 1.5–2 weeks | **The long pole** |
| **Phase 3** — burn, relayer, bond, deadline, challenge | 1–1.5 weeks | Includes the misbehaving-relayer mode |
| **Testnet repeat** (BSV testnet + Solana devnet, DAA on) | 2–3 days | Proves the DAA flag and real difficulty |
| **Total to a demoable PoC** | **~5–6 weeks** | |

**The trustless-mint claim alone** (Phases 0, 1 and 2, with the relayer stubbed) lands in **~2.5–3 weeks** and is already the strongest single claim in the design.

---

## 10. Decisions taken

All six were open questions; these are the answers, and the rest of this document already reflects them.

| # | Decision | Chosen | Consequence |
|---|---|---|---|
| 1 | **Host** | **x86_64 VM** | §2.1. The arm64 box stays the development machine for Phase 1A, which needs no toolchain |
| 2 | **Deposit payload** | **`OP_RETURN`** | §6.1.1. Single-step peg-in. The checkers already implement it, so Phase 1A can build on tested code |
| 3 | **Off-chain language** | **Python** | Not Go, and deliberately not locked in. The PoC is a *research artefact*: its job is to answer questions, not to commit the production stack. The service language gets decided with the team, on the evidence the PoC produces |
| 4 | **User surface** | **Minimal web page** | So the §6.1 acceptance test — "a browser and a wallet, no terminal" — can actually be run rather than asserted |
| 5 | **Regtest time scale** | **1 hour → 1 second** | §2.3.1. Peg-in waits **2 seconds**, peg-out **6 seconds**, unbonding **30 seconds** |
| 6 | **Advancer for the demo** | Open | Run a hostile or third-party advancer publicly, or keep it in-process for the PoC? Low stakes; can be decided at demo time |

### 10.1 On decision 3 — why Python is the right call

The plan previously recommended Go to match an assumed production stack. That was premature. Choosing the service language now would bake a team decision into a research artefact, and the PoC's output — measured compute budgets, the fixture format, the shape of the relayer's loops — is exactly the evidence needed to make that choice well.

Keeping the PoC in Python has three concrete advantages:

1. **One language across the whole PoC.** The BSV primitives are already Python and already validated. The verifier, the chain builder, the relayer and the test harness all sit on top of code that is proven against real chain data.
2. **It is the reference implementation.** Python stays the oracle that any future Go or Rust port must match — the checkers already serve this role, and the fixture in §3 makes it explicit.
3. **Zero toolchain.** Every phase up to and including 1B runs with dependencies that either already exist or install in one line.

The cost is honest: Python is not the production language, so some code will be rewritten. That is acceptable for a PoC whose deliverable is *knowledge*, and the rewrite is bounded because the fixture format and the test vectors carry over unchanged.

---

## 11. Companion documents, and Phase 5

| Document | What it covers |
|---|---|
| [`RUNNING.md`](RUNNING.md) | **Start here to run it yourself.** Fresh clone to a working result in about a minute; what needs the x86_64 host; how to read the results |
| [`PHASE5_MONITORING.md`](PHASE5_MONITORING.md) | **Phase 5 — public monitoring on solbeam.me.** The **absolute** cost to attack BSV, total SHA-256 available for rent, the feasibility gate, value at risk published *alongside* rather than as a ratio, TVL and the reserve invariant. Plan only |
| [`ADVERSARY_PLAYBOOK.md`](ADVERSARY_PLAYBOOK.md) | How a person participates as the man in the middle, and the results record to commit. Includes the two plays the protocol **cannot** win |
| [`adversary/attack.py`](adversary/attack.py) | Runs the plays. `python3 poc/adversary/attack.py --all` — 13 runnable now; the rest are listed so the whole threat model is visible in one place |
| [`VERSIONS.md`](VERSIONS.md) | Pin policy, the verified pins, and the architecture facts that cost real time to establish |
| [`scripts/`](scripts/) | `bootstrap.sh`, `doctor.sh`, `regtest-up.sh`, `cloud-init.sh` — Phase 0's deliverables, including a startup script for a cloud VM |

### Where Phase 5 sits

Phase 5 depends on nothing except data that Phases 1–3 produce, and **5a is worth doing early**: publishing the reserve address and the `solBSV` supply, and letting anyone check `reserve ≥ supply` for themselves, is a stronger statement about this project than any amount of copy. It needs no attack-cost model and no history.

The attack-cost work (5b) is the genuinely novel part, and it is also where the honesty rules matter most — an attack cost is a **lower bound** derived from a rental price, and on BSV specifically the relevant market is **global SHA-256 rental**, not BSV's own hash rate. That caveat belongs on the page, not in a footnote.

---

Next: [PoC plan](README.md) · [Phase 5 monitoring](PHASE5_MONITORING.md) · [Adversary playbook](ADVERSARY_PLAYBOOK.md) · [Trust model](../docs/04-trust-model.md)
