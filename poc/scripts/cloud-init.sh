#!/usr/bin/env bash
#
# SOLBEAM — cloud-init user-data for the PoC box (DigitalOcean, or any provider).
#
# Paste this whole file into the droplet's "User Data" / "Startup scripts"
# field. It runs once, on first boot, as root. It is written to be safe in that
# environment, which the four bare commands are not:
#
#   * re-execs the work as the login user, because every installer writes into
#     $HOME and root's $HOME is not yours
#   * waits for apt/dpkg to be free, which is the usual cause of a first-run
#     failure with "Could not get lock /var/lib/dpkg/lock-frontend"
#   * waits for the network before assuming it is there
#   * clones over HTTPS — the repo is public, so no key is needed. Only pushing
#     needs a key; see RUNNING.md
#   * leaves a marker file, so success and failure are both obvious
#
# Watch it with:
#     tail -f /var/log/solbeam-startup.log
#     ls -l /home/ubuntu/SOLBEAM_*
#
# When it finishes you want:
#     /home/ubuntu/SOLBEAM_READY                       (marker)
#     /home/ubuntu/solbeam/poc/fixtures/node_merkleproof_raw.json
#
# That last file is the Phase 1B artefact: the real shape of getmerkleproof2.

set -uo pipefail

TARGET_USER="${SOLBEAM_USER:-ubuntu}"
TARGET_HOME="$(getent passwd "$TARGET_USER" | cut -d: -f6)"
[ -n "$TARGET_HOME" ] || TARGET_HOME="/home/$TARGET_USER"
LOG=/var/log/solbeam-startup.log
REPO_URL="${SOLBEAM_REPO_URL:-https://github.com/solbeam-bot/solbeam.git}"
RPC_PORT="${SOLBEAM_RPCPORT:-18443}"

exec >"$LOG" 2>&1
echo "=== solbeam startup begin $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "    user=$TARGET_USER home=$TARGET_HOME"
echo "    repo=$REPO_URL"

fail() {
  echo "!!! $1"
  touch "$TARGET_HOME/SOLBEAM_FAILED"
  chown "$TARGET_USER:$TARGET_USER" "$TARGET_HOME/SOLBEAM_FAILED" 2>/dev/null || true
  echo "=== solbeam startup FAILED $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  exit 1
}

# -- 1. network --------------------------------------------------------------
echo "--- waiting for network"
for i in $(seq 1 60); do
  if curl -sS -o /dev/null -m 8 https://api.github.com 2>/dev/null; then
    echo "    ok"
    break
  fi
  [ "$i" = "60" ] && fail "no network after 5 minutes"
  sleep 5
done

# -- 2. let apt settle -------------------------------------------------------
# NOT 'cloud-init status --wait': this script is itself run by cloud-init's
# final stage, so waiting on cloud-init to finish would wait on ourselves.
echo "--- waiting for apt/dpkg to be free"
for i in $(seq 1 120); do
  busy=0
  for p in unattended-upgr apt-get apt dpkg; do
    pgrep -x "$p" >/dev/null 2>&1 && busy=1
  done
  [ "$busy" = "0" ] && break
  [ "$i" = "120" ] && echo "    still busy after 10 min — proceeding; apt will wait on the lock itself"
  sleep 5
done
echo "    clear"

# -- 3. the actual work, as the login user -----------------------------------
echo "--- running as $TARGET_USER"

sudo -u "$TARGET_USER" -H bash -lc '
set -uo pipefail
cd "$HOME" || exit 1

if [ ! -d solbeam/.git ]; then
  echo "--- cloning"
  git clone "$1" solbeam || exit 1
fi
cd solbeam || exit 1
git pull --ff-only 2>/dev/null || true

echo "--- bootstrap (this is the slow one)"
./poc/scripts/bootstrap.sh || { echo "!!! bootstrap failed"; exit 1; }

echo "--- doctor"
./poc/scripts/doctor.sh || { echo "!!! doctor failed"; exit 1; }

echo "--- regtest node"
./poc/scripts/regtest-up.sh || { echo "!!! regtest-up failed"; exit 1; }

export SOLBEAM_RPC="http://127.0.0.1:$2"
export SOLBEAM_RPC_USER="${SOLBEAM_RPC_USER:-solbeam}"
export SOLBEAM_RPC_PASS="${SOLBEAM_RPC_PASS:-solbeam}"
export SOLBEAM_REQUIRE_NODE=1

echo "--- the suite, with the live pin"
bash poc/checks/run_all.sh || { echo "!!! suite failed"; exit 1; }
' _ "$REPO_URL" "$RPC_PORT"
rc=$?

# -- 4. marker ---------------------------------------------------------------
if [ "$rc" -eq 0 ]; then
  touch "$TARGET_HOME/SOLBEAM_READY"
  echo "=== solbeam startup OK $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  echo "    fixtures:"
  ls -l "$TARGET_HOME/solbeam/poc/fixtures/" 2>/dev/null || true
  if [ -f "$TARGET_HOME/solbeam/poc/fixtures/node_merkleproof_raw.json" ]; then
    echo "    ^ node_merkleproof_raw.json is the artefact we wanted."
    echo "      Commit it, or paste it back — it pins the node's response shape."
  else
    echo "    NOTE: no node_merkleproof_raw.json. The live pin did not run;"
    echo "          check the suite output above."
  fi
else
  fail "work step exited $rc"
fi

chown "$TARGET_USER:$TARGET_USER" "$TARGET_HOME"/SOLBEAM_* 2>/dev/null || true
exit 0
