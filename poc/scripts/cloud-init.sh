#!/usr/bin/env bash
#
# SOLBEAM — cloud-init user-data for the PoC box (DigitalOcean, or any provider).
#
# WHERE THIS GOES ON DIGITALOCEAN
#
#   Droplet creation page -> Additional Options -> "Startup scripts" -> paste
#   the whole file. That field IS the user-data field: DigitalOcean's docs say
#   "Enable Startup scripts and add your user data in the box that appears", so
#   the UI label and the API's `user_data` are the same mechanism.
#
#   You can save it and reuse it on later droplets. You cannot edit it after the
#   droplet exists.
#
# HOW IT BEHAVES
#
# Runs once, on first boot, as root — cloud-init's rules. It:
#
#   * picks the right account to install for (see below), because every
#     installer writes into $HOME and root's $HOME may not be yours
#   * waits for apt/dpkg to be free, the usual cause of a first-run failure
#     with "Could not get lock /var/lib/dpkg/lock-frontend"
#   * checks the network before assuming it is there
#   * clones over HTTPS — the repo is public, so no key is needed
#   * leaves a marker, so success and failure are both obvious
#
# WHICH ACCOUNT
#
# DigitalOcean's Ubuntu images log you in as ROOT and do not create an 'ubuntu'
# user — that is AWS's convention. The first version of this script assumed
# 'ubuntu', so on a DO droplet it tried `sudo -u ubuntu`, failed immediately,
# and did nothing at all. It now looks for a conventional cloud user and falls
# back to the current account. Set SOLBEAM_USER to override.
#
# WATCHING IT
#
#   tail -f /var/log/solbeam-startup.log     <- everything goes here
#   ls -l "$HOME"/SOLBEAM_*
#
#   The script redirects its own output, so cloud-init's own
#   /var/log/cloud-init-output.log looks quiet after the first line. Expected.
#
# WHEN IT FINISHES YOU WANT
#
#   $HOME/SOLBEAM_READY
#   $HOME/solbeam/poc/fixtures/node_merkleproof_raw.json
#
# That last file is the Phase 1B artefact: the real shape of getmerkleproof2.

set -uo pipefail

# Refuse to run interactively. This script is user-data: it re-execs, waits for
# apt, and runs a ~10-minute bootstrap, with all its output going to a log
# rather than the terminal. Run by hand it looks like it has hung.
if [ -t 0 ] && [ "${SOLBEAM_FORCE:-0}" != "1" ]; then
  cat >&2 <<'EOF'

cloud-init.sh is user-data. It is meant to be pasted into your provider's
"Startup scripts" / User Data field, not run by hand — interactively it looks
like it hangs, because everything goes to /var/log/solbeam-startup.log.

Run these instead:

    ./poc/scripts/bootstrap.sh      # installs everything, ~10 min
    ./poc/scripts/doctor.sh         # must exit 0
    ./poc/scripts/regtest-up.sh     # SV Node in regtest
    bash poc/checks/run_all.sh      # the suite, with the live pin

To run this anyway:  SOLBEAM_FORCE=1 ./poc/scripts/cloud-init.sh

EOF
  exit 2
fi

LOG=/var/log/solbeam-startup.log
REPO_URL="${SOLBEAM_REPO_URL:-https://github.com/solbeam-bot/solbeam.git}"
RPC_PORT="${SOLBEAM_RPCPORT:-18443}"

exec >"$LOG" 2>&1

echo "=== solbeam startup begin $(date -u +%Y-%m-%dT%H:%M:%SZ)"
echo "    running as: $(id -un)"

# -- pick the account to install for ----------------------------------------
# Prefer an explicitly requested user; otherwise a conventional cloud account if
# one exists; otherwise the account we are running as (root, on DigitalOcean).
TARGET_USER="${SOLBEAM_USER:-}"
if [ -z "$TARGET_USER" ]; then
  for candidate in ubuntu debian admin ec2-user centos; do
    if getent passwd "$candidate" >/dev/null 2>&1; then
      TARGET_USER="$candidate"
      break
    fi
  done
fi
if [ -z "$TARGET_USER" ]; then
  TARGET_USER="$(id -un)"
  echo "    note: no conventional cloud user found, so installing for '$TARGET_USER'"
  echo "          everything will land under $HOME, which is /root on DigitalOcean."
fi

TARGET_HOME="$(getent passwd "$TARGET_USER" | cut -d: -f6)"
[ -n "$TARGET_HOME" ] || TARGET_HOME="/home/$TARGET_USER"

if [ ! -d "$TARGET_HOME" ]; then
  echo "    WARNING: $TARGET_HOME does not exist; creating it"
  mkdir -p "$TARGET_HOME" || true
fi

echo "    user=$TARGET_USER home=$TARGET_HOME"
echo "    repo=$REPO_URL"

fail() {
  echo "!!! $1"
  touch "$TARGET_HOME/SOLBEAM_FAILED" 2>/dev/null || true
  chown "$TARGET_USER:$TARGET_USER" "$TARGET_HOME/SOLBEAM_FAILED" 2>/dev/null || true
  echo "=== solbeam startup FAILED $(date -u +%Y-%m-%dT%H:%M:%SZ)"
  exit 1
}

# -- 1. network --------------------------------------------------------------
#
# This runs ON the droplet, so it uses the droplet's internet access, not your
# laptop's. A DigitalOcean droplet reaches github.com with no configuration and
# no key — the repo is public.
#
# The two hosts are probed separately so a failure says which one it is:
# "no network" and "GitHub unreachable from this droplet" need different fixes.
echo "--- checking network"
STATE=""
for i in $(seq 1 60); do
  if curl -sS -o /dev/null -m 8 https://api.github.com 2>/dev/null; then
    STATE="github"
    echo "    ok — github.com is reachable"
    break
  fi
  if curl -sS -o /dev/null -m 8 https://www.cloudflare.com 2>/dev/null; then
    STATE="no-github"
    echo "    network is up, but github.com is NOT reachable from this droplet"
    break
  fi
  sleep 5
done

case "$STATE" in
  github) : ;;
  no-github)
    fail "the droplet has internet, but cannot reach github.com, so the repo cannot be cloned.
    Check for a firewall rule or a region restriction. If GitHub is genuinely
    blocked, clone the repo somewhere else and copy it up as a tarball — every
    other step depends on having it. See RUNNING.md." ;;
  *)
    fail "no network after 5 minutes" ;;
esac

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
  [ "$i" = "120" ] && echo "    still busy after 10 min — proceeding; apt waits on the lock itself"
  sleep 5
done
echo "    clear"

# -- 3. the work -------------------------------------------------------------
# Kept in a variable so it can be run either directly (when we are already the
# target account, which is the DigitalOcean case) or via sudo.
WORK='
set -uo pipefail
cd "$HOME" || exit 1

if [ ! -d solbeam/.git ]; then
  echo "--- cloning"
  git clone "$1" solbeam || exit 1
fi
cd solbeam || exit 1
git pull --ff-only 2>/dev/null || true

echo "--- bootstrap (the slow one, ~10 min)"
./poc/scripts/bootstrap.sh  || { echo "!!! bootstrap failed";  exit 1; }

echo "--- doctor"
./poc/scripts/doctor.sh     || { echo "!!! doctor failed";     exit 1; }

echo "--- regtest node"
./poc/scripts/regtest-up.sh || { echo "!!! regtest-up failed"; exit 1; }

export SOLBEAM_RPC="http://127.0.0.1:$2"
export SOLBEAM_RPC_USER="${SOLBEAM_RPC_USER:-solbeam}"
export SOLBEAM_RPC_PASS="${SOLBEAM_RPC_PASS:-solbeam}"
export SOLBEAM_REQUIRE_NODE=1

echo "--- the suite, with the live pin"
bash poc/checks/run_all.sh  || { echo "!!! suite failed"; exit 1; }
'

echo "--- running the work as $TARGET_USER"
if [ "$TARGET_USER" = "$(id -un)" ]; then
  bash -lc "$WORK" _ "$REPO_URL" "$RPC_PORT"
  rc=$?
else
  sudo -u "$TARGET_USER" -H bash -lc "$WORK" _ "$REPO_URL" "$RPC_PORT"
  rc=$?
fi

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
