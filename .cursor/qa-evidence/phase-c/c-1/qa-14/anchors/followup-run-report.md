## Anchor gate: dev · c-1 - runtime-migration

Run `dev-20261106T120416Z-c-1`, model `jev-1.14.0`, threshold 0.6. Counts: asserted_only 1, underspecified 1, satisfied 14, excluded 1.

> **c-1 · ANC-008 · asserted only** Anchor: Each repository must have at most one built-in beads-cache bd sync schedule in sc-cron, the SQLite cron job scheduler. You said (phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement, Oct 25): "I expect 'sc-cron' to be a general tool that can be used for scheduling jobs.  limiting 'builtin' sync to one per repo makes sense." Compliance is asserted but no concrete evidence (paths, diffs, commands, tests) is cited.
>   Pair with the deterministic check: a test shows a second built-in sync row is refused while other rows are allowed

> **c-1 · ANC-004 · underspecified** Anchor: beads-cache must support an optional consumer-registered post-sync hook that receives the bd sync results and errors. You said (phase-b planning session, 2026-10-25 (Rand); wording by bc-lead under Rand's instruction to use judgement, Oct 25): "there can also be hooks necessary to report/escelate based on [the first consumer's] requirements [...] We would also likely want a 'post-sync' hook that could contain sync results and errors and allow consumer [the first consumer] to do custom things at that point (Rand, 2026-10-25, paraphrased; the verbatim text is in git history)." Jev was not confident (min confidence 0.58); the anchor or the work may be underspecified. (leaning: satisfied)
>   Low: compliance `implemented` answered yes at 0.58.
>   Unstable: two identical requests gave underspecified then satisfied.

> **c-1 · ADR-BDC-0009 · excluded** Anchor: The bdc CLI must offer two command surfaces: a beads surface shaped like the bd /v0 interface, and a separate cache-management surface. Not judged: c-1 migrates scheduler dependencies and internals while preserving existing bdc commands and bd-cache-api request vocabulary; it adds, renames or changes no command or request vocabulary covered by this anchor scope.

