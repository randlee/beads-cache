---
id: b-9
phase: b
status: planned
closure_type: integration
target_boundary: repository
track: docs
wave: 5 (code and tests), 7 (contract text and lint gate)
branch: sprint/b-9-boundary-lint
worktree: sprint/b-9-boundary-lint
recommended_agent: rust-developer
recommended_model: sonnet
must_follow: [b-8]
parallel_safe: [b-7]
requirements:
  owns: ["REQ-BDC-0804 (amended)"]
  constrained_by: [REQ-BDC-0801, REQ-BDC-0812, NFR-BDC-0004, NFR-BDC-0005, REQ-BDC-0006]
adrs: [ADR-BDC-0010, ADR-BDC-0009, ADR-STORE-0001]
owned_paths:
  - "crates/bd-cache-store/tests/compat*"
  - "crates/bd-cache-py/tests/** (the existing files)"
  - "crates/sc-*/** except the b-8 runner files (comments, provenance and tests only)"
  - "docs/sc-*/**"
  - "docs/bd-cache-py/**"
  - "scripts/bd-compat/**"
  - ".github/workflows/** (ci.yml included; not the kit's workflows)"
  - "crates/bd-cache-api/**"
  - "crates/bd-cache-cli/src/commands/manage.rs and its tests"
  - "docs/bd-cache-cli/**"
  - "docs/bd-cache-api/**"
  - "scripts/lint_no_atm.py and scripts/test_lint_no_atm.py (the exemption lists and their tests)"
  - "docs/research/**"
  - "docs/requirements.md and docs/architecture.md (provenance wording only)"
  - "README.md"
  - "CHANGELOG.md"
---

# Sprint b-9 — neutral tests, docs and API, and the lint gate (WP-7, WP-8, WP-10)

## Outcome

No consumer concern remains in beads-cache source, tests, scripts or product docs, and the
boundary lint (`scripts/lint_no_atm.py`) runs in CI on the whole tree (ADR-BDC-0010). It merges last.

## Deliverables

1. **Neutral tests** (WP-7; REQ-BDC-0804 amended, REQ-BDC-0812, NFR-BDC-0005). Delete
   `crates/bd-cache-store/tests/compat/`, `tests/compat.rs`,
   the vendored call-log tests in `crates/bd-cache-py/tests/` (the files whose paths the lint
   flags) and the compat jobs in the workflows. A neutral
   0.1 call corpus, generated in-repo against fake-bd, covers every 0.1 verb and reply shape,
   with the before and after verb lists compared in the PR. A format golden pins the v3 DDL
   plus b-7's extensions: the v3 tables and indexes are unchanged, only nullable columns and new
   tables are added, and `PRAGMA user_version` stays 3 (ADR-STORE-0001); b-9 itself makes no schema change. `tests/test_additive.py:82-88` is causal (a gate fake bd, no
   wall-clock bound).
2. **Neutral crates and docs** (WP-7; ADR-BDC-0010). Comments, provenance and tests in
   `sc-beads`, `sc-bd-cli` and `sc-cron` (no version bump). `docs/bd-cache-py/` covers no
   runner injection (wrap the binary with `bd_binary`), that a detached `bdc cache flush` must
   log its output, one handle per process, and the binding's own API with no mapping from
   another implementation.
   The provenance wording of `docs/requirements.md` and `docs/architecture.md` (their
   overviews, `Source:` lines and earlier amendment notes) was restated neutrally by the
   planning PR, with no change of meaning, as was the requirement and ADR text.
3. **#86** (WP-8; REQ-BDC-0801, NFR-BDC-0004, REQ-BDC-0006). Health (oldest pending age, last
   drain, unresolved escalations) and `refresh scopes --timeout-ms`, whose every bd call is
   killed at its timeout capped by the time left before that deadline (NFR-BDC-0004;
   `refresh_scopes_timeout_ms_bounds_the_sync`), with the QA fixes already
   made on #91 (`last_drain_at` from the newest applied entry; the contract §10 test reads
   every name). Delete `docs/bd-cache-cli/cutover-runbook.md`; the baseline design document
   (`docs/baseline/`) was deleted by the planning PR.
   A CLI scenario: with bd unavailable, `issues get` answers from the cache and the outage is
   recorded for the scope (ADR-BDC-0005; the b-3 finding).
4. **Contract** (WP-8; ADR-BDC-0009). §10 becomes "0.1 → 0.2 API differences", naming no
   consumer: `outage` on Ok, the `issues_list` default exclusions, optional `changed` /
   `already_closed`, one handle per process, install, and `cache_path` for an existing file. Every
   item in that list is additive or opt-in: the 0.1 verbs, arguments and returned dicts are unchanged
   for existing calls, and the neutral 0.1 call corpus test is the proof (REQ-BDC-0812). It
   adds the scopes, schedule, cron, sync and ingest verbs, the `bd-cache.run.v1` shapes and
   the new-codes table, under b-8's merged names. The contract keeps its two surfaces, a bd
   /v0-shaped beads surface and the cache-management surface; every new verb joins the
   cache-management surface (ADR-BDC-0009); the Python namespaces map to them, `cache.beads` to
   the beads surface and `cache.manage` to the cache-management surface. Scopes are named as bd
   filters throughout the contract and `bd-cache-api`, with no prefix rule (ANC-009). The text states the behaviour b-8 built, as the
   anchors require: the tick runs at the times in the cache file's sc-cron table, each repository has at most
   one built-in beads-cache bd sync schedule in sc-cron (ANC-002, ANC-008); scopes are bd filters, including `--all` (ANC-009); a consumer's Python sync
   row replaces the built-in row, switched with `bdc cache sync use-python` / `use-builtin`,
   and the built-in row's post-sync hook receives the results and errors (ANC-003, ANC-004); `schedule enable` and
   `disable` flip the row's `enabled` flag, and an sc-cron stop hook returns continue, disable
   or delete (ANC-006, ANC-007).
5. **Lint gate** (WP-10; ADR-BDC-0010). The CI step "Strict consumer boundary (ADR-BDC-0010)";
   `python3 scripts/lint_no_atm.py` exits 0 on the whole tree, and any residual fix is listed in
   the PR. The script's exemptions become only the lint script and its tests and
   `docs/team-protocol.md`: the `docs/plans/` directory exemption (`EXEMPT_DIRS`) and the
   ADR-BDC-0010 section exemption (`EXEMPT_SECTIONS`) are removed, with their tests and
   docstring text (ADR-BDC-0010).
   The lint's prohibited list is the one place the words are written. Every plan, anchor,
   requirement and doc names them only as 'a prohibited word', so the whole tree, including
   `docs/plans/` and the anchor record (`docs/plans/phase-b/anchors/`), passes the lint with no
   exemption beyond the lint itself, its tests and `docs/team-protocol.md`.
   The prohibited list also gains the first consumer's maintainer handle, which today only an
   ad hoc grep checks (ANC-001).
   CI also asserts, in the unit-test jobs, that `bd` and `dolt` are not on PATH (NFR-BDC-0005),
   and that no crate depends on a Dolt or MySQL client (REQ-BDC-0006).

## Acceptance

1. AC-0 on this fence throughout; at the end, the whole-tree lint exits 0 and the CI step is
   green on the three OSes.
2. The format golden matches the v3 DDL plus the extensions; `PRAGMA user_version` stays 3.
3. The neutral corpus covers every 0.1 verb, with no loss against the deleted corpus; pytest passes with `-W error::DeprecationWarning` (REQ-BDC-0812).
4. The register written in the planning PR matches the merged names; any difference is raised
   with the lead.
5. AC-Q.

## Reference

PR #91 (b-8 attempt: #86, neutral corpus and golden, test_additive, contract §10, lint step;
QA and dev gate passed there), #86 and #96 (the first consumer's maintainer's lint draft), all REFERENCE ONLY. Reuse their
commits where they match this doc.

## Does not close

Rand's 0.2.0 release go, and PR #82 (integrate/phase-b → develop).
