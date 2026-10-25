# bd-cache / bdc CLI contract

Status: **Draft for approval** (Rand). Design only; nothing here is implemented.
Contract version: `1.0.0-draft`.

Scope: the `bd-cache` binary (alias `bdc`, REQ-BDC-0805), the later MCP server
that maps the same operations, and the reply shapes of the Python binding
`bd_cache_rs`, so all three front ends share one vocabulary.

Governing inputs: REQ-BDC-0801..0805, 0901..0912, NFR-BDC-0002; ADR-BDC-0002,
0003, 0005, 0008 (Draft); lead ruling S59 (REQ-BDC-0806 superseded: parity
with `bd_cache.py` is behavioural, and the CLI follows sc-command and `/v0`).

## 1. Shape of the design

The CLI has two surfaces, and the command tree shows which one a command is on:

| Surface | Command prefix | Vocabulary | Purpose |
|---|---|---|---|
| (a) Beads | `bdc issues …`, `bdc ready`, `bdc blocked`, `bdc dependencies …`, `bdc context` | bd serve `/v0` (beads `origin/main`, the proposed interface) | An agent sees the shape it would get from beads directly. |
| (b) Cache management | `bdc cache …` | Conventional cache terms: refresh, flush, evict, journal, scopes, status, health | Operating the replica: fill, drain, prune, escalations, outage. |

Why the top-level noun is the `/v0` resource: an MCP client that knows `/v0`
finds `issues_get`, `ready` and `dependencies_add` under the same names. Cache
operations sit under one `cache` noun, so they cannot collide with a future
`/v0` resource.

The rules every command follows:

1. **One JSON envelope on stdout** for every command and every failure,
   usage errors included (sc-command `Envelope`, §4). There is no prose on
   stdout. Logs go to stderr.
2. **`/v0` bodies are kept as they are, plus one `cache` member.** On surface
   (a), `data` is the `/v0` response body. Anything the cache adds lives in a
   single top-level member named `cache`. A `/v0` client that ignores unknown
   members keeps working. Rows stay exactly as bd returns them, with the
   journal overlay applied. Row-level cache facts are in `cache.rows`, keyed
   by id, and never inside the row.
3. **A journaled write is not an applied write.** Every write reply carries
   `cache.seq` and `cache.state`. `/v0` fields the cache cannot know
   (`revision`, `open_children`) are **left out, never faked** (absent, not
   `0`).
4. **Nothing is faked.** A `/v0` operation, parameter or patch member the
   cache cannot serve is refused with `BD_CACHE.UNSUPPORTED` (§4). It is never
   ignored silently.
5. **One request model per operation.** The same JSON object comes from the
   flags, from `--input`, or as the MCP tool arguments (§8).

## 2. Global options

Global options are accepted before or after the subcommand (clap `global =
true`). Long options must be spelled in full: abbreviations are refused.

| Flag | Env (`BD_CACHE__…`) | `.beads/bd-cache.toml` | Default | Meaning |
|---|---|---|---|---|
| `--repo DIR` | `REPO` | n/a | cwd (no upward search) | Beads repository root. `.beads/metadata.json` is read here, or under `$BEADS_DIR` when that is set. |
| `--cache-path PATH` | `CACHE_PATH` | `cache_path` | `.bd-cache/<dolt_database>.bd-cache.sqlite3` | The cache file. A relative path is resolved against `--repo`. |
| `--bd PATH` | `BD` | `bd` (new key) | `bd` on `PATH` | The bd binary (closes the prototype gap). |
| `--format json\|text` | `FORMAT` | n/a | `json` | `text` renders the same `data` for people. It is never auto-selected from a TTY. |
| `--input FILE\|-` | n/a | n/a | none | The whole request as JSON (§8). It cannot be combined with request flags (`BD_CACHE.USAGE`). |
| `-v` / `-q` | `LOG` | n/a | warn | Verbosity of the stderr log only. It never changes stdout. |

Precedence: flag > env > `.beads/bd-cache.toml` > built-in default. The TOML
keys `scopes`, `call_timeout_s`, `miss_fill_timeout_ms`, `max_attempts` and
`[tick]` are unchanged. Each one also takes an env override with the same
name (for example `BD_CACHE__CALL_TIMEOUT_S`). `BD_CACHE__SCOPES` is
comma-separated.

**The default cache path is Rust's `.bd-cache/`.** A repository that wants to
keep the Python file sets `cache_path = ".atm-bd/<db>.bd-cache.sqlite3"` for
the cutover. That is one line of config, not a code path.

**`get` latency (NFR-BDC-0002, < 50 ms new process).** A cache hit opens the
store and reads one row. It starts no bd process, runs no backend check and
does no schema-package work. Config loading is one optional TOML read plus the
env. CI asserts the budget against the fake bd.

### Config resolution (the sc-config gap)

At `prerelease/v0.1.0`, sc-config loads JSON only, from `<dir>/default.json`
(required) plus `local.json` plus `<APP>__` env. bdc is configured from
`.beads/bd-cache.toml`, and there a missing file is valid.

**Decision for now.** Keep bd-cache-store's `load_repo_config` as the one TOML
loader. bdc applies env overrides itself, using sc-config's naming rule
(`AppName::env_prefix("bd-cache")` = `BD_CACHE__`), so that switching to
sc-config later changes no names. bdc does not call `sc_config::load`.

**Request to the sc-runtime team (to file):**
1. A file source with an explicit path, where a missing file is allowed and
   yields defaults.
2. A TOML format for that source, or a pluggable format.
3. An env-only overlay (`env_overrides(app) -> Value`) that a caller can
   merge over its own loader.

With (1) and (2), bdc moves to sc-config with no change visible to users.

## 3. Exit codes

| Exit | `ok` | Meaning |
|---|---|---|
| 0 | `true` | Success, including degraded success such as an outage or a cached fallback, which is reported in `data`. |
| 2 | `false` | Usage error: an unknown command or flag, a missing or conflicting argument, `--input` mixed with flags, or an abbreviated option. `error.code` is `BD_CACHE.USAGE`, `kind` is `invalid_input`, `details` is `{arg?, usage}`, and `suggested_action` names the `--help` to run. |
| 3 | `false` | A typed error: every other `BD_CACHE.*` code (§4). |
| 4 | `true` | Partial success. Only `cache refresh` with several targets returns it: `data.results[]` holds one result per target, and at least one carries `error`. |

`--help` prints text and exits 0. `bdc schema` is the machine-readable help
(§8). A panic is caught at the top level and reported as `BD_CACHE.INTERNAL`
with exit 3, never a backtrace on stdout. Invalid JSON in `--input`, `--patch`
or `--set-metadata` is `BD_CACHE.BAD_ARGS` (exit 3), never a crash.

## 4. Envelope and error mapping

The envelope is sc-command 0.1.0, exactly:
`{version: "1", ok, data, error: {kind, code, message, details, suggested_action}}`.
`data` is `null` on failure and `error` is `null` on success. The decoder
rejects extra keys, so nothing is added at either level.

The mapping from `CacheError` to `OpError`:

| `CacheError` today | Envelope field |
|---|---|
| `code` (`BD_CACHE.*`) | `error.code`, unchanged. sc-observability-types 1.5.0 `ErrorCode` is an unvalidated string, so `BD_CACHE.*` passes as it is. |
| `detail` | `error.message` |
| `fix` | `error.suggested_action` (a recoverable remediation) |
| `reason` (`PRECONDITION`) | `details.reason`, using `/v0` names |
| payload: `unmet`, `seq`, `bead`, `field`, `phase`, `cause`, `known`, `source` | `details.*`. `phase` is renamed `scope` and given in label form (`phase-bk`). `unmet` is also spelled the `/v0` way, as `details.expected_status` / `details.actual_status` / `details.expected_assignee` / `details.actual_assignee`. |
| (new) | `details.v0_code`: the `/v0` Problem `code` a client of beads would get for the same failure (table below). |

`details` never uses the key `justification`, which sc-command reserves for a
non-recoverable remediation.

### Code → kind table

The proposal is accepted as given, with five additive codes marked †. Kind is
fixed per code, so automation can branch on either one.

| Code | Kind (HTTP) | `details.v0_code` | Emitted by the CLI |
|---|---|---|---|
| `USAGE` † | invalid_input (400) | `invalid_argument` | yes (exit 2) |
| `BAD_OP` | invalid_input | `invalid_argument` | no: the command implies the op (bindings only) |
| `BAD_ARGS` | invalid_input | `invalid_argument` | yes |
| `BAD_PRECONDITION` | invalid_input | `invalid_argument` | yes |
| `UNSUPPORTED` † | invalid_input | `invalid_argument` (`reason: unknown_parameter`) | yes; `details: {operation?, param?, member?}` |
| `CONFIG_INVALID` † | invalid_input | — | yes; wraps `ConfigLoadError` and `ComposeError`, and keeps the source code (`store_config_*`, `sync_config_*`, `sync_beads_dir_unresolvable`, `sync_repo_root_not_utf8`) in `details.source` |
| `NOT_FOUND` | not_found (404) | `not_found` | yes |
| `NOT_CACHED` † | not_found | `not_found` | yes: `--miss-timeout-ms 0` on a miss (no bd call) |
| `SCOPE_UNKNOWN` | not_found | — | yes; `details.known` |
| `NO_DATABASE` | not_found | — | yes |
| `PRECONDITION` | conflict (409) | the `/v0` reason (`precondition_failed`, `not_closable`, `already_claimed`) | yes |
| `IDEM_CONFLICT` | conflict | `already_exists` | yes; `details.seq` |
| `PRUNE_BLOCKED` | conflict | — | yes |
| `NOT_ESCALATED` | conflict | — | yes |
| `WRONG_DATABASE` | conflict | `invalid_argument` (`reason: project_mismatch`) | yes |
| `SCHEMA_VERSION` | conflict | — | yes |
| `STORE_IN_USE` | conflict | — | no: one handle per process (bindings only) |
| `BD_UNAVAILABLE` | unavailable (503) | `db_unavailable` | yes |
| `BD_VERSION_FAILED` | unavailable | `db_unavailable` | yes |
| `BD_SCHEMA_FAILED` | unavailable | `db_unavailable` | yes |
| `BD_LIST_FAILED` | unavailable | `db_unavailable` | yes |
| `SYNC_FAILED` | unavailable | `db_unavailable` | inside `cache refresh` results only; `details.cause` |
| `BD_WRITE_FAILED` | unavailable | — | no: it lives in flush reports and journal entries |
| `BD_CONTRACT` | internal (500) | `internal` | yes |
| `STORE_FAILED` | internal | `internal` | yes |
| `INTERNAL` † | internal | `internal` | yes: a caught panic |

`BD_CACHE.` is left off the code column. `NO_TOML` stays retired.
`EXTENSION_INVALID` (ADR-BDC-0008) is reserved and not emitted.

## 5. Surface (a): Beads (`/v0`)

The reference is beads `origin/main` @ [`73f64fd`][main], the proposed
interface. Released behaviour is v1.3.2 @ [`9b65b9e`][rel]. **main-only**
marks an operation or field that is not in v1.3.2. The branches
`feat/http-update-claim`, `feat/http-client-write-roles`,
`fix/served-close-refusal-names-blockers` and `feat/ready-policy-in-workapi`
change the spec, but not any field this contract maps (they touch `claim`,
dependent protection and close-refusal `blockers`).

This contract copies no spec content. It names only the members the CLI maps;
everything else is in the spec at the cited lines.

### Shared rules

- **Row.** bd's list row (`IssueWithCounts`; the same struct that `bd list
  --json` prints and that sc-beads `Bead` models), with pending journal
  entries replayed. The counts are bd's as of the last fetch: the overlay
  does not adjust them.
- **Scope selection.** The cache serves list-like reads only inside a known
  scope (REQ-BDC-0902). Every `label` value is checked in order, and the
  first one that names a known scope (`phase-<x>`) selects it. All `label`
  values are still applied as an AND filter, so the result is the one bd
  would give. `--scope` names the scope explicitly. With neither, the reply
  is `SCOPE_UNKNOWN` with `details.known`: the cache never answers from a
  partial replica.
- **`cache` member on reads:** `{scope: str|null, last_ok_at: ts|null,
  outage: {since: ts, last_error: str}|null, rows: {<id>: {stale: bool,
  blocked_by: [id]|null}}}`. In `blocked_by`, `null` means unknown and `[]`
  means not blocked (ADR-BDC-0002). Timestamps are RFC 3339 strings.
- **`cache` member on writes:** `{seq: int, state:
  "pending"|"applied"|"escalated"|"resolved", duplicate: bool}`. `state` is
  read back from the journal. A duplicate idempotency key returns the
  original entry's seq and state.
- **Idempotency.** `idempotency_key` is optional. Without one, the key is
  derived from the whole request (bead, op, canonical args, guards, actor),
  so a repeated identical request is reported as `duplicate: true`
  (REQ-BDC-0004). Open question Q3.
- **Guards.** `expected_status` and `expected_assignee` are checked at write
  time and again at flush time. `expected_version` is `UNSUPPORTED`: the CLI
  backend has no `revision`.

### Operation map

| Command (MCP tool) | `/v0` operation | Spec (main / v1.3.2) | Cache verb | Mutates |
|---|---|---|---|---|
| `issues get` (`issues_get`) | `GET /issues/{id}` getIssue | [L2982-3336][m-get] / [L2042-2347][r-get] | `get`, or `cached` with `--miss-timeout-ms 0` | no (a miss fill stores a row) |
| `issues batch-get` (`issues_batch_get`) | `POST /issues:batchGet` **main-only** | [L4753-4847][m-bget] | `cached` per id | no |
| `issues list` (`issues_list`) | `GET /issues` listIssues | [L1129-2354][m-list] / [L995-1579][r-list] | `list` plus front-end filters | no |
| `ready` (`ready`) | `GET /ready` listReadyWork | [L386-611][m-ready] / [L349-571][r-ready] | `ready(scope, fresh_within)` | no (a fresh answer refreshes the stored base) |
| `blocked` (`blocked`) | `GET /blocked` **main-only** | [L695-785][m-blocked] | **new core verb** `blocked(scope)`, built from the same adjusted base as `ready` | no |
| `dependencies list` (`dependencies_list`) | `GET /dependencies` | [L6060-6168][m-deps] / [L4667-4775][r-deps] | the cached view's `dependencies` | no |
| `issues update` (`issues_update`) | `PATCH /issues/{id}` updateIssue | [L2982-3336][m-get] / [L2042-2347][r-get]; body [L12909-13328][m-upd] | `write(op=update)` | **journals** |
| `issues close` (`issues_close`) | `POST /issues/{id}:close` | [L4027-4195][m-close] / [L2938-3074][r-close] | `write(op=close)` | **journals** |
| `dependencies add` (`dependencies_add`) | `POST /dependencies:add` | [L6463-6597][m-dadd] / [L5051-5174][r-dadd] | `write(op=dep_add)` | **journals** |
| `context` (`context`) | `GET /context` getContext | [L362-385][m-ctx] / [L325-348][r-ctx] | `status` meta plus `metadata.json` | no |

### Per operation

**`bdc issues get <id> [--miss-timeout-ms N]`.** The `data` is the row, plus
`cache: {scope, last_ok_at, outage, stale, blocked_by, pending: int}`. The
`/v0` getIssue answer is an IssueDetails. The cache answers with the list
row, so IssueDetails-only members (`revision`, `dependents`,
dependency-with-metadata objects, `epic_*`) are **absent**. Its
`include_comments`, `include_dependents` and `brief_deps` options are
`UNSUPPORTED`. A miss with N > 0 makes one bounded `bd list --id` call. With
N = 0 it makes no bd call and a miss is `NOT_CACHED`. Errors: `NOT_FOUND`,
`NOT_CACHED`, `BD_UNAVAILABLE`, `BD_CONTRACT`, `STORE_FAILED`, plus the
open-time errors (`NO_DATABASE`, `WRONG_DATABASE`, `SCHEMA_VERSION`,
`CONFIG_INVALID`), which every command can return.

**`bdc issues batch-get <id>…`** (**main-only**). The `data` is
`{issues: [row], missing: [id], cache: {rows, outage}}`. It reads the cache
only and never calls bd. An uncached id is listed in `missing`. Errors:
`BAD_ARGS` (more than 200 ids).

**`bdc issues list`.** The mapped parameters are `status` (repeatable),
`label` (AND), `label_any`, `exclude_label`, `assignee`, `id` (**main-only**),
`all`, `sort` (`created` | `priority`), `limit` (default 50; 0 means
unlimited) and `cursor`. Plus the cache's `--scope`. The `data` is
`{items: [row], has_more: bool, next_cursor?: str, cache}`. The default
exclusions are `closed` rows, templates and `gate` rows, unless `status` or
`all` is set. The workspace's configured infra types are not known to the
cache, so they are **not** excluded (a deviation). The cursor is opaque
(`v1.` plus base64 of the sort key and id), and a stale or foreign cursor is
`BAD_ARGS` (`details.v0_code: invalid_cursor`). Any other listIssues
parameter is `UNSUPPORTED`. Errors: `SCOPE_UNKNOWN`, `BAD_ARGS`,
`UNSUPPORTED`.

**`bdc ready [--fresh-within-ms N]`.** The mapped parameters are `label`,
`label_any`, `exclude_label`, `assignee`, `unassigned`, `priority` and
`limit`. Everything else (`sort`, `type`, `parent`, `metadata_field`, …) is
`UNSUPPORTED`. The `data` is `{items: [row], has_more, cache: {scope,
last_ok_at, rows, basis: {kind: "fresh", fetched_at} | {kind: "cached",
fetched_at, stale: bool} | {kind: "unknown"}, locally_adjusted: bool,
read_fallback: str|null, missing_rows: [id]}}`. The answer reflects our own
journaled writes with no bd call (REQ-BDC-0911). It never takes longer than N
(REQ-BDC-0912). A bead whose blocked state is unknown is never listed.
Errors: `SCOPE_UNKNOWN`, `UNSUPPORTED`.

**`bdc blocked`** (**main-only** in `/v0`). The mapped parameters are
`label`, `label_any` and `exclude_label`. The `data` is `{items: [row +
blocked_by_count: int + blocked_by: [id]], has_more: false, cache: {scope,
last_ok_at, basis, locally_adjusted}}`. It needs the new core verb, which
has no `fresh_within` in this version (deferred). Errors: `SCOPE_UNKNOWN`.

**`bdc dependencies list --issue-id ID… [--type T…]`.** The `data` is
`{items: [{issue_id, depends_on_id, type}], missing: [id], cache: {outage}}`.
These are the outgoing edges of cached views, with any pending `dep_add`
replayed. `/v0` `Dependency.created_at` is **absent**, because the cached
edge does not carry it. An uncached id is listed in `missing` (no bd call).

**`bdc issues update <id> --actor A`** with the patch flags `--status`,
`--assignee` (`""` unassigns), `--add-label L…`, `--remove-label L…`,
`--append-notes TEXT`, `--set-metadata K=V…`, `--unset-metadata K…`, or the
whole patch as `--patch JSON`; plus `--expected-status`,
`--expected-assignee` and `--idempotency-key`. The request is `{id, actor,
patch: {status?, assignee?, add_labels?, remove_labels?, append_notes?,
metadata?: {set?, unset?}}, expected_status?, expected_assignee?,
idempotency_key?}`. BD-CACHE's `set_metadata` / `unset_metadata` are accepted
inside `--patch` and normalized. Any other patch member, `claim`,
`expected_version` or a `force_*` member is `UNSUPPORTED`. An empty patch is
`BAD_ARGS`. In `--set-metadata`, V is read as JSON when it parses as an
integer or a boolean, and as a string otherwise (REQ-BDC-0303). The `data`
is `{issue: row (the view after the write), changed: bool, cache: {seq,
state, duplicate}}`. Here `changed` means the overlay changes the view. It
is a prediction: bd decides at flush time. `revision` is **absent**. Errors:
`BAD_ARGS`, `BAD_PRECONDITION`, `UNSUPPORTED`, `PRECONDITION` (`reason:
precondition_failed`), `IDEM_CONFLICT`, `NOT_FOUND`, `BD_UNAVAILABLE` (an
uncached bead during an outage, REQ-BDC-0903).

**`bdc issues close <id> --actor A [--reason R] [--expected-status S]
[--expected-assignee W] [--idempotency-key K]`.** `/v0` close has no status
or assignee guard. These two are cache extensions, kept from BD-CACHE's
precondition and named as in `/v0` PATCH. `session`, `force`,
`expected_version` and `auto_close_molecule` (**main-only**) are
`UNSUPPORTED`. The `data` is `{issue: row, already_closed: bool, cache:
{seq, state, duplicate}}`, where `already_closed` is the view before the
write. `open_children` and `revision` are **absent**. A `not_closable` or
`already_claimed` refusal from bd arrives at flush time as an escalation; it
is not a write-time error. The write-time errors are as for update.

**`bdc dependencies add <issue_id> <depends_on_id> --type T --actor A
[--idempotency-key K]`.** The request is `{actor, edges: [{issue_id,
depends_on_id, type}], idempotency_key?}`, with exactly one edge. More than
one is `UNSUPPORTED`, because `/v0` adds edges all-or-nothing and the
journal has one edge per entry. The `data` is `{added: [edge], cache: {seq,
state, duplicate}}`. Errors: as for update, plus `BAD_ARGS` for a type bd
does not know (REQ-BDC-0304).

**`bdc context`.** It makes no bd call. The mapped members are
`bd_version` (from the last fingerprint; absent before the first one),
`database`, `project_id`, `beads_dir`, `repo_root`, and `capabilities` (the
operationIds of this table that the cache serves, so `/v0` capability
probing works). `api_version`, `schema_version`, `backend` and `dolt_mode`
are **absent**, because the cache does not know bd's values. The `cache`
member is `{cache_path, contract_version, store_schema_version: int,
bd_schema_sha256: str|null, bd_schema_version: str|null, model_current:
bool|null}`.

### `/v0` operations the cache does not serve

Each of these returns `UNSUPPORTED` (`details.operation`) when called through
MCP, and has no CLI command:

| Operation | Why |
|---|---|
| createIssue, batchCreate, delete, sweep | The journal has no create or delete op (BD-CACHE v1). |
| claim, release, heartbeat, claimNext, reclaim | A claim needs bd's atomic check. A journaled claim could not honestly answer `already_claimed` at write time. |
| reopen, casMetadata, batchApply, batchClose, dependencies:remove | No journal op yet. A whole-metadata replace is an ADR-BDC-0008 open item. |
| issues:query, issues:count, ready:count, stats, dependencies tree/cycles/count/blocking, related, comments | Deferred: these are computable from the replica, but only within a scope. |
| config, memories, events, events:watch, healthz | Not cache data. `cache health` is the cache's own health check. |

## 6. Surface (b): Cache management

| Command (MCP tool) | Core verb(s) | Old BD-CACHE verb | Mutates |
|---|---|---|---|
| `cache status` | `status` | `status` | no |
| `cache health [--probe]` | `open_work`, `status().outage`; with `--probe`, the backend check | (`doctor`, in cron) | no |
| `cache scopes` | `status().phases`, config scopes, `sync_alert` | (part of `status`) | no |
| `cache refresh --scope S… \| --id ID…` | `sync` per scope / `refresh(ids)` | `sync` | **yes** (store) |
| `cache flush` | `drain(budget)` | `drain` | **yes** (bd) |
| `cache evict --scope S` | `prune` | `prune` | **yes** (store) |
| `cache journal get <seq>` | `entry` | (none; core only) | no |
| `cache journal list [--state escalated] [--unannounced]` | `status().escalated` / `unannounced_escalations` | (part of `status`) | no |
| `cache journal resolve <seq> --note T --actor A` | `resolve` | `resolve` | **yes** (store) |
| `cache journal announce <seq>` | `mark_announced` | (none; cron) | **yes** (store) |
| `cache tick [--budget-ms N] [--atm PATH]` | bd-cache-sync `tick` | (`bd_cache_cron.py`) | **yes** (bd, store, atm) |

Not offered: **invalidate** (marking rows stale without a fetch). The core has
no such verb, and `refresh` is the honest version of it. `journal list
--state pending|applied|resolved` is **deferred**: it needs a store listing
verb.

### Shapes

| Command | `data` |
|---|---|
| `cache status` | `{cache_path: str, contract_version: str, store_schema_version: int, project: {database: str, project_id: str}, backend: {bd_version: str\|null, bd_schema_sha256: str\|null, bd_schema_version: str\|null}, journal: {pending: int, applied: int, escalated: int, resolved: int, by_scope: {<scope>: {<state>: int}}, unscoped: {<state>: int}}, scopes: [Scope], escalated: [Entry], outage: Outage\|null, stale: [id]}`. This replaces the raw `meta` dump and the `None:pending` keys: entries with no scope are counted in `unscoped`. |
| `cache health` | `{state: "ok"\|"degraded"\|"down", outage: Outage\|null, open_work: {pending: int, escalated_unannounced: int}, stale_rows: int, probe?: {bd_version: str, model_current: bool, rebuilt: bool}}`. `degraded` means an outage, stale rows or escalations; `down` means `--probe` found bd unreachable. It exits 0 in every state: health is data. |
| `cache scopes` | `{scopes: [Scope]}`, where Scope is `{scope: "phase-bk", configured: bool, on_demand: bool, last_started_at: ts\|null, last_ok_at: ts\|null, last_error: str\|null, alert: str\|null, beads: int}` |
| `cache refresh --scope` | `{results: [{scope, ok: bool, report?: {rebuilt: bool, model_current: bool, stored: int, blocked: int, deferred: bool, outage: Outage\|null}, error?: OpError}]}`. Each scope runs even when an earlier one failed, and the reply shows every one; this fixes the prototype stopping at the first failure. Exit 4 if any `error`. An unreachable backend is a `report` with `outage` set (ADR-BDC-0005). |
| `cache refresh --id` (REQ-BDC-0907) | `{issues: [row], cache: {rows}}`: one `bd list --id`, stored with force, pending entries replayed. `--timeout-ms` (default `miss_fill_timeout_ms`). |
| `cache flush [--start-gate-ms N] [--deadline-ms M]` | `{busy: bool, applied: [seq], retrying: [seq], escalated: [{seq, bead, op, actor, scope, reason}], deferred: [seq], deferred_in_flight: [seq], outage: Outage\|null}`. Defaults: gate 25 s, hard 28 s (REQ-BDC-0402). Requires M > N. |
| `cache evict --scope S` | `{scope, beads_deleted: int, out_of_scope_swept: int}`. Error `PRUNE_BLOCKED`. |
| `cache journal get` / each `journal list` item | Entry: `{seq: int, idempotency_key: str, scope: str\|null, bead: id, op: "close"\|"update"\|"dep_add", args: {}, expected_status?: str, expected_assignee?: str, actor: str, state, attempts: int, last_error: str\|null, announced_at: ts\|null, created_at: ts, applied_at: ts\|null}`. `args` uses the `/v0` patch names (`metadata: {set, unset}`). |
| `cache journal list` | `{entries: [Entry]}` |
| `cache journal resolve` | `{entry: Entry}` (read back after the resolve; the prototype returned a constant). Error `NOT_ESCALATED`. |
| `cache journal announce` | `{entry: Entry}`. Error `NOT_FOUND`. |
| `cache tick` | bd-cache-sync `TickReply` as it is: `{outcome: "pass-through"\|"refused", refused: [{code, detail, fix}], drain, active, synced, deferred, unreadable_roots, sent, unsent, stopped, busy, poisoned}`. A refusal is data (exit 0). |

## 7. Auditing every mutation

| Mutation | Read that verifies it |
|---|---|
| `issues update` / `issues close` / `dependencies add` | `cache journal get <seq>` (state: pending → applied / escalated). `issues get <id>` shows the overlaid view now. After a flush, `cache refresh --id <id>` shows bd's row. |
| `cache flush` | `cache journal get <seq>` for each seq in the report; `cache status` (journal counts) |
| `cache refresh --scope` | `cache scopes` (`last_ok_at`, `last_error`); `issues list --scope` |
| `cache refresh --id` | `issues get <id>` (`cache.stale` false) |
| `cache evict` | `cache scopes` (`beads`); `issues list --scope` is `SCOPE_UNKNOWN` |
| `cache journal resolve` / `announce` | `cache journal get <seq>`; `cache journal list --unannounced` |
| `cache tick` | `cache status`, `cache scopes`, `cache journal list --unannounced` |

## 8. MCP seam

- **One operation layer.** `request model → operation → data model →
  Envelope` is shared, and both the clap parser and the MCP tool feed the
  same request struct. The MCP server returns sc-command's
  `IntoMcp::into_mcp(envelope)`, which is `structuredContent` = the envelope,
  with `isError` set on failure. There is no reshaping.
- **Tool name** = the command path joined with `_` (`issues_get`,
  `cache_journal_resolve`). It matches `^[a-z_]{1,64}$`.
- **Tool input schema** = the request model. It is the same JSON that
  `--input` takes, and its members are the long flags with `-` replaced by
  `_`. **Tool output schema** = the envelope, with that command's `data`
  schema.
- **Annotations:** reads are `readOnlyHint: true` (a miss fill or base
  refresh changes the cache file only). Journaled writes are
  `idempotentHint: true` (they have keys). `cache evict` is
  `destructiveHint: true`. `cache tick` is not offered over MCP: it sends
  `atm` messages.
- **`bdc schema [<command>]`** prints the tool list `{name, description,
  input_schema, output_schema, annotations}`, generated from the models
  (schemars). The MCP server serves the same list.
- **Tests.** Shared fixtures run on both paths (CLI JSON and MCP structured
  content) and assert identical envelopes. They run against sc-bd-cli's
  stateful fake bd (`testing/fake_bd.rs`) with fault injection (timeouts,
  exit 13, contract break). Live bd runs only on the testbed (NFR, CI
  policy).

## 9. Python binding alignment (`bd_cache_rs`)

Fenix designs against the wheel 0.1.0 dicts, so **every change below is
additive**. The new methods return exactly the CLI `data` and leave the old
verbs as they are. Changing an old verb's dict would be a breaking change for
the wheel. None is proposed.

| Binding today (0.1.0) | Surface | New method (`data` shape, as in the CLI) | How today's dict differs |
|---|---|---|---|
| `get(bead, miss_timeout)` | a | `issues.get(id, miss_timeout_ms=)` | today: `{row: {bead, stale, blocked}, scope_last_ok_at, pending, outage}`; new: row + `cache` |
| (none) | a | `issues.batch_get(ids)` | new |
| `list(phase, status, label)` | a | `issues.list(**v0 params)` | today: `{scope: "bk", last_ok_at, rows: [view]}`, no paging; scope is bare `bk`, not `phase-bk` |
| `ready(phase, fresh_within)` | a | `ready(**params, fresh_within_ms=)` | today: `ready: [view]` + top-level basis; new: `items` + `cache.basis` |
| (none) | a | `blocked(**params)` | new |
| `write(bead, op, args, precondition, idem_key, actor)` | a | `issues.update`, `issues.close`, `dependencies.add` | today: `{seq, duplicate}`, an unknown op is `ValueError`, and `idem_key` is required |
| `refresh(ids, timeout)` | b | `cache.refresh(ids=…)` | today: a bare list of views |
| `sync(phase, timeout)` | b | `cache.refresh(scopes=[…])` | today: one report; new: `results[]` |
| `drain(start_gate_s, hard_s)` | b | `cache.flush(start_gate_ms, deadline_ms)` | `phase` → `scope` in escalated items |
| `prune(phase)` | b | `cache.evict(scope)` | same fields |
| `entry(seq)`, `unannounced_escalations()` | b | `cache.journal.get`, `cache.journal.list(unannounced=True)` | today: `idem_key`, `phase`, `precondition: {status, assignee}`; new: `idempotency_key`, `scope`, `expected_*` |
| `resolve(seq, note, actor)` → `None` | b | `cache.journal.resolve` → `{entry}` | read-back added |
| `mark_announced(seq)` | b | `cache.journal.announce` → `{entry}` | |
| `status()` | b | `cache.status()` | today: raw `meta`, `<phase>:<state>` keys (`None:` for no phase) |
| `open_work()`, `status()["outage"]` | b | `cache.health()` | |
| `sync_alert`, `set_sync_alert` | b | `cache.scopes()` (read, `alert`); the setter stays binding-only | the CLI does not set alerts (the tick does) |
| `CacheError(code, detail, fix, reason, payload, body, exit_code)` | both | adds `kind`, `message`, `details`, `suggested_action`, and `envelope_error` (the §4 object) | additive; `body` stays the BD-CACHE shape |
| `ValueError` for a bad argument | both | the new methods raise `CacheError(BD_CACHE.BAD_ARGS / USAGE)` | old verbs unchanged |

Plan: wheel **0.2.0** adds the namespaced methods (`cache.issues`,
`cache.cache` …; final spelling at implementation). Old verbs are documented
as deprecated, and are removed only in a 1.0 that fenix has agreed to. Fenix
can adopt with the 0.1 verbs and move one call at a time.

## 10. Versioning

- `envelope.version` is sc-command's envelope format (`"1"`). It is not this
  contract's version.
- This contract is semver. `contract_version` is reported by `bdc context`,
  `cache status` and `bdc --version` (JSON). The wheel's minor version tracks
  the contract's minor version.
- **Breaking (major):** removing or renaming a command, tool, flag, field or
  code; changing a field's type or meaning; making an optional field
  required; changing a code's kind or an exit code; changing a default
  (cache path, limit, exclusions).
- **Additive (minor):** a new command or tool, a new optional request
  member, a new `data` field, a new code (clients must default-branch on an
  unknown code, as `/v0` requires), or newly served `/v0` members.
- `/v0` evolution: following an additive `/v0` change is minor. A field
  marked main-only that is dropped before a beads release is removed in a
  major.

## 11. Deferred

- `fresh_within` for `blocked`.
- `journal list` by pending, applied or resolved.
- Every operation in the "not served" table.
- Scope kinds other than `phase-<x>` (label set, assignee): `--scope` takes
  a scope id string, so they are additive.
- Schema packages: ADR-BDC-0008 is Draft. `EXTENSION_INVALID` and a
  `cache.violations` member are reserved.
- The HTTPS MCP transport and auth.

## 12. Open questions for Rand

1. **Reference beads ref.** This contract uses `origin/main` @ `73f64fd`
   (main-only items are marked). Should it pin a beads release tag instead,
   or a branch?
2. **Required `/v0` fields the cache cannot fill:** `revision` and
   `open_children` on write replies, and IssueDetails on get. Leaving them
   out breaks a strict `/v0` client. Is that acceptable, or should those
   operations ask bd and wait?
3. **Idempotency key:** derive one when it is missing (dedupes identical
   retries, but also a deliberate identical `append_notes`), or require it?
4. **Exit 4 for partial success** (`cache refresh` of several scopes), or
   exit 0 with `results[].error` only?
5. **The `cache` member's name:** `cache`, or something with no chance of a
   future `/v0` collision (`x_cache`)?
6. **Default cache path:** confirm the Rust `.bd-cache/` default, with
   atm-bd setting `cache_path` for the cutover.
7. **sc-runtime request (§2):** file it as written?

[main]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml
[rel]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml
[m-ctx]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L362-L385
[m-ready]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L386-L611
[m-blocked]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L695-L785
[m-list]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L1129-L2354
[m-get]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L2982-L3336
[m-close]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L4027-L4195
[m-bget]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L4753-L4847
[m-deps]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L6060-L6168
[m-dadd]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L6463-L6597
[m-upd]: https://github.com/gastownhall/beads/blob/73f64fdfce978a39608f9a3c7340d28354f5c05e/internal/httpapi/spec/openapi.v0.yaml#L12909-L13328
[r-ctx]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L325-L348
[r-ready]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L349-L571
[r-list]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L995-L1579
[r-get]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L2042-L2347
[r-close]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L2938-L3074
[r-deps]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L4667-L4775
[r-dadd]: https://github.com/gastownhall/beads/blob/9b65b9e198acbcc98e7fd3a87a5d55baa065a572/internal/httpapi/spec/openapi.v0.yaml#L5051-L5174
