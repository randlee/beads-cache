# sc-beads

The beads backend contract: the traits, row types, errors and clock that Rust
code uses to talk to a beads `bd` backend. Shared by beads-cache and
beads-task-issue-tracker (btit).

This crate holds contracts and pure logic only. It spawns no process and opens
no file. The `bd` CLI implementation is
[`sc-bd-cli`](https://crates.io/crates/sc-bd-cli).

## Contents

- **btit's contract, unchanged.** The `BeadsBackend`, `CliBackend`,
  `DoltOperations` and `CloseSuggestions` traits, `BeadsError`, and the btit
  data types (`ProjectRef`, `BdRawIssue`, `ListQuery`, ...), ported from
  randlee/beads-task-issue-tracker at `8d8e82ae9f85`. A btit consumer switches
  by changing its `btit_types::` and `btit_beads::` imports to `sc_beads::`.
  The one deliberate change: the `log_*!` macros emit through `tracing`, and
  `logging::tracing` replaces `logging::log`.
- **The cache extension.** `CacheBackend: BeadsBackend` holds the calls a
  cache needs: `fingerprint`, `list_by_ids`, `list_scope`, `list_blocked`,
  `list_ready`, `close_guarded`, `update_guarded` and `dep_add_typed`. Each
  one takes a `Deadline` and returns `BeadsCacheError` (`Unavailable`,
  `Refused(Refusal)`, `Contract`, `Failed`, `CheckFailed`, or
  `Btit(BeadsError)`). Its value types are `BeadId`, `Actor`, `Assignee`,
  `Edge`, `Patch`, `Guards`, `MetaValue`, `ScopeFilter`, `SyncCursor` and
  `Fingerprint`. Constructors validate, so an invalid value is never built.
- **Strict rows.** `Bead` and `BlockedRow` are generated from the `bd schema`
  output of bd 1.3.1. `parse_bd_row`, `parse_bd_rows` and `parse_blocked_rows`
  reject an unknown key, a missing required key or a wrong type with
  `BeadsCacheError::Contract`, which names the bead and the field path.
  `BD_VERSION` (`"1.3.1"`), `BD_SCHEMA_VERSION` (`"1"`) and `BD_SCHEMA_SHA256`
  identify the pinned schema.
- **Time.** `Clock`, `SystemClock`, `Deadline`, `Timestamp` (RFC 3339 UTC,
  second precision) and the `rfc3339_to_unix` / `unix_to_rfc3339` helpers.

## Example

```rust
use std::time::Duration;
use sc_beads::{parse_bd_rows, BeadsCacheError, Clock, Deadline, SystemClock};

// A call is handed the earlier of the caller's limit and the per-call cap.
let clock = SystemClock;
let call_cap = Deadline::after(&clock, Duration::from_secs(30));
let caller = Deadline::after(&clock, Duration::from_millis(500));
let d = caller.earliest(call_cap);
assert!(d.remaining(&clock) <= Duration::from_millis(500));

// Validate `bd list --json` output against the strict row contract.
let stdout = r#"[{"id": "bk-1", "title": "t", "status": "open", "priority": 2,
  "issue_type": "task", "created_at": "2026-10-23T09:00:00Z",
  "updated_at": "2026-10-23T09:00:00Z"}]"#;
let rows = parse_bd_rows(stdout)?;
assert_eq!(rows[0].id.as_str(), "bk-1");

// An unknown key is a contract error naming the bead and the field.
let bad = parse_bd_rows(r#"[{"id": "bk-2", "surprise": 1}]"#);
assert!(matches!(bad, Err(BeadsCacheError::Contract { .. })));
# Ok::<(), BeadsCacheError>(())
```

## Feature flags

| Feature | Enables |
|---|---|
| `testing` | `testing::ManualClock`, `testing::FakeBackend` (an in-memory bd), the `bead`, `id`, `actor` and `edge` fixtures, and the `CacheBackend` contract suite (`testing::contract`, `backend_contract_tests!`) that every implementation must pass |

With `testing`, a `FakeBackend` stands in for bd:

```rust
use std::sync::Arc;
use std::time::Duration;
use sc_beads::testing::{bead, id, FakeBackend, ManualClock};
use sc_beads::{CacheBackend, Deadline, ProjectRef};

let clock = Arc::new(ManualClock::new("2026-10-23T09:00:00Z"));
let bd = FakeBackend::new(Arc::clone(&clock));
bd.insert(bead("bk-1"));

let project = ProjectRef::local(Some("/work/project".to_owned()));
let d = Deadline::after(&*clock, Duration::from_secs(5));
let rows = bd.list_by_ids(&project, &[id("bk-1")], d)?;
assert_eq!(rows.len(), 1);
# Ok::<(), sc_beads::BeadsCacheError>(())
```

## Compatibility

The strict rows match bd 1.3.1's `bd schema` (schema version 1). When bd
changes its row contract incompatibly, the change is made here and in
`sc-bd-cli` and released as a semver-major version of both, so each consumer
upgrades once. The rest of btit's code targets bd 1.x (`MIN_SUPPORTED_BD_MAJOR`
is 1). MSRV: Rust 1.98.1. The crate uses edition 2021, so the ported btit code
stays verbatim.

## Links

- Repository: <https://github.com/randlee/beads-cache>
- Architecture: [`docs/sc-beads/architecture.md`](https://github.com/randlee/beads-cache/blob/develop/docs/sc-beads/architecture.md),
  and ADR-BDC-0004 and ADR-BDC-0007 in [`docs/architecture.md`](https://github.com/randlee/beads-cache/blob/develop/docs/architecture.md)
- Requirements: [`docs/sc-beads/requirements.md`](https://github.com/randlee/beads-cache/blob/develop/docs/sc-beads/requirements.md)

License: MIT.
