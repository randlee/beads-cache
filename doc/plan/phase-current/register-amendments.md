# Register amendments for the phase-B plan update

These are inputs to the phase-B plan update. Each one is applied to `docs/requirements.md` by the
sprint that owns the change, with Rand's rulings of 2026-10-25 as the authority.

## REQ-BDC-0804 (drop-in compatibility)

- **Today:** the store file is shared with the consumer's own implementation, and a vendored compat
  suite in beads-cache proves that compatibility.
- **Conflict:** under Rand's strict-boundary ruling, compatibility with a consumer's
  implementation is the consumer's own test, run against the published wheel.
- **Amend to:** the store file stays readable and writable by any v3 reader. This is the format
  guarantee already anchored as ADR-STORE-0001. The vendored compat suite is removed.

## REQ-BDC-0701..0707 (the tick)

- **Today:** these port the consumer's cron: an ATM team and lead, phase files, messages,
  `refused` / `pass-through` replies, and a stop rule.
- **Amend each:**

| REQ | Amended to | Anchor |
|---|---|---|
| 0701 | At most one built-in sync schedule row per repository; sc-cron holds any other rows. | ANC-008 |
| 0702 | A scope is a bd filter (labels, status, type, assignee or `--all`), never a phase. | ANC-009 |
| 0703 | Kept. The budget is the schedule row's timeout. | REQ-BDC-0703 |
| 0704–0706 | The cache records sync failure state and escalations. A registered post-sync hook receives the results and errors; beads-cache sends nothing. | ANC-004, ANC-001 |
| 0707 | Removed. Pausing is the row's `enabled` flag, and a stop hook returns continue, disable or delete. | ANC-006, ANC-007 |
| new | The built-in sync runs from the SQLite cron table, and a consumer can replace it with a registered Python sync that feeds the cache through `ingest_scope`. | ANC-002, ANC-003 |
