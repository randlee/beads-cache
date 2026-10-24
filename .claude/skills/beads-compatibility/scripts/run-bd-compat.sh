#!/usr/bin/env bash
# Live bd compatibility runs on the testbed (atmbench@rand-m5). Copied there as
# ~/run-bd-compat-<version>.sh by the beads-compatibility skill and run as:
#
#   ~/run-bd-compat-<version>.sh <bd version> <sha7>
#
# Expects the tree under test at ~/github/beads-cache-compat-<version>-<sha7>
# (a `git archive` of that commit). Installs or confirms bd <version> and the
# workflow's Dolt release with the repo's install scripts, then runs:
#   1. scripts/bd-compat/run-live.sh                        (pinned check)
#   2. SC_BD_PINNED_ANY=1 scripts/bd-compat/run-live.sh     (any bd release)
#   3. crates/bd-cache-sync/tests/run-live.sh               (a-7 live, NFR-BDC-0003)
# Logs: ~/bd-compat-<version>-<sha7>-{pinned,pinnedany,a7}.log and a summary in
# ~/bd-compat-<version>-<sha7>-summary.txt. Never exits early on a failing run.
set -uo pipefail

version="${1:?usage: run-bd-compat.sh <bd version> <sha7>}"
sha="${2:?usage: run-bd-compat.sh <bd version> <sha7>}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]] || { echo "not a bd version: $version" >&2; exit 2; }
[[ "$sha" =~ ^[0-9a-f]{7,40}$ ]] || { echo "not a sha: $sha" >&2; exit 2; }

# Keep in step with DOLT_VERSION in .github/workflows/bd-compat.yml.
dolt_version="${DOLT_VERSION:-2.3.5}"
tree="$HOME/github/beads-cache-compat-$version-$sha"
logs="$HOME/bd-compat-$version-$sha"
summary="$logs-summary.txt"
export PATH="$HOME/.cargo/bin:$PATH"
cd "$tree" || { echo "no tree at $tree" >&2; exit 9; }

# bd and dolt: reuse an earlier install of the same release, else install it.
bd_dir="$HOME/bd-$version"
if ! "$bd_dir/bd" version 2>/dev/null | grep -q "^bd version $version"; then
  bd_dir="$(scripts/bd-compat/install-bd.sh "$version" "$bd_dir")" || { echo "bd install failed" >&2; exit 10; }
fi
dolt_dir="$HOME/dolt-$dolt_version"
if ! "$dolt_dir/dolt" version 2>/dev/null | grep -q "$dolt_version"; then
  dolt_dir="$(scripts/bd-compat/install-dolt.sh "$dolt_version" "$dolt_dir")" || { echo "dolt install failed" >&2; exit 11; }
fi
export PATH="$bd_dir:$dolt_dir:$PATH"
export SC_BD_BIN="$bd_dir/bd"

{
  echo "host $(hostname) $(date -u +%FT%TZ) tree $tree"
  bd version
  dolt version | head -n 1
  rustup show active-toolchain 2>&1 | tail -n 1
} | tee "$summary"

scripts/bd-compat/run-live.sh >"$logs-pinned.log" 2>&1
echo "pinned_rc=$?" | tee -a "$summary"

SC_BD_PINNED_ANY=1 scripts/bd-compat/run-live.sh >"$logs-pinnedany.log" 2>&1
echo "pinnedany_rc=$?" | tee -a "$summary"

if [ -x crates/bd-cache-sync/tests/run-live.sh ]; then
  SC_BD_PINNED_ANY=1 crates/bd-cache-sync/tests/run-live.sh >"$logs-a7.log" 2>&1
  echo "a7_rc=$?" | tee -a "$summary"
  grep -h "NFR-BDC-0003:" "$logs-a7.log" | tee -a "$summary"
else
  echo "a7_rc=absent (crates/bd-cache-sync/tests/run-live.sh is not in this commit)" | tee -a "$summary"
fi

# Which probes failed, per run (cargo prints `test <name> ... FAILED`).
for run in pinned pinnedany a7; do
  [ -f "$logs-$run.log" ] || continue
  grep -hE "^test result:|^test .* \.\.\. FAILED" "$logs-$run.log" | sed "s/^/$run: /" | tee -a "$summary"
done
exit 0
