#!/usr/bin/env bash
# Runs the live tests of bd-cache-sync (tests/live.rs) in a scratch workspace.
#
# Usage: crates/bd-cache-sync/tests/run-live.sh [cargo test arguments...]
#
# Needs `bd` and `dolt` on PATH (or SC_BD_BIN and DOLT_BIN_DIR) and a Rust
# toolchain. Used by the team-lead on the m5-atmbench testbed. It reuses the
# scratch-workspace scripts of sprint a-2: everything is created in a scratch
# directory on a scratch Dolt server and removed on exit; no shared database
# is read or written, and no `atm` message is sent.
#
# The whole run is bounded: LIVE_TIMEOUT_S (default 1800) seconds, after which
# cargo and its tests are terminated and the scratch server is torn down.
set -euo pipefail
repo="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
scratch_env="$(mktemp)"
cleanup() {
  # shellcheck disable=SC1090
  [ -s "$scratch_env" ] && source "$scratch_env" && [ -n "${SC_BD_LIVE_ROOT:-}" ] \
    && "$repo/scripts/bd-compat/teardown-scratch.sh" "$SC_BD_LIVE_ROOT" || true
  rm -f "$scratch_env"
}
trap cleanup EXIT

# Runs a command for at most $1 seconds, with no dependency on GNU timeout
# (macOS has none). The command runs in its own process group (job control), so
# the watchdog's TERM, then KILL, reaches cargo, the test binary and any `bd`
# it started; on a normal exit the watchdog and its `sleep` are stopped.
run_bounded() {
  local secs="$1"; shift
  set -m
  "$@" &
  local pid=$!
  set +m
  (
    sleep "$secs" &
    local nap=$!
    trap 'kill "$nap" 2>/dev/null; exit 0' TERM
    wait "$nap" || exit 0
    echo "run-live.sh: timed out after ${secs}s; terminating the test run" >&2
    kill -TERM -- "-$pid" 2>/dev/null || kill -TERM "$pid" 2>/dev/null || true
    sleep 10 &
    nap=$!
    wait "$nap" || exit 0
    kill -KILL -- "-$pid" 2>/dev/null || kill -KILL "$pid" 2>/dev/null || true
  ) &
  local watchdog=$!
  local status=0
  wait "$pid" || status=$?
  kill -TERM "$watchdog" 2>/dev/null || true
  wait "$watchdog" 2>/dev/null || true
  return "$status"
}

"$repo/scripts/bd-compat/setup-scratch.sh" "$scratch_env"
set -a
# shellcheck disable=SC1090
source "$scratch_env"
set +a
cd "$repo"
run_bounded "${LIVE_TIMEOUT_S:-1800}" \
  cargo test -p bd-cache-sync --all-features --test live "$@" -- --ignored --test-threads=1 --nocapture
