The private diagnostic protocol and classification control are pushed on `sprint/c-1-runtime-migration` at `8c1aefafd7fd269c82c7974e610e848816d026d8`. The worktree is clean and frozen. Ordinary merge `c88c543` consumed exact integration `b731c09e`, retaining owning histories and candidate-scoped closures.

Separate bounded main and worker checkpoint files live inside the existing owned scratch directory, independent of captured stderr. The actual worker return and SQLite code are recorded before result or stall output. Parent receive times use the original owner Instant captured before spawn; child and setup phases use one body clock. Receipt time is an observation bound, not an exact execution timestamp. Wall-clock correlation is diagnostic only. The original 30 ms cancellation, 500 ms observation, five-second child lifetime and seven-second supervisor assertion remain.

The planted busy call returns `DatabaseBusy`, then parks before the required marker. Normal negative qualification still fails. The outer regression passes only after verifying that failure, accurate classification and actual root cleanup. The post-push direct negative command exited **101** in 5.025715 seconds, classified `worker_returned_required_marker_missing`, with a successful root cleanup receipt before driver fallback. No fallback was used.

Final local evidence:

- Affected store and protocol controls: **8/8 passed**, 15.05 seconds.
- Default helper ownership health cases: **2/2 passed**, 5.00 seconds.
- Observer error and panic: actual kill/wait receipts true. Incomplete cleanup evidence: root waited, cleanup receipt false, repeated cleanup refused, all before driver fallback.
- Strict affected Clippy, default library Clippy, shared-helper compatibility Clippy, workspace formatting and explicit included-module formatting passed. Initial lint failures remain retained and were corrected without suppressions.

Source equality is qualified precisely: 158 other `src` Rust files are byte-identical. The whole `schedule.rs` file changed in its test module; its production prefix remains identical (SHA256 `49438f00b7310fb4d385355dbc887ab3c353264eed25e05df9f824e5872da05a`). Private fixture helpers changed; the reviewed session fixture remains unchanged. Production policy, public interfaces, dependencies and guards are preserved.

Exact-source Windows [job 126430893018](https://github.com/randlee/beads-cache/actions/runs/41786865484/job/126430893018) is **in_progress**. Native Darwin proof does not establish terminal Windows, Linux or overall CI acceptance.

All five findings remain **OPEN**: REQ-QA-004, ARCH-001, RBQA-F001, QA2-001 and RQA9-001. The historical RQA9 cause remains unknown and unlocalized. This control is not a historical-cause correction or waiver. Independent QA10 and the sprint, anchor, CI and merge gates remain required.

Cargo is **RELEASED**, with zero active owned commands. Stopped for independent QA10. Exact commands, logs and hashes, binary binding, structured checkpoints, cleanup evidence and platform limits are in `phase-protocol-result.json`.
