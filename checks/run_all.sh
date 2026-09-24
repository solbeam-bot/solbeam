#!/usr/bin/env bash
# Run every SOLBEAM BSV checker. Requires only Python 3 and network access.
set -uo pipefail
cd "$(dirname "$0")"

fail=0
for checker in check_bsv_core.py check_bsv_tx.py check_bsv_deposit.py; do
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
