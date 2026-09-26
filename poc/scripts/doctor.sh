#!/usr/bin/env bash
#
# SOLBEAM — Phase 0 doctor.
#
# Asserts the environment, one line per requirement, and exits non-zero if
# anything needed is missing. This is the acceptance test for Phase 0: a clean
# machine is "bootstrapped" when this exits 0.
#
#   ./doctor.sh              # everything the PoC needs
#   ./doctor.sh 1a           # only what Phase 1A needs — runs anywhere
#   ./doctor.sh 0 1b         # Phase 0 plus the SV Node work
#
# Phases: 0 environment | 1a peg-in offline | 1b SV Node pin | 2 Solana | 3 peg-out

set -uo pipefail

PHASES=("$@")
if [ ${#PHASES[@]} -eq 0 ]; then PHASES=(0 1a 1b 2 3); fi

pass=0; fail=0; warn=0

if [ -t 1 ]; then G=$'\033[32m'; R=$'\033[31m'; Y=$'\033[33m'; D=$'\033[2m'; Z=$'\033[0m'
else G=""; R=""; Y=""; D=""; Z=""; fi

ok()   { printf '  %s OK %s  %s\n' "$G" "$Z" "$1"; pass=$((pass + 1)); }
bad()  { printf '  %sFAIL%s  %s\n' "$R" "$Z" "$1"; fail=$((fail + 1)); }
note() { printf '  %sWARN%s  %s\n' "$Y" "$Z" "$1"; warn=$((warn + 1)); }
info() { printf '  %s    %s%s\n' "$D" "$1" "$Z"; }
hdr()  { printf '\n%s\n' "$1"; }

want() { local p; for p in "${PHASES[@]}"; do [ "$p" = "$1" ] && return 0; done; return 1; }
has()  { command -v "$1" >/dev/null 2>&1; }
first_line() { "$@" 2>&1 | head -1 | cut -c1-60; }

# req <phase> <command> <label> [version-args...]
req() {
  local phase="$1" cmd="$2" label="$3"; shift 3
  want "$phase" || return 0
  if has "$cmd"; then
    if [ $# -gt 0 ]; then ok "$label — $(first_line "$cmd" "$@")"
    else ok "$label"; fi
  else
    bad "$label — missing, needed by Phase $phase"
  fi
}

# ---------------------------------------------------------------------------

printf '\nSOLBEAM doctor — phases: %s\n' "${PHASES[*]}"
printf '%s\n' "==========================================================="

# -- platform ---------------------------------------------------------------

hdr "Platform"

ARCH="$(uname -m 2>/dev/null || echo unknown)"
OS="$(uname -s 2>/dev/null || echo unknown)"
info "host: $OS / $ARCH"

# The single most expensive discovery in this project: neither heavyweight
# dependency ships an aarch64 Linux build. Checked, not assumed.
if [ "$ARCH" = "x86_64" ]; then
  ok "architecture is x86_64 — both prebuilt binaries are available"
else
  for p in 1b 2 3; do
    if want "$p"; then
      bad "architecture is $ARCH, but Phase $p needs x86_64"
      info "Agave/Solana publishes no aarch64 Linux build (only x86_64-linux and"
      info "aarch64-darwin), and SV Node's only published binary is x86_64."
      info "Run the PoC on an x86_64 host, or use Docker with platform emulation."
      break
    fi
  done
  if ! want 1b && ! want 2 && ! want 3; then
    note "architecture is $ARCH — fine for Phase 1A, which needs no toolchain"
  fi
fi

# -- Phase 1a: the only thing a peg-in proof needs -------------------------

hdr "Phase 1A — peg-in on BSV, offline"

if want 1a; then
  if has python3; then
    PYV="$(python3 -c 'import sys;print("%d.%d.%d"%sys.version_info[:3])' 2>/dev/null || echo 0)"
    PYM="$(( ${PYV%%.*} * 100 + $(echo "$PYV" | cut -d. -f2) ))"
    if [ "$PYM" -ge 311 ]; then ok "python3 $PYV"
    else bad "python3 $PYV is too old — 3.11+ required"; fi
  else
    bad "python3 — missing, needed by Phase 1a"
  fi

  HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  if [ -f "$HERE/../checks/run_all.sh" ]; then
    ok "checker suite present"
    if [ "${SOLBEAM_DOCTOR_RUN_CHECKS:-1}" = "1" ]; then
      if OUT="$(bash "$HERE/../checks/run_all.sh" 2>&1)"; then
        ok "checker suite passes — $(printf '%s' "$OUT" | grep -c 'checks passed') checker(s), $(printf '%s' "$OUT" | grep -oE '[0-9]+/[0-9]+ checks passed' | awk -F'/' '{s+=$1} END {print s}') checks"
      else
        bad "checker suite FAILED — run: bash poc/checks/run_all.sh"
      fi
    fi
  else
    bad "checker suite not found at $HERE/../checks — run doctor from poc/scripts/"
  fi
fi

# -- Phase 0: the box itself ------------------------------------------------

hdr "Phase 0 — environment"

req 0 python3 "python3" --version
req 0 git     "git"     --version
req 0 curl    "curl"    --version
req 0 jq      "jq"      --version

if want 0; then
  FREE_GB="$(df -Pk /home 2>/dev/null | awk 'NR==2{print int($4/1048576)}')"
  if [ -n "${FREE_GB:-}" ] && [ "$FREE_GB" -ge 20 ]; then ok "disk: ${FREE_GB} GB free (20 GB recommended)"
  else note "disk: ${FREE_GB:-?} GB free — the Solana toolchain alone is several GB"; fi

  if curl -sS -o /dev/null -m 10 https://api.github.com 2>/dev/null; then ok "network egress to github.com"
  else bad "no network egress — dependency installs and the core checkers will fail"; fi
fi

# -- Phase 1b: the SV Node --------------------------------------------------

hdr "Phase 1B — real SV Node format pin"

req 1b bitcoind    "bitcoind (SV Node)"    --version
req 1b bitcoin-cli "bitcoin-cli (SV Node)" --version

if want 1b; then
  if has bitcoind; then
    VER="$(bitcoind --version 2>/dev/null | head -1)"
    case "$VER" in
      *"Bitcoin SV"*) ok "this is an SV Node, not Bitcoin Core — $VER" ;;
      *) note "bitcoind is not SV Node: $VER"
         info "Bitcoin Core will NOT do: different genesis, no getmerkleproof2, and" ;;
    esac
  fi
  if [ -n "${SOLBEAM_RPC:-}" ]; then
    if curl -sS -m 10 --user "${SOLBEAM_RPC_USER:-}:${SOLBEAM_RPC_PASS:-}" \
        --data-binary '{"jsonrpc":"1.0","id":"doctor","method":"getblockchaininfo","params":[]}' \
        -H 'content-type: text/plain' "$SOLBEAM_RPC" 2>/dev/null | grep -q '"result"'; then
      ok "SV Node RPC reachable at $SOLBEAM_RPC"
    else
      bad "SV Node RPC at $SOLBEAM_RPC did not answer getblockchaininfo"
    fi
  else
    note "SOLBEAM_RPC not set — skipping the live RPC check (see .env.example)"
  fi
fi

# -- Phases 2 and 3: Solana -------------------------------------------------

hdr "Phases 2–3 — Solana"

req 2 rustc   "rustc"     --version
req 2 cargo   "cargo"     --version
req 2 solana  "solana CLI" --version
req 2 anchor  "anchor"    --version

if want 2; then
  # Presence is not enough. Ubuntu ships Node 18; @anchor-lang/core requires
  # >= 20.18, and the failure it produces is an ESM/CJS error in a transitive
  # dependency that gives no hint about the real cause.
  if has node; then
    NODE_MAJOR="$(node -e 'console.log(process.versions.node.split(".")[0])' 2>/dev/null || echo 0)"
    if [ "${NODE_MAJOR:-0}" -ge 20 ] 2>/dev/null; then
      ok "node — $(first_line node --version)"
    else
      bad "node is $(first_line node --version) — Anchor 1.x needs >= 20.18"
      info "  Ubuntu's nodejs is 18. Install Node 22: https://deb.nodesource.com/setup_22.x"
    fi
  else
    bad "node — missing, needed by Phase 2"
  fi

  if has solana-test-validator; then ok "solana-test-validator present"
  else bad "solana-test-validator — missing; it ships with the Solana CLI"; fi

  # `anchor test` deploys with the provider wallet and refuses to start without it
  if [ -f "$HOME/.config/solana/id.json" ]; then
    ok "solana keypair at ~/.config/solana/id.json"
  else
    bad "no solana keypair — 'anchor test' stops with 'Unable to read keypair file'"
    info "  solana-keygen new --no-bip39-passphrase -o ~/.config/solana/id.json"
  fi
fi

# -- what the PoC needs installed but nothing checks for --------------------

hdr "Notes"

info "Phase 1A needs nothing but python3. It runs on any platform."
info "Phases 1B–3 need an x86_64 host. See TEST_PLAN.md §2.1."
if [ -n "${SOLBEAM_RPC:-}" ]; then info "SOLBEAM_RPC=$SOLBEAM_RPC"; else info "Copy .env.example to .env to configure the node and validator."; fi

printf '\n%s\n' "==========================================================="
printf '  %d ok, %d warning(s), %d failure(s)\n' "$pass" "$warn" "$fail"
if [ "$fail" -eq 0 ]; then
  printf '  environment is ready for: %s\n\n' "${PHASES[*]}"
  exit 0
fi
printf '  NOT ready — fix the failures above, then re-run.\n\n'
exit 1
