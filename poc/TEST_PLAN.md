# SOLBEAM PoC — phased test scope and work plan

Status: **draft for review.** This supersedes the `P0`–`P5` list in `README.md` §5 and reorganises the work into four phases, ordered so that the parts that can be proven without a toolchain are proven first.

The intent is unchanged: **get the design in front of reviewers as running code.** What changes is the sequencing — and, as §2 shows, the machine it runs on.

---

## 0. Reset — where this actually stands

**This section supersedes §1 and any status language elsewhere in this plan.** A second
adversarial audit found the documents had drifted ahead of the code: several artefacts were
described as working that are specified but not built. §1 is kept as history, not as status.

### 0.1 Built, and passing

- **The light client** — trusted checkpoint, a 192-block rolling window storing a block hash, chainwork and time
  per header, linkage, and proof of work with the target taken **from the chain** rather than from
  the submitted header. 34 on-chain tests.
- **`solBSV`** — classic SPL, 8 decimals, no freeze authority, mint authority a program PDA.
- **The mint** — verifies a deposit against the window: Merkle fold, on-chain transaction parsing,
  exact output script, confirmation depth, replay refusal. Then mints.
- **Fork staging** — per-submitter, batched, abandonable; reorg following with a strictly-heavier
  commit **by accumulated chainwork**, re-anchored at the fork point recorded when the branch was
  staged (P2).
- **The Python reference and the live SV Node pin** — all checkers pass; 21/21 against a real node.

**The deploy procedure is now load-bearing, not incidental.** `initialize` and `initialize_bridge`
accept **only the program's upgrade authority**, read on-chain from the BPF upgradeable loader's
`ProgramData` account (`upgrade_authority_address == Some(payer)`). A program whose upgrade authority
is `Pubkey::default()` — which is what Anchor's legacy validator does by default, and what
`solana program deploy --final` produces — can therefore **never be initialised by any key**, so the
bridge can never start. It fails closed, which is the right direction, but it is a silent dead
deployment if nobody checks. Two things follow:

1. **Localnet tests** set `[test] upgradeable = true` in `poc/solana/Anchor.toml`. Anchor's legacy
   validator embeds workspace programs in genesis instead of deploying them; that flag is what makes
   the embed use `--upgradeable-program` with the wallet as the authority. Without it the two
   authority-**rejection** tests pass vacuously (every signer is rejected) and the authority-success
   tests fail. The acceptance condition is a run where the rejection tests fail *for the right
   reason* — the payer is a genuinely different key — and the success tests pass.
2. **Production** must deploy **upgradeable** and keep the authority key (a threshold-held,
   timelocked authority per F4/D10), or the deployment cannot be initialised at all.

### 0.2 Designed, and NOT built

Everything below is specified in [`docs/02-how-it-works.md`](../docs/02-how-it-works.md) and
[`docs/07-decisions.md`](../docs/07-decisions.md), and does not exist in code. **The vault itself is
built** (`release_mint`, `burn_staged`, `set_maturity`), but its protective window ships at **0**, so
the reversal is a race rather than a window. Everything downstream — the federation, the Greycore,
governance and peg-out — is still a specification rather than a property.

- The **vault** and both gates — maturity, release, burn
- The **order book**, staking, bonds, `owed_R`, relayer consent, `k`
- **Per-relayer deposit scripts** — currently one bridge-wide P2PKH, which is the pooled reserve
  that decision A6 exists to remove
- **All of peg-out** — escrow, deadline, payout proof, challenge, settlement, refunds
- **`MIN_PEG_IN` / `MAX_PEG_IN`**, the aggregate mint cap and committed-depth parsing. *(DAA and chainwork were on this list and are now built — W1.6 and W1.7.)*

**The consequence worth stating plainly:** with no underwriter concept anywhere in code, **every
peg-in today is the D6 unbacked path.** The D6 risk acceptance is the system's current whole
posture, not a seeded-book special case.

### 0.3 Open defects in code that exists

| | Defect | Effect |
|---|---|---|
| ~~**F7**~~ | ~~`expected_bits` is set at `initialize` and never updated~~ | **Fixed by W1.6.** The retarget is cw-144, from the node's `src/pow.cpp`, and it is verified against real data: **324/324 mainnet headers predicted exactly**. The old `bits == expected_bits` requirement is gone for every header with 147 records behind it |
| **F6** | ~~`MAX_USED = 200`~~ — **the list is removed; replay is a nullifier PDA** (P5, built). `MIN_PEG_IN` still unimplemented | **A hard ceiling of 200 peg-ins per 32-hour window**, with no attacker required |
| **C3** | `initialize` accepts any header meeting **its own** declared `bits` | The first caller picks the trusted root *and* its difficulty |
| **A6** | One bridge-wide P2PKH deposit script | The pooled reserve the design removes |
| **A5** | Program upgrade authority | Out of scope for the PoC; recorded so it is not forgotten |
| **A9/A10/A14** | No `MIN_PEG_IN`, no aggregate cap, committed depth unparsed | Unimplemented |

Full findings, including which are inherent and which merely unbuilt, are in
[`docs/10-audit-history.md`](../docs/10-audit-history.md).

### 0.4 Next, in order

1. ~~**F7**~~ — **closed.** cw-144 is implemented (`difficulty.rs`) and verified 324/324 against
   real mainnet headers (`difficulty-vectors/`). What is *not* closed: the algorithm is hard-coded,
   and BSV's own documentation says it will revert to 2016-block retargeting at some point, so the
   code needs a way to change the rule without a redeploy (recorded as X3).
2. **F6** — enforce `MIN_PEG_IN` and size or replace the replay list.
3. **The vault** — the component the rest of the design rests on.
4. **Per-relayer deposits, `owed_R`, consent** — what turns a fraud from something holders absorb
   into something a relayer is charged for.
5. **Peg-out**, then the website.

## 1. Where we are today

### 1.1 Done — the BSV primitives, validated against live chain data

The Python checker suite passes **159/159** offline, and **160/160** with a live SV Node (the live pin adds one check that needs a real node). It runs anywhere Python runs. These are the regression vectors every later implementation must match.

| Checker | Proves | Result |
|---|---|---|
| `checks/check_bsv_core.py` | Header serialisation + double-SHA256, PoW against the compact `bits` target, parent linkage, Merkle root rebuilt from real blocks, branch build/fold, odd-level duplication, tamper rejection | **20/20** — mainnet block 800000; testnet blocks with 5, 8 and 13 txs; synthetic 4- and 5-leaf trees |
| `checks/check_bsv_tx.py` | Legacy tx codec (byte-exact round-trip, txid), P2PKH parsing, `SIGHASH_FORKID` preimage + digest, **real network signatures verified against digests computed from scratch** | **51/51** — 3 real testnet txs, 12 inputs |
| `checks/check_bsv_deposit.py` | P2PKH address encoding vs real addresses, `OP_RETURN` carrying a Solana recipient, deposit tx shape, RFC-6979 signing, redemption tx shape | **17/17** |
| `checks/check_bsv_pegin.py` | **Phase 1A — complete.** A synthetic regtest chain (mining, coinbase maturity, reorg), deposit construction, the proof builder and the verifier: confirmation depth, tampering, malformed deposits, replay, odd Merkle counts, orphaned branches. Emits `fixtures/deposit_1.json` | **51/51** — the fixture is byte-deterministic across runs |
| `checks/check_bsv_node.py` | **Phase 1B — complete.** Pins our byte formats against a real SV Node: txid, the codec vs `decoderawtransaction`, the 80-byte header vs the node's raw header, the Merkle root, and the branch vs `getmerkleproof2` | **21/21 live** — passed against SV Node v1.1.1, 2026-09-26. Raw response committed as `fixtures/node_merkleproof_raw.json` |
| `adversary/attack.py` | 13 attacks against a fresh synthetic chain each, plus an honest control | **13/13 as documented** |

`checks/bsvchain.py` builds the synthetic chain; `checks/bsvlib.py` holds the single implementation of headers, PoW and Merkle folding used by the core checker, the chain builder and the node pin.

`bash checks/run_all.sh` runs everything. Without `SOLBEAM_RPC` the live pin prints `SKIPPED`; with it, and `SOLBEAM_REQUIRE_NODE=1`, a skip becomes a failure.

### 1.2 Phase 0 and Phase 1B: done

- **Phase 0 — executed.** `bootstrap.sh` ran end to end on a clean x86_64 Ubuntu droplet and installed SV Node v1.1.1, Rust 1.98.1, Solana CLI 4.1.2, Anchor 1.2.0, node and `solana-test-validator`. `doctor.sh` reports **19 ok, 1 warning, 0 failures** and exits 0.
- **Phase 1B — passed.** `check_bsv_node.py` pinned every byte format against a live SV Node in regtest. It took four corrections to get there, all of them about one RPC's wire format rather than about our own logic — see [`VERSIONS.md`](VERSIONS.md#sv-node-rpc-facts).
- **Phase 1B is idempotent against a chain that already has blocks.** It ran five times against the same node without ever resetting it, and passed on every run after the fixes. The chain height confirms the accounting exactly: 101 blocks from `regtest-up.sh`, 102 from the run that failed partway, and 114 (101 + 1 + 12) from each of the three later runs — the 545 the node reports. A pin that quietly assumed a fresh chain would have failed here.

### 1.3 Not started

- **Phase 2 — built.** The light client, the deposit verifier and the `solBSV` mint are built and verified on-chain — **20 tests** against the fixture, including a hostile-advancer suite and fork staging with reorg following. **Chainwork for the fork choice is built** (W1.7): `commit_fork` compares accumulated chainwork, not branch length. See §4.5, and §0 for the authoritative status.
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

**Passing now (51 checks, offline, no node, ~0.8 s):**

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
- Regtest has a **fixed difficulty target**, which hides how the target varies on a real chain: this is why the DAA is replayed against 324/324 real mainnet headers in `difficulty-vectors/` — see §4.2.

**Solana**

- `solana-test-validator --reset` is the regtest equivalent: a single-node local cluster.
- A funded payer keypair (`solana airdrop` against localnet).
- An Anchor workspace with the program ID pinned, so the ID is stable across resets.

**Keys and secrets**

- A regtest BSV keypair for the deposit address and one per relayer for its own float.
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
2. **A BSV light client program** — a checkpoint, a rolling window of 192 header records, and instructions to push a header and to answer "is this tx in this block". **Chainwork and time are stored per record** (52 B each): `commit_fork` compares accumulated chainwork, not branch length (§4.5).
3. **The bridge program** — deposit registry, `verify_deposit` (there is no `verify_and_mint`), the used-`(txid,vout)` set, and the authority-gated pause. **The caps are not built**: `MIN_PEG_IN`/`MAX_PEG_IN` are unimplemented (F6/A9).
4. **The advancer** — an untrusted off-chain loop that reads headers from a BSV node and submits them. The on-chain tests drive headers directly; a standalone advancer loop is not shipped.
5. **A hostile advancer** — the same loop, deliberately malformed. It exists only to drive the negative tests.

### 4.2 The DAA — **cw-144, implemented and verified** (F7 closed; X3 open)

An earlier draft called DAA "a flag, not an omission". That was wrong, and the correction was then recorded as the opposite: DAA was **actively rejected** (F7). `push_header` required `bits == expected_bits`, set once at `initialize` from the checkpoint header and never refreshed — `set_checkpoint` did not refresh it either — so on any chain whose target changes every header after a retarget was rejected `UnexpectedRetarget` and the client **halted permanently at the first difficulty change**.

**Fixed in W1.6.** The target is now computed per block by **cw-144**, the rule in the SV Node's `src/pow.cpp`, and the implementation is replayed against **324/324 real mainnet headers** with no tolerance and no fitting (`difficulty-vectors/`). Each record stores hash, cumulative chainwork and time (52 B), because the rule consumes a work difference and a time difference across a **147-record** lookback — which is what fixes the window at 192.

**The open item is X3, not F7:** the rule is hard-coded and BSV's own documentation says it will revert to 2016-block retargeting at some point, so the algorithm needs a way to change without a redeploy. Regtest's fixed target cannot exercise any of this, which is why the mainnet replay is the test that matters.

### 4.3 Test scope

> **Built vs planned.** §0 is authoritative: the built set is the light client, the token, the mint and fork staging (34 on-chain tests). Cases 2.11 (second-advancer recovery), 2.12 (caps) and 2.14 (browser wallet) describe work that is **not built**, and are marked as such in the row. Case 2.5 (DAA) is built and is covered by the mainnet replay.

| # | Case | Expected |
|---|---|---|
| 2.1 | Mint properties | 8 decimals; freeze authority absent; mint authority == the bridge PDA |
| 2.2 | Light client accepts a valid header sequence | Checkpoint + window stored; each header links to the tip, and each record carries its own cumulative chainwork and timestamp |
| 2.3 | Bad PoW / bad linkage / wrong checkpoint | Each rejected |
| 2.4 | **Rolling window bound** | A header outside the window is rejected; **rent cost measured and recorded** |
| 2.5 | **Difficulty retarget** | **Built and tested.** A valid retarget is accepted and a wrong one is rejected `UnexpectedRetarget` (fixture-level); the rule itself is replayed against **324/324 real mainnet headers** in `difficulty-vectors/`, which is the test that matters. **Not covered end to end on a chain whose difficulty actually varies** — the fixture is constant-difficulty, so the on-chain assertion exercises the no-retarget path |
| 2.6 | **Cross-implementation agreement** | The on-chain Merkle/header logic and the Python verifier accept and reject the *same* fixtures. This is the single most valuable test in the PoC — it is where two independent implementations are forced to agree |
| 2.7 | **Mint from the Phase 1 fixture** | Balance rises by exactly the deposited amount, at the right ATA, with 8 decimals |
| 2.8 | Replay the same proof | Rejected |
| 2.9 | Tampered branch / wrong `vout` / wrong amount / wrong recipient | Each rejected |
| 2.10 | **Hostile advancer** — fabricated header, out-of-order header, duplicated header | All rejected; it cannot mint, and it cannot corrupt the window |
| 2.11 | **Stalling advancer** | Chain stops advancing; **no funds are at risk**. **Not built**: a second advancer recovering the tip is a planned demo, not a shipped path |
| 2.12 | `pause` — and the caps that do not exist | Mint while paused rejected; authority-gated. **Caps are not built** — `MIN_PEG_IN`/`MAX_PEG_IN` are unimplemented (F6/A9), and burn does not exist |
| 2.13 | **Compute-unit budget** | The SPV-verify path's CU cost is measured and recorded. The design estimate is ~2,842 CU per deposit proof — **confirm or correct it** |
| 2.14 | User role, end to end | **Not built.** Planned: a browser wallet pointed at localnet sees and moves `solBSV` |

### 4.4 Acceptance and artefact

- `anchor test` green against a fresh `solana-test-validator`.
- The CU table committed alongside the program.
- The fixture from Phase 1 consumed **unmodified** — if the layout had to change, that is a Phase 1 bug and it must be fixed there.

**Phase 2 is done when** a deposit fixture produced by Phase 1 mints `solBSV` on a clean local validator, and a hostile advancer cannot forge one.

---

### 4.5 Reorg handling — **closed**

**Found by the hostile-advancer tests, not by inspection.** `push_header` requires a header to extend the current tip. That is what makes the client safe against a hostile advancer — it cannot reorder, replay, rewind or substitute a branch — but it had a second consequence:

**the client could not follow a legitimate reorg either.** A real reorg presents headers built on an older block. They were rejected, correctly, and the client stayed on the abandoned branch, stalling permanently rather than switching. A correctness gap rather than a security one — no false proof became acceptable, but minting stopped with the chain.

Three parts were needed, and all three are now built:

1. **a way to submit a competing branch** — `init_staging` / `push_fork_header` / `commit_fork` (§4.6);
2. **the replacement policy** — **strictly heavier wins, ties keep the incumbent**, so an equal-length branch cannot be used to churn the tip; a branch may fork back to any height still inside the window, and deeper than that needs a checkpoint reset, which is a governance action;
3. **what happens to already-minted deposits** — in the **shipped** program, **nothing.** The mint went straight to the depositor, no vault exists, and `solBSV` deliberately has no freeze authority, so a *released* balance cannot be reversed. (In the designed system a still-*staged* mint is burned out of the program-owned vault; a released mint still cannot be reversed.) The trade is explicit: no confiscation, at the cost of a possible unbacked mint after a reorg deeper than twelve blocks.

**Chainwork is built now, and the comparison is by work.** Each `HeaderRecord` stores the cumulative
work of the target its header declares — `work = 2^256 / (target + 1)`, summed from the checkpoint —
and `commit_fork` compares that, so a longer but lower-work branch loses. On a constant-difficulty
chain the two rules agree, which is why this was invisible on regtest; the arithmetic is what cw-144
needs anyway, since the retarget *is* a function of the work difference between two blocks.

**What is still not exercised:** a branch choice decided *by* differing work. The fixture's chain
mines every block at the same target, so the on-chain test cannot distinguish the chainwork
comparison from a length comparison. The per-header work derivation is verified against real mainnet
headers; the comparison on a varying-difficulty branch is not.

### 4.6 The branch cannot be submitted in one transaction

The first `push_fork` took the whole branch as one argument, and it cannot work. The failure is instructive:

```
RangeError: Invalid bytes for "branch_bytes": length 5760 exceeds N remaining bytes
```

That is a **sizing** error, not an encoding one. **A Solana transaction is capped at 1232 bytes.** The fixture's competing branch is 72 headers — 5,760 bytes — so it does not fit, and never could. The practical ceiling is roughly **13 headers per transaction** once signatures, accounts and instruction overhead are accounted for.

The error names the symptom and says nothing about the cause, which is why it read as a serialisation bug for a while.

**The staging area, as built.** Branch headers go up in batches of twelve — what a 1232-byte transaction actually carries — so the 72-header fixture costs six transactions rather than seventy-two:

1. `init_staging(fork_height)` creates a staging PDA and records the fork point;
2. `push_fork_header(branch_bytes)` appends up to twelve headers, validating linkage and proof of work across the batch exactly as `push_header` does, and writing nothing unless all of them check out;
3. `commit_fork()` swaps the window onto the branch if it is strictly heavier, and **closes the account**, returning the rent;
4. `abandon_staging()` closes the account without committing, also returning the rent.

**Why `abandon_staging` has to exist, and why it is not housekeeping.** `commit_fork` refuses a branch that is not strictly heavier. A refused instruction **reverts**, so the `close` constraint on its account never runs — and nothing else could close it. A staged branch that never became heavier therefore stranded its rent permanently. That is a capital leak rather than a fee, and it is exactly the kind of thing that only shows up when someone asks what the path costs to *use*, not what it costs to build. The test asserts the staging account still exists after a refused commit, then that `abandon_staging` removes it.

**The griefing decision, and why it went the way it did.** A single shared staging slot can be occupied with junk, denying legitimate reorgs to everyone. The options were per-submitter accounts, a bond on a shared slot, or accepting the contention.

**Per-submitter, no bond.** The staging PDA is seeded `[b"staging", submitter]`, so no two submitters can contend for the same slot — the problem is removed structurally rather than priced. A bond would still leave one slot to fight over and would drag in slashing machinery for what is only a denial of reorg-following. And since rent is a refundable deposit rather than a fee, an attacker creating many staging accounts costs themselves opportunity cost and harms nobody, which is a better failure mode than a slashed bond.

**Re-anchoring (P2).** `init_staging` records the hash of the block at `fork_height` **once**, and
`commit_fork` refuses unless the chain still holds that block at that height. Without it, a branch
staged on block X at height H survives a competing commit forked **below** H: that commit replaces X
with X', and the stale branch — whose first header links to X — is spliced onto X' regardless. The
window then holds `headers[..=H]` from one chain stapled to a branch from another at a broken link,
with no linkage between them — a mint forgery rather than a nuisance. (Two branches staged at the
*same* height cannot do this: `commit_fork` keeps the prefix up to and including the fork point, so
that block's hash is unchanged, and the second branch is spliced onto the same block the first was.)
The branch's first header is linked to the recorded hash, not to a fresh lookup, for the same
reason. `ForkPointMoved` is the error; re-staging is the remedy. The stale-branch test asserts its
branch is **strictly heavier** than the incumbent before it asserts `ForkPointMoved`, so a tie
cannot be what refuses it; the check was verified by removing it and watching that test fail.

**An off-by-one worth recording.** `fork_height` is the **last block the two branches share** — the common ancestor — so a branch of N headers commits at tip `fork_height + N`. The fixture's own `fork.from_height` uses the *other* convention: it is the first block of the competing branch. The original `push_fork` computed `from_height + len`, which for the fixture is 191 rather than the correct 190. **That bug was invisible because the test was skipped** — it would have failed on its first real run, which is precisely the cost of leaving a gap marked rather than closed.

### 4.7 How large the rolling window should be

The window was **64 headers** because 64 was a number that worked. Nothing justified it, and 64 headers is only about ten hours — a duration nobody chose. **The window should be sized by time, not by an arbitrary count**, and it is now **32 hours — 192 records** at BSV's ten-minute target.

**It is not 48 hours, and the reason is worth recording, because the first answer was wrong.** The window has to hold enough history for the difficulty rule to verify the *next* header: cw-144 looks back `nHeight - 144` and takes a median of three blocks at each end, so it needs **147 records** before it can check a single header. The window must therefore be **at least 147**, which rules out anything measured against a shorter horizon. Above that floor it is bounded by the account cap, and 192 is the largest that fits with real margin. **147 of the 192 are consumed by the difficulty rule itself.**

A day is still the right scale for the *reasoning*: if BSV reorganises by more than a day, the problem is not that the window should have been larger — it is that BSV is broken, and the peg has far larger problems than its header history. But 32 hours is what the arithmetic allows rather than what the argument would prefer, and the deposit deadline is set by the window, not by the argument.

> **This is the section where a wrong premise survived longest.** It previously said the record needed only a hash, because "`bits` … read once on push, dead afterwards". That is true of Bitcoin's 2016-block difficulty and false of BSV's, which recalculates **every block** (see [`W1`](../workstreams/W1-light-client-verification.md)). Chainwork and time are needed on every header, and the record is 52 bytes, not 32.

**The change was nearly a trap, and the reason is worth recording.** Solana caps account creation at **10,240 bytes**: `init` allocates the whole `LightClient` in one instruction, and exceeding the cap makes `initialize` revert. It does not degrade, truncate or warn. With the original 116-byte `HeaderRecord`, a window of this length is far past the cap, so the obvious change would have failed outright. The window is only affordable because the record was cut, then had to grow again:

| Field | Original | Now | Why |
|---|---|---|---|
| `height` | 8 | — | Derived from a single `window_start`; the window is contiguous, so a per-record height is implied |
| `hash` | 32 | **32** | Linkage, and the tip |
| `prev` | 32 | — | Linkage already uses the stored `tip_hash`; records link by position |
| `merkle_root` | 32 | — | **Redundant**: the root is a field inside the header, so the header's hash already commits to it. The claim supplies the header and the program reads the root out of it |
| `chainwork` | — | **16** | **cw-144's numerator.** The target is derived from the *work difference* between two suitable blocks; it cannot be recomputed from `bits` alone |
| `time` | 4 | **4** | The clamps are on the time difference, so it must be stored |
| `bits` | 4 | — | Still derivable from the header on push and re-checked against cw-144 |
| `nonce` | 4 | — | Never used after the proof-of-work check |

`123 + 192 × 52 = 10,107 bytes`. A **const assertion fails the build** if `LightClient::SPACE` ever exceeds the cap, so this cannot be rediscovered on testnet.

Dropping the root only holds if the claim proves its header, so `verify_deposit` checks `hash(claim.header) == record.hash` before folding the branch. **Without that check a claimant could substitute a header of its own choosing and prove anything**, which is the whole risk of the change; `refuses a claim whose header is not the canonical block` covers it. The suite is now **27 passing** (§0).

Two latent bugs on the reorg path were fixed while the fields were being reshaped: `push_fork` never advanced `window_start` when it pruned the rebuilt window, and it indexed `headers[fork_idx]` directly — which would **panic** in the one state that is genuinely empty, immediately after `initialize`.


### 5.1 What gets built

1. `burn(amount, bsv_destination)` — escrows `solBSV` into the program vault and records a redemption with a deadline.
2. **A relayer binary** — watches burns, pays BSV from **its own float**, builds the payout proof, and submits `fulfil`.
3. `fulfil(id, proof)` — verifies the payout on-chain against the requested destination and amount.
4. `refund(id)` — after the deadline, with no valid payout proven, **returns the escrow to the holder**. Supply is unchanged; the bond is **not** additionally transferred; no failure path mints.
5. `challenge(...)` / `slash(...)` — the unmatched-spend path.
6. **Bond custody**: locked `solBSV`, and an **unbonding period** longer than the deadline plus the challenge window.
7. **A deliberately misbehaving relayer** — a mode that selects an attack. This is a *testability requirement*, not a nicety: it is the only way the negative cases can be driven deterministically.

### 5.2 Test scope

| # | Case | Expected |
|---|---|---|
| 3.1 | Happy path | Redemption closes; supply decrements; BSV lands at the destination |
| 3.2 | Relayer never pays | After the deadline the escrow is **returned to the holder**; supply is unchanged; the bond is **not** additionally transferred. No failure path mints |
| 3.3 | Wrong address / underpayment | Proof cannot match the request → not fulfilled → the deadline returns the escrow |
| 3.4 | **Double-claim** — `fulfil` then `refund`, and `refund` then `fulfil` | Both orderings impossible; exactly one terminal state |
| 3.5 | **Naked spend** (no redemption outstanding) | Bounded by the float cap; **requires a challenger** to slash. Assert the bound is what the parameters claim |
| 3.6 | Naked spend, **nobody challenges** | The loss is bounded and the bond is *not* seized. **Record this as the honest residual**, with the no-idle-float rule as the mitigation |
| 3.7 | **Unbonding period** | A relayer cannot exit mid-commitment; exit succeeds only after commitments settle and the notice period elapses |
| 3.8 | **Bond denomination and deflationary slash** | A slashed theft removes supply as the reserve falls; assert backing per token does **not** fall — and rises when `bond_R > owed_R` |
| 3.9 | Reorg after payout | The 6-hour window catches it; the closed redemption's behaviour is documented |
| 3.10 | Concurrent redemptions | All settle; no double spend of a relayer's float |
| 3.11 | Relayer offline mid-flight | The deadline still protects the holder: the escrow is returned |
| 3.12 | **Round-trip invariant, after every case** | `custodied BSV ≥ outstanding solBSV` asserted by the off-chain harness as a hard failure. **D8: monitored, not enforced on-chain** — the reserve is off-chain BSV the program cannot read |

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
| **A bond in `solBSV`** | **Locked** on Solana, sized **`≥ k × owed_R` with `k = 1`** (D5) — against the liability the program measured, **including staged mints**, not against the relayer's own float (F4). Not a balance it can move |
| **Proof data** | Headers and a Merkle branch for its own payout. Its own SPV or a public API — the *source need not be trusted*, because the program verifies the proof |
| **Monitoring** | Its own float level, `bond_R` against `owed_R`, pending deadlines, missed fulfilments. A relayer that cannot see its own deadlines will miss them |
| **An exit path** | Announce, settle outstanding commitments, wait out the unbonding period |
| **A misbehaviour mode** | **Required for the PoC.** A flag that selects an attack — steal the float, vanish, underpay, pay the wrong address, refuse to unbond — so the negative tests are deterministic rather than hand-driven |
| **Two instances** | Run two relayers concurrently, to prove independence of float and bond, and that competition does not break settlement |

Note the capital consequence, which the PoC should make visible rather than hide: at **`k = 1`** a relayer locks at least its measured liability `owed_R` — **not** the float it serves, which the bond does not cover — and bonded `solBSV` **cannot be redeemed while bonded**. That locked capital, not gas, is what the fee has to cover.

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
| **Cold-reserve covenant** (cold → hot only, tranches, hot-balance cap) | Not needed, because **the design removes the pooled reserve rather than securing it** (A6): deposits pay each relayer's own BSV script, and each relayer is bonded against its own `owed_R`, so there is no single cold key or aggregated pot to protect. *Designed, not built* — the shipped program still has one bridge-wide P2PKH deposit script, the pooled reserve A6 exists to remove |
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

- **Phases 0, 1A, 1B and 2 are built.** The critical path is no longer Phase 2; it is the ordered list in §0.4 — **F6** (the replay-list ceiling), then the vault, then per-relayer deposits / `owed_R` / consent, then peg-out. (F7, the retarget, is closed; X3 remains.)
- **Chainwork is built** (W1.7) — `commit_fork` compares accumulated chainwork, stored per header. **On regtest it cannot be distinguished from height**, since every block shares one target, so the chainwork path is exercised by the fixture replay (324/324 real mainnet headers) rather than by the on-chain suite.
- **Phase 1B is done** — every byte format is pinned against a live SV Node, so the on-chain verifier has a measured target rather than an assumed one. It cost four corrections, all to one RPC's wire format; see [`VERSIONS.md`](VERSIONS.md#sv-node-rpc-facts).
- **Phase 3 depends on the vault and on the relayer**, but its negative tests can be specified now.

The one ordering rule worth enforcing: **the fixture layout is owned by Phase 1.** If Phase 2 needs it changed, the fix goes in Phase 1 so that both halves keep consuming the same bytes.

---

## 9. Effort

Indicative, one focused developer. Note that Phase 1 is new work that the earlier `P0`–`P5` list folded into other phases, and that the negative tests are now inside each phase rather than a separate block.

| Phase | Duration | Notes |
|---|---|---|
| **Phase 0** — environment, bootstrap, doctor | 2–3 days | ✅ **Done.** `doctor.sh`: 19 ok, 1 warning, 0 failures on the x86_64 droplet |
| **Phase 1A** — synthetic chain + deposit proof | 3–4 days | ✅ **Done.** |
| **Phase 1B** — real SV Node format pin | 1–2 days | ✅ **Done.** Took four live iterations, all on `getmerkleproof2`'s wire format |
| **Phase 2** — token, light client, mint, hostile advancer, fork staging | 1.5–2 weeks | ✅ **Done** — 34 on-chain tests. Chainwork comparison is built (W1.7), exercised by the mainnet fixture replay |
| **Phase 3** — burn, relayer, bond, deadline, challenge | 1–1.5 weeks | Includes the misbehaving-relayer mode |
| **Testnet repeat** (BSV testnet + Solana devnet) | 2–3 days | **Not blocked** — the retarget is implemented (W1.6, cw-144, verified 324/324). What to watch is **X3**: the rule is hard-coded, so a BSV consensus change would need a redeploy |
| **Total to a demoable PoC** | **~5–6 weeks** | |

**The trustless-mint claim alone** (Phases 0, 1 and 2, with the relayer stubbed) landed in **~2.5–3 weeks** and remains the strongest single claim in the design.

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

Next: [PoC plan](README.md) · [Phase 5 monitoring](PHASE5_MONITORING.md) · [Adversary playbook](ADVERSARY_PLAYBOOK.md) · [Trust model](../docs/05-trust-model.md)
