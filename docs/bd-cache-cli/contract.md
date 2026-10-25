# bd-cache / bdc CLI contract

Status: **Draft for approval** (Rand). This document is a design; nothing in
it is implemented yet. Contract version: `1.0.0-draft`.

Scope: the `bd-cache` binary (alias `bdc`, REQ-BDC-0805), the MCP server that
comes later, and the Python binding `bd_cache_rs`. All three share one
request, data and error vocabulary, owned by the new `bd-cache-api` crate
(§9).

Governing inputs:

- REQ-BDC-0004, 0801..0805, 0901..0912 and NFR-BDC-0002.
- ADR-BDC-0002, 0003, 0005 and 0008 (Draft).
- Lead ruling S59: REQ-BDC-0806 is superseded; parity with `bd_cache.py` is
  behavioural; the CLI follows sc-command and `/v0`.

This contract replaces the command-structure clause of ADR-BDC-0003. It
depends on **ADR-BDC-0009** (Proposed, in `docs/architecture.md`) and on the
requirement amendments in Appendix A. Both wait for Rand's approval.

## 1. Shape

| Surface | Commands | Vocabulary |
|---|---|---|
| (a) Beads | `bdc issues get\|list\|update\|close`, `bdc ready`, `bdc dependencies list\|add`, `bdc context` | bd serve `/v0` at the **pinned release, beads v1.3.2** @ [`9b65b9e`][rel] |
| (b) Cache management | `bdc cache status\|health\|scopes\|refresh scopes\|refresh issues\|flush\|evict\|journal get\|journal list\|journal resolve\|journal announce\|tick` | conventional cache terms |

Surface (a) is **shaped like `/v0`, but does not conform to it**. The
conformance statement is the divergence list in §5.4. beads `origin/main` @
[`73f64fd`][main] is tracked only as the proposed interface (§5.5). An
operation that exists only on main is not offered until a beads release
contains it.

Rules:

1. **One envelope on stdout** for every command, including `--help` and
   `--version`, and for every failure (sc-command `Envelope`, §4). Stdout
   carries no prose unless `--format text` is given. Logs go to stderr.
2. **`/v0` bodies as is, plus `x_cache`.** On surface (a), `data` is the
   `/v0` response body with one added top-level member, `x_cache`. Rows are
   bd's list rows with the journal overlay applied. Cache facts about a row
   live in `x_cache.rows[id]`, never inside the row.
3. **Absent versus null.** In the `/v0` part, a member the cache cannot know
   is **absent**, never `null` or `0`. In `x_cache` and on surface (b), every
   member is always present, and `null` means unknown. Timestamps the cache
   produces are UTC `YYYY-MM-DDTHH:MM:SS.sssZ`. bd's row timestamps pass
   through verbatim.
4. **A journaled write is not an applied write.** Every write reply carries
   `x_cache.seq`, `x_cache.state` and `x_cache.idempotency_key`.
5. **Nothing is faked.** A `/v0` operation, parameter or member the cache
   does not serve is `BD_CACHE.UNSUPPORTED`. An unknown member is
   `BD_CACHE.BAD_ARGS`. Neither is ever silently ignored.
6. **One decoder.** The CLI turns argv into a JSON request, and the MCP
   handler passes its raw arguments. Both go to the same `bd-cache-api`
   decoder, so validation lives there and every front end returns the same
   code for the same input (§8).
7. **Every list has a stated order** (§5.3, §6). A parameter that carries
   the served default (for example `sort=priority` on `ready`) is accepted.

## 2. Global options and configuration

Global options are accepted before or after the subcommand. Long options must
be spelled out in full; abbreviations are refused.

| Flag | Env `BD_CACHE__…` | `.beads/bd-cache.toml` | Default | Meaning |
|---|---|---|---|---|
| `--repo DIR` | `REPO` | n/a | cwd | The repository root. |
| `--cache-path PATH` | `CACHE_PATH` | `cache_path` | `.bd-cache/<dolt_database>.bd-cache.sqlite3` | The cache file. A relative path is resolved against `--repo`. |
| `--bd PATH` | `BD` | **never** | `bd` on `PATH` | The bd binary. It is never read from TOML, so a checked-in file cannot choose an executable. |
| `--format json\|text` | **never** | **never** | `json` | Read from the flag only. `text` renders the same `data` for a person. |
| `--input FILE\|-` | n/a | n/a | none | The whole request as JSON. Combining it with request flags is `USAGE`. |
| `-v` / `-q` | `LOG` | n/a | `warn` | The verbosity of the stderr log. The flag wins over the env, and stdout never changes. |

The TOML keys `scopes`, `call_timeout_s`, `miss_fill_timeout_ms`,
`max_attempts` and `[tick]` are unchanged. Each one can be overridden from
the env under the same name, for example `BD_CACHE__CALL_TIMEOUT_S`.
`BD_CACHE__SCOPES` is comma-separated. Nested keys use `__`:
`BD_CACHE__TICK__TEAM`, `…__TICK__FALLBACK_LEAD`, `…__TICK__PHASES_DIR` and
`…__TICK__PHASES` (comma-separated). Precedence is flag > env > TOML >
default.

- **Where files are read from.** `.beads/bd-cache.toml` is always read from
  `<repo>/.beads/`. `metadata.json` is read from `$BEADS_DIR` when that is
  set, and from `<repo>/.beads/` otherwise. This is how `load_repo_config`
  and the store behave today.
- **No upward search.** bd searches parent directories for `.beads`; bdc
  deliberately does not. Hooks run with an explicit cwd and the bindings do
  not search either, so the same command always opens the same file.
- **Legacy file.** bdc refuses with `CONFIG_INVALID` (`details: {key:
  "cache_path", legacy_path}`) when all three of these hold:
  - no `cache_path` is configured;
  - the default file does not exist;
  - `<repo>/.atm-bd/<db>.bd-cache.sqlite3` does exist.

  The suggested action is to set
  `cache_path = ".atm-bd/<db>.bd-cache.sqlite3"`. This check is bdc's only;
  how the binding opens a file in 0.1 is unchanged.
- **sc-config.** Phase a keeps bd-cache-store's `load_repo_config`, so a
  missing TOML is still valid. bdc applies env overrides itself, using
  sc-config's naming rule (`BD_CACHE__`), and does not call
  `sc_config::load`. **bdc is not blocked on sc-runtime.** We make one
  request to the sc-runtime team: a file source with an explicit path, where
  a missing file is allowed. An env-only overlay would help but is optional;
  TOML is a nice-to-have.
- **NFR-BDC-0002 (< 50 ms for a new-process `get`).** The claim covers a
  cache hit with **no write in flight on the bead's scope**. Such a `get`
  opens the store and reads one row (ADR-CORE-0206), with no bd call, no
  backend check and no schema packages. The latency test also runs a
  pending-write case (10 pending entries, a 1,000-row scope). That case is
  reported but not gated, because the core adjusts the whole base while a
  write is pending.

## 3. Exit codes

| Exit | `ok` | Meaning |
|---|---|---|
| 0 | `true` | Success. Degraded results (an outage, a cached fallback, a partial refresh) are reported in `data`; `cache refresh scopes` reports `data.failed`. |
| 2 | `false` | `BD_CACHE.USAGE`, for the shape of argv only: an unknown command or flag, a missing or conflicting argument, an abbreviated option, or `--input` mixed with flags. |
| 3 | `false` | Any other typed error. Retryable and permanent errors share this exit, so callers branch on `error.kind`, or on `retryable` in the registry (§4). |

A caught panic is `BD_CACHE.INTERNAL` (exit 3). Invalid JSON in `--input`,
`--patch` or `--set-metadata` is `BAD_ARGS`.

## 4. Envelope, errors, registry

The envelope is sc-command 0.1.0's, exactly: `{version: "1", ok, data,
error: {kind, code, message, details, suggested_action}}`. Its decoder
rejects extra keys. Each field of today's `CacheError` maps as follows:

| `CacheError` | Envelope |
|---|---|
| `code` | `error.code`, unchanged. sc-observability-types 1.5.0 `ErrorCode` is an unvalidated string, so `BD_CACHE.*` codes pass as they are. |
| `detail` | `error.message` |
| `fix` | `error.suggested_action` |
| `reason` (ADR-BDC-0003's `/v0` refusal name) | `details.v0_code` |
| `unmet` | one shape only: `details.unmet: {status?: {expected, actual}, assignee?: {expected, actual}}` |
| `seq`, `bead`, `field`, `cause`, `known`, `source` | `details.*` under the same names. `phase` becomes `details.scope`, in label form. |

The `details.*` keys this contract defines are covered by semver:

| Key | Meaning |
|---|---|
| `v0_code` | The `code` a `/v0` Problem would carry. |
| `v0_reason` | A `/v0` Problem reason, such as `unknown_parameter`. |
| `unsupported` | `"operation"`, `"parameter"` or `"member"`, with `name`. |
| `member` | The unknown member, on `BAD_ARGS`. |
| `retry_after_ms` | On `BD_UNAVAILABLE` during an outage. Advisory: `miss_fill_timeout_ms`, at least 1000. |
| `timed_out` | A bool, set when a miss fill or a `refresh issues` was cut off by its timeout. |
| `did_you_mean`, `valid` | On `USAGE`. |

The key `justification` is never used, because sc-command reserves it.

### Registry

`bdc schema` publishes this table, together with a JSON Schema for each
code's `details`. Every code carries the `BD_CACHE.` prefix, which the table
leaves out. † marks a new code.

| Code | Kind | Exit | Retryable | `v0_code` | Notes |
|---|---|---|---|---|---|
| `USAGE` † | invalid_input | 2 | no | — | `suggested_action`: "run `bdc schema <cmd>`" |
| `BAD_ARGS` | invalid_input | 3 | no | `invalid_argument` | Also an unknown member (`details.member`), and a bad cursor (`v0_code: invalid_cursor`). |
| `BAD_PRECONDITION` | invalid_input | 3 | no | `invalid_argument` | An invalid guard. |
| `UNSUPPORTED` † | invalid_input | 3 | no | `invalid_argument` (`v0_reason: unknown_parameter` for a parameter or member) | `details.unsupported` names the kind. The suggested action is to call bd or bd serve directly. |
| `CONFIG_INVALID` † | invalid_input | 3 | no | — | `details.source` keeps the underlying code: `store_config_*`, `sync_config_*`, `sync_beads_dir_unresolvable` or `sync_repo_root_not_utf8`. |
| `BAD_OP` | invalid_input | 3 | no | `invalid_argument` | Only the binding's old `write` verb returns it; the CLI never does. |
| `NOT_FOUND` | not_found | 3 | no | `not_found` | Neither the cache nor bd has the bead, or no such entry exists. |
| `NO_DATABASE` | not_found | 3 | no | — | |
| `NOT_CACHED` † | unavailable | 3 | **yes** | — | A miss with `miss_timeout_ms: 0`. It is not a `/v0` 404, because bd was never asked. The suggested action is to retry with `--miss-timeout-ms` above 0. |
| `SCOPE_UNKNOWN` | **conflict** | 3 | no | — | See below. `details.known`. |
| `PRECONDITION` | conflict | 3 | no | `precondition_failed` | Write time only. `already_claimed` and `not_closable` occur only as escalation reasons at flush. |
| `IDEM_CONFLICT` | conflict | 3 | no | — | `details.seq` |
| `PRUNE_BLOCKED` | conflict | 3 | no | — | |
| `NOT_ESCALATED` | conflict | 3 | no | — | |
| `WRONG_DATABASE` | conflict | 3 | no | — | Not `/v0` `project_mismatch`, which is a 400 about a request header. |
| `SCHEMA_VERSION` | conflict | 3 | no | — | |
| `STORE_IN_USE` | conflict | 3 | no | — | Bindings only. |
| `BD_UNAVAILABLE`, `BD_VERSION_FAILED`, `BD_SCHEMA_FAILED`, `BD_LIST_FAILED`, `SYNC_FAILED` | unavailable | 3 | yes | `db_unavailable` | `SYNC_FAILED` only appears inside `refresh scopes` results. |
| `BD_WRITE_FAILED` | unavailable | — | yes | — | Only appears inside `Entry.last_error`. |
| `BD_CONTRACT`, `STORE_FAILED`, `INTERNAL` † | internal | 3 | no | `internal` | |
| `TICK_<REFUSAL>` † | invalid_input for config and phase-file refusals; unavailable for an undelivered message or a missing lead; conflict for `BUSY` | — | per kind | — | Only appears inside `cache tick` `refused[]`. `<REFUSAL>` is bd-cache-sync's `RefusalCode`, upper-cased. |

**Why `SCOPE_UNKNOWN` is a conflict, not a 404.** The request is valid, and
bd may well hold those beads. What refuses is the replica, which does not
cover that scope (REQ-BDC-0902). A `/v0` 404 would tell an agent that the
beads do not exist. The fix is a change of state (`cache refresh scopes`) or
a scope the cache knows. That matches this table's meaning of conflict: the
request is well formed, the state refuses it, and nothing was written.

`EXTENSION_INVALID` (ADR-BDC-0008) is reserved. `NO_TOML` stays retired.

## 5. Surface (a): Beads

### 5.1 Shared rules

- **Row.** A row is bd's list row (`IssueWithCounts`, modelled in sc-beads
  as `Bead`), with pending writes replayed. Its counts are as of the last
  fetch.
- **Scope selection.** A list-like read must name a known scope, with
  `--scope` or with a `phase-<x>` `label`. The first such label selects the
  scope, and every label is still applied as an AND filter. Naming none
  gives `SCOPE_UNKNOWN`.
- **Filters, paging and order** are applied in `bd-cache-api`, on top of
  `Cache::list(scope)`. The core filters nothing beyond the scope.
- **`x_cache` on reads:** `{scope: str|null, last_ok_at: ts|null, outage:
  Outage|null, rows: {<id>: {stale: bool, blocked_by: [id]|null}}}`.
  `blocked_by` is `null` when unknown and `[]` when not blocked. `Outage` is
  `{since: ts, last_error: OpError}`.
- **`x_cache` on writes:** `{seq: int, state:
  "pending"|"applied"|"escalated"|"resolved", duplicate: bool,
  idempotency_key: str}`.
- **Idempotency key.** It is **required** on every front end
  (REQ-BDC-0004) and is never derived from the request's content. Two writes
  are the same write when they have the same `(bead, op, canonical args)`.
  Guards and actor are **not** compared, so a retry with another actor or
  guard is still a duplicate. On a duplicate, `issue` is the current view,
  and `changed` / `already_closed` are the values stored with the original
  entry.
- **Guards.** `expected_status` and `expected_assignee` are checked when the
  write is journaled, and again at flush. `expected_version` is
  `UNSUPPORTED`, because the CLI backend has no `revision`.

### 5.2 Operation map (v1.3.2)

Spec lines cited are at [`9b65b9e`][rel].

| Command / tool | `/v0` operationId · capability | Spec | Core verb | Annotations |
|---|---|---|---|---|
| `issues get` / `issues_get` | getIssue · `issues.get` | [L2042-2124][r-get] | `get` (or `cached` when the miss timeout is 0) | readOnly; openWorld (miss fill) |
| `issues list` / `issues_list` | listIssues · `issues.list` | [L995-1579][r-list]; page [L7808-7824][r-ipage] | `list(scope)` + api filters | readOnly |
| `ready` / `ready` | listReadyWork · `ready.list` | [L349-571][r-ready]; page [L7749-7763][r-rpage] | `ready(scope, fresh_within)` | readOnly; openWorld (fresh) |
| `dependencies list` / `dependencies_list` | listDependencies · `dependencies.list` | [L4667-4775][r-deps]; [L8287-8322][r-dedges] | the view's `dependencies` | readOnly |
| `issues update` / `issues_update` | updateIssue · `issues.update` | [L2125-2347][r-patch]; request [L10219-10512][r-upd]; reply [L10513-10548][r-updr] | **new** `write_returning(op=update)` | destructive false; idempotent |
| `issues close` / `issues_close` | closeIssue · `issues.close` | [L2938-3074][r-close]; [L9921-10047][r-closeb] | **new** `write_returning(op=close)` | destructive false; idempotent |
| `dependencies add` / `dependencies_add` | addDependencies · `dependencies.add` | [L5051-5174][r-dadd]; [L9411-9505][r-daddb] | **new** `write_returning(op=dep_add)` | destructive false; idempotent |
| `context` / `context` | getContext (no capability token) | [L325-348][r-ctx]; [L7181-7299][r-ctxb] | `status` + `metadata.json` | readOnly |

Each tool's schema carries `x_v0_operation: {operation_id, capability,
spec}`. `context.capabilities` lists the `/v0` capability tokens the cache
serves: `issues.get`, `issues.list`, `ready.list`, `dependencies.list`,
`issues.update`, `issues.close` and `dependencies.add`. The spec's
capabilities member uses tokens, not operationIds ([L7181-7299][r-ctxb]).

### 5.3 Per operation

Every command can also return the errors of opening the cache:
`NO_DATABASE`, `WRONG_DATABASE`, `SCHEMA_VERSION`, `CONFIG_INVALID` and
`STORE_FAILED`.

| Command | Request (the mapped `/v0` names; the rest are in the spec) | `data` | Order | Errors |
|---|---|---|---|---|
| `issues get <id> [--miss-timeout-ms N]` | `id`, `miss_timeout_ms` | the row, + `x_cache: {scope, last_ok_at, outage, rows: {<id>: …}, pending: int}` | n/a | `NOT_FOUND`, `NOT_CACHED`, `BD_UNAVAILABLE` (`timed_out`), `UNSUPPORTED` (`include_comments`, `include_dependents`, `brief_deps`) |
| `issues list` | `status[]`, `label[]`, `label_any[]`, `exclude_label[]`, `assignee`, `all`, `sort`, `limit` (default 50; 0 = unlimited), `cursor`, `scope` | `{items, has_more, next_cursor?, x_cache}` | `sort=created`: `(created_at desc, id asc)`; `sort=priority`: `(priority asc, created_at desc, id asc)`. Both are total orders, so the cursor is a keyset. | `SCOPE_UNKNOWN`, `BAD_ARGS` (`invalid_cursor`), `UNSUPPORTED` |
| `ready [--fresh-within-ms N]` | `label[]`, `label_any[]`, `exclude_label[]`, `assignee`, `unassigned`, `priority`, `limit` (default **100**), `sort` (`priority` only), `scope`, `fresh_within_ms` | `{items, has_more, x_cache: {scope, last_ok_at, outage, rows, basis, locally_adjusted: bool, read_fallback, read_fallback_error: OpError\|null, missing_rows: [id]}}` | `(priority asc, created_at desc, id asc)`. The tie-break is the cache's own; the spec does not state bd's. | `SCOPE_UNKNOWN`, `UNSUPPORTED` (`hybrid`, `oldest`, `type`, `parent`, …) |
| `dependencies list --issue-id ID… [--type T…]` | `issue_id[]`, `type[]` | `{items: [{issue_id, depends_on_id, type}], missing: [id], x_cache: {outage, not_cached: [id]}}` | request order of `issue_id`, then `depends_on_id` | `BAD_ARGS` |
| `issues update <id>` | `id`, `actor`, `idempotency_key`, `patch: {status?, assignee?, add_labels?, remove_labels?, append_notes?, metadata?: {set?, unset?}}`, `expected_status?`, `expected_assignee?` | `{issue, changed: bool, x_cache}` | n/a | `BAD_ARGS` (empty patch; `status: "closed"`; unknown member), `BAD_PRECONDITION`, `UNSUPPORTED` (`claim`, `expected_version`, `force_*`, other patch members), `PRECONDITION`, `IDEM_CONFLICT`, `NOT_FOUND`, `BD_UNAVAILABLE` |
| `issues close <id>` | `id`, `actor`, `idempotency_key`, `reason?`, and the cache-extension guards `expected_status?`, `expected_assignee?` | `{issue, already_closed: bool, x_cache}` | n/a | as update; `UNSUPPORTED` (`session`, `force`, `expected_version`) |
| `dependencies add <issue_id> <depends_on_id> --type T` | `actor`, `idempotency_key`, `edges: [{issue_id, depends_on_id, type}]` (exactly one edge) | `{added: [edge], x_cache}` | n/a | as update; `UNSUPPORTED` (more than one edge) |
| `context` | none | `{bd_version?, database, project_id, beads_dir?, repo_root?, capabilities: [token], x_cache: {cache_path, contract_version, store_schema_version, bd_schema_sha256, bd_schema_version, model_current}}` | n/a | the open-time errors only |

Notes by command:

- **`issues get`.** `pending` counts the bead's own journal entries that
  have not been applied. The core needs to add `GetReply.scope`.
- **`issues list`.**
  - The default exclusions are `closed` rows, templates and `gate` rows,
    unless `status` or `all` is set. The workspace's configured infra types
    are unknown to the cache, so they are **not** excluded.
  - The cursor is `v1.` followed by base64url of `[sort, key…]`, and its
    format is covered by semver.
- **`ready`.**
  - `basis` is `{kind: "fresh", fetched_at}`,
    `{kind: "cached", fetched_at, stale}` or `{kind: "unknown"}`.
  - `read_fallback` is `"timed_out"`, `"unavailable"`, `"failed"` or `null`.
  - Membership follows the released **`bd ready` / `bd blocked` CLI** as of
    the last sync, plus the local adjustment (REQ-BDC-0911, 0912). It does
    not follow `/v0` listReadyWork.
  - A bead whose blocked state is unknown is never listed.
- **`issues update`.**
  - `status: "closed"` is refused and points to `issues close`, so a PATCH
    can never get around bd's close checks.
  - `assignee: ""` unassigns.
  - Only the `/v0` spellings are accepted. `set_metadata` and
    `unset_metadata` remain only in the binding's old `write` verb.
  - `changed` is computed when the write is journaled: the overlay changes
    the view. It is a prediction, because bd decides at flush.
- **`issues close`.**
  - `already_closed` describes the view before the write.
  - `not_closable` and `already_claimed` show up only as escalations, read
    through `cache journal get`.
- **Writes in general.** `issue` is the view after the write.

### 5.4 Conformance statement: what differs from v1.3.2

| `/v0` operation | Divergence |
|---|---|
| all | Errors use the sc-command envelope, not a Problem document (spec [L10819-11138][r-problem]); `details.v0_code` carries the Problem `code`. Every list-like read needs a scope (REQ-BDC-0902). |
| getIssue | Returns the list row, not IssueDetails. Absent: `revision` (required), `dependents`, dependency objects with metadata, and `epic_*`. |
| listIssues | Infra types are not excluded. Parameters other than the mapped ones are `UNSUPPORTED`. |
| listReadyWork | `sort=priority` only. Membership comes from the bd CLI plus the local adjustment. Parameters that are not mapped are `UNSUPPORTED`. |
| updateIssue | Journaled. `revision` (required) is absent. `changed` is a prediction. `status: closed` is refused. |
| closeIssue | Journaled. `open_children` and `revision` (both required) are absent. Extra guards are accepted. `not_closable` appears only at flush. |
| addDependencies | Exactly one edge per request, journaled. |
| listDependencies | `Dependency.created_at` is absent (it is required; [L6855-6876][r-dep]), because the cached edge does not carry it. |
| getContext | `api_version`, `schema_version`, `backend` and `dolt_mode` (all required) are absent. `bd_version` (required) is absent until the first fingerprint. |
| not served (`UNSUPPORTED`, `unsupported: operation`) | createIssue, batchCreate, delete, sweep, claim, release, claimNext, reopen, casMetadata, batchApply, batchClose, dependencies:remove, issues:query, issues:count, ready:count, stats, the dependency tree/cycles/count/blocking, related, comments, config, memories, events, healthz. |

The output schemas in `bdc schema` are the cache's own required sets, and
they do not `$ref` `/v0`.

### 5.5 Proposed: beads `origin/main` @ `73f64fd` (not offered in 1.0)

| On main | Spec on main | What the cache does once a beads release contains it |
|---|---|---|
| `GET /blocked`, listBlockedIssues · `blocked.list` | [L695-785][m-blocked]; BlockedIssue `blocked_by_count`, `blocked_by` [L10031-10123][m-bissue] | `bdc blocked`, over a new core verb `blocked(scope)` built from the ready base. Ordered `(priority asc, created_at desc, id)`, as the spec states. |
| `POST /issues:batchGet` · `issues.batchGet` | [L4753-4847][m-bget] | `bdc issues batch-get`, from the cache only. Uncached ids go in `missing` and in `x_cache.not_cached`; results keep the request order. |
| listIssues `id` filter | [L1457][m-id] (within [L1129-2354][m-list]) | `issues list --id` |
| UpdateIssueRequest `claim` | [L12929][m-claim] (within [L12909-13035][m-upd]) | Stays `UNSUPPORTED`: a journaled claim cannot honestly answer `already_claimed`. |
| ContextResponse `wire_revision`, `min_client_wire_revision` | [L8948-9355][m-ctx] | absent |
| close `auto_close_molecule` | [L4027-4195][m-close] | `UNSUPPORTED` |

Four branches were reviewed: `feat/http-update-claim`,
`feat/http-client-write-roles`, `fix/served-close-refusal-names-blockers`
and `feat/ready-policy-in-workapi`. None changes a mapped member.
`feat/ready-policy-in-workapi` does change which beads count as ready and
claimable (the external-dependency policy, `not_claimable`). That is one more
reason ready parity is stated against the released CLI.

## 6. Surface (b): Cache management

| Command / tool | Core verb | Old verb | Mutates | Annotations |
|---|---|---|---|---|
| `cache status` / `cache_status` | `status` + **new** `journal_counts()` | `status` | no | readOnly |
| `cache health [--probe]` / `cache_health` | `open_work`, outage, stale rows; with `--probe`, also the backend check | (`doctor`) | no | readOnly; openWorld (probe) |
| `cache scopes` / `cache_scopes` | `status().phases` + config + `sync_alert` per scope (N store reads, no bd) | (in `status`) | no | readOnly |
| `cache refresh scopes [S…]` / `cache_refresh_scopes` | `sync` per scope; with no S, every known scope | `sync` | store | destructive false; openWorld |
| `cache refresh issues ID…` / `cache_refresh_issues` | `refresh(ids)` (REQ-BDC-0907) | — | store | destructive false; openWorld |
| `cache flush` / `cache_flush` | `drain(budget)` | `drain` | bd | destructive false; openWorld |
| `cache evict --scope S` / `cache_evict` | `prune` | `prune` | store | **destructive** |
| `cache journal get <seq> \| --idempotency-key K` / `cache_journal_get` | `entry`; **new** facade `entry_by_key` (the store already has it) | — | no | readOnly |
| `cache journal list [--state pending\|escalated] [--unannounced]` / `cache_journal_list` | `pending_entries`, `status().escalated`, `unannounced_escalations` | (in `status`) | no | readOnly |
| `cache journal resolve <seq> --note T --actor A` / `cache_journal_resolve` | `resolve` | `resolve` | store | destructive false; idempotent |
| `cache journal announce <seq>` / `cache_journal_announce` | `mark_announced` | (cron) | store | destructive false; idempotent |
| `cache tick [--budget-ms N] [--atm PATH]` / not an MCP tool (`mcp: false`) | bd-cache-sync `tick` | (`bd_cache_cron.py`) | bd, store, atm | — |

Three cases have fixed answers:

- A bare `cache refresh` is `USAGE`, with `details.valid: [scopes, issues]`.
- `journal list` with no flag means `--state escalated`. The `applied` and
  `resolved` states are `UNSUPPORTED` until the store has a listing for them.
- No **invalidate** command is offered. The core has no such verb, and
  `refresh` is the honest form of it.

| Command | `data` | Order |
|---|---|---|
| `cache status` | `{cache_path, contract_version, store_schema_version: int, project: {database, project_id}, backend: {bd_version, bd_schema_sha256, bd_schema_version}, journal: {pending, applied, escalated, resolved, by_scope: {<scope>: {<state>: int}}, unscoped: {<state>: int}}, scopes: [Scope], escalated: [Entry], outage, stale: [id]}` | scopes by id; entries by seq, ascending |
| `cache health` | `{state: "ok"\|"degraded"\|"down", outage, open_work: {pending, escalated_unannounced}, stale_rows: int, probe: {bd_version, model_current, rebuilt}\|null}` | — |
| `cache scopes` | `{scopes: [Scope]}`. A Scope is `{scope: "phase-bk", configured: bool, on_demand: bool, last_started_at, last_ok_at, last_error: OpError\|null, alert, beads: int}`, where `alert` is `"backend_unavailable"`, `"sync_failed"`, a `BD_CACHE` code, or `null`. | by scope id |
| `cache refresh scopes` | `{failed: int, results: [{scope, report: {rebuilt, model_current, stored, blocked, deferred, outage}\|null, error: OpError\|null}]}` | request order (by scope id when no scope is named) |
| `cache refresh issues` | `{issues: [row], x_cache: {rows}}`. The error is `BD_UNAVAILABLE` (`timed_out`). Also takes `timeout_ms`. | request order |
| `cache flush [--start-gate-ms N] [--deadline-ms M]` | `{busy, applied: [seq], retrying: [seq], escalated: [{seq, bead, op, actor, scope, reason: OpError}], deferred: [seq], deferred_in_flight: [seq], outage}`. N and M default to 25,000 and 28,000 ms, and M must be greater than N. | seq, ascending |
| `cache evict` | `{scope, beads_deleted: int, out_of_scope_swept: int}` | — |
| `journal get` | `{entry: Entry}` | — |
| `journal list` | `{entries: [Entry]}` | seq, ascending |
| `journal resolve` | `{entry, already_resolved: bool}`. An entry already resolved returns `true` and is left unchanged; a pending or applied entry is `NOT_ESCALATED`. | — |
| `journal announce` | `{entry, already_announced: bool}` | — |
| `cache tick` | `TickReply`, with each `refused[]` item an OpError | as produced |

`Entry` is `{seq, idempotency_key, scope, bead, op, args, expected_status,
expected_assignee, actor, state, attempts, last_error: OpError|null, note:
str|null, announced_at, created_at, applied_at}`.

- `args` is the `/v0` patch. `bd-cache-api` maps it from the stored BD-CACHE
  form (for example `set_metadata` becomes `metadata.set`); the store is not
  rewritten.
- `last_error` and `note` require the store to keep an error code apart from
  a resolve note, as additive nullable columns `last_error_code` and `note`
  (REQ-BDC-0804).
- `Outage.last_error` is decoded from the stored text `CODE: detail`, and an
  alert is decoded from either writer's format (`AlertCause::from_stored`).

## 7. Audit pairs

| Mutation | Read that verifies it |
|---|---|
| `issues update` / `close`, `dependencies add` | `cache journal get <seq>`, or `--idempotency-key K` after a lost reply; `issues get <id>` for the view; after a flush, `cache refresh issues <id>` for bd's row |
| `cache flush` | `cache journal get <seq>` for each seq it reports; `cache status` |
| `cache refresh scopes` | `cache scopes` (`last_ok_at`, `last_error`) |
| `cache refresh issues` | `issues get` (`x_cache.rows[id].stale` is false) |
| `cache evict` | `cache scopes`; `issues list --scope` returns `SCOPE_UNKNOWN` |
| `journal resolve` / `journal announce` | `cache journal get <seq>`; `cache journal list --unannounced` |
| `cache tick` | `cache status`, `cache scopes`, `cache journal list --unannounced` |

## 8. MCP seam and `bdc schema`

- **One path through `bd-cache-api`.** clap argv becomes a JSON request, and
  the MCP handler passes its raw arguments. Both go through the decoder, the
  operation and the data model, and come out as `Result<T, OpError>`.
  - The CLI wraps that result in `Envelope`.
  - The MCP server returns `IntoMcp::into_mcp(result)`: the envelope as
    `structuredContent`, with `isError` set on failure. The output schema
    allows `data: null`.
  - Nothing is reshaped.
- **The request model is canonical.** Flags are a convenience: `bdc schema`
  publishes each member's flag as `x-cli-flag`, so no naming rule is implied.
- **`bdc schema [<cmd>]`.** The document is stamped with `contract_version`
  and `envelope_version`. It is
  `{tools: [{name, cli, mcp: bool, x_v0_operation, description, input_schema,
  output_schema, annotations, errors: [code]}], errors: [{code, kind, exit,
  retryable, details_schema}]}`.
- **`--help` and `--version`.**
  - `bdc <cmd> --help` returns the envelope, with that command's `tools[]`
    entry as `data`. It prints text only with `--format text`.
  - `--version` returns `{name, version, contract_version}` and also follows
    `--format`.
- **Tool names** are bdc's own: the command path joined with `_`.
  `x_v0_operation` links each tool to `/v0`.
- **Annotations** are set explicitly on every tool, with the values in the
  §5.2 and §6 tables. A hint a table does not set is false: `readOnlyHint`,
  `destructiveHint`, `idempotentHint` and `openWorldHint` all default to
  false. Journaled writes are idempotent because the key is required.

### Testing (simulator across the process boundary)

| Layer | What it is |
|---|---|
| In process | `bd_cache_cli::run(argv, env, invoker, clock) -> (exit, stdout)`. Each shared fixture (request JSON → expected envelope) runs through `run` and through the MCP handler, and the two envelopes must be identical. |
| Real process | A `fake-bd` executable over sc-bd-cli's stateful `FakeBd`, with state seeded from a scenario file and a fixed clock. The binary tests and the NFR-BDC-0002 latency check use it. |
| Named negative scenarios | an outage during a write; an escalation at flush (`not_closable`, and a guard miss); a partial `refresh scopes`; `fresh_within` running out; a schema change (rebuild); `WRONG_DATABASE`; a retry after a lost reply with the same key (a duplicate, then `journal get --idempotency-key`) |
| Live bd | The testbed and the bd-compat workflow only. |

## 9. Ownership: the `bd-cache-api` crate

| Crate | Holds | May depend on |
|---|---|---|
| `bd-cache-api` (new) | Request and data models; the decoder (validation, `UNSUPPORTED` / `BAD_ARGS`); the operation layer (filters, paging, order, `/v0` mapping); the error registry; schemars schemas | `bd-cache`, `bd-cache-model`, `sc-beads`, serde, serde_json, schemars. **No** sc-command and no clap: errors are its own `ApiError {kind, code, message, details, suggested_action}`. |
| `bd-cache-cli` (new; binaries `bd-cache`, `bdc`) | clap argv → request, the envelope, `--format text`, `run()` | `bd-cache-api`, `bd-cache-store`, `sc-bd-cli`, `bd-cache-sync` (tick, compose), `bd-cache-model`, `sc-beads`, sc-command, clap |
| MCP server (later) | rmcp tools over `bd-cache-api` | `bd-cache-api`, `bd-cache-sync` (compose), sc-command (server) |
| `bd-cache-py` | maps `ApiError` to `CacheError` attributes | as today, plus `bd-cache-api`; no sc-command, no clap |

Core changes this contract needs:

- `write_returning`: one store transaction that returns the entry and the
  views before and after the write, and stores `changed`.
- `GetReply.scope`.
- `entry_by_key`.
- `journal_counts()`.
- The store columns `last_error_code` and `note`.

`blocked(scope)` is deferred (§5.5). The CI boundary check reads the
Boundaries table in `docs/architecture.md`, so adding the rows there (done on
this branch, marked Proposed) is the whole CI change.

## 10. Python binding

Fenix designs against the wheel 0.1.0 dicts, so every change here is
**additive**:

- nothing that exists changes;
- 0.2.x issues no `DeprecationWarning`;
- the new methods live in two namespaces that mirror the tools. A method's
  name is the tool name (without the `cache_` prefix for surface (b)), it
  takes the tool's request as keyword arguments, and it returns the tool's
  `data`.

| 0.1 verb (unchanged) | New in 0.2 | How the 0.1 shape differs |
|---|---|---|
| `get(bead, miss_timeout)` | `cache.beads.issues_get(id=, miss_timeout_ms=)` | `{row: {bead, stale, blocked}, scope_last_ok_at, pending, outage}`. **Unit change:** 0.1 takes seconds (float), 0.2 takes milliseconds (int). |
| `list(phase, status, label)` | `cache.beads.issues_list(...)` | `{scope: "bk", last_ok_at, rows}`; no paging |
| `ready(phase, fresh_within)` | `cache.beads.ready(...)` | the beads are in `ready`; the basis is top-level; seconds |
| `write(...)` | `cache.beads.issues_update`, `issues_close`, `dependencies_add` | `{seq, duplicate}`; an unknown op raises `ValueError`; BD-CACHE spellings |
| — | `cache.beads.dependencies_list`, `cache.beads.context` | new |
| `refresh(ids, timeout)` | `cache.manage.refresh_issues(...)` | a bare list; seconds |
| `sync(phase, timeout)` | `cache.manage.refresh_scopes(...)` | one report, not `results[]` |
| `drain(start_gate_s, hard_s)` | `cache.manage.flush(...)` | seconds; `phase`; `reason` is a string |
| `prune(phase)` | `cache.manage.evict(...)` | same fields |
| `entry(seq)`, `unannounced_escalations()` | `cache.manage.journal_get(...)`, `journal_list(unannounced=True)` | `idem_key`, `phase`, `precondition`, BD-CACHE args; `last_error` is a string |
| `resolve(...)` → `None`; `mark_announced(seq)` | `cache.manage.journal_resolve`, `journal_announce` | no read-back |
| `status()`, `open_work()` | `cache.manage.status()`, `health()`, `scopes()` | raw `meta`; `<phase>:<state>` keys |
| `sync_alert`, `set_sync_alert` | unchanged; the setter stays binding-only | — |
| `CacheError` | adds `kind`, `message`, `details`, `suggested_action`, `retryable` | additive |

Every method, old and new, accepts a bare (`bk`) or label (`phase-bk`) scope
id. The new methods raise `CacheError` for every contract error. A
Python-level type mistake still raises `TypeError`.

## 11. Versioning

| Item | Rule |
|---|---|
| `envelope.version` | sc-command's envelope format (`"1"`), not this contract's version. |
| `contract_version` | Semver. Reported by `context`, `cache status`, `--version` and `bdc schema`. |
| Wheel | Exposes `bd_cache_rs.CONTRACT_VERSION`. Wheel 0.2.x implements contract 1.0.x. Each contract minor bumps the wheel minor. A contract major is a wheel release marked breaking and needs fenix's agreement. |
| Covered | commands and flags; tool names and schemas; `data` members; error codes, kinds, exits and `retryable`; the `details.*` keys in §4; the `v1.` cursor format; the `bdc schema` document; defaults (path, limits, exclusions); orders |
| Breaking (major) | removing or renaming any covered item; changing a type or a meaning; making a member required; changing a code's kind or exit; changing a default or an order |
| Additive (minor) | a new tool or command; a new optional request member; a new `data` member; a new code (clients must handle codes they do not know); a `/v0` operation newly offered after a beads release ships it |
| Pin | The contract follows beads v1.3.2. Moving the pin to a later release is minor when the change is additive, and major otherwise. |

## 12. Deferred

- `blocked`, `issues batch-get` and the `id` filter on `issues list`, until a
  beads release has them (§5.5).
- `journal list --state applied|resolved`.
- The operations not served (§5.4).
- Scope kinds other than `phase-<x>`.
- Schema packages (ADR-BDC-0008).
- MCP transport and auth.

## 13. Open questions

| # | Question | Position |
|---|---|---|
| Q1 | Which beads reference? | **Recommended, pending Rand's approval:** pin v1.3.2 @ 9b65b9e. Track main 73f64fd only as the proposed table. Offer an operation that exists only on main once a beads release contains it. |
| Q2 | Required `/v0` fields the cache cannot fill | **Recommended, pending Rand's approval:** they stay absent. Surface (a) is shaped like `/v0` but does not conform to it; §5.4 is the conformance statement; output schemas are the cache's own. |
| Q3 | Idempotency key | **Recommended, pending Rand's approval:** required on every front end, never derived (REQ-BDC-0004). |
| Q4 | Partial success | **Recommended, pending Rand's approval:** no exit 4. Exit 0 with `data.failed`, and `results[]` always. |
| Q5 | Name of the extra member | **Recommended, pending Rand's approval:** `x_cache`. |
| Q6 | Default cache path | **Recommended, pending Rand's approval:** `.bd-cache/`, with a `CONFIG_INVALID` naming `cache_path` when the legacy `.atm-bd/` file exists and the default does not. |
| Q7 | Request to sc-runtime | **Recommended, pending Rand's approval:** one ask, a file source with an explicit path where a missing file is allowed. The env overlay is optional, TOML is a nice-to-have, and bdc is not blocked on it. |
| Q8 | Approve ADR-BDC-0009 and the Appendix A amendments? | open |

## Appendix A. Requirement amendments (pending Rand's approval)

| REQ | Amended text |
|---|---|
| REQ-BDC-0801 | The CLI offers the commands of `docs/bd-cache-cli/contract.md`: a beads surface shaped like `/v0`, and a `cache` management surface. Each command prints one sc-command envelope on stdout. The BD-CACHE verbs map to these commands as the contract's §5 and §6 list (ADR-BDC-0009). |
| REQ-BDC-0802 | Exit 0 is `ok: true`. Exit 2 is a usage error (`BD_CACHE.USAGE`), reported in the envelope. Exit 3 is any other typed error, with `{kind, code, message, details, suggested_action}` in `error`. |
| REQ-BDC-0803 | The error codes are those of BD-CACHE.md, plus the additive `SCOPE_UNKNOWN`, `STORE_FAILED`, `STORE_IN_USE`, `USAGE`, `UNSUPPORTED`, `CONFIG_INVALID`, `NOT_CACHED`, `INTERNAL` and `TICK_*`. Each code has one fixed kind, given in the contract's registry. `NO_TOML` is never emitted. |
| REQ-BDC-0806 | **Superseded** (lead ruling S59): parity with `bd_cache.py` is behavioural, and the CLI follows sc-command and the contract. |

[main]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml
[rel]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml
[r-ctx]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L325-L348
[r-ready]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L349-L571
[r-list]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L995-L1579
[r-get]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L2042-L2124
[r-patch]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L2125-L2347
[r-close]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L2938-L3074
[r-deps]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L4667-L4775
[r-dadd]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L5051-L5174
[r-dep]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L6855-L6876
[r-ctxb]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L7181-L7299
[r-rpage]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L7749-L7763
[r-ipage]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L7808-L7824
[r-dedges]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L8287-L8322
[r-daddb]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L9411-L9505
[r-closeb]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L9921-L10047
[r-upd]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L10219-L10512
[r-updr]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L10513-L10548
[r-problem]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L10819-L11138
[m-blocked]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L695-L785
[m-list]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L1129-L2354
[m-id]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L1457
[m-close]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L4027-L4195
[m-bget]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L4753-L4847
[m-ctx]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L8948-L9355
[m-bissue]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L10031-L10123
[m-upd]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L12909-L13035
[m-claim]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L12929
