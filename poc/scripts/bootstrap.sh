#!/usr/bin/env bash
#
# SOLBEAM — Phase 0 bootstrap. Target: x86_64 Linux (Ubuntu 22.04 / 24.04).
#
# Installs what Phases 0–3 need. Re-runnable: every step checks before it acts.
#
#   ./bootstrap.sh --dry-run     # print the plan, change nothing (works anywhere)
#   ./bootstrap.sh               # do it
#
# Why x86_64 only: Agave/Solana publishes no aarch64 Linux build (every release
# is x86_64-linux or aarch64-darwin), and SV Node's only published binary is
# x86_64. See TEST_PLAN.md §2.1. Phase 1A needs none of this — it is pure Python.
#
# Anything already installed is left alone. The versions this resolves to are
# written to VERSIONS.lock so the pin is captured from reality, not guessed.

set -uo pipefail

DRY_RUN=0
SKIP_SOLANA=0
SKIP_SVNODE=0

for arg in "$@"; do
  case "$arg" in
    --dry-run)     DRY_RUN=1 ;;
    --skip-solana) SKIP_SOLANA=1 ;;
    --skip-svnode) SKIP_SVNODE=1 ;;
    -h|--help)     sed -n '2,14p' "$0"; exit 0 ;;
    *) echo "unknown argument: $arg" >&2; exit 2 ;;
  esac
done

# -- pins -------------------------------------------------------------------
# Only pin what has been verified to exist. Everything else resolves to the
# current release and is recorded in VERSIONS.lock afterwards.
SVNODE_VERSION="${SVNODE_VERSION:-1.1.1}"     # the last release publishing a binary
SVNODE_MODE="${SVNODE_MODE:-binary}"          # binary | build
SOLANA_CHANNEL="${SOLANA_CHANNEL:-stable}"    # e.g. v2.1.0 to pin an exact release
RUST_TOOLCHAIN="${RUST_TOOLCHAIN:-}"          # empty = rustup default

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
POC="$(cd "$HERE/.." && pwd)"
LOCK="$POC/VERSIONS.lock"

say()  { printf '\n== %s\n' "$1"; }
info() { printf '   %s\n' "$1"; }
warn() { printf '   WARN: %s\n' "$1"; }
die()  { printf '\nERROR: %s\n' "$1" >&2; exit 1; }

run() {
  if [ "$DRY_RUN" = "1" ]; then printf '   [dry-run] %s\n' "$*"; return 0; fi
  "$@" || die "command failed: $*"
}

shell_run() {
  if [ "$DRY_RUN" = "1" ]; then printf '   [dry-run] %s\n' "$1"; return 0; fi
  bash -c "$1" || die "command failed: $1"
}

have() { command -v "$1" >/dev/null 2>&1; }

# -- preflight ---------------------------------------------------------------

say "preflight"

ARCH="$(uname -m)"
[ "$ARCH" = "x86_64" ] || die "this host is $ARCH; Phases 1B-3 need x86_64.
  Agave/Solana ships no aarch64 Linux build and SV Node's only binary is x86_64.
  Use an x86_64 VM (TEST_PLAN.md §2.1), or run only Phase 1A, which needs no toolchain:
      bash poc/checks/run_all.sh"

info "host: $(uname -s) / $ARCH"
info "poc:  $POC"
[ "$DRY_RUN" = "1" ] && warn "dry run — nothing will be changed"

SUDO=""
if [ "$(id -u)" -ne 0 ]; then
  if have sudo; then SUDO="sudo"; else die "need root or sudo for apt packages"; fi
fi

APT_PACKAGES=(build-essential pkg-config libssl-dev libboost-all-dev libevent-dev
              libzmq3-dev libdb++-dev libtool autoconf automake python3 python3-pip
              git curl jq ca-certificates)

# -- system packages --------------------------------------------------------

say "system packages (apt)"
if [ "$DRY_RUN" = "1" ]; then
  info "[dry-run] apt-get install ${APT_PACKAGES[*]}"
else
  run $SUDO apt-get update -qq
  run $SUDO apt-get install -y -qq "${APT_PACKAGES[@]}"
fi

# -- rust -------------------------------------------------------------------

say "Rust toolchain"
if have rustc && have cargo; then
  info "already present — $(rustc --version)"
else
  shell_run "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --no-modify-path"
  if [ "$DRY_RUN" = "0" ]; then
    # shellcheck disable=SC1091
    . "$HOME/.cargo/env"
    [ -n "$RUST_TOOLCHAIN" ] && run rustup toolchain install "$RUST_TOOLCHAIN"
  fi
fi

# -- solana CLI -------------------------------------------------------------

if [ "$SKIP_SOLANA" = "1" ]; then
  say "Solana CLI — skipped (--skip-solana)"
else
  say "Solana CLI ($SOLANA_CHANNEL)"
  if have solana; then
    info "already present — $(solana --version 2>/dev/null | head -1)"
  else
    shell_run "sh -c \"\$(curl -sSfL https://release.anza.xyz/$SOLANA_CHANNEL/install)\""
    info "solana-test-validator ships with the CLI; no separate install"
  fi

  say "Anchor (avm)"
  if have anchor; then
    info "already present — $(anchor --version 2>/dev/null | head -1)"
  elif have avm; then
    run avm install latest
    run avm use latest
  else
    run cargo install --git https://github.com/coral-xyz/anchor avm --locked --force
    run avm install latest
    run avm use latest
  fi
fi

# -- SV node ----------------------------------------------------------------

if [ "$SKIP_SVNODE" = "1" ]; then
  say "SV Node — skipped (--skip-svnode)"
else
  say "SV Node (bitcoin-sv $SVNODE_VERSION, mode=$SVNODE_MODE)"
  if have bitcoind; then
    info "already present — $(bitcoind --version 2>/dev/null | head -1)"
  elif [ "$SVNODE_MODE" = "binary" ]; then
    TARBALL="bitcoin-sv-${SVNODE_VERSION}-x86_64-linux-gnu.tar.gz"
    URL="https://github.com/bitcoin-sv/bitcoin-sv/releases/download/v${SVNODE_VERSION}/${TARBALL}"
    info "downloading $TARBALL"
    info "note: v1.2.x publishes no binaries, which is why this pins $SVNODE_VERSION"
    if [ "$DRY_RUN" = "1" ]; then
      info "[dry-run] curl -L $URL | tar -xz -C /opt"
    else
      mkdir -p "$HOME/solbeam-svnode"
      curl -fsSL "$URL" -o "$HOME/solbeam-svnode/$TARBALL" \
        || die "download failed — try SVNODE_MODE=build"
      tar -xzf "$HOME/solbeam-svnode/$TARBALL" -C "$HOME/solbeam-svnode"
      info "extracted; add its bin/ to PATH:"
      info "  export PATH=\"\$HOME/solbeam-svnode/bitcoin-sv-${SVNODE_VERSION}/bin:\$PATH\""
    fi
  else
    info "building from source — this needs a C++20 toolchain and takes a while"
    shell_run "git clone --depth 1 --branch v${SVNODE_VERSION} https://github.com/bitcoin-sv/bitcoin-sv.git \$HOME/solbeam-svnode-src"
    shell_run "cd \$HOME/solbeam-svnode-src && ./autogen.sh && ./configure --without-gui --disable-tests --disable-bench && make -j\"\$(nproc)\""
  fi
fi

# -- record what actually resolved ------------------------------------------

if [ "$DRY_RUN" = "0" ]; then
  say "recording resolved versions to $LOCK"
  {
    echo "# Generated by poc/scripts/bootstrap.sh — do not edit by hand."
    echo "# Captured $(date -u +%Y-%m-%dT%H:%M:%SZ) on $(uname -s)/$(uname -m)"
    echo
    for probe in "python3 --version" "git --version" "jq --version" "rustc --version" \
                 "cargo --version" "solana --version" "anchor --version" \
                 "bitcoind --version" "node --version"; do
      cmd="${probe%% *}"
      if have "$cmd"; then printf '%s: %s\n' "$cmd" "$($probe 2>&1 | head -1)"; fi
    done
  } > "$LOCK"
  info "written"
fi

# -- verify -----------------------------------------------------------------

say "verify"
if [ "$DRY_RUN" = "1" ]; then
  info "[dry-run] would now run: $HERE/doctor.sh"
else
  "$HERE/doctor.sh" || die "doctor still reports failures — see above"
fi

printf '\nPhase 0 bootstrap complete.\n'
printf 'Next: poc/scripts/regtest-up.sh  (starts the SV Node in regtest)\n\n'
