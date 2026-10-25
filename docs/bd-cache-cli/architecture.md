# bd-cache-cli Architecture

Status: Draft (sprint b-3a). Crate-local decisions (domain code `CLI`).
Repo-level context: [`../architecture.md`](../architecture.md) (ADR-BDC-0009
puts the crate in the Boundaries table). Requirements:
[`requirements.md`](requirements.md). Contract: [`contract.md`](contract.md).
Created by sprint b-3a.

`bd-cache-cli` holds the binaries `bd-cache` and `bdc` and the `fake-bd` test
harness. In sprint b-3a it is a skeleton: `--version` and the `USAGE` error.
Sprint b-3 adds the commands over `bd-cache-api`.

| Module / target | Contents |
|---|---|
| `lib` | `run(argv, env, invoker, clock) -> (i32, String)`, `run_process()`, `NAME`, `CONTRACT_VERSION`; the clap grammar (`--version`, `--repo`, `--cache-path`, `--bd`) |
| `scenario` (feature `fake-bd`) | `Scenario`, `Faults`, `RefuseWrite`, `serve`: the scenario file and one `fake-bd` invocation |
| `bin/bd-cache`, `bin/bdc` | `fn main() -> ExitCode { run_process() }`, identical |
| `bin/fake-bd` (feature `fake-bd`) | reads `$FAKE_BD_SCENARIO`, calls `serve`, prints, exits |
| `tests/support` | `TestRepo` (temp repo with `.beads/metadata.json` and a scenario), `parse_envelope`, process and in-process `BdCli` constructors |

## ADR-CLI-0001: sc-command is a git dependency, so the crate is `publish = false`

**Status:** Draft

Context: the CLI replies in sc-command's `Envelope` (contract §4, ADR-BDC-0009).
sc-command is in randlee/sc-runtime and is not published to crates.io; a crate
with a git dependency cannot be published. The three crates that are published
(`sc-beads`, `sc-bd-cli`, `sc-cron`) have no such dependency, and CI dry-runs
their publication.

Decision:

- sc-command is a git dependency on `https://github.com/randlee/sc-runtime`
  at the tag `prerelease/v0.1.0` (`ba971c1`), declared once in
  `[workspace.dependencies]`. A tag, not a branch, so the build does not move.
  If the tag fails to build on a CI OS, the dependency is pinned by `rev`
  instead and the failure is raised with the sc-runtime team; there is no local
  fork (phase b plan, P-7).
- `bd-cache-cli` is `publish = false`. The CLI ships as binaries (GitHub
  releases and package channels, sprint b-5), not as a crate.
- The crates other than this one (`bd-cache-api` and `bd-cache-py` especially)
  take neither sc-command nor clap, so the published and bound surfaces are
  unaffected. CI checks it.
- `sc-observability-types` (the type of an sc-command error code) is a normal
  crates.io dependency at `1.5`, the version sc-command itself uses.
- `Cargo.lock` is committed, so the git revision is fixed by the lock as well.

The pin was checked first on all three CI OSes (ubuntu, macOS, Windows),
and on a Windows machine over SSH, before the harness was built on it.

Consequences: a release needs network access to GitHub for the dependency.
Moving the pin is a deliberate edit to `Cargo.toml` and `Cargo.lock`. cargo
prints one line, `error: invalid key-value pair, expected key`, for the
`template/` directory of the sc-runtime checkout while it resolves the
dependency; it is cargo's note about a file that is not a manifest, the command
still exits 0, and it is not ours to fix.

## ADR-CLI-0002: `fake-bd` is a feature-gated binary over `FakeBd` with scenario-file state

**Status:** Draft

Context: contract §8 "Testing" needs a real-process simulator: tests run `bdc`
as a process with `--bd <fake-bd>`, and the NFR-BDC-0002 latency check does too.
sc-bd-cli already has a stateful in-process `FakeBd` over `FakeBackend`
(feature `test-support`), which is the model of bd the rest of the tests use.

Decision:

- `fake-bd` runs that same `FakeBd`; it does not model bd a second time.
- The binary has `required-features = ["fake-bd"]`, and the feature enables
  `sc-bd-cli/test-support` and `sc-beads/testing`. A **self dev-dependency**
  with the feature (`bd-cache-cli = { path = ".", features = ["fake-bd"] }`)
  turns it on for `cargo test`, so the integration tests can use
  `CARGO_BIN_EXE_fake-bd`. A release build (`cargo build --release --bin
  bd-cache --bin bdc`) does not build dev-dependencies and does not build
  `fake-bd`. The boundary check does not count a crate's dev-dependency on
  itself as an edge.
- The state is a file, so that a sequence of processes sees one bd. It is
  loaded on every start and written back (temporary file, then rename) after
  a call that changed a row. Calls are sequential; there is no file lock.
- Faults are fields of the scenario, not flags: they apply to every call of
  the scenario, and a test changes them by rewriting the file.
- The harness code that is not the binary (`scenario`) is part of the library
  behind the feature, so the tests build scenarios with the same type that
  `fake-bd` reads, and a typo in a hand-written scenario is an error
  (`deny_unknown_fields`).

Consequences: `fake-bd` answers only the command forms `FakeBd` serves (the
cache's calls, `version`, `schema`; `--version` as `version`). A bd command it
does not serve is exit 2, as a usage error.

### The scenario file

One JSON object. Every key is optional.

| Key | Type | Meaning |
|---|---|---|
| `now` | RFC 3339 string | The fixed UTC time of every process. A write stamps `updated_at` with it. Default `2026-10-23T10:00:00Z`. An invalid value panics (a scenario bug). |
| `version` | string | What `bd version` prints. Default: the pinned release. |
| `schema` | string | What `bd schema` prints. The schema fingerprint the cache sees is the sha256 of this text. Default: the pinned bd 1.3.1 schema. |
| `rows` | array of bd rows | The beads, in `sc_beads::Bead` form (`id`, `title`, `status`, `priority`, `issue_type`, `created_at`, `updated_at` required; `labels`, `dependencies` of `{issue_id, depends_on_id, type}` and the others optional). Rewritten after a call that changed a row. |
| `blocked` | object: id → array of ids | Each bead is blocked by the beads listed: applied as `blocks` edges on top of `rows`. The blocked set that `bd blocked` reports follows bd's rule, so a closed blocker stops blocking. An id that is not a row is a harness error. |
| `faults.unavailable` | bool | Every call that reaches the database fails as bd does with its Dolt server down: exit 1, "Dolt server unreachable ... connection refused". `bd version` and `bd schema` still answer. |
| `faults.slow_ms` | integer | Every call sleeps this long before it answers. The delay is real, so the caller's own timeout ends it. |
| `faults.refuse_write` | `{exit_code}` | Every write (`close`, `update`, `dep add`) fails with this exit code and changes nothing. `13` is bd's refused guard (precondition); another code is a plain failure with that code. |

Any other key is rejected. A scenario that cannot be read, parsed or applied
is exit 70 with the reason on stderr; bd's own codes are 0, 1, 2 and 13.

Example:

```json
{
  "now": "2026-10-23T10:00:00Z",
  "rows": [
    {"id": "b-1", "title": "b-1", "status": "open", "priority": 2,
     "issue_type": "task", "labels": ["phase-x"],
     "created_at": "2026-10-23T09:00:00Z", "updated_at": "2026-10-23T09:00:00Z"},
    {"id": "b-2", "title": "b-2", "status": "open", "priority": 2,
     "issue_type": "task", "labels": ["phase-x"],
     "created_at": "2026-10-23T09:00:00Z", "updated_at": "2026-10-23T09:00:00Z"}
  ],
  "blocked": {"b-2": ["b-1"]},
  "faults": {"slow_ms": 500}
}
```
