#!/usr/bin/env bash
#
# SOLBEAM — bring up an SV Node in regtest, for Phase 1B.
#
#   ./regtest-up.sh            # start (idempotent) and mine 101 blocks
#   ./regtest-up.sh --reset    # wipe the datadir, then start
#   ./regtest-up.sh stop       # stop the node
#   ./regtest-up.sh status     # chain, height, hashrate
#
# Two parameters are REQUIRED on modern SV Node: -excessiveblocksize and
# -maxstackmemoryusageconsensus. Without them, large scripts are rejected
# consensus-side and the failure looks like a covenant bug rather than a
# configuration one. See poc/VERSIONS.md.

set -uo pipefail

DATADIR="${SOLBEAM_DATADIR:-$HOME/.solbeam/regtest}"
RPCPORT="${SOLBEAM_RPCPORT:-18443}"
RPCUSER="${SOLBEAM_RPC_USER:-solbeam}"
RPCPASS="${SOLBEAM_RPC_PASS:-solbeam}"
CLI=(bitcoin-cli -regtest -datadir="$DATADIR" -rpcuser="$RPCUSER" -rpcpassword="$RPCPASS")

info() { printf '   %s\n' "$1"; }
die()  { printf '\nERROR: %s\n' "$1" >&2; exit 1; }

command -v bitcoind    >/dev/null 2>&1 || die "bitcoind not found — run poc/scripts/bootstrap.sh first"
command -v bitcoin-cli >/dev/null 2>&1 || die "bitcoin-cli not found — run poc/scripts/bootstrap.sh first"

# An SV Node is required. Bitcoin Core has a different genesis, no
# getmerkleproof2, and would make Phase 1B meaningless.
VERSION_LINE="$(bitcoind --version 2>/dev/null | head -1)"
case "$VERSION_LINE" in
  *"Bitcoin SV"*) : ;;
  *) die "this bitcoind is not Bitcoin SV: $VERSION_LINE
  Phase 1B needs an SV Node. Bitcoin Core cannot pin the formats we depend on." ;;
esac

case "${1:-start}" in
  stop)
    "${CLI[@]}" stop >/dev/null 2>&1 && info "stopped" || info "was not running"
    exit 0 ;;
  status)
    "${CLI[@]}" getblockchaininfo >/dev/null 2>&1 || die "node is not running"
    info "chain:    $("${CLI[@]}" getblockchaininfo | jq -r '.chain')"
    info "blocks:   $("${CLI[@]}" getblockcount)"
    info "hashrate: $("${CLI[@]}" getnetworkhashps 2>/dev/null || echo 'n/a')"
    exit 0 ;;
  --reset)
    info "wiping $DATADIR"
    rm -rf "$DATADIR" ;;
  start) : ;;
  *) die "unknown argument: $1 (try start, stop, status, --reset)" ;;
esac

mkdir -p "$DATADIR"
cat > "$DATADIR/bitcoin.conf" <<EOF
regtest=1
server=1
txindex=1
rpcuser=$RPCUSER
rpcpassword=$RPCPASS
rpcport=$RPCPORT
# Required on modern SV Node — see VERSIONS.md
excessiveblocksize=2000000000
maxstackmemoryusageconsensus=100000000
minminingtxfee=0.00000001
fallbackfee=0.00001
EOF

if "${CLI[@]}" getblockcount >/dev/null 2>&1; then
  info "node already running"
else
  info "starting bitcoind in regtest"
  bitcoind -datadir="$DATADIR" -daemon >/dev/null || die "bitcoind failed to start"
  for i in $(seq 1 30); do
    sleep 1
    "${CLI[@]}" getblockcount >/dev/null 2>&1 && break
    if [ "$i" = "30" ]; then
      die "node did not accept RPC within 30s — check $DATADIR/regtest/debug.log"
    fi
  done
fi

HEIGHT="$("${CLI[@]}" getblockcount)"
if [ "$HEIGHT" -lt 101 ]; then
  ADDR="$("${CLI[@]}" getnewaddress "" 2>/dev/null || "${CLI[@]}" getnewaddress)"
  "${CLI[@]}" generatetoaddress 101 "$ADDR" >/dev/null || die "generatetoaddress failed"
  info "mined 101 blocks to $ADDR (coinbase in block 1 is now spendable)"
fi

info "chain:  $("${CLI[@]}" getblockchaininfo | jq -r '.chain')"
info "blocks: $("${CLI[@]}" getblockcount)"
info "rpc:    http://127.0.0.1:$RPCPORT  (user $RPCUSER)"

cat <<EOF

Next — Phase 1B, which pins our formats against this node:

   export SOLBEAM_RPC=http://127.0.0.1:$RPCPORT
   export SOLBEAM_RPC_USER=$RPCUSER
   export SOLBEAM_RPC_PASS=$RPCPASS
   python3 poc/checks/check_bsv_node.py

EOF
