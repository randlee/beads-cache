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

"$repo/scripts/bd-compat/setup-scratch.sh" "$scratch_env"
set -a
# shellcheck disable=SC1090
source "$scratch_env"
set +a
cd "$repo"
cargo test -p bd-cache-sync --all-features --test live "$@" -- --ignored --test-threads=1 --nocapture
