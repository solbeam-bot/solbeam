# SOLBEAM PoC — proof of concept on testnet

Purpose: **get the design in front of reviewers as running code**, and prove the three claims that matter. Everything else is production hardening and deliberately out of scope here.

---

## 0. What the PoC has to prove

| # | Claim | How the PoC demonstrates it |
|---|---|---|
| **T1** | **Minting needs no trusted party** | A BSV deposit is minted on Solana by submitting a **BSV header + Merkle proof**. No oracle, no attestor, no signature from SOLBEAM. Anyone can submit it |
| **T2** | **Redemption is enforced, not promised** | Burn `solBSV` → a bonded relayer pays BSV → the payout is **proved on-chain** → redemption closes. If no valid proof arrives before the deadline, the **escrow is returned to the holder** and supply is unchanged: **no failure path mints**, and the bond is not additionally transferred (it answers deliberate theft or abandonment, not a failed redemption). *Designed, not built* |
| **T3** | **Reorgs are handled** | Roll the BSV chain back on regtest (`invalidateblock`) and show the header window rolls back and a deposit from the orphaned branch is not mintable |
| **T4** | **`solBSV` is an ordinary SPL token** | 8 decimals, classic SPL, no freeze authority — tradeable on Raydium/Orca like any other token |

**Explicitly NOT in the PoC** (confirmed scope):

| Deferred | Why |
|---|---|
| **Cold-reserve covenant** — the cold→hot script that permits paying *only* the hot wallet, in staggered tranches | Not needed, because **the design removes the pooled reserve rather than securing it** (A6): deposits pay each relayer's own BSV script, and each relayer is bonded against its own `owed_R`, so there is no single reserve key to protect. *Designed, not built* — the shipped program still has one bridge-wide P2PKH deposit script, the pooled reserve A6 exists to remove |
| **Signerless peg-out** — a ZK proof of the Solana burn verified inside BSV Script | Research programme, 2–4 years |
| FROST/threshold keys, price governor, proof-of-reserves, insurance, multi-relayer competition, mainnet hardening | Not needed to prove the four claims above |

Record these as known gaps so reviewers know what they are *not* seeing.

### 0.1 Verified primitives so far

Standards-library Python checkers validate the consensus-critical BSV logic against **live chain data**, before any Go or Rust is written. They run anywhere Python runs, and they are the regression vectors the Go/Rust implementations must match.

| Checker | What it proves | Result |
|---|---|---|
| `checks/check_bsv_core.py` | 80-byte header serialisation + double-SHA256; proof-of-work against the compact `bits` target; parent linkage; Merkle root rebuilt from real blocks; branch build, fold, odd-level duplication, tamper rejection | **20/20 pass** — mainnet block 800000; testnet blocks with 5, 8 and 13 txs; synthetic 4- and 5-leaf trees |
| `checks/check_bsv_tx.py` | Legacy transaction codec (byte-exact round-trip, txid); P2PKH parsing; `SIGHASH_FORKID` preimage and digest; **real network signatures verified against digests computed from scratch** | **51/51 pass** — 3 real testnet transactions, 12 inputs |
| `checks/check_bsv_deposit.py` | P2PKH address encoding against real addresses; `OP_RETURN` carrying the Solana recipient; deposit transaction shape; **signing** with deterministic nonces (RFC 6979); redemption transaction shape | **17/17 pass** |

Run them all with `checks/run_all.sh`. Requires only Python 3 and network access.

Why this order: "is this transaction in this block?", "does this signature hash the way we think?" and "can we build the transactions at all?" are the three places where a bug is silent and fatal. All three are now pinned to real data — and the construction checker already caught a real bug in the shared codec during a refactor.

Agreed direction: **Python for all PoC testing and for the off-chain services** (decision 3 in [`TEST_PLAN.md`](TEST_PLAN.md) §10 — the production service language is deliberately left open on the evidence the PoC produces), Rust/Anchor for the Solana program. New repository, delivered by tarball until GitHub access is sorted — see `GITHUB_SETUP.md`.

---

## 1. Minimal architecture

```
     BSV regtest (or testnet)                       Solana local validator (or devnet)
   ┌──────────────────────────┐                ┌──────────────────────────────────────┐
   │  SV Node  (JSON-RPC)     │                │  solbeam (Anchor program)            │
   │   · getblockcount        │                │  BUILT:                              │
   │   · getblockhash         │                │   · initialize / set_checkpoint      │
   │   · getblockheader       │                │   · push_header(header)              │
   │   · getmerkleproof2      │                │   · verify_deposit(tx, branch, idx,  │
   │   · getrawtransaction    │                │        vout, amount, dest)           │
   │   · sendrawtransaction   │                │   · init_staging / push_fork_header  │
   └───────────┬──────────────┘                │     / commit_fork / abandon_staging  │
                │                               │  DESIGNED, NOT BUILT:                │
                │                               │   · burn / fulfil / refund / slash   │
                │            ┌──────────────────┤  solBSV  (SPL mint, 8 dp)            │
                │            │                  │   mint authority = bridge PDA         │
                │            │                  └──────────────────────────────────────┘
   ┌───────────▼────────────▼─────────────────────────────────────────────────────────┐
   │  ONE off-chain process ("the bot") — planned, not built:                          │
   │   1. advancer  — reads BSV headers, pushes them to the program                     │
   │   2. watcher   — notices deposits, builds Merkle proofs, calls verify_deposit      │
   │   3. relayer   — watches burns, pays BSV from its own float, submits payout proof  │
   └───────────────────────────────────────────────────────────────────────────────────┘
```

Decisions baked into this sketch (change them if you disagree):

- **One Anchor program, not two.** Splitting light client and bridge is production polish; one program is far less plumbing for a PoC.
- **One bot process, three loops.** Same reason.
- **Regtest first, devnet/testnet second.** Instant blocks, deterministic difficulty, free coins — you can run the whole test matrix in seconds.
- **Relayer is 1-of-1 and bonded** — enough to exercise bond/slash logic without a signer network.
- **No pooled hot wallet.** Each relayer holds its own float under a plain P2PKH, and deposits pay a relayer's own script; each is bonded against its own `owed_R` (A6). *Designed, not built* — the shipped program still has one bridge-wide deposit script. The covenant-locked cold reserve is production hardening and needs no PoC validation.

---

## 2. Where is the light client hosted? (the question)

**It isn't hosted anywhere.** There is no server and no service to operate. Three distinct things get conflated under "the light client":

| Piece | Where it lives | Who pays | Who can write to it |
|---|---|---|---|
| **The verification code** | A **Solana program** (BPF) deployed to the cluster — devnet, then mainnet | one-off deploy fee | nobody; it's code |
| **The header state** | **Solana accounts** owned by that program (a checkpoint and a rolling window of 192 header recordsshes; **no chainwork yet**) | rent, paid by whoever initialises/extends it | only via the program's instructions |
| **The advancer** | An **off-chain bot**, planned (in the PoC, the same process as the watcher/relayer) | its own SOL for fees | anyone — it is permissionless |

So the flow is: the advancer reads headers from a **BSV node's JSON-RPC** and submits them in a Solana transaction; the program checks proof-of-work, chaining and the **fixed target** before storing them. It does not yet follow a difficulty retarget (F7).

Two consequences that matter for the design review:

1. **The advancer is untrusted.** It cannot fake a header — the program rejects bad PoW/linkage. The worst it can do is stall, which is a *liveness* problem, not a safety one. Anyone can advance the chain, so the fix for a stalled advancer is "someone else's bot".
2. **On devnet there is no real cost.** On mainnet, header state costs rent, which is why production uses a **checkpoint + rolling window** rather than the whole chain. The PoC stores a 32-hour window — **192 header records**, 52 bytes each (10,094 bytes total) — to prove the mechanism; it does not attempt genesis-up sync.

The BSV node is the *only* stateful thing you host yourself, and it's a commodity: run SV Node locally in regtest for the PoC.

---

## 3. Install checklist

Pin every version in the repo once chosen. Suggested order.

### 3.1 Solana side

> **Architecture warning.** Anza/Agave publishes **`x86_64-unknown-linux-gnu` only** for Linux — there is no aarch64 Linux build, and there never has been. On an arm64 machine (like the current dev box) the Solana CLI must come from an x86_64 host or from emulation. See [`TEST_PLAN.md` §2.1](TEST_PLAN.md).

- [ ] **Rust** — `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`
- [ ] **Solana CLI** — `sh -c "$(curl -sSfL https://release.anza.xyz/stable/install)"` (check the current URL in Anza's docs; pin the version)
- [ ] **Local validator** — ships with the Solana CLI: `solana-test-validator`
- [ ] **Anchor** — `cargo install --git https://github.com/coral-xyz/anchor avm --locked --force`, then `avm install latest && avm use latest`
- [ ] **`solana-keygen`** (bundled) — create a deploy keypair + fund from `solana airdrop` on devnet
- [ ] **Node.js 20+** and a package manager — for tests/scripts (`npm` or `pnpm`)

### 3.2 BSV side

- [ ] **Docker** (easiest) *or* a build toolchain: C++20, autotools, boost, libevent, zeromq, openssl — the SV Node is a big C++ build
- [ ] **SV Node (`bitcoin-sv`)** running in **regtest**:
  ```bash
  bitcoind -regtest -server -daemon \
    -rpcuser=solbeam -rpcpassword=solbeam \
    -txindex=1 \
    -excessiveblocksize=2GB \
    -maxstackmemoryusageconsensus=100MB \
    -minminingtxfee=0.00000001
  ```
  Note `-excessiveblocksize` and `-maxstackmemoryusageconsensus` are **required** parameters on modern SV Node.
- [ ] **`bitcoin-cli`** (ships with the node) — `generatetoaddress`, `sendtoaddress`, `getmerkleproof2`, `getrawtransaction`
- [ ] A BSV regtest keypair / address for the deposit address and one per relayer for its own float

### 3.3 Off-chain services — **decided: Python** (decision 3, [`TEST_PLAN.md`](TEST_PLAN.md) §10)

- [x] **Python 3.11+** — the PoC's off-chain language: it validates every BSV-side primitive and needs no toolchain, and the production service language is deliberately left open on the evidence the PoC produces
- The earlier **Go / TypeScript / Rust** options are recorded in [`TEST_PLAN.md`](TEST_PLAN.md) §10.1 and are *not* the PoC's choice

### 3.4 Nice to have

- [ ] `just` or `make` for task running
- [ ] GitHub Actions later (not for the PoC)
- [ ] A throwaway keypair file and an `.env.example`; **never commit keys**

---

## 4. Suggested repo layout

```
solbeam-poc/
├── programs/solbeam/          # Anchor program: headers, Merkle, mint, burn, redeem
│   └── src/lib.rs
├── tests/                     # Anchor/TS integration tests
├── services/bot/              # advancer + watcher + relayer (one process, three loops)
├── scripts/
│   ├── bsv-regtest-up.sh      # start SV Node, generate blocks, fund wallets
│   ├── solana-up.sh           # solana-test-validator --reset, deploy, init checkpoint
│   └── demo.sh                # the end-to-end demo (see §7)
├── .env.example
└── README.md                  # trust assumptions + known gaps
```

---

## 5. Task list (ordered, with acceptance criteria)

> **Superseded.** [`TEST_PLAN.md`](TEST_PLAN.md) restructures the work into Phase 0–3 and replaces the `P0`–`P5` list below, which is kept only for history. Its test matrices are a superset of §6 here.

### P0 — Environment (~2–3 days)

- [ ] SV Node running in regtest; `generatetoaddress 101` gives spendable coins
- [ ] `solana-test-validator` running; a funded payer keypair
- [ ] Anchor program skeleton builds and deploys to the local validator
- [ ] A trivial instruction round-trips from a test script
- **Done when:** `./scripts/demo.sh` can start both chains, deploy, and ping the program

### P1 — BSV light client on Solana (~1–2 weeks, the long pole)

- [ ] `initialize` / `set_checkpoint` — accept a checkpoint, check chaining and PoW against the target, store the 192-hash window. **Chainwork is not stored**
- [ ] `push_header(header)` — append one header, verify `prev_hash` links to the tip, verify PoW
- [ ] Rolling window: keep **192** records, prune older ones (the built storage model)
- [ ] Merkle verification: given txid, branch, index and height, fold to the stored root
- [ ] Reorg handling: **built as fork staging** — `init_staging` / `push_fork_header` / `commit_fork` / `abandon_staging`, strictly-heavier commit, not a walk-back-and-replace
- [ ] Off-chain `advancer` pushes real regtest headers every block
- **DAA is not skipped by choice (F7):** regtest has a **fixed difficulty target**, which hides the defect. `check_daa` has **no caller**, `push_header` requires `bits == expected_bits`, and `expected_bits` is never refreshed — so the client **halts permanently at the first retarget** on testnet or mainnet. Implementing the retarget is required before testnet, not a follow-up flag
- **Done when:** a BSV tx in a regtest block can be proven to have happened, entirely on-chain, with no oracle

### P2 — Peg in: mint (~1 week)

- [ ] SPL `solBSV` mint created; mint authority = bridge PDA
- [ ] Deposit address (P2PKH) generated for the test user; recipient Solana ATA recorded
- [ ] Deposit transaction carries the Solana recipient in an **`OP_RETURN`**
- [ ] `verify_deposit(...)` — verifies inclusion + amount + destination, rejects replays, mints to the ATA. (The built instruction is `verify_deposit`; there is no `verify_and_mint`)
- [ ] Watcher loop: detect the deposit, wait for N confirmations, build the proof, submit the mint
- **Done when:** send regtest BSV → `solBSV` appears in the Solana wallet, with no trusted step in between

### P3 — Peg out: redemption (~1 week)

- [ ] `burn(amount, bsv_destination)` — escrows `solBSV` into the program vault and records a redemption with a deadline
- [ ] Relayer loop: pay the destination from **its own float**, then submit the payout proof
- [ ] `fulfil(id, proof)` — verify the payout on-chain against the requested destination and amount
- [ ] `refund(id)` — after the deadline, if no valid payout is proven, **return the escrow to the holder**; supply is unchanged and the bond is not additionally transferred. No failure path mints
- [ ] Bond accounting for a 1-of-1 relayer — sized `bond_R ≥ k × owed_R` with `k = 1` (D5), including staged mints
- **Done when:** burn → BSV lands at the destination → the redemption closes on-chain; and a deliberately skipped payout **returns the escrow to the holder** (supply unchanged)

### P4 — Negative and adversarial tests (~2–3 days)

See the matrix in §6. **This is the part reviewers will care about most** — the happy path proves little.

### P5 — Public testnet / devnet run (~2–3 days)

- [ ] Repeat P2/P3 against **BSV testnet** + **Solana devnet**
- [ ] Implement the **difficulty retarget** (F7) — required, not optional: the client halts permanently at the first retarget. Confirmations are already fixed at 12
- [ ] Publish the demo recording / transaction links for review

---

## 6. Peg-out test matrix

| # | Scenario | Expected result |
|---|---|---|
| 1 | Happy path: burn → relayer pays → proof submitted | Redemption closes; supply decrements; BSV at the destination |
| 2 | Relayer never pays | After the deadline the escrow is **returned to the holder**; supply unchanged; the bond is **not** additionally transferred. No failure path mints |
| 3 | Relayer pays the **wrong address** | The proof cannot match the requested destination → not fulfilled → the deadline returns the escrow |
| 4 | Relayer **underpays** | Same as 3 — amount mismatch rejects the proof |
| 5 | **Replay**: same payout proof submitted twice | Second submission rejected |
| 6 | **Double-claim**: fulfil then refund | Refund rejected once fulfilled; fulfil rejected once refunded |
| 7 | **Reorg after payout** (regtest `invalidateblock`) | Header window rolls back; the closed redemption is flagged — document the behaviour (known gap: no automatic un-close) |
| 8 | **Unauthorised float spend** | Challenger submits the offending tx + proof; the bond is seized **only if challenged** (the honest residual) |
| 9 | **Concurrent redemptions** | All settle; no double spend of a relayer's float |
| 10 | Relayer offline mid-flight | The deadline still protects the holder: the escrow is returned |
| 11 | **Round-trip invariant** | After every scenario: `custodied BSV ≥ outstanding solBSV`, asserted by the off-chain harness (D8 — monitored, not enforced by the program) |
| 12 | Mint with a **tampered Merkle branch** | Rejected |
| 13 | Mint with a **header not in the window** | Rejected |
| 14 | Deposit from an **orphaned branch** | Not mintable |

---

## 7. The demo (what you show a reviewer)

`./scripts/demo.sh` should, unattended:

1. Start BSV regtest + Solana local validator; deploy the program; create the mint; fund wallets.
2. **Mint:** send 1 BSV to the deposit address with the Solana recipient in `OP_RETURN` → print the Solana balance showing `1 solBSV`.
3. **Redeem:** burn `0.4 solBSV` to a fresh BSV address → print the BSV balance at that address and the closed redemption.
4. **Failed redemption:** burn `0.4 solBSV` to a destination the relayer refuses to pay → wait out the deadline (short in regtest) → print the **returned escrow** and the unchanged supply. Nothing is minted; the bond is not transferred.
5. **Reorg:** deposit, mint, then `invalidateblock` and show the proof no longer verifies.
6. Print the final invariant check (off-chain — **D8: monitored, not enforced by the program**).

That script is the artefact to share for feedback — it makes the trust model concrete.

---

## 8. Questions for you

> **Several of these are settled elsewhere.** The host, deposit payload, off-chain language, user surface and time scale are decisions 1–5 in [`TEST_PLAN.md`](TEST_PLAN.md) §10; the token details (Q5) and the non-goals (Q6) are confirmed in §3.1/§4.1 and §7 of that plan. The rest remain open.

1. **Chain for P1–P4: regtest (recommended — instant, deterministic) or straight to public BSV testnet?** Regtest lets the full matrix run in seconds; testnet is more realistic but exposes the retarget defect (F7) immediately and has 10-minute blocks.
2. **Off-chain stack: Go, TypeScript, or Rust?** **Settled for the PoC: Python** (decision 3, [`TEST_PLAN.md`](TEST_PLAN.md) §10). The production language is deliberately left open.
3. **Peg-out scope for the PoC:** real 1-of-1 bond + slash (recommended — proves the enforcement path) or a trusted relayer stub so the light client gets all the attention?
4. **Where do you want the PoC to run for the demo** — your machine only, or should it deploy to devnet so reviewers can poke at it themselves?
5. **Confirm the token details:** name `solBSV`, 8 decimals, classic SPL, no freeze authority, mint authority = bridge PDA.
6. **Confirm the PoC does *not* need** the cold-reserve covenant or FROST — the pooled reserve is removed by design rather than secured (A6), and those are production-hardening
7. **Who runs the advancer/relayer for the demo**, and do you want it visible (e.g., a status page) so reviewers can see it is permissionless?
8. **Repo:** should the PoC live in the existing `adamski-t/solbeam` repo (e.g. `poc/`) or a new one?

---

## 9. Effort (indicative, one focused developer)

| Phase | Duration |
|---|---|
| P0 environment | 2–3 days |
| P1 light client (**built**; real chainwork outstanding) | 1–2 weeks |
| P2 mint | ~1 week |
| P3 peg-out | ~1 week |
| P4 negative tests | 2–3 days |
| P5 testnet (requires the retarget fix, F7) | 2–3 days |
| **Total to a demoable PoC** | **~4–5 weeks** |

Mint-only (P0–P2, with the relayer stubbed) landed in **~2 weeks** and is already enough to show the strongest claim: trustless minting.

## Licence

MIT — see `LICENSE`. Everything here is intended to be usable, forkable and
reviewable by anyone.

The token ticker is **`solBSV`** (display name SOLBEAM).
