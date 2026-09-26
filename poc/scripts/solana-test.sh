#!/usr/bin/env bash
#
# SOLBEAM — build and test the Phase 2 Solana program.
#
#   ./solana-test.sh                pull, sync keys, install, build, test
#   ./solana-test.sh --build-only   stop after a successful build
#   ./solana-test.sh --skip-pull    do not touch git at all
#   ./solana-test.sh --help
#
# Why a script rather than four commands: three of the four steps have a
# non-obvious failure mode, and one of them is a git trap that has already cost
# a round trip. See the notes under each step.

set -uo pipefail

DO_PULL=1
DO_TEST=1

for a in "$@"; do
  case "$a" in
    --build-only) DO_TEST=0 ;;
    --skip-pull)  DO_PULL=0 ;;
    -h|--help)    sed -n '2,12p' "$0"; exit 0 ;;
    *) printf 'unknown argument: %s\n' "$a" >&2; exit 2 ;;
  esac
done

say()  { printf '\n== %s\n' "$1"; }
info() { printf '   %s\n' "$1"; }
warn() { printf '   WARN: %s\n' "$1"; }
die()  { printf '\nERROR: %s\n' "$1" >&2; exit 1; }
have() { command -v "$1" >/dev/null 2>&1; }

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
POC="$(cd "$HERE/.." && pwd)"
ROOT="$(cd "$POC/.." && pwd)"
WS="$POC/solana"

# ---------------------------------------------------------------------------
# preflight
# ---------------------------------------------------------------------------
say "preflight"

for tool in anchor cargo; do
  if ! have "$tool"; then
    die "$tool is not on PATH.

  If you just installed it, this shell has not re-read ~/.profile:
      source ~/.profile
  Or open a new session. If it is genuinely missing, run:
      poc/scripts/bootstrap.sh"
  fi
done

if have yarn; then
  PKG=yarn
elif have npm; then
  PKG=npm
else
  die "need yarn or npm for the test dependencies"
fi

info "anchor:   $(anchor --version 2>/dev/null | head -1)"
info "cargo:    $(cargo --version 2>/dev/null | head -1)"
info "deps via: $PKG"
info "repo:     $ROOT"

[ -d "$WS" ] || die "no workspace at $WS
  The repo has probably not been pulled yet. Run without --skip-pull."

# ---------------------------------------------------------------------------
# git — and the trap that already bit once
# ---------------------------------------------------------------------------
say "git"

if [ "$DO_PULL" = "0" ]; then
  info "skipped (--skip-pull)"
else
  cd "$ROOT" || die "cannot enter $ROOT"

  # `check_bsv_node.py` WRITES poc/fixtures/node_merkleproof_raw.json, and the
  # repo also TRACKS that file now. So on a machine that has run the live pin,
  # there is an untracked copy in the way, and `git pull` refuses:
  #
  #   error: The following untracked working tree files would be overwritten
  #          by merge: poc/fixtures/node_merkleproof_raw.json
  #
  # It is a generated artefact and the committed copy is identical, so move it
  # aside rather than deleting anything — and say so, because silently
  # discarding a file someone might be looking at is not on.
  STALE="$POC/fixtures/node_merkleproof_raw.json"
  if [ -f "$STALE" ] && ! git ls-files --error-unmatch "$STALE" >/dev/null 2>&1; then
    warn "$STALE is untracked, but the repo now tracks it."
    warn "git would refuse to merge. Moving it to ${STALE##*/}.local-backup"
    mv "$STALE" "$STALE.local-backup" || die "could not move it aside"
  fi

  if ! git pull --ff-only; then
    die "git pull failed.

  Most likely a local change to a tracked file. Sort it out, then re-run with
  --skip-pull to build from what is on disk."
  fi
  info "now at $(git rev-parse --short HEAD) — $(git log -1 --format=%s)"
fi

# ---------------------------------------------------------------------------
# the workspace
# ---------------------------------------------------------------------------
cd "$WS" || die "cannot enter $WS"

say "anchor keys sync"
info "rewrites declare_id! in lib.rs and the program id in Anchor.toml"
info "with YOUR real keypair — that change is meant to be committed"
anchor keys sync || die "anchor keys sync failed"

say "installing test dependencies ($PKG)"
# Anchor 1.x renamed the TS package to @anchor-lang/core. If this step cannot
# resolve it, that rename is the first thing to suspect.
if [ "$PKG" = "yarn" ]; then
  yarn install || die "yarn install failed"
else
  npm install || die "npm install failed"
fi

say "anchor build"
info "the first build compiles the whole Solana toolchain — expect several minutes"
if ! anchor build; then
  die "anchor build failed.

  This is the expected first-build outcome: the program was written against the
  Anchor 1.x API from its release notes, on a machine with no Rust toolchain,
  so it has never been compiled. Paste the errors — they are the point of this
  step, and they come in a batch rather than one at a time."
fi
info "build OK — target/idl and target/types are generated"

if [ "$DO_TEST" = "0" ]; then
  say "done (--build-only)"
  info "re-run without flags to run the tests"
  exit 0
fi

say "anchor test --validator legacy"
# Anchor 1.x uses Surfpool as the default backend for `test` and `localnet`.
# This box has solana-test-validator and probably not Surfpool, so ask for the
# legacy validator explicitly.
info "--validator legacy keeps solana-test-validator (Anchor 1.x defaults to Surfpool)"
if ! anchor test --validator legacy; then
  die "tests failed — paste the output"
fi

say "done"
info "anchor test exited 0 — see the output above for which tests ran"
