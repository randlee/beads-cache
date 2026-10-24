#!/usr/bin/env bash
# Stops the scratch Dolt server and removes the scratch directory made by setup-scratch.sh.
#
# Usage: teardown-scratch.sh <scratch root>
#
# Refuses a directory without the `.sc-bd-live-root` marker, so it can only ever remove
# what setup-scratch.sh created.
set -euo pipefail

root="${1:?usage: teardown-scratch.sh <scratch root>}"
[ -f "$root/.sc-bd-live-root" ] || { echo "$root is not a scratch root; not touching it" >&2; exit 1; }
if [ -f "$root/dolt.pid" ]; then
  pid="$(cat "$root/dolt.pid")"
  kill "$pid" 2>/dev/null || true
  # Wait for the server to exit: it is still writing its data directory until it does.
  for _ in $(seq 1 20); do
    kill -0 "$pid" 2>/dev/null || break
    sleep 0.5
  done
  kill -9 "$pid" 2>/dev/null || true
fi
rm -rf "$root" || { sleep 1; rm -rf "$root"; }
