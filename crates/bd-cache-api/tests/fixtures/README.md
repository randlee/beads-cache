# Shared fixtures (sprint b-2)

Each `*.json` file is one scenario over `FakeBackend` and `FakeStore`. They
run here (`tests/fixtures.rs`), b-3 replays them through
`bd_cache_cli::run`, and b-4 through Python. A file is:

```json
{
  "description": "what it proves, with the requirement",
  "world": {
    "beads": [{"id": "b1", "status": "in_progress"}],
    "edges": [["s1", "b1", "blocks"]],
    "sync": ["x"]
  },
  "steps": [
    {"name": "…", "command": "issues update", "request": {}, "expect": {"ok": true, "data": {}}}
  ]
}
```

- **World.** The clock starts at `2026-10-23T10:00:00Z` and does not move.
  The configured scopes are `[x]`. Each bead is `sc_beads::testing::bead(id)`
  (open, priority 2, type task, created and updated at
  `2026-10-23T09:00:00Z`) with the given members merged over it, and labels
  `["phase-x"]` unless it gives `labels`. `edges` are `[from, to, type]`.
  `sync` lists the scopes synced, in order, before the first step.
- **Facts.** `context` and `cache status` see `cache_path
  "/repo/.bd-cache/beads.bd-cache.sqlite3"`, `store_schema_version 3`,
  `database "beads"`, `project_id "proj-1"`, `beads_dir "/repo/.beads"`,
  `repo_root "/repo"`.
- **Steps** run in order on one cache. `command` is the command path,
  `request` the JSON request.
- **Expect** is matched against the envelope's `{ok, data}` or
  `{ok, error}`: an object matches when each of its keys matches (other
  keys are free), an array when it has the same length and each element
  matches, `"$any"` matches any present value, `"$absent"` requires the
  key to be missing, anything else by equality.
