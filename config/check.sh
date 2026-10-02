#!/usr/bin/env bash
#
# The parameter-change gate. Run this before committing a change to
# `config/params.json` or to a profile overlay `config/params.<profile>.json`.
#
#   config/check.sh              regenerate the mainnet projections, verify the
#                                mainnet projections and every profile, and —
#                                if `config/params.json` differs from the
#                                committed version — run the suite
#   config/check.sh --profile P  regenerate and verify profile `P` only, from
#                                `config/params.json` + `config/params.P.json`,
#                                into `config/out/P/`
#   config/check.sh --suite      always run the suite, even if the sheet is clean
#   config/check.sh --no-suite   refuse if the sheet is dirty; verify only if clean
#   config/check.sh --help
#
# Why this exists: `python3 config/gen.py --check` proves the projections agree
# with **each other** — the CSV, doc 06, `params.rs`, every profile artifact and
# `tests/params.json`. Nothing in it proves the projections agree with the
# **compiled program**. A `v.maturity_blocks` change once left `main` red for
# several commits because the suite was never run. This script is the thing that
# runs it.
#
# **The suite is never run for a profile.** The program is compiled against the
# mainnet `poc/solana/programs/solbeam/src/params.rs`; a profile's
# `config/out/<profile>/params.rs` is not compiled and nothing reads it, so
# `anchor test` could not see a profile change at all. Deploying a profile to
# testnet means copying its `params.rs` over the program's and running the suite
# then — a deployment step, not a check.
#
# It is not CI. There is no CI here, and this repository has no `.github/`. It is
# a maintainer's pre-commit step, and it says out loud what it did and did not
# verify.

set -uo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$(cd "$HERE/.." && pwd)"
WORKSPACE="$ROOT/poc/solana"

WITH_SUITE=auto
PROFILE=""

say()  { printf '\n== %s\n' "$1"; }
info() { printf '   %s\n' "$1"; }
warn() { printf '   WARN: %s\n' "$1" >&2; }
die()  { printf '\nERROR: %s\n' "$1" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --suite)    WITH_SUITE=always; shift ;;
    --no-suite) WITH_SUITE=never; shift ;;
    --profile)
      [ $# -ge 2 ] || { printf 'missing value for --profile\n' >&2; exit 2; }
      PROFILE="$2"; shift 2 ;;
    --profile=*)
      PROFILE="${1#--profile=}"; shift ;;
    -h|--help)
      cat <<'EOF'
The parameter-change gate. Run this before committing a change to
config/params.json or to a profile overlay config/params.<profile>.json.

  config/check.sh                 regenerate the mainnet projections, verify
                                  mainnet and every profile, and run the suite
                                  if config/params.json differs from HEAD
  config/check.sh --profile NAME  regenerate and verify profile NAME only, from
                                  config/params.json + config/params.NAME.json,
                                  into config/out/NAME/
  config/check.sh --suite         always run the suite, even if the sheet is clean
  config/check.sh --no-suite      refuse if the main sheet is dirty; verify only if clean
  config/check.sh --help

The suite is never run for a profile: the compiled program reads the mainnet
params.rs, and config/out/NAME/params.rs is not compiled, so `anchor test`
cannot see the profile. Deploying one means copying its params.rs over the
program's and running the suite then -- a deployment step, not this gate.
EOF
      exit 0 ;;
    *) printf 'unknown argument: %s\n' "$1" >&2; exit 2 ;;
  esac
done

if [ -n "$PROFILE" ]; then
  case "$PROFILE" in
    *[!A-Za-z0-9_-]*) die "invalid profile name: $PROFILE" ;;
  esac
  OVERLAY="$ROOT/config/params.$PROFILE.json"
  OUTDIR="config/out/$PROFILE"
  SHEET_LABEL="config/params.$PROFILE.json (profile $PROFILE)"
  GEN_ARGS=(--profile "$PROFILE")
  [ -f "$OVERLAY" ] || die "no $OVERLAY — a profile is an overlay named
  config/params.<profile>.json; see config/README.md"
else
  SHEET_LABEL="config/params.json"
  GEN_ARGS=()
fi

SHEET="$ROOT/config/params.json"
[ -f "$SHEET" ] || die "no $SHEET — run this from the repository"
cd "$ROOT" || die "cannot enter $ROOT"

# Anything a profile owns that differs from HEAD: overlay sheets (all of
# config/params.*.json except the main sheet) and the generated artifacts.
profile_changes() {
  git -C "$ROOT" status --porcelain -- config/params.*.json config/out 2>/dev/null \
    | grep -v -E '^.. config/params\.json$' || true
}

# ---------------------------------------------------------------------------
# 1. regenerate, then verify the projections
# ---------------------------------------------------------------------------
if [ -n "$PROFILE" ]; then
  say "regenerate the '$PROFILE' profile from config/params.json + config/params.$PROFILE.json"
else
  say "regenerate the projections from config/params.json"
fi
if ! python3 config/gen.py "${GEN_ARGS[@]}"; then
  die "python3 config/gen.py ${GEN_ARGS[*]} failed — nothing was committed and
  nothing is verified"
fi

if [ -n "$PROFILE" ]; then
  say "verify the '$PROFILE' profile projections are in step"
  if ! python3 config/gen.py --check --profile "$PROFILE"; then
    die "a profile projection is stale after a regenerate, which should be
  impossible — that is a generator bug, not a stale file. Do not commit."
  fi
  info "verified: $OUTDIR/params.rs, $OUTDIR/tests-params.json,"
  info "$OUTDIR/parameters.csv and $OUTDIR/06-parameters.md all agree with"
  info "config/params.json overlaid with config/params.$PROFILE.json"
else
  say "verify every projection is in step (python3 config/gen.py --check)"
  if ! python3 config/gen.py --check; then
    die "a projection is stale after a regenerate, which should be impossible —
  that is a generator bug, not a stale file. Do not commit."
  fi
  info "verified: params.rs, tests/params.json, docs/parameters.csv, the"
  info "generated regions of docs/06-parameters.md, and every profile under"
  info "config/out/ all agree with their sheets"
fi

# ---------------------------------------------------------------------------
# 2. does the sheet differ from the committed version?
# ---------------------------------------------------------------------------
say "compare $SHEET_LABEL with the committed version"
DIRTY=unknown
if git -C "$ROOT" rev-parse --git-dir >/dev/null 2>&1; then
  if [ -z "$(git -C "$ROOT" status --porcelain -- "config/params.json" \
        ${PROFILE:+"config/params.$PROFILE.json"} 2>/dev/null)" ]; then
    DIRTY=no
  else
    DIRTY=yes
  fi
else
  warn "not a git repository, so the sheet cannot be compared with a commit"
fi

case "$DIRTY" in
  yes)
    if [ -n "$PROFILE" ]; then
      info "config/params.$PROFILE.json (or the main sheet) differs from HEAD —"
      info "the profile projections were rebuilt from it"
    else
      info "config/params.json DIFFERS from HEAD — the compiled program must be"
      info "re-run against it, because gen.py --check never does that."
    fi ;;
  no)
    info "$SHEET_LABEL is identical to HEAD (no change to gate)" ;;
  *)
    info "$SHEET_LABEL cannot be compared with HEAD; assuming it changed" ;;
esac

# ---------------------------------------------------------------------------
# 3. the suite — the only check of the compiled artifact
# ---------------------------------------------------------------------------
RAN_SUITE=no
if [ -n "$PROFILE" ]; then
  say "suite — NOT run for a profile"
  [ "$WITH_SUITE" = "always" ] && warn "--suite was given, but the suite compiles
  the mainnet params.rs, not config/out/$PROFILE/params.rs, so it would test the
  wrong sheet. Not run."
  info "the compiled program reads poc/solana/programs/solbeam/src/params.rs;"
  info "$OUTDIR/params.rs is not compiled and nothing reads it, so"
  info "\`anchor test --validator legacy\` could not see this profile."
  info "NOT verified: the compiled program (this profile cannot change it)."
  info "To exercise this profile end to end, copy $OUTDIR/params.rs over the"
  info "program's, rebuild, and run the suite: that is a deployment, not a check."
else
  case "$WITH_SUITE:$DIRTY" in
    never:yes|never:unknown)
      die "refusing --no-suite: config/params.json is not known to match the
  committed version, so the suite has not been run against it. A projection
  check cannot see the compiled program. Drop --no-suite, or revert the sheet." ;;
    auto:no|never:no)
      say "suite — SKIPPED"
      info "config/params.json is identical to HEAD, so this run did NOT execute"
      info "the suite and did NOT verify the compiled program. Use --suite to"
      info "force it."
      if [ -n "$(profile_changes)" ]; then
        info "a profile overlay or profile artifact differs from HEAD; that does"
        info "not change the compiled program, so it is not a suite trigger."
      fi ;;
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
fi

# ---------------------------------------------------------------------------
# 4. what this run actually verified
# ---------------------------------------------------------------------------
say "summary"
if [ -n "$PROFILE" ]; then
  info "verified: the '$PROFILE' profile projections are in step with"
  info "          config/params.json + config/params.$PROFILE.json"
  info "NOT verified: the compiled program — a profile is not compiled, and the"
  info "          suite is deliberately not run for one"
else
  info "verified: every mainnet projection and every profile is in step with"
  info "          its sheet"
  if [ "$RAN_SUITE" = "yes" ]; then
    info "verified: the compiled program passes anchor test against the sheet"
  else
    info "NOT verified: the compiled program (suite skipped; the sheet is clean)"
  fi
fi
exit 0
