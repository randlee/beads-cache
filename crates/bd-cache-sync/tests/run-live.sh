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
# State of the bounded run, read by `cleanup`.
run_pid=""      # leader of the cargo process group
watchdog=""     # the watchdog subshell
fired=""        # file the watchdog creates when it fires

# The one cleanup, on EXIT, INT and TERM: it stops the cargo group (TERM, then
# KILL), the watchdog and its `sleep`, removes the marker file and tears the
# scratch workspace down. It is safe to run twice and with nothing started.
cleanup_done=""
cleanup() {
  local rc=$?
  [ -z "$cleanup_done" ] || return 0
  cleanup_done=1
  trap - EXIT INT TERM
  if [ -n "$run_pid" ]; then
    if kill -0 "$run_pid" 2>/dev/null; then
      kill -TERM -- "-$run_pid" 2>/dev/null || true
      sleep 2
    fi
    # What is left in the group (a test binary, a `bd`); the group id is still
    # ours to name while this shell has not reaped $run_pid, or while members remain.
    kill -KILL -- "-$run_pid" 2>/dev/null || true
  fi
  if [ -n "$watchdog" ]; then
    # Its `sleep` is a child: stop the children first, then the subshell.
    pkill -TERM -P "$watchdog" 2>/dev/null || true
    kill -TERM "$watchdog" 2>/dev/null || true
    wait "$watchdog" 2>/dev/null || true
  fi
  [ -z "$fired" ] || rm -f "$fired"
  # shellcheck disable=SC1090
  [ -s "$scratch_env" ] && source "$scratch_env" && [ -n "${SC_BD_LIVE_ROOT:-}" ] \
    && "$repo/scripts/bd-compat/teardown-scratch.sh" "$SC_BD_LIVE_ROOT" || true
  rm -f "$scratch_env"
  return "$rc"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

# Runs a command for at most $1 seconds, with no dependency on GNU timeout
# (macOS has none). The command runs in its own process group (job control), so
# the watchdog's TERM, then KILL, reaches cargo, the test binary and any `bd`
# it started. `cleanup` stops the group, the watchdog and its `sleep` however
# this ends; the watchdog only signals while the group leader is alive.
run_bounded() {
  local secs="$1"; shift
  fired="$(mktemp)"
  rm -f "$fired"
  set -m
  "$@" &
  run_pid=$!
  set +m
  local pid="$run_pid" marker="$fired"
  (
    sleep "$secs" &
    nap=$!
    trap 'kill "$nap" 2>/dev/null; exit 0' TERM
    wait "$nap" || exit 0
    kill -0 "$pid" 2>/dev/null || exit 0
    echo "run-live.sh: timed out after ${secs}s; terminating the test run" >&2
    touch "$marker"
    kill -TERM -- "-$pid" 2>/dev/null || kill -TERM "$pid" 2>/dev/null || true
    sleep 10 &
    nap=$!
    wait "$nap" || exit 0
    kill -0 "$pid" 2>/dev/null || exit 0
    kill -KILL -- "-$pid" 2>/dev/null || kill -KILL "$pid" 2>/dev/null || true
  ) &
  watchdog=$!
  local status=0
  wait "$run_pid" || status=$?
  # After a timeout cargo may exit on TERM while the test binary or a `bd` it
  # started is still in the group: `cleanup` kills what is left. After a normal
  # exit the leader is reaped and its id may be reused: leave the group alone.
  [ -e "$fired" ] || run_pid=""
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
