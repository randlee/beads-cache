# Research: syncing with DoltHub without a local dolt server

Date: 2026-10-23. Sources: beads upstream (`internal/doltserver`,
`docs/architecture/dolt.md`, federation.md), DoltHub docs (linked below),
`bd` 1.3.0 and `dolt` 2.3.5 `--help` output, and the beads research docs.
No database or remote was created or contacted, and no latency was measured.

## Summary

- **bd can work against a remote Dolt with no local server, by two routes it
  already supports.** Neither needs the DoltHub SQL API.
  - **Hosted Dolt over the MySQL wire protocol.** bd points straight at a
    remote sql-server (TLS plus password). No local clone, no local server.
  - **bd's default embedded mode plus `bd sync` / `bd dolt push|pull` to a
    `dolthub://` remote.** A local clone, but no server.
- **The DoltHub SQL API (HTTPS) can read, but writing through it is a dead
  end.** Each write is an async branch/commit/merge, and none of bd's code
  runs, so guards, the events journal and `is_blocked` repair are skipped.
  bd has no HTTP-API backend; an upstream search found nothing.
- **Phase 1 keeps bd as the only write path** behind a backend port. A
  read-only SQL adapter can be an optional later addition.
- **The local research docs are out of date.** `dolt.md:49,100-107` (written
  for v1.0.3) says embedded mode was removed. bd 1.3.0 and the upstream docs
  say embedded is the default again.

## 1. "No local server" options

**(a) DoltHub SQL API.** v2:

- Reads: `GET/POST /api/v2/databases/{owner}/{db}/sql?ref=&q=&limit=`.
  Default limit 1000 rows; timeout 30 s, capped at 60 s.
- Writes: `POST .../sql-writes {from_branch,to_branch,q}` returns 202 and an
  operation, polled at `GET /api/v2/operations/{id}`. The write runs on
  `to_branch` and then merges.
- Auth: `Authorization: Bearer dh_...`, required for private databases.
- No documented rate limits.
- Sources: https://www.dolthub.com/docs/products/dolthub/api/v2/database,
  https://www.dolthub.com/docs/products/dolthub/api/v2/authentication, and
  the v1alpha1 page https://www.dolthub.com/docs/products/dolthub/api/sql.
- bd cannot use it. Estimated (not measured) cost: about 100-300 ms per read,
  seconds per write because of the polling.

**(b) Hosted Dolt (MySQL protocol).** Works with bd today.

- Set `BEADS_DOLT_SERVER_HOST`/`PORT`/`USER`, `BEADS_DOLT_PASSWORD` and
  `BEADS_DOLT_SERVER_TLS=1`. A non-localhost host forces bd into server mode
  (`bd dolt --help`; upstream `internal/doltserver/servermode.go:51-57,133-145`;
  `docs/architecture/dolt.md` Server Mode table).
- `bd dolt push` documents `DOLT_REMOTE_USER`/`DOLT_REMOTE_PASSWORD` "for
  Hosted Dolt".
- Ordinary sql-server multi-writer concurrency: one consistent store for every
  agent. Upstream federation.md (~L222, 242) counts all clients of one Hosted
  Dolt server as "one replica".
- The cheapest instance is $50/month (t2.medium, 50 GB), with read replicas,
  nightly backups and branch permissions
  (https://www.dolthub.com/docs/products/hosted/notable-features.md).
- Each bd call costs one network round trip plus process start. bd-cache's
  measured ~170 ms per `bd list --id` (the baseline design) grows by about the RTT per
  query.

**(c) bd pointed at a remote.**

- Remote sql-server: yes, as in (b).
- DoltHub URL as a live backend: no. `dolthub://` is only a push/pull remote.
- Embedded mode plus a remote: yes. Data lives in `.beads/embeddeddolt/`,
  single writer with a file lock (upstream dolt.md "Embedded Mode", "Lock
  Contention").
- `bd federation` requires CGO. The Homebrew 1.3.0 binary is built without
  CGO (`bd federation --help`), but `bd sync` and `bd dolt push/pull` are
  available.

**(d) Local clone plus push/pull, no server.** This is the embedded route in
(c). `bd sync` does the work: pull, positive conflict detection, `is_blocked`
recompute, then push with bounded retry. Exit codes: 2 conflict, 3 push race,
4 stuck working set (`bd sync --help`).

- Eventually consistent, bounded by the sync interval.
- The events journal is per replica, so after a pull bd-cache must
  re-baseline (`bd events --help`).
- Leases need `node_id` set per store (federation.md ~L205-245).

**(e) bd serve in front of a remote.** `bd serve` is a loopback HTTP API over
whatever backend the workspace is configured for (`bd serve --help`). Pairing
it with (b) is valid and saves a process fork per call. There is also the
experimental `--proxied-server-external-host/-tls` option, a local proxy in
front of an external sql-server (`bd init --help`). That is a local proxy
process, not a server.

## 2. Reads via SQL

- Tables: `issues` (about 50 columns, including `updated_at`, `status`,
  `assignee`, `metadata`), `dependencies` (primary key
  `(issue_id, depends_on_id)`, plus `type`) and `labels`
  (`database-schema-architecture.md:45-160,174-186`).
- Cost: this breaks the baseline design's goal 4 ("never reads dolt tables") and loses
  the `bd schema` row contract, which describes `--json` output, not tables.
  bd-cache would have to track bd's migrations itself, including wisp routing
  and the denormalized `is_blocked`.
- Incremental sync: use `HASHOF('main')` as a cursor, then
  `dolt_commit_diff_issues` / `dolt_commit_diff_dependencies` between the
  stored hash and HEAD, or `DOLT_DIFF()` when the schema changed. `dolt_log`
  gives `commit_hash` and `commit_order`
  (https://www.dolthub.com/docs/sql-reference/version-control/dolt-system-tables).
  Commit-hash cursors catch deletes and edge removals, and an unchanged hash
  means nothing changed.
- Why that matters: the current `--updated-after` cursor probably misses
  deletes, and edge changes that don't bump `updated_at`. Not verified.
- Caveat: uncommitted working-set writes are invisible to `ref=` reads, and
  bd's `--dolt-auto-commit batch` leaves writes uncommitted.
- bd-neutral alternative: `bd diff <from> <to>` takes commit hashes and may
  give a hash-cursor sync without leaving bd. Worth a spike.

## 3. Writes

DoltHub SQL API writes run on `to_branch` and then merge, one commit or merge
per statement. They bypass bd entirely:

- no `bd_events_journal` rows (even raw `bd sql` DML "is NOT journaled",
  `bd events --help`)
- no `is_blocked` recompute
- no lease/claim arbitration, which bd does inside SQL (`bd serve --help`)
- no `content_hash` or wisp routing

Concurrency with agents writing through bd elsewhere:

- (b): they share one server, and the API can't reach Hosted Dolt's live
  working set anyway.
- (d): the DoltHub branch only changes by push. An API write to `main` would
  arrive in every clone's next `bd sync` as plain data (cell-level merge,
  last write wins on issue cells) with no events.

Conclusion: write only through bd.

## 4. What is realistic for a phase-lead team

1. Local or shared sql-server: what runs today. Lowest latency.
2. Hosted Dolt: the most realistic "no local server" option. bd supports it,
   one consistent store, $50+/month. Needs a latency check on the drain's two
   calls per entry, already near the 30 s cron limit.
3. Embedded plus a DoltHub remote on a `bd sync` timer: free, no server. Costs
   sync lag, lease/node_id complexity and journal re-baselining.
4. DoltHub SQL API: a read-only side channel at most.

## 5. Recommendation

A backend port with:

- `version()`, `schema()`
- `list(filter, cursor)` returning rows plus a new cursor
- `get_many(ids)`
- `apply(op)` returning the read-back row
- optional `sync_remote()`, wrapping `bd sync` and mapping exit codes 0-4

Make the cursor opaque (timestamp or commit hash), so a hash-based adapter
needs no schema change. Store `backend_kind`, `endpoint` and `replica_id` in
`sync_state`. Phase 1 builds the `BdCliAdapter`, which covers local, Hosted
Dolt and embedded setups unchanged through bd's environment configuration.

Spike later:

- `BdServeAdapter` (HTTP, no forks; see `bd-serve-http.md`)
- Hosted Dolt latency
- `bd diff` / `HASHOF` cursors
- an optional read-only `DoltHubSqlReadAdapter`, gated on schema version, for
  cold full syncs

Dead ends:

- writes through the DoltHub SQL API
- a DoltHub URL as a live backend
- `bd federation` on non-CGO builds
