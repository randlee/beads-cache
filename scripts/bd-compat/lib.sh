#!/usr/bin/env bash
# Shared helpers for the bd compatibility scripts (REQ-BDC-0905). Sourced, not run.

# The repository root, from this file's location.
bdc_repo_root() {
  cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd
}

# The pinned fingerprint, read from the generated strict model so there is one source.
# Usage: bdc_pinned BD_VERSION | BD_SCHEMA_SHA256 | BD_SCHEMA_VERSION
bdc_pinned() {
  local name="$1" file
  file="$(bdc_repo_root)/crates/sc-beads/src/row/generated.rs"
  # `pub const NAME: &str = "value";`, possibly with the value on the next line.
  tr -d '\n' <"$file" \
    | grep -oE "pub const ${name}: &str =[[:space:]]*\"[^\"]*\"" \
    | head -n 1 \
    | sed -E 's/.*"([^"]*)"$/\1/'
}

# A bd version is a semantic version; anything else is refused before it reaches a URL
# or a shell (the repository_dispatch payload is not trusted).
bdc_valid_version() {
  [[ "$1" =~ ^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$ ]]
}

bdc_sha256() {
  if command -v sha256sum >/dev/null 2>&1; then sha256sum | cut -d' ' -f1; else shasum -a 256 | cut -d' ' -f1; fi
}
