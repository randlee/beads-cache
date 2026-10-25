# bd-cache / bdc CLI contract

Status: **Approved** (lead ruling S60, bc-lead, 2026-10-25, at `8115a0a`).
Implemented in phase B (sprints b-1 to b-5; phase B closed 2026-10-25) by
`bd-cache-api`, the `bd-cache`/`bdc` binary (`bd-cache-cli`) and
`bd_cache_rs` 0.2; the MCP server comes later. Contract version: `1.0.0`
(0.2.0 is unreleased, so the additions made after the first review of phase B
stay within it).

Scope: the `bd-cache` binary (alias `bdc`, REQ-BDC-0805), the MCP server that
comes later, and the Python binding `bd_cache_rs`. All three share one
request, data and error vocabulary, owned by the new `bd-cache-api` crate
(§9).

Governing inputs:

- REQ-BDC-0004, 0801..0805, 0901..0912 and NFR-BDC-0002.
- ADR-BDC-0002, 0003, 0005 and 0008 (Draft).
- Lead ruling S59: REQ-BDC-0806 is superseded; the CLI follows sc-command and
  `/v0`.

This contract replaces the command-structure clause of ADR-BDC-0003 through
**ADR-BDC-0009** (Approved, S60, in `docs/architecture.md`). The requirement
amendments it needs are applied in `docs/requirements.md` (Appendix A).

## 1. Shape

| Surface | Commands | Vocabulary |
|---|---|---|
| (a) Beads | `bdc issues get\|list\|update\|close`, `bdc ready`, `bdc dependencies list\|add`, `bdc context` | bd serve `/v0` at the **pinned release, beads v1.3.2** @ [`9b65b9e`][rel] |
| (b) Cache management | `bdc cache status\|health\|scopes\|refresh scopes\|refresh issues\|flush\|evict\|journal get\|journal list\|journal resolve\|journal announce\|tick` | conventional cache terms |

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

The TOML keys `scopes`, `call_timeout_s`, `miss_fill_timeout_ms` and
`max_attempts` are unchanged. Each one can be overridden from the env under
the same name, for example `BD_CACHE__CALL_TIMEOUT_S`. `BD_CACHE__SCOPES` is
comma-separated. Precedence is flag > env > TOML > default.

beads-cache sends no messages and reads no configuration that names a message
recipient (ADR-BDC-0010). The schedule of the built-in sync is a row in
sc-cron's table, not a TOML table (sprint b-7).

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
- **Any other path.** bdc opens the file at the resolved `cache_path` and
  looks nowhere else. A store file kept at another location is reached by
  setting `cache_path` (REQ-BDC-0804 amended: the file stays readable and
  writable by any v3 reader).
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
| `seq`, `bead`, `field`, `cause`, `known`, `source` | `details.*` under the same names. `phase` becomes `details.scope`, in label form. |

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
| `outage` | A bool on the failures that a caller may mistake for an outage (`BD_UNAVAILABLE`, `BD_VERSION_FAILED`, `BD_SCHEMA_FAILED`, `BD_LIST_FAILED`; new, lands in the fix/fen-tick layer). `true`: bd was unreachable (it did not start, did not answer, or was cut). `false`: bd answered and the call failed (for example `BD_LIST_FAILED` from a `bd list` that exited non-zero). A caller that branches on "bd is unreachable" must read this flag, not the code: `BD_LIST_FAILED` is in the set of codes that can mean unreachable, but it is also a real failure that belongs to the lead. |
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
| `CONFIG_INVALID` † | invalid_input | 3 | no | — | `details.source` keeps the underlying code: `store_config_*`, `sync_config_*`, `sync_beads_dir_unresolvable` or `sync_repo_root_not_utf8`. bdc's own case (S71): `bdc_env_invalid` for a `BD_CACHE__*` value that does not parse. |
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
| `issues update <id>` | `id`, `actor`, `idempotency_key`, `patch: {status?, assignee?, add_labels?, remove_labels?, append_notes?, metadata?: {set?, unset?}}`, `expected_status?`, `expected_assignee?` | `{issue, changed?: bool, x_cache}`. `changed` is **optional**: it is omitted when it is unknown (an entry written without it, by an earlier writer of the file or before the column existed; S68). | n/a | `BAD_ARGS` (empty patch; `status: "closed"`; unknown member), `BAD_PRECONDITION`, `UNSUPPORTED` (`claim`, `expected_version`, `force_*`, other patch members), `PRECONDITION`, `IDEM_CONFLICT`, `NOT_FOUND`, `BD_UNAVAILABLE` |
| `issues close <id>` | `id`, `actor`, `idempotency_key`, `reason?`, and the cache-extension guards `expected_status?`, `expected_assignee?` | `{issue, already_closed?: bool, x_cache}`. `already_closed` is **optional**: it is omitted when it is unknown, as `changed` is (S68). | n/a | as update; `UNSUPPORTED` (`session`, `force`, `expected_version`) |
| `dependencies add <issue_id> <depends_on_id> --type T` | `actor`, `idempotency_key`, `edges: [{issue_id, depends_on_id, type}]` (exactly one edge) | `{added: [edge], x_cache}` | n/a | as update; `UNSUPPORTED` (more than one edge) |
| `context` | none | `{bd_version?, database, project_id, beads_dir?, repo_root?, capabilities: [token], x_cache: {cache_path, contract_version, store_schema_version, bd_schema_sha256, bd_schema_version, model_current}}` | n/a | the open-time errors only |

Notes by command:

- **`issues get`.** `pending` counts the bead's own journal entries that
  have not been applied. The core needs to add `GetReply.scope`.
- **`issues list`.**
  - The default exclusions are `closed` rows, templates and `gate` rows,
    unless `status` or `all` is set. The workspace's configured infra types
    are unknown to the cache, so they are **not** excluded.
  - **This differs from the 0.1 `list(phase, ...)` verb, which returns every
    row of the phase.** A caller that needs closed, template and gate rows
    passes `all`
    (`bdc issues list --scope phase-<x> --all`, `issues_list(scope=, all=True)`
    in the binding), or names the statuses it wants
    (`--status closed`). Moving such a caller to `issues_list` without `all`
    silently drops those rows. Staying on `list` keeps the 0.1 behaviour.
  - Default limit: inside a known scope there is **no limit**, because a
    scope is already bounded; a caller reading a phase expects the
    whole phase. An explicit `limit` pages, with `has_more` and
    `next_cursor`. If a call without a scope is ever allowed, it will take
    `/v0`'s defaults.
  - The cursor is opaque. It is currently `v1.` followed by base64url of
    `[sort, key…]`, but clients must not parse it, and its encoding is not
    covered by semver.
- **Blocked beads, until a beads release ships `/blocked`.** The supported
  replacement for `bd blocked -l phase-<x>` is the `blocked_by` that
  `issues list` reports for each row of the scope:

  ```sh
  bdc issues list --scope phase-<x> --all --sort priority \
    | jq '.data as $d | [$d.items[]
          | select(($d.x_cache.rows[.id].blocked_by // []) | length > 0)
          | . + {blocked_by: $d.x_cache.rows[.id].blocked_by}]'
  ```

  - Membership is the same as the cache's blocked set: bd's
    `bd blocked -l phase-<x>` at the last sync, adjusted for our journaled
    writes (ADR-BDC-0002).
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

| Command | Core verb | Old verb | Mutates |
|---|---|---|---|
| `cache status` | `status` + **new** `journal_counts()` | `status` | no |
| `cache health [--probe] [--max-pending-age-s N]` | `open_work`, outage, stale rows, the oldest pending entry's age, the last drain, unresolved escalations; with `--probe`, also the backend check | (`doctor`) | no; **yes with `--probe`** (store): it records bd's fingerprint, a changed schema rebuilds the cache (cached rows dropped, journal kept), and an unreachable bd is recorded as an outage |
| `cache scopes` | `status().phases` + config + `sync_alert` per scope (N store reads, no bd) | (in `status`) | no |
| `cache refresh scopes [S…] [--timeout-ms N]` | `sync` per scope; with no S, every known scope | `sync` | store |
| `cache refresh issues ID…` | `refresh(ids)` (REQ-BDC-0907) | — | store |
| `cache flush` | `drain(budget)` | `drain` | bd |
| `cache evict --scope S` | `prune` | `prune` | store |
| `cache journal get <seq> \| --idempotency-key K` | `entry`; **new** facade `entry_by_key` (the store already has it) | — | no |
| `cache journal list [--state pending\|escalated] [--unannounced]` | `pending_entries`, `status().escalated`, `unannounced_escalations` | (in `status`) | no |
| `cache journal resolve <seq> --note T --actor A` | `resolve` | `resolve` | store |
| `cache journal announce <seq>` | `mark_announced` | (cron) | store |
| `cache tick [--budget-ms N]` | bd-cache-sync `tick`: runs the built-in sync once, on demand (sprint b-7) | — | bd, store |

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
milliseconds, per scope), and the per-call cap otherwise, so a bd that hangs
times out exactly at it. A single bd call is still capped by the per-call
cap, so a `timeout_ms` above it does not lengthen a call, only the sync as a
whole. A hook that needs a bounded sync passes `timeout_ms`; the 0.1
`sync(phase, timeout)` verb is unchanged; the core then returns its report with `deferred: true` and no outage (a
cut records none). `error` is `BD_UNAVAILABLE` with `details.timed_out:
true`, it counts in `data.failed`, and a scope that was never synced stays
unserved (`SCOPE_UNKNOWN`).

**Health states (I7).** `down` while an outage holds or a probe could not
reach bd. Otherwise `degraded`, with a `reasons[]` item for each cause, when:

| `reasons[].code` | Cause |
|---|---|
| `stale_rows` | a row failed its read-back and is stale |
| `unresolved_escalations` | any escalated entry is not yet resolved, announced or not (`unresolved_escalations` counts them; `open_work.escalated_unannounced` is the subset not yet announced). Settle each with `bdc cache journal resolve <seq> --note T --actor A`. |
| `pending_too_old` | `oldest_pending_age_s` is greater than the limit |

and `ok` otherwise. `oldest_pending_age_s` is whole seconds from the oldest
pending entry's `created_at` to now (`null` with nothing pending; never
negative). The limit is `--max-pending-age-s N` (request member
`max_pending_age_s`), **300 s by default**: the cron drains every minute, so
an entry that old means the drainer is not running or cannot apply it. An age
equal to the limit is `ok`. `last_drain_at` is when the journal last applied
an entry. It is derived from the existing `applied_at` column, with no schema
change: the newest `applied_at` among the 256 most recent entries, so it is
`null` when none of them was applied (a drainer that has applied nothing
lately). Health reads the store and starts no bd call without `--probe`.

**Probe that cannot reach bd (QA ARCH-001).** `cache health --probe` whose
backend check fails with an unreachable code (`BD_UNAVAILABLE`,
`BD_VERSION_FAILED`, `BD_SCHEMA_FAILED`, `BD_LIST_FAILED`; a deadline cut is
`BD_UNAVAILABLE`) is a result, exit 0: `state: "down"`, `probe: null`, and
`outage` populated. The probe's deadline lies past the per-call cap, so a bd
that hangs is ended by the cap and recorded as the outage; `outage` is the
recorded one, or, when the probe recorded none (bd answered `bd version`
with a failure), `{since: <now>, last_error: <the probe's error>}`. Only a
store error is a typed error (exit 3).

Three cases have fixed answers:

- A bare `cache refresh` is `USAGE`, with `details.valid: [scopes, issues]`.
- `journal list` with no flag means `--state escalated`. The `applied` and
  `resolved` states are `UNSUPPORTED` until the store has a listing for them.
- No **invalidate** command is offered. The core has no such verb, and
  `refresh` is the honest form of it.

| Command | `data` | Order |
|---|---|---|
| `cache status` | `{cache_path, contract_version, store_schema_version: int, project: {database, project_id}, backend: {bd_version, bd_schema_sha256, bd_schema_version}, journal: {pending, applied, escalated, resolved, by_scope: {<scope>: {<state>: int}}, unscoped: {<state>: int}}, scopes: [Scope], escalated: [Entry], outage, stale: [id]}` | scopes by id; entries by seq, ascending |
| `cache health` | `{state: "ok"\|"degraded"\|"down", reasons: [{code, message}], outage, open_work: {pending, escalated_unannounced}, stale_rows: int, oldest_pending_age_s: int\|null, last_drain_at: ts\|null, unresolved_escalations: int, probe: {bd_version, model_current, rebuilt}\|null}`. With `--probe` and bd unreachable: `state: "down"`, `outage` set, `probe: null` (ARCH-001, above). The `state` rules are below. | — |
| `cache scopes` | `{scopes: [Scope]}`. A Scope is `{scope: "phase-bk", configured: bool, on_demand: bool, last_started_at, last_ok_at, last_error: OpError\|null, alert, beads: int}`, where `alert` is `"backend_unavailable"`, `"sync_failed"`, a `BD_CACHE` code, or `null`. | by scope id |
| `cache refresh scopes` | takes `scopes`, `timeout_ms`. `{failed: int, results: [{scope, report: {rebuilt, model_current, stored, blocked, deferred, outage}\|null, error: {code, message, kind, retryable, details}\|null}]}`. `error` is an OpError plus its code's `kind` and `retryable` from §4 and the `details` its envelope error would carry (`{}` for none; `timed_out` for `BD_UNAVAILABLE`). A scope that could not reach bd, or whose sync the deadline cut, is a failed result (S75, RCR-001, above). | request order (by scope id when no scope is named) |
| `cache refresh issues` | `{issues: [row], x_cache: {rows}}`. The error is `BD_UNAVAILABLE` (`timed_out`). Also takes `timeout_ms`. Ids that could not reach bd fail the command with `BD_UNAVAILABLE` (S75); there is no per-id result. | request order |
| `cache flush [--start-gate-ms N] [--deadline-ms M]` | `{busy, applied: [seq], retrying: [seq], escalated: [{seq, bead, op, actor, scope, reason: OpError}], deferred: [seq], deferred_in_flight: [seq], outage}`. N and M default to 25,000 and 28,000 ms, and M must be greater than N. | seq, ascending |
| `cache evict` | `{scope, beads_deleted: int, out_of_scope_swept: int}` | — |
| `journal get` | `{entry: Entry}` | — |
| `journal list` | `{entries: [Entry]}` | seq, ascending |
| `journal resolve` | `{entry, already_resolved: bool}`. An entry already resolved returns `true` and is left unchanged; a pending or applied entry is `NOT_ESCALATED`. | — |
| `journal announce` | `{entry, already_announced: bool}` | — |
| `cache tick` | the on-demand run's reply: each scope's result and error (§6.1; the `TickReply` with `refused[]` is removed by sprint b-7) | as produced |

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

### 6.1 Commands and members landing in sprint b-7 (ADR-BDC-0010)

Everything in this subsection lands in sprint b-7
(`docs/plans/phase-b/sprint-b-7-sync-job.md`); until it merges the binary does
not offer it. The data shapes are fixed by b-7 and written into the tables
above when it lands. beads-cache sends no messages: the built-in sync
records state, and a registered hook acts on it.

| Item | Contract | Lands in |
|---|---|---|
| `bdc cache schedule set\|show\|enable\|disable\|clear` | Manages the built-in sync row in the cache file's SQLite cron table. There is at most one built-in row per repository, and a second is refused (REQ-BDC-0701). A row has a cron string, a timeout (the sync budget, REQ-BDC-0703), a JSON payload, an `enabled` flag and a hook target (ADR-BDC-0010). `enable` and `disable` flip only sc-cron's `enabled` column on the existing row, without rewriting it; a consumer disables the built-in row this way before enabling its own Python row. | b-7 |
| `bdc cache tick` | Runs the built-in sync once, on demand, with the job's budget. The schedule stays in the cron table. The `refused` and `pass-through` replies are gone: the reply carries each scope's result and error. | b-7 |
| `bdc cache scopes ingest` and `ingest_scope(scope, rows, blocked, revision=None)` | Feeds the cache from a consumer's own sync (REQ-BDC-0708). Rules: the **strict row decoder** of the built-in sync; **nothing is stored on a bad row** (one bad row rejects the whole ingest, in one all-or-nothing transaction); the **newer-row guard** (an older row never replaces a newer one). Offered in Rust, in the CLI and in the Python binding, as an addition (REQ-BDC-0812). | b-7 |
| Post-sync hook request | After each built-in sync the job calls the optional hook named in its row's payload, through sc-cron's Python runner, bounded by the row's timeout (NFR-BDC-0004). The request carries each scope's result and error, its `failing_since` and notified state, the escalated entries not yet notified, and the pending count. Reads never wait on the hook (REQ-BDC-0001). | b-7 |
| `cache journal announce` and `cache scopes announce --since` | How the hook marks what it handled: `journal announce` for an escalated entry, and the compare-and-set `scopes announce --since` for a scope's sync failure. What the hook does not mark is offered again on the next run (REQ-BDC-0706). | b-7 |
| `details.outage` | An error that can come from bd carries `details.outage: bool`, true only when bd could not be reached. A `BD_LIST_FAILED` from a bd that answered has `outage: false`. Reads keep coming from the cache during an outage (ADR-BDC-0005). | b-7 |

## 7. Audit pairs

| Mutation | Read that verifies it |
|---|---|
| `issues update` / `close`, `dependencies add` | `cache journal get <seq>`, or `--idempotency-key K` after a lost reply; `issues get <id>` for the view; after a flush, `cache refresh issues <id>` for bd's row |
| `cache flush` | `cache journal get <seq>` for each seq it reports; `cache status` |
| `cache refresh scopes` | `cache scopes` (`last_ok_at`, `last_error`) |
| `cache refresh issues` | `issues get` (`x_cache.rows[id].stale` is false) |
| `cache evict` | `cache scopes`; `issues list --scope` returns `SCOPE_UNKNOWN` (see below) |
| `journal resolve` / `journal announce` | `cache journal get <seq>`; `cache journal list --unannounced` |
| `cache tick` | `cache status`, `cache scopes`, `cache journal list --unannounced` |

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
  then the note, which is the 0.1 rendering.
- **S68.** `WriteOutcome.changed` is `Option<bool>`: `Some(stored)` when the
  entry stores it; for an entry stored without it (by an earlier writer
  of the file, or before the column existed), `Some(predicted)` only while the entry is
  pending; `None` (unknown) in any other state. It is never guessed.

`blocked(scope)` is deferred (§5.5). The CI boundary check reads the
Boundaries table in `docs/architecture.md`, so adding the rows there (done with
ADR-BDC-0009) is the whole CI change.

## 10. 0.1 → 0.2 API differences

A caller built against the wheel 0.1.0 dicts keeps working, so every change
here is **additive**:

- nothing that exists changes;
- 0.2.x issues no `DeprecationWarning`;
- the new methods live in two namespaces, `cache.beads` and `cache.manage`,
  that mirror the commands. A method's name is the command path joined with
  `_`, without the `cache_` prefix for surface (b). It takes the command's
  request as keyword arguments and returns the command's `data`.

| 0.1 verb (unchanged) | New in 0.2 | How the 0.1 shape differs |
|---|---|---|
| `get(bead, miss_timeout)` | `cache.beads.issues_get(id=, miss_timeout_ms=)` | `{row: {bead, stale, blocked}, scope_last_ok_at, pending, outage}`. **Unit change:** 0.1 takes seconds (float), 0.2 takes milliseconds (int). |
| `list(phase, status, label)` | `cache.beads.issues_list(...)` | `{scope: "bk", last_ok_at, rows}`; no paging |
| `ready(phase, fresh_within)` | `cache.beads.ready(...)` | the beads are in `ready`; the basis is top-level; seconds |
| `write(...)` | `cache.beads.issues_update`, `issues_close`, `dependencies_add` | `{seq, duplicate}`; an unknown op raises `ValueError`; the stored op spellings |
| — | `cache.beads.dependencies_list`, `cache.beads.context` | new |
| `refresh(ids, timeout)` | `cache.manage.refresh_issues(...)` | a bare list; seconds |
| `sync(phase, timeout)` | `cache.manage.refresh_scopes(...)` | one report, not `results[]` |
| `drain(start_gate_s, hard_s)` | `cache.manage.flush(...)` | seconds; `phase`; `reason` is a string |
| `prune(phase)` | `cache.manage.evict(...)` | same fields |
| `entry(seq)`, `unannounced_escalations()` | `cache.manage.journal_get(...)`, `journal_list(unannounced=True)` | `idem_key`, `phase`, `precondition`, the stored args; `last_error` is a string |
| `resolve(...)` → `None`; `mark_announced(seq)` | `cache.manage.journal_resolve`, `journal_announce` | no read-back |
| `status()`, `open_work()` | `cache.manage.status()`, `health()`, `scopes()` | raw `meta`; `<phase>:<state>` keys |
| `sync_alert`, `set_sync_alert` | unchanged; the setter stays binding-only | — |
| `CacheError` | adds `kind`, `message`, `details`, `suggested_action`, `retryable` | additive |

Every method, old and new, accepts a bare (`bk`) or label (`phase-bk`) scope
id. The new methods raise `CacheError` for every contract error. A
Python-level type mistake still raises `TypeError`.

### Behaviour that differs between a 0.1 call and its 0.2 counterpart

| Ref | What differs | What to do |
|---|---|---|
| D1 | `refresh_scopes` reports a sync that cannot reach bd as a **failed result** (`BD_UNAVAILABLE`, counted in `data.failed`; S75). The 0.1-shaped `sync(phase, timeout)` returns its report with `outage` set instead. | Check `outage` (and `deferred`) on the 0.1 report. A caller that clears its sync alert on "no exception" would clear it during an outage. |
| D2 | 0.2 `issues_list` **excludes closed, template and gate rows by default**, unlike the 0.1 `list`, which returns every row of the phase (§5.3). `refresh_scopes` takes `timeout_ms`, so a bounded sync no longer needs the 0.1 `sync(phase, timeout)`. | Pass `all` (or `status`) to `issues_list` where every row is needed, or stay on `list`. |
| D3 | A resolve note is stored in `note`, not in `last_error`. The 0.1 `entry()` rebuilds the old rendering, the error then ` \| resolved by <actor>: <note>`; the 0.2 `journal_get` returns `note` on its own. | Read `note` through 0.2. |
| D4 | A close counts as achieved when the status is already `closed`, and `append_notes` when the notes **contain** the text, so the **reason of a close is lost** if another writer closed the bead first. Both calls behave this way in 0.1 and 0.2. | Do not rely on `reason` being recorded for an already-closed bead (`already_closed` says so, when known). |
| D5 | `cache health` is new in 0.2. It says `degraded` for an old pending entry and for an unresolved escalation (§6). | Read `cache.manage.health()` where 0.1 `status()` and `open_work()` were combined by hand. |

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

## 12. Deferred

- `blocked`, `issues batch-get` and the `id` filter on `issues list`, until a
  beads release has them (§5.5).
- `journal list --state applied|resolved`.
- The operations not served (§5.4).
- Scope kinds other than `phase-<x>`.
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
| Tool annotations | Every hint is set explicitly, and each defaults to false. `readOnlyHint` is set on the reads; `openWorldHint` on the calls that can reach bd (get, ready with fresh, refresh, flush, health `--probe`); `idempotentHint` on journaled writes, resolve and announce; `destructiveHint` on `cache evict` only. `cache tick` is not a tool (`mcp: false`). |
| MCP parity tests | The shared fixtures also run through the MCP handler, and the envelopes must be identical. |

## 13. Open questions

| # | Question | Position |
|---|---|---|
| Q1 | Which beads reference? | **Approved (S60; agreed with fenix):** pin v1.3.2 @ 9b65b9e for vocabulary. Track main 73f64fd only as the proposed table. Offer an operation that exists only on main once a beads release contains it. The pin fixes names, not behaviour. |
| Q2 | Required `/v0` fields the cache cannot fill | **Approved (S60; agreed with fenix):** they stay absent. Surface (a) shares `/v0`'s vocabulary, not its behaviour, so no client can switch between the cache and bd serve without code changes. §5.4 lists the divergences, and the output shapes are the cache's own. |
| Q3 | Idempotency key | **Approved (S60; agreed with fenix):** required on every front end, never derived (REQ-BDC-0004). |
| Q4 | Partial success | **Approved (S60; agreed with fenix):** no exit 4. Exit 0 with `data.failed`, and `results[]` always. |
| Q5 | Name of the extra member | **Approved (S60; agreed with fenix):** `x_cache`. |
| Q6 | Default cache path | **Approved (S60; agreed with fenix):** `.bd-cache/`. The refusal for a file at another location was removed with ADR-BDC-0010 (sprint b-8); `cache_path` reaches such a file. |
| Q7 | Request to sc-runtime | **Approved (S60; agreed with fenix):** one ask, a file source with an explicit path where a missing file is allowed. The env overlay is optional, TOML is a nice-to-have, and bdc is not blocked on it. |
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
