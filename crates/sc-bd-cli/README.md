# sc-bd-cli

The `bd` CLI implementation of the [`sc-beads`](../sc-beads) backend traits.

- `BdCli`: btit's `BeadsBackend`, `CliBackend` and `DoltOperations` over the
  `bd` binary (ported from `btit-cli` and `btit-bd` at `8d8e82ae9f85`), and the
  cache's `CacheBackend` (`fingerprint`, batched id lists, scope lists, `bd blocked` /
  `bd ready`, guarded writes), each call stopped at the `Deadline` it is handed.
- `CliRunner` / `CliInvoker`: the process runner, the extended `PATH`, the
  per-project lock and the `--version` probe.
- With the `test-support` feature: `RecordingInvoker` and `FakeBd`, test doubles
  that need no `bd` binary.

Exit 13 (a refused `--if-status` / `--if-assignee` guard) is a typed
`Refusal::Precondition`; a timeout, a bd that cannot start and an unreachable
database server are `Unavailable`.
