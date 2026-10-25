#!/usr/bin/env bash
# Compares the installed bd with the pinned fingerprint (REQ-BDC-0905).
#
# Usage: fingerprint-check.sh <expected bd version> <output dir> [expected schema sha]
#
# - `bd version` must report <expected bd version> (the right binary was installed).
# - The sha256 of `bd schema` must equal the pinned BD_SCHEMA_SHA256 (or the sha given
#   as the third argument, a hook for exercising the failure path). On a difference the
#   schema diff against the pinned fixture is written to <output dir>/schema.diff and
#   the script exits 1.
# - A bd version other than the pinned BD_VERSION with an unchanged schema passes, with
#   a notice: the model is current, the pin is just older.
#
# Writes <output dir>/summary.md in every case.
set -euo pipefail
# shellcheck source=lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

expected_version="${1:?usage: fingerprint-check.sh <bd version> <output dir> [schema sha]}"
out="${2:?usage: fingerprint-check.sh <bd version> <output dir> [schema sha]}"
pinned_version="$(bdc_pinned BD_VERSION)"
pinned_sha="$(bdc_pinned BD_SCHEMA_SHA256)"
expected_sha="${3:-$pinned_sha}"
fixture="$(bdc_repo_root)/crates/sc-beads/tests/fixtures/bd-${pinned_version}/bd-schema.json"
mkdir -p "$out"

reported="$(bd version | sed -nE 's/^bd version ([^ ]+).*/\1/p' | head -n 1)"
bd schema >"$out/bd-schema.json"
actual_sha="$(bdc_sha256 <"$out/bd-schema.json")"

status=0
{
  echo "| | pinned | this bd |"
  echo "|---|---|---|"
  echo "| bd version | \`$pinned_version\` | \`$reported\` |"
  echo "| \`bd schema\` sha256 | \`$expected_sha\` | \`$actual_sha\` |"
  echo
} >"$out/summary.md"

if [ "$reported" != "$expected_version" ]; then
  echo "bd reports version $reported, expected $expected_version" | tee -a "$out/summary.md" >&2
  status=1
fi
if [ "$actual_sha" != "$expected_sha" ]; then
  echo "The \`bd schema\` output changed: the strict rows must be regenerated (a maintainer step)." \
    | tee -a "$out/summary.md" >&2
  diff -u "$fixture" "$out/bd-schema.json" >"$out/schema.diff" || true
  status=1
elif [ "$reported" != "$pinned_version" ]; then
  echo "bd $reported has the pinned schema (pin is $pinned_version): the strict model is current." \
    | tee -a "$out/summary.md"
fi
exit "$status"
