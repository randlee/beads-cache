## Anchor gate: dev · c-1 - runtime-migration

Run `dev-20261106T113750Z-c-1`, model `jev-1.14.0`, threshold 0.6. Counts: asserted_only 1, underspecified 3, satisfied 12, excluded 1.

> **c-1 · ANC-008 · asserted only** Anchor: Each repository must have at most one built-in beads-cache bd sync schedule in sc-cron, the SQLite cron job scheduler. You said (phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement, Oct 25): "I expect 'sc-cron' to be a general tool that can be used for scheduling jobs.  limiting 'builtin' sync to one per repo makes sense." Compliance is asserted but no concrete evidence (paths, diffs, commands, tests) is cited.
>   Pair with the deterministic check: a test shows a second built-in sync row is refused while other rows are allowed

> **c-1 · ANC-001 · underspecified** Anchor: Words on the prohibited list of the repository's no-consumer lint (`scripts/lint_no_atm.py`) must not appear in this repository outside that lint, its tests and `docs/team-protocol.md`; the lint passes on the whole tree. You said (phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement, Oct 25): "blocking [a prohibited word: the consumer's cron principal] and the other words on the lint's prohibited list from source or tests (Rand, 2026-10-25, paraphrased; the verbatim text is in git history)." Jev was not confident (min confidence 0.13); the anchor or the work may be underspecified. (leaning: satisfied)
>   Low: relevance `relevant` answered yes at 0.50.
>   Low: compliance `implemented` answered yes at 0.13.
>   Low: conflict `violated` answered no at 0.56.

> **c-1 · NFR-BDC-0004 · underspecified** Anchor: Each bd call, Python sync method and post-sync hook started by beads-cache must be killed at its timeout, capped by the time left before the run's deadline. Jev was not confident (min confidence 0.53); the anchor or the work may be underspecified. (leaning: satisfied)
>   Low: compliance `implemented` answered yes at 0.53 (second ask).
>   Unstable: two identical requests gave satisfied then underspecified.

> **c-1 · NFR-BDC-0005 · underspecified** Anchor: Unit tests for beads-cache must run with no bd and no Dolt, using the deterministic fake bd. Jev was not confident (min confidence 0.46); the anchor or the work may be underspecified. (leaning: satisfied)
>   Low: compliance `implemented` answered yes at 0.46.

> **c-1 · ADR-BDC-0009 · excluded** Anchor: The bdc CLI must offer two command surfaces: a beads surface shaped like the bd /v0 interface, and a separate cache-management surface. Not judged: c-1 migrates scheduler dependencies and internals while preserving existing bdc commands and bd-cache-api request vocabulary; it adds, renames or changes no command or request vocabulary covered by this anchor scope.

