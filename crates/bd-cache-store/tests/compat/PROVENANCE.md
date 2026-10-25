# Vendored reference implementation (BD-CACHE v3)

Copied byte for byte from atm-core `origin/main` at
`16952200554e5b39707362f6dbf3f352be159e92` (`1695220`, merge of release 0.19.2;
first vendored at `05831df`, PR randlee/atm-core#2219, then re-vendored), for
the compatibility tests in `../compat.rs` (REQ-BDC-0804). Do not edit these
files; re-vendor them from a later atm-core commit instead. The local
`.gitattributes` keeps every file here LF on all platforms, so the sha256
values below hold on a Windows checkout too.

| File | Source path at `1695220` | sha256 (LF) |
|---|---|---|
| `bd_cache.py` | `plugins/atm-bd-orchestration/skills/atm-bd-orchestration/scripts/bd_cache.py` | `c398f1533094e145ccb0aa52405c9627d6dd28e88489d9e10d9e6866e07076f5` |
| `bd_models.py` | `plugins/atm-bd-orchestration/skills/atm-bd-orchestration/scripts/bd_models.py` | `612059ef99b212c9abacf8ebfe9b44107373fbabc505b2326ae7c663478e03a6` |
| `fixtures/bd-1.3.1/bd-schema.json` | `plugins/atm-bd-orchestration/skills/atm-bd-orchestration/scripts/tests/fixtures/bd-1.3.1/bd-schema.json` | `caf8fe18e1117c067ff7bbd824484b24849677e2cbf86d793ddf6b64069a31a2` |
| `fixtures/bd-1.3.1/list-rows.json` | `plugins/atm-bd-orchestration/skills/atm-bd-orchestration/scripts/tests/fixtures/bd-1.3.1/list-rows.json` | `23f0a1f8a8bcda73897a61d00a2a7accee61b8cf3c7bb5d02d4ee05090ae5ead` |

Drift from `05831df` to `1695220`: `bd_models.py` and both fixtures are
identical. `bd_cache.py` gains `BdCache.entry(seq)` (one journal row, or
`NO_ENTRY`) and a `__doc__ or` guard in `main`; the store format, the drain and
the failure handling did not change, so no Rust change followed.

The two fixtures are the v3 acceptance fixtures (also vendored, identical,
under `crates/sc-beads/tests/fixtures/bd-1.3.1/`): the recorded rows are what
the reference implementation syncs into a v3 file, and what the Rust store
must read back equal.

`driver.py` is not vendored: it is this crate's test driver. It runs the
reference code with a deterministic bd (version, schema and list answered
from the fixtures; `Runner` refuses write commands). Its `drain` command uses
`WritingRunner`, a fake bd that also accepts `close` and `update` and applies
them to its rows, for the mixed-cutover test (Python drains what Rust
journaled). Its `cli` command runs
`bd_cache.py`'s own CLI (`get`, `status`, `prune`). On Windows it stubs
`fcntl`, which `bd_cache.py` imports only for its drain lock; the commands
run there never take that lock, and the cross-implementation lock test is
Unix-only.

Run by `cargo test -p bd-cache-store --test compat` through
`uv run --no-project --with 'pydantic>=2,<3' python driver.py ...`; CI
installs uv (`.github/workflows/ci.yml`). Without uv the tests fail; they
are never skipped.
