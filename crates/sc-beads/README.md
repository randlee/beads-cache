# sc-beads

The beads backend contract shared by beads-cache, beads-task-issue-tracker
(btit) and later backends.

- btit's `BeadsBackend`, `CliBackend`, `DoltOperations` and `CloseSuggestions`
  traits, its data types and `BeadsError`, unchanged from
  randlee/beads-task-issue-tracker at `8d8e82ae9f85`.
- `CacheBackend: BeadsBackend`: the deadline-bounded calls a cache needs
  (fingerprint, batched id lists, scope lists, `bd blocked` / `bd ready`,
  guarded writes), returning `BeadsCacheError`.
- Strict rows (`Bead`, `BlockedRow`) generated from `bd schema` (bd 1.3.1).
- `Clock`, `SystemClock` and `Deadline`.
- With the `testing` feature: `ManualClock`, `FakeBackend` and the
  `CacheBackend` contract suite.

Implementations live in `sc-bd-cli` (the `bd` CLI) and later `sc-bd-http`.
