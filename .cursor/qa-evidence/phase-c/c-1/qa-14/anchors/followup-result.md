# QA-14 development gate result — FAIL

Actual run: `dev-20261106T120416Z-c-1` on exact source `60f89200fe337d57ede24ecaa55e36171a1c4305`.

- Threshold: 0.6; request cap: 64,000 bytes.
- Complete item: 57,757 characters; no truncation or shortened requests; no Jev errors.
- Full result set: 14 satisfied, 1 asserted-only, 1 underspecified, 1 excluded. Applicable failures: `ANC-008`, `ANC-004`. `ANC-004` is unstable (confirmation changed the answer; minimum confidence 0.58).
- Exclusion: `ADR-BDC-0009`, unchanged configured exclusion.
- Gate verdict: **FAIL**. The corrected evidence resolves the prior `ANC-001`, `NFR-BDC-0004`, and `NFR-BDC-0005` findings; it does not justify a pass or a question change.

## Per-anchor result fence

| Anchor | Verdict | Detail |
|---|---|---|
| ANC-008 | asserted_only | Compliance is asserted but no concrete evidence (paths, diffs, commands, tests) is cited. |
| ANC-004 | underspecified | Jev was not confident (min confidence 0.58); the anchor or the work may be underspecified. Unstable after confirmation. |
| ADR-BDC-0005 | satisfied | The directive was followed and evidenced. |
| ADR-STORE-0001 | satisfied | The directive was followed and evidenced. |
| ANC-001 | satisfied | The directive was followed and evidenced. |
| ANC-002 | satisfied | The directive was followed and evidenced. |
| ANC-003 | satisfied | The directive was followed and evidenced. |
| ANC-009 | satisfied | The directive was followed and evidenced. |
| NFR-BDC-0004 | satisfied | The directive was followed and evidenced. |
| NFR-BDC-0005 | satisfied | The directive was followed and evidenced. |
| REQ-BDC-0001 | satisfied | The directive was followed and evidenced. |
| REQ-BDC-0006 | satisfied | The directive was followed and evidenced. |
| REQ-BDC-0007 | satisfied | The directive was followed and evidenced. |
| REQ-BDC-0008 | satisfied | The directive was followed and evidenced. |
| REQ-BDC-0502 | satisfied | The directive was followed and evidenced. |
| REQ-BDC-0812 | satisfied | The directive was followed and evidenced. |
| ADR-BDC-0009 | excluded | Excluded from this work by the anchor's exclude list: c-1 migrates scheduler dependencies and internals while preserving existing bdc commands and bd-cache-api request vocabulary; it adds, renames or changes no command or request vocabulary covered by this anchor scope. |

## Remaining triage

- **ANC-008 — asserted-only.** The corrected item already cites `tests/cron_rows.rs::a_sync_payload_on_another_row_is_refused_and_a_non_sync_row_coexists` and 10/10 passing. The test source directly asserts the built-in row returns `ok`, an alternate-name built-in payload is refused, and a plain non-sync row remains active/enabled. Jev still returned “no evidence.” Preserve the failure; the result alone does not prove a question defect or product defect. If another evidence pass is authorized, state those direct assertions plainly.
- **ANC-004 — underspecified and unstable.** The sprint states the preservation obligation, but the account lacks a direct delivery-test citation. The current paired check receipt shows `crates/bd-cache-py/tests/test_run.py::test_the_hook_gets_every_result_marks_some_and_the_rest_is_offered_again` in Python hook suite 7/7: successful scope results, failed scope error/failing_since, marking and reoffer. Keep the live failure/instability. A later report can cite this actual receipt; do not tune the question.

## Deterministic checks

The receipt records **14 pass, 2 pending**. `REQ-BDC-0001` remains a real verification gap: no current executed test explicitly reads cache during an active sync/hook and asserts no new `bd` process. Preserve this obligation for the next development loop; do not waive it.

`REQ-BDC-0007` has a stale configured check for ingest error mapping. Its approved statement remains unchanged. Proposed `deterministic_check` text only:

> `sc-beads/tests/strict_rows.rs::a_bad_row_is_a_contract_error_naming_bead_and_field` rejects an unknown bd-row key as `BD_CACHE.BD_CONTRACT` naming the bead and field; `bd-cache/tests/ingest.rs::one_invalid_row_among_valid_ones_stores_none_of_them` rejects an ingest row with an unknown key as `INGEST_INVALID` (`row_invalid`, index 1) and stores no rows.

This preserves strict bd schema validation in approved REQ-BDC-0007 and the more specific ingest behavior in approved REQ-BDC-0726/0729. The proposal changes only the deterministic check; the current configured mismatch is still pending and is not counted as a pass.

Full run report: `followup-run-report.md`. Full per-anchor results, copied from the actual run with triage additions, are in `followup-result.json`. Deterministic receipt: `deterministic-checks.json`.
