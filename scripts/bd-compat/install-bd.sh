#!/usr/bin/env bash
# Installs a released bd on a clean machine and prints the directory holding `bd`.
#
# Usage: install-bd.sh <bd version> [install dir]
#
# Downloads beads_<version>_<os>_<arch>.tar.gz from the steveyegge/beads release,
# checks it against the release's checksums.txt and unpacks `bd`. Linux and macOS,
# amd64 and arm64.
set -euo pipefail
# shellcheck source=lib.sh
source "$(dirname "${BASH_SOURCE[0]}")/lib.sh"

version="${1:?usage: install-bd.sh <bd version> [install dir]}"
bdc_valid_version "$version" || { echo "not a bd version: $version" >&2; exit 2; }
dest="${2:-${RUNNER_TEMP:-${TMPDIR:-/tmp}}/bd-$version}"

case "$(uname -s)" in
  Linux) os=linux ;;
  Darwin) os=darwin ;;
  *) echo "unsupported OS: $(uname -s)" >&2; exit 2 ;;
esac
case "$(uname -m)" in
  x86_64 | amd64) arch=amd64 ;;
  arm64 | aarch64) arch=arm64 ;;
  *) echo "unsupported CPU: $(uname -m)" >&2; exit 2 ;;
esac

asset="beads_${version}_${os}_${arch}.tar.gz"
base="https://github.com/steveyegge/beads/releases/download/v${version}"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

curl --fail --silent --show-error --location --retry 3 --max-time 120 -o "$work/$asset" "$base/$asset"
curl --fail --silent --show-error --location --retry 3 --max-time 60 -o "$work/checksums.txt" "$base/checksums.txt"
expected="$(grep -F "  $asset" "$work/checksums.txt" | cut -d' ' -f1)"
[ -n "$expected" ] || { echo "no checksum for $asset" >&2; exit 1; }
actual="$(bdc_sha256 <"$work/$asset")"
[ "$expected" = "$actual" ] || { echo "checksum mismatch for $asset" >&2; exit 1; }

mkdir -p "$dest"
tar -xzf "$work/$asset" -C "$dest" bd
chmod +x "$dest/bd"
echo "$dest"
