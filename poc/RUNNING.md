# Running the PoC on your own machine

Everything below is copy-paste. A fresh clone to a working result takes about a minute, and most of it needs **only Python 3.11+**.

---

## 1. Thirty-second start

```bash
git clone git@github.com:solbeam-bot/solbeam.git
cd solbeam

# 1. The checker suite: 154 checks, plus 13 adversarial plays
bash poc/checks/run_all.sh
```

You should see, in order: `20/20`, `51/51`, `17/17`, `48/48`, `18/18`, `13/13`, then **`ALL CHECKERS PASSED`**.

```bash
# 2. Is this machine able to run the PoC?
bash poc/scripts/doctor.sh 1a

# 3. Try to break it yourself
python3 poc/adversary/attack.py --list        # what you can attempt
python3 poc/adversary/attack.py --all         # attempt all 13
python3 poc/adversary/attack.py orphan        # or one, with the reasoning printed
```

All of these work from **any** directory — they resolve their own paths.

---

## 2. What you are looking at

| Command | What it proves |
|---|---|
| `check_bsv_core.py` | Header serialisation, proof of work, Merkle roots and branches — validated against **live mainnet and testnet blocks** |
| `check_bsv_tx.py` | Transaction codec and `SIGHASH_FORKID` — validated against **real network signatures** |
| `check_bsv_deposit.py` | Addresses, `OP_RETURN` payloads, and signing both transaction shapes |
| `check_bsv_pegin.py` | **Phase 1A.** A whole deposit path on a generated regtest chain, ending in a portable mint instruction |
| `check_bsv_node.py --selftest` | Phase 1B's harness, exercised without a node |
| `attack.py --all` | Thirteen attacks, each either rejected with the code the design claims, or `ACCEPTED` when that is the correct answer |

**Two of them reach the network** (`check_bsv_core.py`, `check_bsv_tx.py`) because they validate against live chain data. Everything else is offline. With no network they report **failures**, rather than silently passing — which is deliberate.

---

## 3. Reading the results

- **`PASS`** means the behaviour matched what the design claims.
- **`FAIL`** means it did not. In the adversary plays that is a **finding, not a broken test** — if you make the system do something it says it cannot do, that is the single most useful thing that can happen. Write it in the table in [`ADVERSARY_PLAYBOOK.md`](ADVERSARY_PLAYBOOK.md) §7 and commit it.
- **`SKIP`** on the live node pin is expected unless an SV Node is running. The suite stays green, because there is nothing to pin against.

The one result worth doing by hand: change the confirmation depth in `poc/checks/bsvchain.py` (`CONFIRMATIONS_REQUIRED`) and re-run. Watching `early` flip is a better way to understand that parameter than reading about it.

---

## 4. What needs the x86_64 host

Two things do not run on an ARM machine or without a toolchain, and both say so plainly rather than half-working:

| Need | Why |
|---|---|
| **The live SV Node pin** (Phase 1B) | Needs `bitcoind`/`bitcoin-cli` from an SV Node. `regtest-up.sh` refuses to run against Bitcoin Core, which cannot pin what we need |
| **Phases 2–3** | Need the Solana CLI, and Agave publishes no aarch64 Linux build — see [`VERSIONS.md`](VERSIONS.md) |

On the x86_64 VM:

```bash
poc/scripts/bootstrap.sh          # installs everything, writes VERSIONS.lock
poc/scripts/doctor.sh             # all phases; must exit 0
poc/scripts/regtest-up.sh         # SV Node in regtest, 101 blocks mined

export SOLBEAM_RPC=http://127.0.0.1:18443
export SOLBEAM_RPC_USER=solbeam
export SOLBEAM_RPC_PASS=solbeam

bash poc/checks/run_all.sh        # now the live pin runs too, not SKIPPED
```

The first live run records the raw `getmerkleproof2` response to `poc/fixtures/node_merkleproof_raw.json`. **Commit that file** — it turns the node's response shape into part of the repo instead of something someone remembers.

### Is the node running?

```bash
poc/scripts/regtest-up.sh status
```

That prints the chain, the block height and the hash rate, or tells you plainly that the node is not running. It needs no environment variables — it uses the datadir and RPC credentials it wrote when it started the node.

If you would rather ask directly:

```bash
pgrep -x bitcoind >/dev/null && echo running || echo "not running"

bitcoin-cli -regtest -datadir="$HOME/.solbeam/regtest" \
  -rpcuser=solbeam -rpcpassword=solbeam getblockcount
```

**It does not restart itself.** `regtest-up.sh` starts `bitcoind` with `-daemon`, not as a systemd service, so a reboot or a stop leaves it down. `regtest-up.sh` is idempotent — running it again starts the node and leaves the existing chain alone.

**Phase 2 does not need it.** The light client consumes `fixtures/deposit_1.json`, a static file. The node is only needed to re-run the Phase 1B pin or to generate fresh proofs from real blocks.

To make a skipped pin a hard failure (what CI on the VM should do):

```bash
export SOLBEAM_REQUIRE_NODE=1
```

### Automating it: cloud-init

On DigitalOcean, paste [`scripts/cloud-init.sh`](scripts/cloud-init.sh) into **Additional Options → Startup scripts** on the droplet creation page. That field *is* the user-data field — the docs say "Enable **Startup scripts** and add your user data in the box that appears", so the UI label and the API's `user_data` are the same mechanism. You can save it and reuse it on later droplets; you cannot edit it after the droplet exists.

```bash
cat poc/scripts/cloud-init.sh        # read it before you paste it
```

It is safe in that environment in the ways the bare four commands are not:

- it **re-execs as the login user**, because every installer writes into `$HOME` and root's `$HOME` is not yours
- it waits for **apt/dpkg to be free** — unattended-upgrades holding the lock is the single most common cause of a first-run failure
- it waits for the network, rather than assuming it
- it clones over **HTTPS**, so no key is needed on a fresh box

Then watch it:

```bash
tail -f /var/log/solbeam-startup.log
ls -l /home/ubuntu/SOLBEAM_*
```

> The script redirects its own output, so cloud-init's `/var/log/cloud-init-output.log` looks quiet after the first line. That's expected — `solbeam-startup.log` is the one you want.

Success leaves `SOLBEAM_READY`; failure leaves `SOLBEAM_FAILED`. When it works you get `poc/fixtures/node_merkleproof_raw.json` — the Phase 1B artefact — and the log tells you so explicitly.

> **A bug this exercise found.** The first version of `bootstrap.sh` would have failed its own final `doctor.sh` check on any fresh machine: the Solana installer puts its binaries on `PATH` by editing `~/.profile`, which the *running* shell never re-reads. So the install succeeded and then every subsequent step could not find the tools it had just installed. It now exports the install locations for the current run **and** persists them for future shells. This is worth knowing because a startup script would have hit the same wall, with far less visible output.

---

## 5. If something looks wrong

| Symptom | Cause |
|---|---|
| `doctor.sh` warns about `aarch64` | Expected on ARM. Phase 1A is fine; 1B–3 are not |
| `check_bsv_core.py` fails on "at least one real multi-tx block verified" | No network, or WhatsOnChain is unreachable. It is a required check, so it fails loudly |
| `check_bsv_node.py` prints `SKIP` | No `SOLBEAM_RPC` set. Expected unless a node is running |
| `bootstrap.sh` exits 1 immediately | Not an x86_64 host. It refuses rather than half-installing |
| `bootstrap.sh` exits 2 | Unknown argument. `--dry-run`, `--skip-solana`, `--skip-svnode` are the options |

---

## 6. Where to go next

| You want to | Read |
|---|---|
| Understand what is proven and what is not | [`TEST_PLAN.md`](TEST_PLAN.md) §1 |
| Attack it yourself, deliberately | [`ADVERSARY_PLAYBOOK.md`](ADVERSARY_PLAYBOOK.md) |
| Know what the website will eventually publish | [`PHASE5_MONITORING.md`](PHASE5_MONITORING.md) |
| Know exactly what is pinned and why | [`VERSIONS.md`](VERSIONS.md) |
| Understand the trust model itself | [`../docs/04-trust-model.md`](../docs/04-trust-model.md) |
