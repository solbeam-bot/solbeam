#!/usr/bin/env bash
#
# The parameter-change gate. Run this before committing a change to
# `config/params.json`.
#
#   config/check.sh              regenerate, verify, and — if the sheet differs
#                                from the committed version — run the suite
#   config/check.sh --suite      always run the suite, even if the sheet is clean
#   config/check.sh --no-suite   refuse if the sheet is dirty; verify only if clean
#   config/check.sh --help
#
# Why this exists: `python3 config/gen.py --check` proves the projections agree
# with **each other** — the CSV, doc 06, `params.rs` and `tests/params.json`.
# Nothing in it proves the projections agree with the **compiled program**. A
# `v.maturity_blocks` change once left `main` red for several commits because the
# suite was never run. This script is the thing that runs it.
#
# It is not CI. There is no CI here, and this repository has no `.github/`. It is
# a maintainer's pre-commit step, and it says out loud what it did and did not
# verify.

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
SHEET="$ROOT/config/params.json"
WORKSPACE="$ROOT/poc/solana"

WITH_SUITE=auto

say()  { printf '\n== %s\n' "$1"; }
info() { printf '   %s\n' "$1"; }
warn() { printf '   WARN: %s\n' "$1" >&2; }
die()  { printf '\nERROR: %s\n' "$1" >&2; exit 1; }

for a in "$@"; do
  case "$a" in
    --suite)    WITH_SUITE=always ;;
    --no-suite) WITH_SUITE=never ;;
    -h|--help)  sed -n '2,16p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) printf 'unknown argument: %s\n' "$a" >&2; exit 2 ;;
  esac
done

[ -f "$SHEET" ] || die "no $SHEET — run this from the repository"
cd "$ROOT" || die "cannot enter $ROOT"

# ---------------------------------------------------------------------------
# 1. regenerate, then verify the projections
# ---------------------------------------------------------------------------
say "regenerate the projections from config/params.json"
if ! python3 config/gen.py; then
  die "python3 config/gen.py failed — nothing was committed and nothing is verified"
fi

say "verify every projection is in step (python3 config/gen.py --check)"
if ! python3 config/gen.py --check; then
  die "a projection is stale after a regenerate, which should be impossible —
  that is a generator bug, not a stale file. Do not commit."
fi
info "verified: params.rs, tests/params.json, docs/parameters.csv and the"
info "generated regions of docs/06-parameters.md all agree with config/params.json"

# ---------------------------------------------------------------------------
# 2. does the sheet differ from the committed version?
# ---------------------------------------------------------------------------
say "compare config/params.json with the committed version"
DIRTY=unknown
if git -C "$ROOT" rev-parse --git-dir >/dev/null 2>&1; then
  if git -C "$ROOT" diff --quiet HEAD -- config/params.json; then
    DIRTY=no
  else
    DIRTY=yes
  fi
else
  warn "not a git repository, so the sheet cannot be compared with a commit"
fi

case "$DIRTY" in
  yes)
    info "config/params.json DIFFERS from HEAD — the compiled program must be"
    info "re-run against it, because gen.py --check never does that." ;;
  no)
    info "config/params.json is identical to HEAD (no parameter change to gate)" ;;
  *)
    info "config/params.json cannot be compared with HEAD; assuming it changed" ;;
esac

# ---------------------------------------------------------------------------
# 3. the suite — the only check of the compiled artifact
# ---------------------------------------------------------------------------
RAN_SUITE=no
case "$WITH_SUITE:$DIRTY" in
  never:yes|never:unknown)
    die "refusing --no-suite: config/params.json is not known to match the
  committed version, so the suite has not been run against it. A projection
  check cannot see the compiled program. Drop --no-suite, or revert the sheet." ;;
  auto:no|never:no)
    say "suite — SKIPPED"
    info "the sheet is identical to HEAD, so this run did NOT execute the suite"
    info "and did NOT verify the compiled program. Use --suite to force it." ;;
  *)
    say "run the suite (anchor test --validator legacy)"
    # The suite needs the toolchain on PATH; a dropped PATH makes `anchor` die
    # instantly, which is why the exit code is captured rather than piped.
    export PATH="$HOME/.cargo/bin:$HOME/.local/share/solana/install/active_release/bin:$HOME/.avm/bin:$PATH"
    if ! command -v anchor >/dev/null 2>&1; then
      die "anchor is not on PATH, so the suite was NOT run. config/params.json
  differs from HEAD ($DIRTY) and the compiled program is unverified.
  Install the toolchain (poc/scripts/bootstrap.sh), then re-run."
    fi
    [ -d "$WORKSPACE" ] || die "no workspace at $WORKSPACE"
    ( cd "$WORKSPACE" && anchor test --validator legacy )
    status=$?
    if [ "$status" -ne 0 ]; then
      die "the suite FAILED (exit $status). The parameter change does not match
  the compiled program. Nothing is verified; do not commit."
    fi
    RAN_SUITE=yes
    info "the suite PASSED against this config/params.json" ;;
esac

# ---------------------------------------------------------------------------
# 4. what this run actually verified
# ---------------------------------------------------------------------------
say "summary"
info "verified: every projection is in step with config/params.json"
if [ "$RAN_SUITE" = "yes" ]; then
  info "verified: the compiled program passes anchor test against the sheet"
else
  info "NOT verified: the compiled program (suite skipped; the sheet is clean)"
fi
exit 0
