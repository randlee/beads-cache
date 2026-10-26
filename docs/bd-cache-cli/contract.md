# bd-cache / bdc CLI contract

Status: **Approved** (lead ruling S60, bc-lead, 2026-10-25, at `8115a0a`).
Implemented in phase B (sprints b-1 to b-5; phase B closed 2026-10-25) by
`bd-cache-api`, the `bd-cache`/`bdc` binary (`bd-cache-cli`) and
`bd_cache_rs` 0.2; the MCP server comes later. Contract version: `1.0.0`, unreleased: the
phase-B extension's edits below are pre-release edits of 1.0.0 (§11).

Scope: the `bd-cache` binary (alias `bdc`, REQ-BDC-0805), the MCP server that
comes later, and the Python binding `bd_cache_rs`. All three share one
request, data and error vocabulary, owned by the new `bd-cache-api` crate
(§9).

Governing inputs:

- REQ-BDC-0004, 0801..0805, 0901..0912 and NFR-BDC-0002.
- ADR-BDC-0002, 0003, 0005 and 0008 (Draft).
- Lead ruling S59: REQ-BDC-0806 is superseded; parity with the baseline Python
  implementation is behavioural; the CLI follows sc-command and `/v0`.

This contract replaces the command-structure clause of ADR-BDC-0003 through
**ADR-BDC-0009** (Approved, S60, in `docs/architecture.md`). The requirement
amendments it needs are applied in `docs/requirements.md` (Appendix A).

Phase-B extension (Rand, 2026-10-26): the scope filters, the sync rows, the switch, the
post-sync hook, the `cache scopes`, `schedule`, `cron` and `sync` verbs and the codes
`INGEST_INVALID`, `SCHEDULE_INVALID`, `TICK_RUN_FAILED` and `TICK_IN_FLIGHT` are decided by
ADR-BDC-0011..0015 and REQ-BDC-0720..0732 (`docs/architecture.md`, `docs/requirements.md`) and
ADR-SYNC-0008 (`docs/bd-cache-sync/architecture.md`). Sprint b-7 added the four codes' rows to
the §4 error table (with `ERROR_TABLE`, so the error-table test stays green), sprint b-8 removed
the `TICK_<REFUSAL>` row, and sprint b-9 wrote §2, §4, §6, §10, §11 and §12 from that register,
with no change of meaning (ADR-BDC-0009, ADR-BDC-0013, ADR-BDC-0015, REQ-BDC-0811). Where this
text and the register differ, the register governs and the difference is raised with the lead;
the open ones are marked *Open (b-9)* below.

## 1. Shape

| Surface | Commands | Vocabulary |
|---|---|---|
| (a) Beads | `bdc issues get\|list\|update\|close`, `bdc ready`, `bdc dependencies list\|add`, `bdc context` | bd serve `/v0` at the **pinned release, beads v1.3.2** @ [`9b65b9e`][rel] |
| (b) Cache management | `bdc cache status\|health\|scopes\|refresh scopes\|refresh issues\|flush\|evict\|journal get\|journal list\|journal resolve\|journal announce`; `bdc cache scopes add\|set\|remove\|list\|ingest\|announce`; `bdc cache schedule set\|show\|enable\|disable\|clear`; `bdc cache cron run`, `bdc cache tick`, `bdc cache sync use-python\|use-builtin` | conventional cache terms |

Surface (a) **matches `/v0` in vocabulary only, not in behaviour.** It uses
the same operation, parameter and member names, and its rows have the same
shape. It does not behave the same:

- some `/v0`-required fields are left out (`revision`, `open_children`,
  `Dependency.created_at`);
- errors come in the sc-command envelope, not a Problem document;
- default limits differ;
- `ready` follows the bd CLI.

**No client can switch between the cache and bd serve without code
changes.** Matching `/v0` buys a shared vocabulary for agents, the CLI, the
bindings and a later MCP server, and nothing more. §5.4 lists every
divergence.

beads `origin/main` @ [`73f64fd`][main] is tracked only as the proposed
interface (§5.5). An operation that exists only on main is offered once a
beads release contains it.

Rules:

1. **One envelope on stdout** for every command, for `--version`, and for
   every failure, usage errors included (sc-command `Envelope`, §4). The one
   exception is `--help`, which prints text on a TTY (§8). Stdout carries no
   other prose unless `--format text` is given. Logs go to stderr.
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
6. **One decoder.** The CLI turns argv into a JSON request and passes it
   to the `bd-cache-api` decoder. The bindings, and later the MCP server,
   use the same decoder, so validation lives there and every front end
   returns the same code for the same input (§8).
7. **Every list has a stated order** (§5.3, §6). A parameter that carries
   the served default (for example `sort=priority` on `ready`) is accepted.

## 2. Global options and configuration

Global options are accepted before or after the subcommand. Long options must
be spelled out in full; abbreviations are refused.

| Flag | Env `BD_CACHE__…` | `.beads/bd-cache.toml` | Default | Meaning |
|---|---|---|---|---|
| `--repo DIR` | `REPO` | n/a | cwd | The repository root. |
| `--cache-path PATH` | `CACHE_PATH` | `cache_path` | `.bd-cache/<dolt_database>.bd-cache.sqlite3` | The cache file. A relative path is resolved against `--repo`. |
| `--bd PATH` | `BD` | **never** | `bd` on `PATH` | The bd binary. It is never read from TOML, so a checked-in file cannot choose an executable. The default bd is resolved from absolute `PATH` entries first, then the fixed install directories (S77). |
| `--format json\|text` | **never** | **never** | `json` | Read from the flag only. `text` renders the same `data` for a person. The one TTY-dependent behaviour is `--help` (§8). |
| `--input FILE\|-` | n/a | n/a | none | The whole request as JSON. Combining it with request flags is `USAGE`. |
| `-v` / `-q` | `LOG` | n/a | `warn` | The verbosity of the stderr log. The flag wins over the env, and stdout never changes. |

The TOML keys are `cache_path`, `scopes`, the `[[scope]]` tables, `call_timeout_s`,
`miss_fill_timeout_ms` and `max_attempts`. `scopes` and the scalar keys can be overridden from
the env under the same name, for example `BD_CACHE__CALL_TIMEOUT_S`; `BD_CACHE__SCOPES` is
comma-separated. The `[[scope]]` tables have no env form. Precedence is flag > env > TOML >
default.

- **Scopes (REQ-BDC-0721, ADR-BDC-0013).** A scope is a name plus a bd filter. A `[[scope]]`
  table is `name` plus the filter keys `labels_all`, `labels_any`, `status`, `type` and
  `assignee`, or `all = true` alone; a table with no criterion, or with `all` and a criterion,
  is `CONFIG_INVALID`. Each string of `scopes` is a scope by the 0.1 compatibility rule (§10):
  `x` is the label scope whose label is the legacy prefix followed by `x`. A scope the file
  defines is created or updated in the store's registry when the cache opens, and `cache
  scopes set` or `remove` of it is `CONFIG_INVALID` (`details.source: "config_scope"`): the
  file wins. Every other scope is managed with `cache scopes add|set|remove` (§6).
- **No sync settings in the TOML.** There is no `[tick]` table and no `BD_CACHE__TICK__*`
  name. The sync's schedule is a row of sc-cron's table inside the cache file
  (ADR-BDC-0014, `cache schedule`, §6), and `.beads/bd-cache.toml`, which may be checked in,
  cannot name code to run (`CONFIG_INVALID`, REQ-BDC-0724).

- **Where files are read from.** `.beads/bd-cache.toml` is always read from
  `<repo>/.beads/`. `metadata.json` is read from `$BEADS_DIR` when that is
  set, and from `<repo>/.beads/` otherwise. This is how `load_repo_config`
  and the store behave today.
- **A `bd` key in the TOML.** `.beads/bd-cache.toml` is read by
  `load_repo_config`'s rules, with the same `store_config_*` codes, except
  that a `bd` key is dropped with a warning on stderr instead of refused as
  unknown. `bd` is therefore never taken from the TOML (lead ruling S70).
- **Resolving the default `bd` (lead ruling S77).** The default bd is
  resolved from absolute `PATH` entries first, then the fixed install
  directories (Homebrew, Linuxbrew, `/usr/bin`, `/bin`, `~/.local/bin`,
  `~/bin`, `$GOPATH/bin`, `~/.cargo/bin`; on Windows those under
  `USERPROFILE` and `LOCALAPPDATA`) as a fallback. Empty and relative `PATH`
  entries are dropped, and so is any fallback directory under a relative or
  empty `HOME` or `GOPATH`, because every bd child runs in the repository
  root and such an entry would find a `bd` committed there (S76).
- **No upward search.** bd searches parent directories for `.beads`; bdc
  deliberately does not. Hooks run with an explicit cwd and the bindings do
  not search either, so the same command always opens the same file.
- **The cache file is the one named.** bdc opens the file at the resolved
  `cache_path` (the default when none is configured) and looks nowhere else. A
  file kept at another location is reached by setting `cache_path`
  (REQ-BDC-0810 as amended by ADR-BDC-0015: beads-cache knows no consumer path,
  and there is no refusal of a file in another directory).
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
| 0 | `true` | Success. Degraded results (an outage, a cached fallback, a partial refresh) are reported in `data`; `cache refresh scopes` reports `data.failed`, and `cache health --probe` with bd unreachable reports `state: "down"` (§6). |
| 2 | `false` | `BD_CACHE.USAGE`, for the shape of argv only: an unknown command or flag, a missing or conflicting argument, an abbreviated option, or `--input` mixed with flags. |
| 3 | `false` | Any other typed error. Retryable and permanent errors share this exit, so callers branch on `error.kind`, or on `retryable` in the error table (§4). |
| 128 + N | either | Unix: bdc was stopped by signal N (`SIGTERM` 143, `SIGINT` 130, `SIGHUP` 129; RSH-001, ADR-CLI-0005). The running `bd` call's process group was killed and reaped and its locks released; the command's own envelope is still written when stdout is open, and it says what was done before the stop (a journaled write stays journaled). Treat it as interrupted and retry; the envelope's `ok` does not override this exit. |

A caught panic is `BD_CACHE.INTERNAL` (exit 3). Invalid JSON in `--input`,
`--patch` or `--set-metadata` is `BAD_ARGS`.

### Concurrent `bd` calls

Every `bd` call bdc makes on a project holds an in-process mutex and, for an
embedded-Dolt project, a cross-process advisory lock on
`<repo>/.beads/sc-bd-cli.lock` (lead ruling S81, ADR-BDCLI-0006), so two bdc
processes (a cron `cache tick` and an agent's `get`) never run two `bd` on one
embedded database at once. A project whose `.beads/metadata.json` says Dolt
server mode (`dolt_mode: "server"`) takes no file lock: concurrent `bd` is
normal there. The protection is partial in embedded mode: agents and bd's git
hooks that call `bd` directly do not take the lock. Add
`sc-bd-cli.lock` to `.beads/.gitignore`; the file is created on the first
locked call and never removed.

## 4. Envelope and errors

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
| `seq`, `bead`, `field`, `cause`, `known`, `source` | `details.*` under the same names. `phase` becomes `details.scope`, the scope's name as written (ADR-BDC-0013). |

This contract defines the following `details.*` keys. They are informative
in 1.0, not covered by semver (§11):

| Key | Meaning |
|---|---|
| `v0_code` | The `code` a `/v0` Problem would carry. |
| `v0_reason` | A `/v0` Problem reason, such as `unknown_parameter`. |
| `unsupported` | `"operation"`, `"parameter"` or `"member"`, with `name`. |
| `member` | The unknown member, on `BAD_ARGS`. |
| `retry_after_ms` | On `BD_UNAVAILABLE` during an outage. Advisory: `miss_fill_timeout_ms`, at least 1000. |
| `timed_out` | A bool, set when a miss fill or a `refresh issues` was cut off by its timeout. |
| `outage` | A bool on every error that can come from bd (REQ-BDC-0729): `true` only when bd could not be reached (it did not start, did not answer in time, was locked out, or its transport failed); `false` when bd answered and the call failed, for example a `BD_LIST_FAILED` from a `bd list` that exited non-zero. A scope result of a sync run carries it flat, as `error.outage` (§6). |
| `did_you_mean`, `valid` | On `USAGE`. |

The key `justification` is never used, because sc-command reserves it.

### Error table

This table ships in 1.0. Its code, kind, exit and retryable columns are
covered by semver (§11). Every code carries the `BD_CACHE.` prefix, which
the table leaves out. † marks a new code.

| Code | Kind | Exit | Retryable | `v0_code` | Notes |
|---|---|---|---|---|---|
| `USAGE` † | invalid_input | 2 | no | — | Always JSON, even on a TTY. `suggested_action`: "run `bdc <cmd> --help`". |
| `BAD_ARGS` | invalid_input | 3 | no | `invalid_argument` | Also an unknown member (`details.member`), and a bad cursor (`v0_code: invalid_cursor`). |
| `BAD_PRECONDITION` | invalid_input | 3 | no | `invalid_argument` | An invalid guard. |
| `UNSUPPORTED` † | invalid_input | 3 | no | `invalid_argument` (`v0_reason: unknown_parameter` for a parameter or member) | `details.unsupported` names the kind. The suggested action is to call bd or bd serve directly. |
| `CONFIG_INVALID` † | invalid_input | 3 | no | — | `details.source` keeps the underlying code: `store_config_*`, `sync_config_*`, `sync_beads_dir_unresolvable` or `sync_repo_root_not_utf8`. bdc's own cases (S71): `bdc_env_invalid` for a `BD_CACHE__*` value that does not parse. The registry's own case: `config_scope`, for `scopes set` or `scopes remove` of a scope `.beads/bd-cache.toml` defines (§2, REQ-BDC-0721). |
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
| `INGEST_INVALID` † | invalid_input | 3 | no | — | `ingest_scope`, `scopes ingest` (REQ-BDC-0729). `details.scope`, `details.reason` (`row_invalid`, `empty`, `too_many_rows`, `too_large`, `membership`), `details.index` for a row. The stored scope is untouched. Suggested action: fix the named row or limit and resend the whole snapshot. |
| `SCHEDULE_INVALID` † | invalid_input | 3 | no | — | `schedule set\|enable`, `sync use-python` (REQ-BDC-0729). `details.field`, `details.reason`. Suggested action: fix the named option; switch rows with `bdc cache sync use-python\|use-builtin`. |
| `TICK_RUN_FAILED` † | unavailable | 3 | no | — | `sync use-python`, `cron run --once`; also embedded, as an error object, in the `hook.error` member of a built-in run's reply, whose own exit is 0 (REQ-BDC-0729). `details.row`, `run_id`, `reason` (`exit`, `timeout`, `spawn`, `oversize`, `stop_answer`, `untrusted`, `signal`), `exit_status`, `stderr`, `restored` (the command form only). Suggested action: read `stderr`; `bdc cache sync use-builtin` restores the built-in sync. |
| `TICK_IN_FLIGHT` † | conflict | 3 | yes | — | The lease, for `sync use-python`; a scheduled or on-demand run that finds the lease replies `skipped: "in_flight"` with exit 0 instead (REQ-BDC-0722, 0729). `details.holder` (`pid`, `host`, `expires_at`). Suggested action: retry after `expires_at`. |

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
- **Scope selection.** A list-like read names a known scope with `scope`
  (`--scope`), by its name as written (ADR-BDC-0013, ANC-009); a scope is a
  bd filter, so its name carries no prefix. Every `label` is applied as an
  AND filter. Naming no known scope gives `SCOPE_UNKNOWN`. The only prefix
  rule is the 0.1 compatibility rule of the 0.1 Python verbs (§10).
  *Open (b-9, raised with the lead):* the code also selects a scope from the
  first legacy-prefixed `label` when `scope` is absent (`bd-cache-api`'s
  `select_scope`, through `ScopeId::from_labels`, ADR-CORE-0204 as amended);
  ADR-BDC-0013 applies the prefix rule on the 0.1 paths only.
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

| Command | `/v0` operationId · capability | Spec | Core verb |
|---|---|---|---|
| `issues get` | getIssue · `issues.get` | [L2042-2124][r-get] | `get` (or `cached` when the miss timeout is 0) |
| `issues list` | listIssues · `issues.list` | [L995-1579][r-list]; page [L7808-7824][r-ipage] | `list(scope)` + api filters |
| `ready` | listReadyWork · `ready.list` | [L349-571][r-ready]; page [L7749-7763][r-rpage] | `ready(scope, fresh_within)` |
| `dependencies list` | listDependencies · `dependencies.list` | [L4667-4775][r-deps]; [L8287-8322][r-dedges] | the view's `dependencies` |
| `issues update` | updateIssue · `issues.update` | [L2125-2347][r-patch]; request [L10219-10512][r-upd]; reply [L10513-10548][r-updr] | **new** `write_returning(op=update)` |
| `issues close` | closeIssue · `issues.close` | [L2938-3074][r-close]; [L9921-10047][r-closeb] | **new** `write_returning(op=close)` |
| `dependencies add` | addDependencies · `dependencies.add` | [L5051-5174][r-dadd]; [L9411-9505][r-daddb] | **new** `write_returning(op=dep_add)` |
| `context` | getContext (no capability token) | [L325-348][r-ctx]; [L7181-7299][r-ctxb] | `status` + `metadata.json` |

`context.capabilities` lists the `/v0` capability tokens the cache
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
| `issues list` | `status[]`, `label[]`, `label_any[]`, `exclude_label[]`, `assignee`, `all`, `sort`, `limit` (default: **no limit** inside a scope; an explicit value pages; 0 = unlimited), `cursor`, `scope` | `{items, has_more, next_cursor?, x_cache}` | `sort=created`: `(created_at desc, id asc)`; `sort=priority`: `(priority asc, created_at desc, id asc)`. Both are total orders, so the cursor is a keyset. | `SCOPE_UNKNOWN`, `BAD_ARGS` (`invalid_cursor`), `UNSUPPORTED` |
| `ready [--fresh-within-ms N]` | `label[]`, `label_any[]`, `exclude_label[]`, `assignee`, `unassigned`, `priority`, `limit` (default: **no limit** inside a scope; an explicit value caps the list), `sort` (`priority` only), `scope`, `fresh_within_ms` | `{items, has_more, x_cache: {scope, last_ok_at, outage, rows, basis, locally_adjusted: bool, read_fallback, read_fallback_error: OpError\|null, missing_rows: [id]}}` | `(priority asc, created_at desc, id asc)`. The tie-break is the cache's own; the spec does not state bd's. | `SCOPE_UNKNOWN`, `UNSUPPORTED` (`hybrid`, `oldest`, `type`, `parent`, …) |
| `dependencies list --issue-id ID… [--type T…]` | `issue_id[]`, `type[]` | `{items: [{issue_id, depends_on_id, type}], missing: [id], x_cache: {outage, not_cached: [id]}}` | request order of `issue_id`, then `depends_on_id` | `BAD_ARGS` |
| `issues update <id>` | `id`, `actor`, `idempotency_key`, `patch: {status?, assignee?, add_labels?, remove_labels?, append_notes?, metadata?: {set?, unset?}}`, `expected_status?`, `expected_assignee?` | `{issue, changed?: bool, x_cache}`; `changed` is absent when it is unknown (S68: an entry written without it, by another writer of the file or before the column existed) | n/a | `BAD_ARGS` (empty patch; `status: "closed"`; unknown member), `BAD_PRECONDITION`, `UNSUPPORTED` (`claim`, `expected_version`, `force_*`, other patch members), `PRECONDITION`, `IDEM_CONFLICT`, `NOT_FOUND`, `BD_UNAVAILABLE` |
| `issues close <id>` | `id`, `actor`, `idempotency_key`, `reason?`, and the cache-extension guards `expected_status?`, `expected_assignee?` | `{issue, already_closed?: bool, x_cache}`; `already_closed` is absent when it is unknown, as `changed` is | n/a | as update; `UNSUPPORTED` (`session`, `force`, `expected_version`) |
| `dependencies add <issue_id> <depends_on_id> --type T` | `actor`, `idempotency_key`, `edges: [{issue_id, depends_on_id, type}]` (exactly one edge) | `{added: [edge], x_cache}` | n/a | as update; `UNSUPPORTED` (more than one edge) |
| `context` | none | `{bd_version?, database, project_id, beads_dir?, repo_root?, capabilities: [token], x_cache: {cache_path, contract_version, store_schema_version, bd_schema_sha256, bd_schema_version, model_current}}` | n/a | the open-time errors only |

Notes by command:

- **`issues get`.** `pending` counts the bead's own journal entries that
  have not been applied. The core needs to add `GetReply.scope`.
- **`issues list`.**
  - The default exclusions are `closed` rows, templates and `gate` rows,
    unless `status` or `all` is set. The workspace's configured infra types
    are unknown to the cache, so they are **not** excluded.
  - Default limit: inside a known scope there is **no limit**, because a
    scope is already bounded and a caller reading a scope expects the whole
    of it. An explicit `limit` pages, with `has_more` and `next_cursor`. If a
    call without a scope is ever allowed, it will take `/v0`'s defaults.
  - **The default exclusions differ from the 0.1 `list(phase, ...)` verb,**
    which returns every row of the scope (§10). A caller that needs closed,
    template and gate rows passes `all` (`bdc issues list --scope <s> --all`,
    `issues_list(scope=, all=True)` in the binding) or names the statuses it
    wants (`--status closed`).
  - The cursor is opaque. It is currently `v1.` followed by base64url of
    `[sort, key…]`, but clients must not parse it, and its encoding is not
    covered by semver.
- **Blocked beads, until a beads release ships `/blocked`.** The supported
  replacement for `bd blocked` with the scope's filter is the `blocked_by`
  that `issues list` reports for each row of the scope:

  ```sh
  bdc issues list --scope <s> --all --sort priority \
    | jq '.data as $d | [$d.items[]
          | select(($d.x_cache.rows[.id].blocked_by // []) | length > 0)
          | . + {blocked_by: $d.x_cache.rows[.id].blocked_by}]'
  ```

  - Membership is the same as the cache's blocked set: bd's `bd blocked`
    with the scope's filter at the last sync, intersected with the scope's
    membership and adjusted for our journaled writes (ADR-BDC-0002 as amended
    by ADR-BDC-0013).
  - The order, `(priority asc, created_at desc, id asc)`, is `/v0` blocked's.
  - `blocked_by: null` means the blocked state is unknown. The query leaves
    those rows out; select `blocked_by == null` to list them, and report
    them as unknown, never as unblocked.
  - Template and `gate` rows are not listed.
  - The deferral of `blocked` is therefore never a reason to bypass the
    cache.
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

### 5.4 Divergences from v1.3.2

Surface (a) shares `/v0`'s names, not its behaviour. A client written for bd
serve needs code changes to use the cache, and the reverse is also true. The
differences are:

| `/v0` operation | Divergence |
|---|---|
| all | Errors use the sc-command envelope, not a Problem document (spec [L10819-11138][r-problem]); `details.v0_code` carries the Problem `code`. Every list-like read needs a scope (REQ-BDC-0902). |
| getIssue | Returns the list row, not IssueDetails. Absent: `revision` (required), `dependents`, dependency objects with metadata, and `epic_*`. |
| listIssues | The default `limit` is no limit inside a scope, where `/v0`'s is 50. Infra types are not excluded. Parameters other than the mapped ones are `UNSUPPORTED`. |
| listReadyWork | The default `limit` is no limit inside a scope, where `/v0`'s is 100. Only `sort=priority` is accepted. Membership comes from the bd CLI plus the local adjustment. Parameters that are not mapped are `UNSUPPORTED`. |
| updateIssue | Journaled. `revision` (required) is absent. `changed` is a prediction. `status: closed` is refused. |
| closeIssue | Journaled. `open_children` and `revision` (both required) are absent. Extra guards are accepted. `not_closable` appears only at flush, and only from an HTTP backend: over the bd CLI a refused close is exit 13 (`precondition_failed`), so it escalates as `BD_CACHE.PRECONDITION` (lead ruling S69). |
| addDependencies | Exactly one edge per request, journaled. |
| listDependencies | `Dependency.created_at` is absent (it is required; [L6855-6876][r-dep]), because the cached edge does not carry it. |
| getContext | `api_version`, `schema_version`, `backend` and `dolt_mode` (all required) are absent. `bd_version` (required) is absent until the first fingerprint. |
| not served (`UNSUPPORTED`, `unsupported: operation`) | createIssue, batchCreate, delete, sweep, claim, release, claimNext, reopen, casMetadata, batchApply, batchClose, dependencies:remove, issues:query, issues:count, ready:count, stats, the dependency tree/cycles/count/blocking, related, comments, config, memories, events, healthz. |

The cache's output shapes are its own. They do not reference `/v0`.

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

Every command below is on the cache-management surface (ADR-BDC-0009). The sync's verbs
(`cache schedule *`, `cache cron run`, `cache tick`, `cache sync *`) are decoded by
`bd-cache-api` like every other command and run over `bd-cache-sync` (ADR-BDC-0015), so the CLI
and the binding give the same codes and details for the same input.

| Command | Core verb | Old verb | Mutates |
|---|---|---|---|
| `cache status` | `status` + **new** `journal_counts()` | `status` | no |
| `cache health [--probe] [--max-pending-age-s N]` | `open_work`, outage, stale rows, the oldest pending entry's age, the last drain, unresolved escalations, the sync rows' state; with `--probe`, also the backend check | (`doctor`) | no; **yes with `--probe`** (store): it records bd's fingerprint, a changed schema rebuilds the cache (cached rows dropped, journal kept), and an unreachable bd is recorded as an outage |
| `cache scopes`, alias `cache scopes list` | the registry, `status().phases` + config + `sync_alert` per scope (N store reads, no bd) | (in `status`) | no |
| `cache scopes add <scope> <filter>` | register a scope (REQ-BDC-0721) | — | store |
| `cache scopes set <scope> <filter>` | change a registered scope's filter; its memberships start again | — | store |
| `cache scopes remove <scope>` | `cache evict`, then remove it from the registry | — | store |
| `cache scopes ingest <scope> [--revision R] [--allow-empty]` | `ingest_scope` (REQ-BDC-0726), the snapshot on stdin | — | store |
| `cache scopes announce <scope> --since TOKEN` | `scope_announce` (REQ-BDC-0728) | — | store |
| `cache refresh scopes [S…] [--timeout-ms N]` | `sync` per scope; with no S, every known scope | `sync` | store |
| `cache refresh issues ID…` | `refresh(ids)` (REQ-BDC-0907) | — | store |
| `cache flush` | `drain(budget)` | `drain` | bd |
| `cache evict --scope S` | `prune` | `prune` | store |
| `cache journal get <seq> \| --idempotency-key K` | `entry`; **new** facade `entry_by_key` (the store already has it) | — | no |
| `cache journal list [--state pending\|escalated] [--unannounced]` | `pending_entries`, `status().escalated`, `unannounced_escalations` | (in `status`) | no |
| `cache journal resolve <seq> --note T --actor A` | `resolve` | `resolve` | store |
| `cache journal announce <seq>` | `mark_announced` | — | store |
| `cache schedule set <cron-expr> [--budget-ms N] [--hook-reserve-ms N] [--post-sync <module:function> --python <interpreter> [--env NAME]...]` | write the built-in sync row (REQ-BDC-0720) | — | store (the cache file's cron table) |
| `cache schedule show` | both sync rows, the mode, the hook, the lease, each scope's sync state (REQ-BDC-0731) | — | no |
| `cache schedule enable`, `cache schedule disable` | flip the built-in row's `enabled` flag | — | store |
| `cache schedule clear` | end the built-in row | — | store |
| `cache cron run [--once] [--shutdown-timeout-ms N]` | run the cache file's cron rows (REQ-BDC-0730) | — | bd, store |
| `cache tick` | one run of the built-in row's code, now (REQ-BDC-0720) | — | bd, store |
| `cache sync use-python --python <interpreter> --module <module> [--cron <expr>] [--timeout-ms N] [--env NAME]...` | make the Python row the sync and run it at once (REQ-BDC-0724) | — | store; runs the script |
| `cache sync use-builtin` | make the built-in row the sync (REQ-BDC-0724) | — | store |

### 6.1 The sync

beads-cache syncs bd on rows of the sc-cron table inside the cache file (REQ-BDC-0720,
ADR-BDC-0014, ANC-002): the sync runs at the times of its row, scheduled times come only from
the rows, and an on-demand `tick` does not change them. There is at most one built-in sync row
per repository (`bd-cache.sync.builtin`, a Rust job; ANC-008) and at most one Python sync row
(`bd-cache.sync.python`, a consumer's script; ANC-003), and at most one of them is enabled at
any time. Other sc-cron rows are allowed. The sync mode is `builtin`, `python` or `none`.

- **Scopes are bd filters** (ADR-BDC-0013, ANC-009): a scope is a name plus a filter, `all`
  (every bead, `--all`) or criteria over labels (all of, any of), status, type and assignee.
  Scopes are named as written everywhere; the only prefix rule is the 0.1 compatibility rule
  of the 0.1 Python verbs (§10). The built-in row syncs every registered scope, each by its
  filter; a run with no scope still drains and runs the hook (REQ-BDC-0721).
- **The built-in row.** `schedule set` writes it in one store transaction: an existing row
  keeps its `enabled` flag, a new row is enabled unless the Python row is enabled (then it is
  written disabled), and a row written enabled disables the Python row in the same
  transaction. `--budget-ms` defaults to 25000 (1000–600000); `--hook-reserve-ms` to 5000 (0 to
  budget − 1000, at least 1000 with a hook). The row's sc-cron timeout is `budget_ms + 10000`.
  A run takes the lease, drains, syncs every scope and calls the post-sync hook; `cron run`,
  `cron run --once`, `cache tick` and the binding's `tick()` run that same code. `schedule
  enable` and `disable` flip only the row's `enabled` flag (ANC-006); `schedule enable` while
  the Python row is enabled is `SCHEDULE_INVALID` (`details.reason: "python_sync_enabled"`).
- **The switch** (ANC-003). `sync use-python` checks its inputs (an absolute interpreter that
  is a regular executable file and imports `bd_cache_rs` within 5 s, a dotted module, a timeout
  of 1000–600000 ms, default 60000; else `SCHEDULE_INVALID`), takes the lease, writes the one
  Python row enabled and disables the built-in row in one transaction, and runs the row at once
  under that lease, so the caller sees any error the script emits. A failed run (non-zero exit,
  timeout, spawn failure, oversize output, a stop answer other than `continue`, an untrusted
  cache file, a signal) restores, in one transaction, the state held before the command and
  fails with `TICK_RUN_FAILED` (`details.restored`; `false` names `sync use-builtin` as the
  recovery). A re-run replaces the one Python row in place. `--cron` defaults to the built-in
  row's expression and is required when there is none. `sync use-builtin` takes no lease and
  makes the built-in row the enabled one in one transaction; with no built-in row it is
  `SCHEDULE_INVALID` (`details.reason: "no_builtin_row"`). Neither command, nor `schedule set`,
  leaves both rows enabled at any moment. While the Python row is enabled, `cache tick` (and
  `tick()`) is `PRECONDITION` (`details.reason: "python_sync_enabled"`) and starts no bd.
- **One sync at a time** (REQ-BDC-0722). Every run takes the `tick_lease` row of the cache file
  first, expiring at its deadline plus 10 s. A scheduled or on-demand run that finds a live
  lease does nothing and replies `skipped: "in_flight"` (exit 0); `sync use-python` fails with
  `TICK_IN_FLIGHT`. An expired lease is taken over.
- **The post-sync hook** (ANC-004, REQ-BDC-0723, REQ-BDC-0727). It belongs to the built-in row,
  whose payload names it. After the sync it runs as `<interpreter> -I -m bd_cache_rs.run
  post_sync <module:function>`, from a private temp cwd, with the environment cleared except
  `PATH`, `HOME`, the locale variables, `BEADS_DIR`, `BD_CACHE_PATH` and the row's `--env`
  names, stdout and stderr each capped at 1 MiB, killed with everything it started at its
  deadline, with at least `hook_reserve_ms` left. It receives the results and errors in one
  JSON request (`bd-cache.run.v1`, at most 4 MiB):

  ```json
  {"protocol": "bd-cache.run.v1", "kind": "post_sync", "run_id": "...", "now": "...",
   "deadline": "...", "cache_path": "...", "repo_root": "...",
   "drain": {"applied": 0, "escalated": 0, "error": null},
   "scopes": [{"scope": "...", "result": "ok|failed|outage|skipped",
               "error": {"code": "...", "message": "...", "outage": true},
               "failing_since": "<opaque token or null>", "notified": false}],
   "escalated": [{"seq": 1, "bead": "...", "op": "...", "actor": "...", "attempts": 3,
                  "error": {"code": "...", "message": "..."}, "created_at": "...",
                  "first_offered_at": "..."}],
   "escalated_truncated": false, "escalated_remaining": 0,
   "pending": {"count": 0, "oldest_age_s": null}}
  ```

  `escalated` holds the escalated entries not yet notified, oldest first, at most 500. The
  hook answers `{"protocol": "bd-cache.run.v1", "ok": true, "result": ...}` or `{"ok": false,
  "error": {"type", "message"}}`. The run marks nothing: the hook marks what it handled with
  `cache journal announce` and `cache scopes announce --since`, and anything unmarked is offered
  again on a later run, with `first_offered_at` unchanged, so the hook must be idempotent. A
  failed hook is `TICK_RUN_FAILED` in the run's `hook.error`, the run itself still `ok: true`.
  From the third consecutive failure the hook is backed off, from one period of the row
  doubling to at most one hour; one period is the gap between the row's next two scheduled
  times at evaluation. The cache sends nothing (ADR-BDC-0010).
- **The Python sync row** (REQ-BDC-0725). With it enabled, the consumer's script is the only
  sync: no `bd list` runs for the scopes, and the script fills the cache through
  `ingest_scope`. It runs with sc-cron's Python hook protocol (`sc-cron.hook.v1`): `cron.stop`
  before every run (answered `{"action": "continue"}` to go on), then `cron.tick`, each with one
  JSON object on stdin, `{"protocol": "sc-cron.hook.v1", "event", "schedule", "payload",
  "deadline"}`, under the environment allowlist above. Its `payload` is the row's payload plus
  the run:

  ```json
  {"target": "python", "interpreter": "/abs/python", "module": "consumer.sync",
   "cache_path": "...", "beads_dir": "...", "env": ["NAME"], "timeout_ms": 60000,
   "run_id": "...", "repo_root": "..."}
  ```

  `bd_cache_rs.sync.main(func)` is the harness a script calls: it answers `cron.stop` with
  `continue` and, on `cron.tick`, opens `Cache(payload["repo_root"],
  cache_path=payload["cache_path"])` and calls `func(cache, payload)`. The run is judged by its
  exit status; its timeout is the row's `timeout_ms`.
- **Trust** (REQ-BDC-0723). No row's or hook's code runs from a cache file that is not owned by
  the current user, or that is group- or world-writable (Unix; the Windows ACL check is a
  documented gap); the check runs before every launch.
- **sc-cron's stop hook** (ANC-006, ANC-007, ADR-CRON-0009). Every sc-cron row has an
  `enabled` flag, and a Python row's `cron.stop` answers `continue`, `disable` (the row's flag
  cleared) or `delete` (the row ended). For the immediate run of `sync use-python`, any answer
  other than `continue` is a failed run (above).
- **`cron run`** (REQ-BDC-0730). It handles `SIGINT` and `SIGTERM` (Ctrl-C and Ctrl-Break on
  Windows): it starts no new run, waits for the runs in flight up to `--shutdown-timeout-ms`
  (default 10000, 0–600000), then kills every run still going with its process tree, releases
  the lease on every exit path, and exits 130 for `SIGINT`, 143 for `SIGTERM`, 130 for Ctrl-C or
  Ctrl-Break on Windows; `SIGHUP` is handled as `SIGTERM` (exit 129). `cron run --once` runs what
  is due, waits for each run it started up to its own deadline, and exits 0, or 3 with
  `TICK_RUN_FAILED` naming each failed row. It logs to stderr, one structured event per run step,
  at the `-v` / `-q` levels.

### 6.2 Results that are not errors

**Refresh that cannot reach bd (lead ruling S75).** A `cache refresh scopes`
result for a scope whose sync could not reach bd is a failed result: `error`
is `BD_UNAVAILABLE` (kind `unavailable`, retryable) with the outage's message,
and it counts in `data.failed`, even though the core returned its report with
an `outage` (kept in `report`). ADR-BDC-0005 keeps serving stale rows; that is
a read-path rule, not a refresh success. `cache refresh issues` already fails
as a whole with `BD_UNAVAILABLE`. The core is unchanged; the mapping is
bd-cache-api's refresh op.

A sync the deadline cut is a failed result the same way (QA RCR-001). Each
sync's deadline is `timeout_ms` when the request gives one (`--timeout-ms N`,
milliseconds, per scope) and the per-call cap otherwise; every bd call of the
sync is killed at its own timeout capped by the time left before that
deadline (NFR-BDC-0004), so a bd that hangs is ended by it. The core then
returns its report with `deferred: true` and no outage (a cut records none).
`error` is `BD_UNAVAILABLE` with `details.timed_out: true` and a message that
names the deadline in ms, it counts in `data.failed`, and a scope that was
never synced stays unserved (`SCOPE_UNKNOWN`). A `timeout_ms` above the
per-call cap does not lengthen a single bd call, only the sync as a whole.

**Probe that cannot reach bd (QA ARCH-001).** `cache health --probe` whose
backend check fails with an unreachable code (`BD_UNAVAILABLE`,
`BD_VERSION_FAILED`, `BD_SCHEMA_FAILED`, `BD_LIST_FAILED`; a deadline cut is
`BD_UNAVAILABLE`) is a result, exit 0: `state: "down"`, `probe: null`, and
`outage` populated. The probe's deadline lies past the per-call cap, so a bd
that hangs is ended by the cap and recorded as the outage; `outage` is the
recorded one, or, when the probe recorded none (bd answered `bd version`
with a failure), `{since: <now>, last_error: <the probe's error>}`. Only a
store error is a typed error (exit 3).

**Health states (#86, REQ-BDC-0732).** `down` while an outage holds or a probe
could not reach bd. Otherwise `degraded`, with one `reasons[]` item for each
cause, when:

| `reasons[].code` | Cause |
|---|---|
| `stale_rows` | a row failed its read-back and is stale |
| `unresolved_escalations` | an escalated entry is not yet resolved, announced or not (`unresolved_escalations` counts them; `open_work.escalated_unannounced` is the subset not yet announced). Settle each with `bdc cache journal resolve <seq> --note T --actor A`. |
| `pending_too_old` | `oldest_pending_age_s` is greater than the limit |
| `no_sync_row_enabled` | no sync row is enabled: a sync row exists and both are disabled (mode `none`) |
| `sync_stale` | the last successful sync of a registered scope is older than three periods of the enabled row (one period as the hook's backoff defines it, evaluated when health runs). Before a scope's first successful sync its age is measured from the enabled row's `created_at` (when the row was written or last replaced), so a new scope or a new row is not stale at once. It also covers a worker or an OS scheduler that is not running. A row with no period (no next run, or none after it) is not judged. |
| `sync_failing` | the enabled row's last three runs failed |
| `hook_failing` | the post-sync hook the built-in row names failed three times in a row |

and `ok` otherwise. `oldest_pending_age_s` is whole seconds from the oldest
pending entry's `created_at` to now (`null` with nothing pending; never
negative). The limit is `--max-pending-age-s N` (request member
`max_pending_age_s`), **300 s by default**: an entry that old means the drain
is not running or cannot apply it. An age equal to the limit is `ok`.
`last_drain_at` is when the journal last applied an entry: the newest applied
time of any entry the journal still holds (an index read, never a window of
seqs, so the gaps `cache evict` leaves in the seqs do not matter), `null` when
none was applied. `sync` is the sync mode and the enabled row's last run, read
from the cache file's cron table by every front end. Health reads the store
and starts no bd call without `--probe`. *Open (b-9, raised with the lead):*
a cache file with no sync row at all is mode `none` without
`no_sync_row_enabled`, as `schedule show`'s `both_disabled` flag and b-9's
acceptance ("both rows are disabled") read REQ-BDC-0732; the register's wording
"no sync row is enabled" also reads on a file with no row.

Three cases have fixed answers:

- A bare `cache refresh` is `USAGE`, with `details.valid: [scopes, issues]`.
- `journal list` with no flag means `--state escalated`. The `applied` and
  `resolved` states are `UNSUPPORTED` until the store has a listing for them.
- No **invalidate** command is offered. The core has no such verb, and
  `refresh` is the honest form of it.

### 6.3 Data

A `Scope` is `{scope: "bk", configured: bool, on_demand: bool, last_started_at, last_ok_at,
last_error: OpError|null, alert, beads: int, filter}`, where `scope` is the name as written,
`alert` is `"backend_unavailable"`, `"sync_failed"`, a `BD_CACHE` code, or `null`, and
`filter` is `{all?, labels_all?, labels_any?, status?, type?, assignee?}` (only the members
set) or `null` for a scope with the 0.1 meaning (the label scope of the legacy prefix).

| Command | `data` | Order |
|---|---|---|
| `cache status` | `{cache_path, contract_version, store_schema_version: int, project: {database, project_id}, backend: {bd_version, bd_schema_sha256, bd_schema_version}, journal: {pending, applied, escalated, resolved, by_scope: {<scope>: {<state>: int}}, unscoped: {<state>: int}}, scopes: [Scope], escalated: [Entry], outage, stale: [id]}` | scopes by id; entries by seq, ascending |
| `cache health` | `{state: "ok"\|"degraded"\|"down", reasons: [{code, message}], outage, open_work: {pending, escalated_unannounced}, stale_rows: int, oldest_pending_age_s: int\|null, last_drain_at: ts\|null, unresolved_escalations: int, sync: {mode: "builtin"\|"python"\|"none", last_run: {row: "builtin"\|"python", last_run_at: ts\|null, outcome: str\|null, run_id: str\|null, consecutive_failures: int}\|null}, probe: {bd_version, model_current, rebuilt}\|null}`. With `--probe` and bd unreachable: `state: "down"`, `outage` set, `probe: null` (ARCH-001, above). | `reasons` in the order of the table above |
| `cache scopes`, `cache scopes list` | `{scopes: [Scope]}` | by scope id |
| `cache scopes add`, `cache scopes set` | `{scope, filter}`. The request is `scope` plus the filter: `all`, or `labels_all[]`, `labels_any[]`, `status[]`, `type`, `assignee`; `BAD_ARGS` for no criterion, `all` with one, or `scopes add` of a name already registered; `scopes set` of an unknown scope is `SCOPE_UNKNOWN`; either of a scope the TOML defines is `CONFIG_INVALID` (`config_scope`) | — |
| `cache scopes remove` | `{scope, beads_deleted: int, out_of_scope_swept: int, removed: bool}`; `PRUNE_BLOCKED` as `cache evict`. Membership rows go, bead rows another scope still holds stay. | — |
| `cache scopes ingest` | `{scope, rows: int, stored: int, skipped_newer: int, revision}`. stdin is a rows array or `{"rows": [...], "blocked": [...]}` and nothing else; at most 50 000 rows and 64 MiB, enforced while it is read; the strict row decoder, declared membership, the `blocked` rule, the newer-row guard and one transaction; an empty snapshot is refused unless `--allow-empty`. Any violation is `INGEST_INVALID` with the stored scope untouched. | — |
| `cache scopes announce` | `{scope, since, notified: bool}`; marks only when `since` equals the current `failing_since`, else `PRECONDITION` (REQ-BDC-0728). `failing_since` is set on the first non-ok result after an ok, kept across consecutive failures, cleared on ok, and opaque. | — |
| `cache refresh scopes` | takes `scopes`, `timeout_ms`. `{failed: int, results: [{scope, report: {rebuilt, model_current, stored, blocked, deferred, outage}\|null, error: {code, message, kind, retryable, details}\|null}]}`. `error` is an OpError plus its code's `kind` and `retryable` from §4 and the `details` its envelope error would carry (`{}` for none; `timed_out` for `BD_UNAVAILABLE`). A scope that could not reach bd, or whose sync the deadline cut, is a failed result (S75, RCR-001, above). | request order (by scope id when no scope is named) |
| `cache refresh issues` | `{issues: [row], x_cache: {rows}}`. The error is `BD_UNAVAILABLE` (`timed_out`). Also takes `timeout_ms`. Ids that could not reach bd fail the command with `BD_UNAVAILABLE` (S75); there is no per-id result. | request order |
| `cache flush [--start-gate-ms N] [--deadline-ms M]` | `{busy, applied: [seq], retrying: [seq], escalated: [{seq, bead, op, actor, scope, reason: OpError}], deferred: [seq], deferred_in_flight: [seq], outage}`. N and M default to 25,000 and 28,000 ms, and M must be greater than N. A detached `bdc cache flush` (one a hook starts and does not wait for) must send its stdout and stderr to a log of the caller's, or its result and errors are lost. | seq, ascending |
| `cache evict` | `{scope, beads_deleted: int, out_of_scope_swept: int}` | — |
| `journal get` | `{entry: Entry}` | — |
| `journal list` | `{entries: [Entry]}` | seq, ascending |
| `journal resolve` | `{entry, already_resolved: bool}`. An entry already resolved returns `true` and is left unchanged; a pending or applied entry is `NOT_ESCALATED`. | — |
| `journal announce` | `{entry, already_announced: bool}` | — |
| `cache schedule set` | `{row, enabled, generation, expr, next_run_at, timeout_ms, mode}`; every input is checked before the row is written (`SCHEDULE_INVALID` naming the field) | — |
| `cache schedule show` | `{mode, rows: {builtin: Row\|null, python: Row\|null}, hook: {module, consecutive_failures, first_failure_at, last_error, next_attempt_at}\|null, lease: {pid, host, expires_at}\|null, scopes: [{scope, last_ok_at, failing_since}], flags: ["python_never_run"\|"both_disabled"\|"builtin_payload_invalid"\|"python_payload_invalid"]}`. A `Row` is `{kind, name, enabled, expr, next_run_at, last_run_at, outcome, run_id, consecutive_failures, first_failure_at, generation}` plus the built-in row's `{budget_ms, hook_reserve_ms}` or the Python row's `{module, python, timeout_ms}` (or `{payload_invalid: true}`). | — |
| `cache schedule enable`, `disable` | `{row, enabled, mode}` | — |
| `cache schedule clear` | `{row, cleared: bool, mode}` | — |
| `cache cron run` | `{once: false, stopped: true, drained: bool, idle: bool}` after a signal; with `--once`, `{once: true, ran: [{row, outcome}]}` | — |
| `cache tick` | the built-in run's reply: `{run_id, row, outcome: "ok"\|"skipped"\|"stopped", skipped: "in_flight"\|null, holder, took_over, drain: {busy, applied, retrying, escalated, deferred, deferred_in_flight, outage}\|null, scopes: [{scope, result: "ok"\|"failed"\|"outage"\|"skipped", error: {code, message, outage}\|null, failing_since, notified, stored}], hook: {status: "none"\|"backed_off"\|"ok"\|"failed"\|"skipped", module, offered, escalated_remaining, result, stderr, error: <TICK_RUN_FAILED error object>\|null, next_attempt_at}}`. It has no `refused[]` and no flag naming a send command: the cache sends nothing (ADR-BDC-0010, ADR-BDC-0015). | scopes by id |
| `cache sync use-python` | `{row, run_id, exit_status, stderr, mode: "python"}`; `stderr` cut to 8 KiB with the values of the row's `--env` variables redacted | — |
| `cache sync use-builtin` | `{row, mode: "builtin"}` | — |

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
| `cache evict` | `cache scopes`; `issues list --scope` returns `SCOPE_UNKNOWN` (see below) |
| `journal resolve` / `journal announce` | `cache journal get <seq>`; `cache journal list --unannounced` |
| `cache tick`, `cache cron run` | `cache schedule show` (each row's last run, outcome, run id and failures; the hook; the lease), `cache scopes` (`last_ok_at`, `last_error`), `cache health` (`sync`), `cache journal list --unannounced` |
| `cache scopes add` / `set` / `remove` | `cache scopes list` (`filter`; the scope gone after `remove`) |
| `cache scopes ingest` | `issues list --scope <s> --all`; `cache scopes` (`last_ok_at`) |
| `cache scopes announce` | `cache schedule show` (`scopes[].failing_since`); the next hook request's `notified` |
| `cache schedule set` / `enable` / `disable` / `clear`, `cache sync use-python` / `use-builtin` | `cache schedule show` (`mode`, each row's `enabled`, `flags`) |

**A scope with no completed sync is not served** (lead ruling S67). A scope
with no completed sync since it was last evicted, or ever, is not served:
`issues list` and `ready` on it return `SCOPE_UNKNOWN`, with
`suggested_action` `bdc cache refresh scopes --scope <s>`, and never an
empty list. Eviction deletes the scope's sync state, so after
`cache evict` the scope stays unserved until its next sync, even when it
is configured. `details.known` lists the served scopes.

## 8. Help, the MCP constraint, testing

- **`--help`.**
  - On a TTY, without `--format`, it prints text.
  - Otherwise, or with `--format json`, it prints the envelope, with
    `data: {command, synopsis, arguments: [{name, flag, required,
    description}]}`.
  - Exit 0.
  - `USAGE` is always JSON. Its `details` carry `did_you_mean` and `valid`.
- **`--version`** returns `{name, version, contract_version}` and follows
  `--format`.
- **MCP design constraint (not a 1.0 deliverable).**
  - There is one request model per command, and every front end calls the
    same `bd-cache-api` operation, which returns `Result<T, ApiError>`.
  - The later MCP server returns the same envelope as `structuredContent`
    (sc-command `IntoMcp`), and the CLI wraps the same value in `Envelope`.
  - Neither front end reshapes the result.
  - Tool names will be the command path joined with `_`. Everything else
    for MCP is in §12a.

### Testing (simulator across the process boundary)

| Layer | What it is |
|---|---|
| In process | `bd_cache_cli::run(argv, env, invoker, clock) -> (exit, stdout)`. Shared fixtures (a request JSON and the envelope it must produce) run through `run`. The MCP-handler parity run is added with the MCP server. |
| Real process | A `fake-bd` executable over sc-bd-cli's stateful `FakeBd`, with state seeded from a scenario file and a fixed clock. The binary tests and the NFR-BDC-0002 latency check use it. |
| Named negative scenarios | an outage during a write; an escalation at flush (a refused close, which over the bd CLI is `PRECONDITION` (S69, §5.4), and a guard miss); a partial `refresh scopes`; `fresh_within` running out; a schema change (rebuild); `WRONG_DATABASE`; a retry after a lost reply with the same key (a duplicate, then `journal get --idempotency-key`) |
| Live bd | The testbed and the bd-compat workflow only. |

## 9. Ownership: the `bd-cache-api` crate

| Crate | Holds | May depend on |
|---|---|---|
| `bd-cache-api` (new) | Request and data models; the decoder (validation, `UNSUPPORTED` / `BAD_ARGS`); the operation layer (filters, paging, order, `/v0` mapping); the error table; schemars schemas (published with the MCP server, §12a) | `bd-cache`, `bd-cache-model`, `sc-beads`, serde, serde_json, schemars. **No** sc-command and no clap: errors are its own `ApiError {kind, code, message, details, suggested_action}`. |
| `bd-cache-cli` (new; binaries `bd-cache`, `bdc`) | clap argv → request, the envelope, `--format text`, `run()` | `bd-cache-api`, `bd-cache-store`, `sc-bd-cli`, `bd-cache-sync` (tick, compose), `bd-cache-model`, `sc-beads`, sc-command, clap |
| MCP server (later) | rmcp tools over `bd-cache-api` | `bd-cache-api`, `bd-cache-sync` (compose), sc-command (server) |
| `bd-cache-py` | maps `ApiError` to `CacheError` attributes | as today, plus `bd-cache-api`; no sc-command, no clap |

Core changes this contract needs:

- `write_returning`: one store transaction that returns the entry and the
  views before and after the write, and stores `changed`.
- `GetReply.scope`.
- `entry_by_key`.
- `journal_counts()`.
- The additive nullable store columns `last_error_code`, `note` and
  `changed`. `changed` holds the value a duplicate returns; a close stores
  `!already_closed` (lead ruling S62, bc-lead, 2026-10-25).

Lead rulings on the core work (bc-lead, 2026-10-25, recorded with sprint b-1):

- **S64.** `write_returning`'s atomicity (REQ-BDC-0809) covers the journal
  entry and its `changed`, stored in one transaction. `before` and `after`
  are snapshots read on either side of it, so `after` may already reflect a
  concurrent drain.
- **S65.** `Entry.note` is `resolved by <actor>: <note>`: the journal keeps
  the resolver in the note, and the 0.1 `last_error` is the error, `" | "`,
  then the note, as the baseline Python implementation writes it.
- **S68.** `WriteOutcome.changed` is `Option<bool>`: `Some(stored)` when the
  entry stores it; for an entry stored without it (by the baseline Python implementation, or
  before the column existed), `Some(predicted)` only while the entry is
  pending; `None` (unknown) in any other state. It is never guessed.

`blocked(scope)` is deferred (§5.5). The CI boundary check reads the
Boundaries table in `docs/architecture.md`, so adding the rows there (done with
ADR-BDC-0009) is the whole CI change.

## 10. 0.1 → 0.2 API differences

A caller built against the wheel 0.1.0 dicts keeps working: every difference below is
additive, or opt-in for an existing 0.1 call (REQ-BDC-0812). The neutral 0.1 call corpus
(`crates/bd-cache-py/tests/corpus/`, generated in this repository against the fake bd and
compared on every pytest run) is the proof: it records every 0.1 verb's replies, errors and bd
argv, and a change to any of them is a diff.

- nothing that exists changes;
- 0.2.x issues no `DeprecationWarning`;
- the new methods live in two namespaces that mirror the two surfaces: `cache.beads` maps to
  the beads surface (§5) and `cache.manage` to the cache-management surface (§6). A method's
  name is the command path joined with `_`, without the `cache_` prefix for surface (b). It
  takes the command's request as keyword arguments and returns the command's `data`;
- the sync's verbs are methods of the handle itself (ADR-PY-0006), on the cache-management
  surface: `tick()`, `schedule_set`, `schedule_show`, `schedule_enable`, `schedule_disable`,
  `schedule_clear`, `sync_use_python`, `sync_use_builtin`, `scopes_add`, `scopes_set`,
  `scopes_remove`, `scopes_list`, `ingest_scope` and `scope_announce`. `cache cron run` is
  the CLI's alone. `bd_cache_rs.run` (the post-sync hook's runner) and `bd_cache_rs.sync` (the
  Python sync row's harness) are `-m` modules of the package (§6.1).

| 0.1 verb (unchanged) | New in 0.2 | How the 0.1 shape differs |
|---|---|---|
| `get(bead, miss_timeout)` | `cache.beads.issues_get(id=, miss_timeout_ms=)` | `{row: {bead, stale, blocked}, scope_last_ok_at, pending, outage}`. **Unit change:** 0.1 takes seconds (float), 0.2 takes milliseconds (int). |
| `list(phase, status, label)` | `cache.beads.issues_list(...)` | `{scope: "bk", last_ok_at, rows}`; no paging; every row of the scope (no default exclusions, below) |
| `ready(phase, fresh_within)` | `cache.beads.ready(...)` | the beads are in `ready`; the basis is top-level; seconds |
| `write(...)` | `cache.beads.issues_update`, `issues_close`, `dependencies_add` | `{seq, duplicate}`; an unknown op raises `ValueError`; the stored op spellings |
| — | `cache.beads.dependencies_list`, `cache.beads.context` | new |
| `refresh(ids, timeout)` | `cache.manage.refresh_issues(...)` | a bare list; seconds |
| `sync(phase, timeout)` | `cache.manage.refresh_scopes(...)` | one report, not `results[]`; an unreachable bd is the report's `outage`, not a failed result |
| `drain(start_gate_s, hard_s)` | `cache.manage.flush(...)` | seconds; `phase`; `reason` is a string |
| `prune(phase)` | `cache.manage.evict(...)` | same fields |
| `entry(seq)`, `unannounced_escalations()` | `cache.manage.journal_get(...)`, `journal_list(unannounced=True)` | `idem_key`, `phase`, `precondition`, the stored args; `last_error` is a string |
| `resolve(...)` → `None`; `mark_announced(seq)` | `cache.manage.journal_resolve`, `journal_announce` | no read-back |
| `status()`, `open_work()` | `cache.manage.status()`, `health()`, `scopes()` | raw `meta`; `<phase>:<state>` keys |
| `sync_alert`, `set_sync_alert` | unchanged; the setter stays binding-only | — |
| `CacheError` | adds `kind`, `message`, `details`, `suggested_action`, `retryable` | additive |

The differences a 0.1 caller can meet, each additive or opt-in:

| Ref | Difference | For a 0.1 call |
|---|---|---|
| D1 | **`outage` on Ok.** A 0.1 `sync(phase, timeout)` that cannot reach bd returns its report with `outage` set; it does not raise (ADR-BDC-0005). The 0.2 `refresh_scopes` reports the same sync as a failed result (`BD_UNAVAILABLE`, counted in `failed`; S75), and every error that can come from bd gains `details.outage` (§4). | unchanged; check `outage` (and `deferred`) on the 0.1 report, since a caller that clears its alert on "no exception" would clear it during an outage |
| D2 | **The `issues_list` default exclusions.** 0.2 `issues_list` leaves out closed, template and gate rows unless `status` or `all` is given (§5.3); the 0.1 `list` returns every row of the scope. | unchanged; a caller moving to `issues_list` passes `all=True` (or `status`) where it needs every row |
| D3 | **Optional `changed` / `already_closed`.** The 0.2 write replies omit `changed` (`issues_update`) and `already_closed` (`issues_close`) when they are unknown, for an entry written without them (S68, §5.3). | unchanged: the 0.1 `write` reply is `{seq, duplicate}` |
| D4 | **One handle per process.** One process holds one connection per cache file: a second `Cache` on the same file in the same process is `STORE_IN_USE` until the first is closed (ADR-PY-0003); separate processes are fine. The 0.2 namespaces, `tick()` and the sync methods all run on the handle's own connection (ADR-PY-0006). | unchanged (the rule is 0.1's); a hook or script opens its own handle in its own process |
| D5 | **Install.** The wheel is `bd-cache-rs` (abi3, CPython 3.9 or later) and imports as `bd_cache_rs`; 0.2 is a mixed layout (the native module `bd_cache_rs._native` and the modules `bd_cache_rs.run` and `bd_cache_rs.sync`), and `import bd_cache_rs` gives every 0.1 name unchanged. | unchanged import; install the 0.2 wheel in place of 0.1 |
| D6 | **`cache_path` for an existing file.** The default file is `.bd-cache/<dolt_database>.bd-cache.sqlite3`. A file kept elsewhere is opened by passing `cache_path`; there is no search for it and no refusal of it (ADR-BDC-0015). | unchanged: 0.1 `cache_path` is kept |
| D7 | **The 0.1 bare-name rule** (ADR-BDC-0013, REQ-BDC-0812 as amended). The 0.1 verbs keep the 0.1 scope rule as a compatibility rule: a bare `x` and the legacy-prefixed `x` both mean the label scope whose label is the legacy prefix followed by `x`, and a 0.1 `sync` of an unregistered `x` registers that scope. Every other scope argument (the 0.2 namespaces, the new methods, the CLI, the `[[scope]]` tables) takes a scope name as written: there a legacy-prefixed name is another, longer name, and an unknown one is `SCOPE_UNKNOWN`. A registered filter scope named `x` is never what a 0.1 `x` means (`SCOPE_UNKNOWN` on the 0.1 verbs). | unchanged: the corpus pins the rule, and fails when it is dropped |

The new methods raise `CacheError` for every contract error. A Python-level type mistake still
raises `TypeError`.

## 11. Versioning

| Item | Rule |
|---|---|
| `envelope.version` | The sc-command envelope format (`"1"`), not this contract's version. |
| `contract_version` | Semver, reported by `context`, `cache status` and `--version`. |
| Wheel | Exposes `bd_cache_rs.CONTRACT_VERSION`. Wheel 0.2.x implements contract 1.0.x. A contract minor bumps the wheel minor. A contract major is a wheel release marked breaking, and needs its consumers' agreement. |
| **Covered in 1.0** | Commands, positional arguments and flags; the `data` members documented in §5–§6, with their defaults (path, limits, exclusions) and orders; the envelope; the error table's code, kind, exit and retryable; exit codes. |
| **Not covered in 1.0** | The contents of `details` (informative); the cursor encoding (opaque); `--format text` and the help shapes; everything in §12a, which becomes covered when the MCP server ships. |
| Breaking (major) | Removing or renaming a covered item; changing its type or meaning; making a member required; changing a code's kind, exit or retryable; changing a default or an order. |
| Additive (minor) | A new command or flag; an optional request member; a new `data` member; a new code (clients must handle codes they do not know); a `/v0` operation newly offered after a beads release. |
| Pin | Follows beads v1.3.2. Moving the pin is minor if the change is additive, and major otherwise. |

**The phase-B extension, classified (REQ-BDC-0811).** Checked on 2026-10-26: no stable release
exists. The only GitHub release is the prerelease `prerelease/v0.1.0`, built from `develop` at
`577cca9`, before `bd-cache-api` and this contract existed, with wheel 0.1.0 assets; PyPI and
TestPyPI have no `bd-cache-rs` project; `CONTRACT_VERSION` 1.0.0 exists only on
`integrate/phase-b`. Contract 1.0.0 is therefore unreleased, and the extension's changes to it
are pre-release edits, not §11 changes:

| Change | Class |
|---|---|
| the scopes, schedule, cron and sync verbs; `max_pending_age_s`, `timeout_ms` on `refresh scopes`; the health members `reasons`, `oldest_pending_age_s`, `last_drain_at`, `unresolved_escalations`, `sync`; `Scope.filter`; `details.outage`; the codes `INGEST_INVALID`, `SCHEDULE_INVALID`, `TICK_RUN_FAILED`, `TICK_IN_FLIGHT` | additive |
| removed: `cache tick`'s flag naming a send command; the `TICK_<REFUSAL>` codes and `TickReply.refused[]`; the `[tick]` table and its `BD_CACHE__TICK__*` names; the phase-shaped scope ids (scopes are named as written, §5.1); the §12 deferral of other scope kinds; the legacy-file rule and `bdc_legacy_cache_file` (§2) | pre-release edits of the unreleased 1.0.0 |

The prerelease wheel 0.1.0 has none of the removed items, so its verbs are untouched (§10).
`CONTRACT_VERSION` stays 1.0.0 and the wheel stays 0.2.0. *Open for Rand (phase-b-plan.md,
Open items):* this version choice is his decision; the recommendation is contract 1.0.0 and
wheel 0.2.0 with the edits above as pre-release edits.

## 12. Deferred

- `blocked`, `issues batch-get` and the `id` filter on `issues list`, until a
  beads release has them (§5.5).
- `journal list --state applied|resolved`.
- The operations not served (§5.4).
- Schema packages (ADR-BDC-0008).
- MCP transport and auth.

## 12a. Deferred with the MCP server

None of these items is in 1.0. They ship, and become covered by semver, with
the MCP server:

| Item | Design (kept for then) |
|---|---|
| `bdc schema [<cmd>]` | `{contract_version, envelope_version, tools: [{name, cli, mcp: bool, x_v0_operation: {operation_id, capability, spec}, description, input_schema, output_schema, annotations, errors: [code]}], errors: [{code, kind, exit, retryable, details_schema}]}`. The output schemas are the cache's own required sets and allow `data: null`. |
| `x-cli-flag` | The flag of each request member. There is no flag-to-member naming rule. |
| `details_schema` | A JSON Schema for each code's `details`. |
| Tool annotations | Every hint is set explicitly, and each defaults to false. `readOnlyHint` is set on the reads; `openWorldHint` on the calls that can reach bd (get, ready with fresh, refresh, flush, health `--probe`); `idempotentHint` on journaled writes, resolve and announce; `destructiveHint` on `cache evict` and `cache scopes remove`. `cache tick`, `cache cron run` and `cache sync use-python` (which run code) are not tools (`mcp: false`). |
| MCP parity tests | The shared fixtures also run through the MCP handler, and the envelopes must be identical. |

## 13. Open questions

| # | Question | Position |
|---|---|---|
| Q1 | Which beads reference? | **Approved (S60; agreed in the contract review):** pin v1.3.2 @ 9b65b9e for vocabulary. Track main 73f64fd only as the proposed table. Offer an operation that exists only on main once a beads release contains it. The pin fixes names, not behaviour. |
| Q2 | Required `/v0` fields the cache cannot fill | **Approved (S60; agreed in the contract review):** they stay absent. Surface (a) shares `/v0`'s vocabulary, not its behaviour, so no client can switch between the cache and bd serve without code changes. §5.4 lists the divergences, and the output shapes are the cache's own. |
| Q3 | Idempotency key | **Approved (S60; agreed in the contract review):** required on every front end, never derived (REQ-BDC-0004). |
| Q4 | Partial success | **Approved (S60; agreed in the contract review):** no exit 4. Exit 0 with `data.failed`, and `results[]` always. |
| Q5 | Name of the extra member | **Approved (S60; agreed in the contract review):** `x_cache`. |
| Q6 | Default cache path | **Approved (S60):** `.bd-cache/`. *Amended by ADR-BDC-0015:* the refusal of a file kept in another directory is removed; `cache_path` reaches such a file (§2, §10 D6). |
| Q7 | Request to sc-runtime | **Approved (S60; agreed in the contract review):** one ask, a file source with an explicit path where a missing file is allowed. The env overlay is optional, TOML is a nice-to-have, and bdc is not blocked on it. |
| Q8 | Approve ADR-BDC-0009 and the Appendix A amendments? | **Approved (S60).** |

## Appendix A. Requirement amendments

Applied in `docs/requirements.md` (REQ-BDC-0801, 0802, 0803 amended;
REQ-BDC-0806 Superseded; lead ruling S60, ADR-BDC-0009).

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
