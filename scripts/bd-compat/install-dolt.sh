#!/usr/bin/env bash
# Installs the Dolt release the scratch server runs and prints the directory holding `dolt`.
#
# Usage: install-dolt.sh [dolt version] [install dir]
#
# Only the live probes need Dolt, as the database server behind a scratch workspace.
set -euo pipefail

version="${1:-2.3.5}"
[[ "$version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]] || { echo "not a dolt version: $version" >&2; exit 2; }
dest="${2:-${RUNNER_TEMP:-${TMPDIR:-/tmp}}/dolt-$version}"

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

work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT
curl --fail --silent --show-error --location --retry 3 --max-time 180 \
  -o "$work/dolt.tar.gz" \
  "https://github.com/dolthub/dolt/releases/download/v${version}/dolt-${os}-${arch}.tar.gz"
mkdir -p "$dest"
tar -xzf "$work/dolt.tar.gz" -C "$work"
install -m 0755 "$work/dolt-${os}-${arch}/bin/dolt" "$dest/dolt"
echo "$dest"
