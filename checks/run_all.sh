#!/usr/bin/env bash
# Run every SOLBEAM BSV checker. Requires only Python 3; core and tx also use
# network access to validate against live chain data, the rest are offline.
set -uo pipefail
cd "$(dirname "$0")"

# check_bsv_pegin.py is Phase 1A: it needs no network and no BSV node.
fail=0
for checker in check_bsv_core.py check_bsv_tx.py check_bsv_deposit.py check_bsv_pegin.py; do
  echo "==================================================================="
  echo "  $checker"
  echo "==================================================================="
  if python3 "$checker"; then
    echo "--> $checker OK"
  else
    echo "--> $checker FAILED"
    fail=1
  fi
  echo
done

if [ "$fail" -eq 0 ]; then
  echo "ALL CHECKERS PASSED"
else
  echo "SOME CHECKERS FAILED"
fi
exit "$fail"
