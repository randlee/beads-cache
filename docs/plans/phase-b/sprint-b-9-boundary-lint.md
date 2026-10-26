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
  - "docs/baseline/**"
  - "docs/research/**"
  - "docs/requirements.md and docs/architecture.md (provenance wording only)"
  - "README.md"
  - "CHANGELOG.md"
---

# Sprint b-9 — neutral tests, docs and API, and the lint gate (WP-7, WP-8, WP-10)

## Outcome

No consumer concern remains in beads-cache source, tests, scripts or product docs, and the
no-ATM lint runs in CI on the whole tree (ADR-BDC-0010). It merges last.

## Deliverables

1. **Neutral tests** (WP-7; REQ-BDC-0804 amended, REQ-BDC-0812, NFR-BDC-0005). Delete
   `crates/bd-cache-store/tests/compat/`, `tests/compat.rs`,
   `crates/bd-cache-py/tests/atm_core_05831df_*` and the compat jobs in the workflows. A neutral
   0.1 call corpus, generated in-repo against fake-bd, covers every 0.1 verb and reply shape,
   with the before and after verb lists compared in the PR. A format golden pins the v3 DDL
   plus b-7's extensions. `tests/test_additive.py:82-88` is causal (a gate fake bd, no
   wall-clock bound).
2. **Neutral crates and docs** (WP-7; ADR-BDC-0010). Comments, provenance and tests in
   `sc-beads`, `sc-bd-cli` and `sc-cron` (no version bump). `docs/bd-cache-py/` covers no
   runner injection (wrap the binary with `bd_binary`), that a detached `bdc cache flush` must
   log its output, one handle per process, and the binding's own API with no mapping from
   another implementation.
   The provenance wording of `docs/requirements.md` and `docs/architecture.md` (their
   overviews, `Source:` lines and earlier amendment notes, which cite the baseline document
   and the first consumer by name) is restated neutrally, with no change of meaning; the
   requirement and ADR text itself is the planning PR's.
3. **#86** (WP-8; REQ-BDC-0801, NFR-BDC-0004, REQ-BDC-0006). Health (oldest pending age, last
   drain, unresolved escalations) and `refresh scopes --timeout-ms`, with the QA fixes already
   made on #91 (`last_drain_at` from the newest applied entry; the contract §10 test reads
   every name). Delete `docs/bd-cache-cli/cutover-runbook.md` and `docs/baseline/BD-CACHE.md`.
4. **Contract** (WP-8; ADR-BDC-0009). §10 becomes "0.1 → 0.2 API differences", naming no
   consumer: `outage` on Ok, the `issues_list` default exclusions, optional `changed` /
   `already_closed`, one handle per process, install, and `cache_path` for an existing file. It
   adds the scopes, schedule, cron, hooks and ingest verbs, the `bd-cache.run.v1` shapes and
   the new-codes table, under b-8's merged names.
5. **Lint gate** (WP-10; ADR-BDC-0010). The CI step "Strict consumer boundary (ADR-BDC-0010)";
   `python3 scripts/lint_no_atm.py` exits 0 on the whole tree, and any residual fix is listed in
   the PR.

## Acceptance

1. AC-0 on this fence throughout; at the end, the whole-tree lint exits 0 and the CI step is
   green on the three OSes.
2. The format golden matches the v3 DDL plus the extensions; `PRAGMA user_version` stays 3.
3. The neutral corpus covers every 0.1 verb, with no loss against the deleted corpus.
4. The register written in the planning PR matches the merged names; any difference is raised
   with the lead.
5. AC-Q.

## Reference

PR #91 (b-8 attempt: #86, neutral corpus and golden, test_additive, contract §10, lint step;
QA and dev gate passed there), #86 and #96 (fenix's lint draft), all REFERENCE ONLY. Reuse their
commits where they match this doc.

## Does not close

Rand's 0.2.0 release go, and PR #82 (integrate/phase-b → develop).
