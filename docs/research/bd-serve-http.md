# Research: `bd serve` v0 HTTP API as a bd-cache backend

Date: 2026-10-23. Sources: beads `internal/httpapi/spec/openapi.v0.yaml` at
tag v1.3.0 (line numbers below refer to it), beads main, `bd serve --help`
(bd 1.3.0), and the beads research docs (`usage.md`,
`agent-coordination-architecture.md`, `security.md`, `architecture.md`).
Nothing was run against a real database.

## Summary

- Use a `BdBackend` port with two adapters, CLI now and HTTP later. Don't go
  HTTP-only yet.
- In released bd 1.3.0/1.3.1, `GET /v0/beads/issues` has no `id` filter and no
  `updated_after` filter. Both landed on main (spec commits #7550 on
  2026-10-14 and #7647 on 2026-10-17) and are not in any tag through
  v1.3.2-rc.3.
- `bd serve` refuses embedded Dolt and `--readonly`, so it needs a Dolt SQL
  server.
- Where it runs, it beats the CLI: the same pinned row structs, typed 409s, a
  `revision` CAS token, and a resumable events journal.

## 1. Endpoint mapping (v1.3.0)

| bd-cache operation | HTTP | Notes |
|---|---|---|
| List by ids | none (`id` param main only) | Workarounds: `GET /v0/beads/issues:query?q=id=a OR id=b...&all=true&limit=0` (L1580; no cursor; `limit=0` refused when non-loopback), or one `GET /issues/{id}` per id. `batchGet` is main only. |
| Label + updated-after (sync) | `GET /issues` with `label` (AND), `label_any`, `exclude_label`, `status` (repeatable, replaces the default exclusions), `limit` (default 50, 0 = unlimited), opaque `cursor` (L1098-1360) | No `updated_after`, only `created_*`. Workaround: `issues:query?q=label=phase-x AND updated>T&all=true`. `sort` is closed to `created` and `priority`; `updated` is excluded on purpose because it is not a stable cursor key. |
| Get | `GET /issues/{id}` | Returns `IssueDetails`: `revision`, labels and dependency objects (L2042). |
| Close with reason | `POST /issues/{id}:close` `{actor, reason, session, force, expected_version}` | First close wins. No `expected_status` (ApplyCloseItem L9241). |
| Update | `PATCH /issues/{id}` `{actor, patch, expected_version \| expected_status \| expected_assignee}` (L10219) | `patch` covers status, assignee (empty string unassigns), `labels` (replace) plus `add_labels`/`remove_labels`, `append_notes` or `notes`, and `metadata: {replace\|merge\|set\|unset}`. Cannot write `close_reason`. Response `{issue, changed, revision}`. |
| Multi-op atomic write | `POST issues:batchApply` | Per-item guards. |
| Dep add | `POST /dependencies:add` `{actor, edges:[{issue_id, depends_on_id, type}]}` | Max 100 edges, all-or-nothing. A same-type re-add is idempotent and echoed back. |
| Edge read | `GET /dependencies?issue_id=&type=` | No paging. |
| Schema / version | `GET /v0/beads/context` | `api_version`, `bd_version`, `schema_version` (diagnostic only), `project_id`, `dolt_mode`, `capabilities[]` (L7181). `wire_revision` main only. |

## 2. Row shape

- `IssueWithCounts` (list, ready and query rows) and
  `Issue`/`IssueDetails`/`EventRecord` are pinned via `x-go-type` to the same
  Go structs that `bd list --json` and `bd schema` reflect. They are "one
  compatibility domain", enforced by a bijection test (spec L265-275;
  `bd schema --help`).
- `/v0` only adds optional fields, so a deny-unknown-fields validator breaks
  on additive releases. Gate it on the backend fingerprint, as the cache does
  today.
- `IssueDetails` is a superset of the list row (`revision`, dependency
  objects, dependents). It needs its own serde type or a projection.
- The OpenAPI 3.0.3 YAML is hand-written and is the source of truth (Go types
  are generated from it). It is usable for codegen (progenitor,
  openapi-generator). Expect repeated property lists rather than `allOf`.
- `revision` appears on get, write responses and batchApply results only,
  never on list rows. List rows double as `bd export` JSONL, and a per-write
  random token would churn that file (L1063-1093). Model an absent `revision`
  as absent, not 0.
- `revision` has been a decimal string since 1.3.0. It was an integer before;
  the change is wire_revision 1.

## 3. Preconditions

- HTTP offers `expected_status` and `expected_assignee`, which match
  `--if-status`/`--if-assignee`, plus `expected_version` (full-row CAS).
- A miss returns `409 precondition_failed` and writes nothing. `actual_*` is
  deliberately absent, so the caller re-reads (L2296).
- Close takes no `expected_status`. For a guarded close, GET the `revision`,
  then `:close` with `expected_version`. That guard is checked before the
  idempotent `already_closed`.
- For the drain loop (achieved, precondition, write, read-back), HTTP is
  strictly better:
  - The typed 409 code vocabulary (`precondition_failed`, `not_closable`,
    `already_claimed`) replaces exit 13.
  - `changed` / `already_closed` tell you whether the write happened.
  - The write response carries the post-write issue and `revision`, so the
    read-back is free.
- Keep journal entries field-level (`expected_status`/`expected_assignee`)
  and store `revision` as an optional opaque string.

## 4. Events journal

Endpoints: `GET /v0/beads/events?since=` (poll) and `events:watch` (SSE;
`id:` = seq, resume with `Last-Event-ID`, ~20 s heartbeat, at most 48
streams).

- `seq` is gapless and strictly increasing per replica and branch. Each
  record carries a full post-mutation snapshot.
- Off by default, and every writer must enable it. Otherwise the journal is
  empty, and empty looks the same as caught up.
- Retention defaults to 7 days / 100k rows. Falling behind returns
  `410 events_journal_truncated` (or an in-band `truncated` event); then
  re-baseline.
- Not journaled: `bd dolt pull`, merges and `bd sql`. Seq is per server URL,
  so keep one checkpoint per URL.
- With one shared dolt sql-server it can replace `--updated-after` polling.
  Across replicas or federation it cannot. Keep `updated_after` (or
  `q=updated>T`) as the safety net and re-baseline path.

## 5. Performance and deployment

- A pooled keep-alive request takes about 12 ms on loopback (runbook
  request-log example). A CLI call costs 130 ms or more, plus Dolt handshake
  stalls; the long-lived server pays the handshake once.
- Needs a Dolt SQL server. Refuses embedded workspaces permanently
  (SERVE_RUNBOOK L418) and refuses `--readonly` (L419).
- Remote use needs `--allow-non-loopback` plus `--auth-token-file` (bearer
  token, one per line, live reload), `--allowed-host` for service names, and
  your own TLS or mesh. `limit=0` is refused there.
- Send `Bd-Project-Id` (the value from `/context`) so a wrong server refuses
  the request.
- Hooks don't fire on HTTP writes.
- Liveness is `/healthz`; readiness is `GET /v0/beads/ready?limit=1`.
- Capability probing:
  - operations: `capabilities[]`
  - behaviour: `bd_version`
  - parameters: send one; `400 reason=unknown_parameter` means unsupported
    (the `sort` probe pattern)

## 6. Stability

- The spec is absent at v1.0.3, v1.1.2 and v1.2.2. It was in the withdrawn
  v1.2.0 and first shipped stable in v1.3.0 (2026-09-15).
- One silent breaking change under `/v0` already: `revision` went from
  integer to string in 1.3.0. `wire_revision` / `Bd-Wire-Revision` were added
  afterwards (main; `CurrentWireRevision=2`).
- Main moves fast: about 15 spec commits in October. Main also ships its own
  Go HTTP client in bd (`internal/httpclient`, 2026-10-05), so bd itself is
  heading towards an HTTP backend.
- The stated promise is "additive only; breaking changes cut `/v1`", but the
  API is young.

## 7. Recommendation

A port with both adapters, CLI first. Phase 1 should:

- Define backend-neutral operations: `list_by_ids`, `list_since(label, t)`,
  `get`, `close`, `update(patch, guards)`, `dep_add`, `fingerprint`.
- Make the patch model mirror `IssuePatchBody` (label replace/add/remove,
  metadata set/unset/merge, append_notes). The CLI adapter maps it to flags.
- Return a typed `Refusal` enum (`Precondition`, `NotClosable`,
  `AlreadyClaimed`, `Other`), with exit 13 mapped to `Precondition`.
- Store `revision: Option<String>`, null under the CLI.
- Validate rows against the shared Go-struct schema (`bd schema`
  `types.issue`), keyed by a backend fingerprint of `bd_version` plus the
  schema hash. Add `/context` capabilities later.
- Add a nullable per-backend-URL `journal_seq` checkpoint.
- Keep the actor explicit on every write.

Turn the HTTP adapter on when a release ships the `id` and `updated_after`
filters and the deployment runs a Dolt SQL server.
