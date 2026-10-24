# Vendored reference implementation (BD-CACHE v3)

Copied byte for byte from randlee/atm-core at
`05831dfe866d389feed138add3c29cb93cd6c82b` (PR randlee/atm-core#2219), for
the compatibility tests in `../compat.rs` (REQ-BDC-0804). Do not edit these
files; re-vendor them from a later atm-core commit instead. The local
`.gitattributes` keeps every file here LF on all platforms, so the sha256
values below hold on a Windows checkout too.

| File | Source path at `05831df` | sha256 (LF) |
|---|---|---|
| `bd_cache.py` | `.claude/skills/atm-bd-orchestration/scripts/bd_cache.py` | `0021c85c9ede8e4a752eef681600bc2d2520148427e4430da8ce1aabb78d7bc0` |
| `bd_models.py` | `.claude/skills/atm-bd-orchestration/scripts/bd_models.py` | `612059ef99b212c9abacf8ebfe9b44107373fbabc505b2326ae7c663478e03a6` |
| `fixtures/bd-1.3.1/bd-schema.json` | `.claude/skills/atm-bd-orchestration/scripts/tests/fixtures/bd-1.3.1/bd-schema.json` | `caf8fe18e1117c067ff7bbd824484b24849677e2cbf86d793ddf6b64069a31a2` |
| `fixtures/bd-1.3.1/list-rows.json` | `.claude/skills/atm-bd-orchestration/scripts/tests/fixtures/bd-1.3.1/list-rows.json` | `23f0a1f8a8bcda73897a61d00a2a7accee61b8cf3c7bb5d02d4ee05090ae5ead` |

The two fixtures are the v3 acceptance fixtures (also vendored, identical,
under `crates/sc-beads/tests/fixtures/bd-1.3.1/`): the recorded rows are what
the reference implementation syncs into a v3 file, and what the Rust store
must read back equal.

`driver.py` is not vendored: it is this crate's test driver. It runs the
reference code with a deterministic bd (version, schema and list answered
from the fixtures; no write commands), and its `cli` command runs
`bd_cache.py`'s own CLI (`get`, `status`, `prune`). On Windows it stubs
`fcntl`, which `bd_cache.py` imports only for its drain lock; the commands
run there never take that lock, and the cross-implementation lock test is
Unix-only.

Run by `cargo test -p bd-cache-store --test compat` through
`uv run --no-project --with 'pydantic>=2,<3' python driver.py ...`; CI
installs uv (`.github/workflows/ci.yml`). Without uv the tests fail; they
are never skipped.
