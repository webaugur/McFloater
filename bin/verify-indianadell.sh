#!/usr/bin/env bash
#
# verify-indianadell.sh  (McFloater stub)
#
# Purpose: optional hook called by IndianaDell's bin/fix-indianadell.sh
#          when it wants to know the lab AI / Home Assistant / voice stack status.
#
# This stub is intentionally minimal.  Real verification belongs in
# McFloater's deploy/ or tools/ (e.g. deploy/thumper/restore-now.sh or
# a future master-node verify script).
#
# When run with --verify-only it should exit 0 if the components the
# caller considers "required for this host" are present, non-zero otherwise.
# It may print its own OK/MISS lines; fix-indianadell.sh ignores output
# except for the hook's exit status.
#
# IndianaDell sessions: this file was created so that
#   bin/fix-indianadell.sh
# can discover and invoke McFloater without hard-coding paths or failing
# when McFloater is absent or incomplete.  The hook is deliberately non-fatal.
#
set -euo pipefail

case "${1:-}" in
  --verify-only)
    echo "[McFloater] verify hook present (stub)"
    exit 0
    ;;
  *)
    echo "Usage: $0 --verify-only"
    exit 1
    ;;
esac
