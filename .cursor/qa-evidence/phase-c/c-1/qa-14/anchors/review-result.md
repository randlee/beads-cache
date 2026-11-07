## QA-14 anchor sprint review

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

## Failure triage

### ANC-008 — report_defect

The submitted account omitted the concrete at-most-one-row test; the current QA14 backend-free test receipt includes `a_sync_payload_on_another_row_is_refused_and_a_non_sync_row_coexists` and reports 10/10 cron_rows tests passing.

**Evidence correction:** Cite the current `tests/cron_rows.rs::a_sync_payload_on_another_row_is_refused_and_a_non_sync_row_coexists` receipt, which shows the alternate built-in payload is refused while a non-sync row coexists (10/10 `cron_rows` tests passed).

**Required deterministic check:** a test shows a second built-in sync row is refused while other rows are allowed

### ANC-001 — report_defect

The submitted account says only `no ATM concerns`; the exact current-source whole-tree lint receipt is `python3 -I scripts/lint_no_atm.py` exit 0 with output `no ATM concerns (ADR-BDC-0010)`. No per-sprint `--paths` receipt was found.

**Evidence correction:** Name the exact `python3 -I scripts/lint_no_atm.py` command and its `no ATM concerns (ADR-BDC-0010)` output, and state separately whether a per-sprint `--paths` check ran; the approved deterministic-lint exception does not waive this anchor.

**Required deterministic check:** See the locked anchor check named in the to_fix record; this verdict is underspecified, so no deterministic_checks entry is emitted for it.

### NFR-BDC-0004 — report_defect; unstable Jev answer

The original account omitted concrete timeout test names. Current QA14 has job_threading 2/2 and cron_rows 10/10; QA13 actual output on the previous SHA names a slow-bd budget test and a hanging-hook deadline-kill test. All 262 Rust files are byte-identical through 60f8920; current CI is 15/15 success. The slow-bd test asserts <12 seconds for a 2-second budget, so it is not a precise 2-second measurement or a literal hang.

**Evidence correction:** Cite the actual slow-bd budget, hanging-hook deadline-kill, current `job_threading` and `cron_rows` receipts, identifying each commit and result; do not present the slow-bd test as a literal hang or a precision timing claim.

**Required deterministic check:** See the locked anchor check named in the to_fix record; this verdict is underspecified, so no deterministic_checks entry is emitted for it.

### NFR-BDC-0005 — report_defect/evidence gap

The original item stated the rule but did not cite the guarded run. The initial host PATH probe found bd and dolt; the parent-authorized process-local guard then printed `no bd or dolt on PATH`, and the same selected sync-test command passed 60/60. Current hosted CI is 15/15 success, with its backend-free path check preceding the full workspace tests.

**Evidence correction:** Cite the process-local no-backend PATH guard followed by the same selected test run (60/60 passed), and name the fake-bd fixtures demonstrated; do not generalize that evidence to untested suites or claim a universal per-test negative mapping.

**Required deterministic check:** See the locked anchor check named in the to_fix record; this verdict is underspecified, so no deterministic_checks entry is emitted for it.

No source defect, anchor-scope defect, or independently oracle-proven question defect is established. The NFR-BDC-0004 result is unstable (first satisfied, then underspecified); do not tune or waive from that signal.
