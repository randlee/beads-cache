#!/usr/bin/env bash
# Runs the live probes (crates/sc-bd-cli/tests/live.rs) in a scratch workspace.
#
# Usage: run-live.sh [cargo test arguments...]
#
# Needs `bd` and `dolt` on PATH (or SC_BD_BIN and DOLT_BIN_DIR), and a Rust toolchain.
# Used by hand on a testbed and by .github/workflows/bd-compat.yml.
# Everything it creates is removed on exit; no shared database is read or written.
set -euo pipefail
# shellcheck source=lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

repo="$(bdc_repo_root)"
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
cargo test -p sc-bd-cli --all-features --test live "$@" -- --ignored --test-threads=1
