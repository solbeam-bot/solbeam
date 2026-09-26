#!/usr/bin/env bash
# Run every SOLBEAM checker. Requires only Python 3.
#
#   check_bsv_core.py, check_bsv_tx.py         validate against live chain data
#   check_bsv_deposit.py, check_bsv_pegin.py   offline (Phase 1A)
#   check_bsv_node.py --selftest               offline
#   check_bsv_node.py                          Phase 1B's real pin; runs only
#                                              if SOLBEAM_RPC is set (an SV Node)
set -uo pipefail
cd "$(dirname "$0")"

fail=0

run() {
  local label="$1"; shift
  echo "==================================================================="
  echo "  $label"
  echo "==================================================================="
  if "$@"; then
    echo "--> $label OK"
  else
    echo "--> $label FAILED"
    fail=1
  fi
  echo
}

for checker in check_bsv_core.py check_bsv_tx.py check_bsv_deposit.py check_bsv_pegin.py; do
  run "$checker" python3 "$checker"
done

# Phase 1B's harness, exercised with no node. It pins our code paths, not the
# node's real response — only the live run below settles that.
run "check_bsv_node.py --selftest" python3 check_bsv_node.py --selftest

if [ -n "${SOLBEAM_RPC:-}" ]; then
  run "check_bsv_node.py LIVE ($SOLBEAM_RPC)" python3 check_bsv_node.py
else
  echo "==================================================================="
  echo "  check_bsv_node.py (live) — SKIPPED"
  echo "==================================================================="
  echo "  SOLBEAM_RPC is not set, so Phase 1B's real pin did not run."
  echo "  To run it:"
  echo "      poc/scripts/regtest-up.sh"
  echo "      export SOLBEAM_RPC=http://127.0.0.1:18443"
  echo "      python3 poc/checks/check_bsv_node.py"
  echo
fi

if [ "$fail" -eq 0 ]; then
  echo "ALL CHECKERS PASSED"
else
  echo "SOME CHECKERS FAILED"
fi
exit "$fail"
