---
id: b-9
phase: b
status: complete
closure_type: integration
target_boundary: repository
track: docs
wave: 7 (merges last)
branch: sprint/b-9-boundary-lint
worktree: sprint/b-9-boundary-lint
recommended_agent: rust-developer
recommended_model: sonnet
must_follow: [b-8]
parallel_safe: []
requirements:
  owns:
    - REQ-BDC-0732
    - "REQ-STORE-0006 (amended)"
    - "REQ-PY-0009 (superseded; its test and vendored log removed)"
  constrained_by:
    - "REQ-BDC-0001 (a cached read starts no bd: the corpus case)"
    - REQ-BDC-0006
    - "REQ-BDC-0105, REQ-BDC-0201, REQ-BDC-0301 (cited by the restated comments and research docs)"
    - REQ-BDC-0720
    - REQ-BDC-0721
    - REQ-BDC-0722
    - REQ-BDC-0724
    - REQ-BDC-0725
    - REQ-BDC-0727
    - REQ-BDC-0729
    - REQ-BDC-0731
    - REQ-BDC-0801
    - REQ-BDC-0803
    - REQ-BDC-0804
    - REQ-BDC-0807
    - REQ-BDC-0808
    - REQ-BDC-0810
    - REQ-BDC-0811
    - REQ-BDC-0812
    - "REQ-BDC-0909 (on-demand scopes: contract D7)"
    - NFR-BDC-0004
    - NFR-BDC-0005
    - "REQ-BEADS-0001..0005, REQ-BDCLI-0001..0009, REQ-CRON-0001..0018 (comments, provenance and tests only)"
    - "REQ-API-0001..0008 (health, scope selection: REQ-API-0004 as amended, the refresh timeouts, the typed reply members), REQ-CLI-0001..0012 (health through bd_cache_sync::execute_api; tests)"
    - "REQ-PY-0001..0014"
    - "REQ-STORE-0005 (evidence), REQ-MODEL-0003, REQ-MODEL-0004 (the port's last_applied_at, the contract suite, FakeStore)"
    - "REQ-STORE-0004 (SqliteStore::last_applied_at under the store's rules: one read snapshot)"
    - "REQ-BDC-0912 (bounded fresh ready: the typed read_fallback cause)"
adrs:
  - "ADR-BDC-0001..0004 (the contract's and the research docs' subjects)"
  - ADR-BDC-0005
  - ADR-BDC-0006
  - ADR-BDC-0007
  - ADR-BDC-0009
  - ADR-BDC-0010
  - ADR-BDC-0011
  - ADR-BDC-0012
  - ADR-BDC-0013
  - "ADR-BDC-0014 (health reads the schedule table through it)"
  - "ADR-BDC-0015 (and its Amendment 1, sprint b-9)"
  - "ADR-BDC-0012 Amendment 2 (sprint b-9)"
  - "ADR-SYNC-0008 (deliverable 4; health's sync facts)"
  - "ADR-MODEL-0001..0004 (ADR-MODEL-0003: scope rules, the v3 phase column)"
  - "ADR-CORE-0001, 0002, 0101..0103, 0201..0206 (ADR-CORE-0203 and its sprint b-9 amendment: the bounded fresh ready's typed cause; ADR-CORE-0204: read shapes)"
  - "ADR-SYNC-0001, 0002, 0006, 0008 (bd-cache-sync's health facts; 0003..0005 and 0007 are Superseded)"
  - "ADR-STORE-0001..0006 (ADR-STORE-0006 Amendment 2, sprint b-9)"
  - "ADR-BEADS-0001..0004"
  - "ADR-BDCLI-0001..0006"
  - "ADR-CRON-0001..0009"
  - "ADR-API-0001, 0002"
  - "ADR-CLI-0001..0005"
  - "ADR-PY-0001..0006"
adrs_superseded: "ADR-API-0003 is superseded (b-3); it does not bind this sprint"
owned_paths:
  - "crates/bd-cache-store/tests/compat* (removal)"
  - "every existing file of crates/bd-cache-py/tests/ (the two vendored call-log files and test_mapping_doc.py removed), the new crates/bd-cache-py/tests/corpus/** and crates/bd-cache-py/tests/test_corpus.py"
  - "crates/sc-beads/**, crates/sc-bd-cli/**, crates/sc-cron/** (comments, provenance, tests and fixtures only; no version bump)"
  - "crates/bd-cache-api/src/ops/manage.rs and crates/bd-cache-cli/src/commands/manage.rs with their tests (health)"
  - "the new crates/bd-cache-cli/tests/outage_scenario.rs"
  - "docs/bd-cache-cli/contract.md, docs/bd-cache-cli/cutover-runbook.md (deleted if present), docs/bd-cache-py/**, docs/research/**"
  - "scripts/bd-compat/**, scripts/lint_no_atm.py, scripts/test_lint_no_atm.py"
  - ".github/workflows/{ci.yml,python.yml,bd-compat.yml} (not the kit's workflows)"
  - "README.md"
  - "CHANGELOG.md"
  # Fence amendment, fix round 1 (lead ruling 7; B9Q1-001, B9Q1-008): the health read path and
  # every other file the sprint and its fix round edit, each with its reason.
  - "the health read path and its seam (REQ-BDC-0732, ADR-BDC-0015 Amendment 1, ADR-STORE-0006 Amendment 2): crates/bd-cache-api/src/{data/*,decode.rs,error.rs,filter.rs,lib.rs,ops/*,request/manage.rs} with tests/{common/mod.rs,manage.rs,fixtures/issues_list_filters.json}; crates/bd-cache-cli/src/commands/mod.rs with tests/{fixtures/cli_commands.json,scenarios.rs,process.rs,sync_process.rs}; crates/bd-cache-model/src/{compat.rs,config.rs,error.rs,lib.rs,op.rs,reply.rs,scope.rs,state.rs,store.rs,testing/contract.rs,testing/store.rs} with tests/{goldens.rs,fixtures/PROVENANCE.md}; crates/bd-cache-store/src/{lib.rs,schema.rs,store.rs} with tests/store.rs; crates/bd-cache/src/{lib.rs,ready.rs} with tests/read_ready.rs; crates/bd-cache-sync/src/{health.rs,hook.rs,lib.rs,payload.rs,python_job.rs,schedule.rs} with tests/{common/repo.rs,health.rs,schedule_switch.rs}; crates/bd-cache-py/src/{api.rs,compose.rs,handle.rs}"
  - "crates/sc-bd-cli/src/locks.rs (the private LockKey, B9Q1-038, ADR-BDCLI-0006; no version bump), crates/sc-bd-cli/src/{argv.rs,testing/fake_bd.rs} (comments only, B9Q1-018)"
  - "scripts/check_no_backend.py and scripts/test_check_no_backend.py (B9Q1-030); scripts/test_prerelease_tag.py (one Windows skip, so the scripts tests run on the three OSes, B9Q1-011)"
  - "Cargo.toml (the unused dunce entry removed, B9Q1-048)"
  - "doc/plan/phase-current/ (removed, B9Q1-012) and .beads/issues.jsonl (its 13 pointers to that record repointed to docs/plans/phase-b/anchors/anchors.json)"
  - ".claude/skills/checking-beads-compatibility/{SKILL.md,references/pin-locations.md} (B9Q1-049)"
  - "register edits the lead approved (rulings 2, 8, 9, 11, 14; listed below): docs/architecture.md, docs/requirements.md, docs/bd-cache-api/{architecture,requirements}.md, docs/bd-cache-cli/architecture.md, docs/bd-cache-store/{architecture,requirements}.md, docs/bd-cache-sync/{architecture,requirements}.md, docs/sc-cron/architecture.md (the b-8 QA-2 group-kill note), docs/sc-bd-cli/architecture.md"
  - "docs/plans/phase-a/{phase-a-plan,sprint-a-4-store,sprint-a-7-sync-integration,sprint-a-8-python-bindings}.md (the consumer-repository commit hash removed, ruling 11, no other change); docs/plans/phase-b/{phase-b-plan,sprint-b-8-tick-python,phase-b-boundary}.md and docs/project-plan.md (status rows: b-8 merged, b-9 in review; this fence amendment)"
---

# Sprint b-9 — neutral tests, docs and API text, health, and the lint gate (WP-7, WP-8, WP-10)

## Outcome

No consumer concern remains in beads-cache source, tests, scripts or product docs, and the
boundary lint (`scripts/lint_no_atm.py`) runs in CI on the whole tree with only the exemptions
ADR-BDC-0012 leaves (ADR-BDC-0010 as amended). The contract states what b-8 built, written from
the approved register. It merges last.

## Deliverables

1. **Neutral tests** (WP-7; REQ-STORE-0006 amended, ADR-STORE-0002 as amended by ADR-STORE-0006,
   REQ-PY-0009 superseded, REQ-BDC-0812, NFR-BDC-0005). Delete
   `crates/bd-cache-store/tests/compat/`, `tests/compat.rs`, the vendored call-log tests in
   `crates/bd-cache-py/tests/` (the two files whose paths the lint flags) and
   `test_mapping_doc.py`, and the compat jobs in the workflows. A neutral 0.1 call corpus,
   generated in-repo against fake-bd (`crates/bd-cache-py/tests/corpus/`, driven by
   `test_corpus.py`), covers every 0.1 verb and reply shape, with the before and after verb
   lists compared in the PR. It includes the planted negative of the 0.1 bare-name rule: a 0.1
   verb with a bare name against a legacy-prefixed scope, run against the corpus, fails if the
   rule is dropped (REQ-BDC-0812 as amended). `tests/test_additive.py:82-88` is causal (a gate
   fake bd, no wall-clock bound). This sprint verifies b-7's format golden and makes no schema
   change.
2. **Neutral crates and docs** (WP-7; ADR-BDC-0010, ADR-PY-0006). Comments, provenance, tests and
   fixtures in `sc-beads`, `sc-bd-cli` and `sc-cron` (no version bump), including the
   maintainer handle in `crates/sc-beads/tests/fixtures/bd-1.3.1/`. `docs/bd-cache-py/` covers no
   runner injection (wrap the binary with `bd_binary`), that a detached `bdc cache flush` must log
   its output, one handle per process, and the binding's own API; its mapping from another
   implementation is removed with its test (ADR-PY-0006). The register itself was written by the
   planning PR; this sprint edits no REQ or ADR text.
3. **#86 and health** (WP-8; REQ-BDC-0732, REQ-BDC-0801, NFR-BDC-0004, REQ-BDC-0006). Health
   (oldest pending age, last drain, unresolved escalations) and `refresh scopes --timeout-ms`,
   whose every bd call is killed at its timeout capped by the time left before that deadline
   (NFR-BDC-0004; `refresh_scopes_timeout_ms_bounds_the_sync`), with the QA fixes already made on
   #91 (`last_drain_at` from the newest applied entry; the contract §10 test reads every name).
   Health adds the sync mode, the enabled row's last run and the degraded states of
   REQ-BDC-0732. Delete `docs/bd-cache-cli/cutover-runbook.md`; PR #97 deleted the baseline design document
   (`docs/baseline/`) in applying A8's no-consumer-wording ruling, awaiting Rand's
   confirmation (phase-b-plan.md, Open items). A CLI scenario
   (`crates/bd-cache-cli/tests/outage_scenario.rs`): with bd unavailable, `issues get` answers
   from the cache and the outage is recorded for the scope (ADR-BDC-0005; the b-3 finding).
4. **Contract** (WP-8; ADR-BDC-0009, ADR-BDC-0013, ADR-BDC-0015, REQ-BDC-0811). The contract text
   is written from the approved register (REQ-BDC-0720..0732, ADR-BDC-0011..0015, ADR-SYNC-0008),
   with no change of meaning; a difference between the register and b-8's code is raised with
   the lead, never settled here.
   - §2: the legacy-directory refusal and `bdc_legacy_cache_file` are removed (REQ-BDC-0810 as
     amended); the `[[scope]]` tables.
   - §4: the error-table rows of `INGEST_INVALID`, `SCHEDULE_INVALID`, `TICK_RUN_FAILED` and
     `TICK_IN_FLIGHT` (one row each, as REQ-BDC-0729 gives them) are b-7's and the removal of the
     `TICK_<REFUSAL>` row is b-8's, each with `error_table.rs` (phase-b-plan.md, Shared files);
     this sprint checks those rows against the register and adds `details.outage`.
   - §6: the scopes, schedule, cron, sync and ingest verbs; `cache scopes` as `scopes list`'s
     alias; `cache tick` without `refused[]` and the consumer's flag; the `bd-cache.run.v1`
     `post_sync` request and the Python sync row's payload; scopes named as bd filters
     throughout, with no prefix rule except the 0.1 compatibility rule (ANC-009).
   - §10 becomes "0.1 → 0.2 API differences", naming no consumer: `outage` on Ok, the
     `issues_list` default exclusions, optional `changed` / `already_closed`, one handle per
     process, install, `cache_path` for an existing file, and the 0.1 bare-name rule. Every item
     is additive or opt-in for existing 0.1 calls; the neutral corpus is the proof (REQ-BDC-0812).
   - §11: the classification below; §12: the scope-kind deferral is removed (scopes are filters
     now).
   - The text states the behaviour b-8 built, as the anchors require: the sync runs at the times
     in the cache file's sc-cron table, each repository has at most one built-in sync row
     (ANC-002, ANC-008); scopes are bd filters, including `--all` (ANC-009); a consumer's Python
     sync row replaces the built-in row, switched with `bdc cache sync use-python` /
     `use-builtin`, and the built-in row's post-sync hook receives the results and errors
     (ANC-003, ANC-004); `schedule enable` and `disable` flip the row's `enabled` flag, and an
     sc-cron stop hook returns continue, disable or delete (ANC-006, ANC-007). The contract keeps
     its two surfaces; every new verb joins the cache-management surface (ADR-BDC-0009), and the
     Python namespaces map to them, `cache.beads` to the beads surface and `cache.manage` to the
     cache-management surface.
   - **Versioning (§11, REQ-BDC-0811).** Checked on 2026-10-26: no stable release exists; the
     only GitHub release is the prerelease `prerelease/v0.1.0`, built from `develop` at `577cca9`
     before `bd-cache-api` and the contract existed, carrying wheel 0.1.0 assets; PyPI and
     TestPyPI have no `bd-cache-rs` project; `CONTRACT_VERSION` 1.0.0 exists only on
     `integrate/phase-b`. So contract 1.0.0 is unreleased, and its removed items (`cache tick`'s
     consumer flag, the `TICK_<REFUSAL>` codes and `TickReply.refused[]`, the phase-shaped scope
     ids, the §12 deferral, the legacy-file rule) are pre-release edits, not §11 breaking
     changes: `CONTRACT_VERSION` stays 1.0.0 and the wheel stays 0.2.0. The prerelease wheel 0.1.0
     has none of them, so its verbs are untouched. The plan marks this version choice as a
     decision for Rand (phase-b-plan.md, Open items), with this recommendation.
5. **Lint gate** (WP-10; ADR-BDC-0010 as amended by ADR-BDC-0012). The CI step "Strict consumer
   boundary (ADR-BDC-0010)" in the `ci` lint job; `python3 scripts/lint_no_atm.py` exits 0 on the
   whole tree, and any residual fix is listed in the PR. The script's exemptions become only the
   lint script and its tests and `docs/team-protocol.md`: `EXEMPT_DIRS` (`docs/plans/`) and
   `EXEMPT_SECTIONS` (ADR-BDC-0010's section) are removed, with their tests and docstring text.
   The prohibited list, the one place the words are written, gains the first consumer's
   maintainer handle as a word rule with hit and non-hit tests (ANC-001). The `scope-prefix` rule
   is narrowed for the one `LEGACY_SCOPE_PREFIX` declaration, with a hit test for any other
   literal and a non-hit test for that declaration (ADR-BDC-0012). Every plan, anchor,
   requirement and doc names the words only as "a prohibited word", so the whole tree, including
   `docs/plans/` and the anchor record, passes. CI also asserts, in the unit-test jobs, that `bd`
   and `dolt` are not on PATH (NFR-BDC-0005), and that no crate depends on a Dolt or MySQL client
   (REQ-BDC-0006). This sprint added no supply-chain gate. The gate was added at phase end
   (phase-end review finding PE1-RSH-001): `cargo deny check advisories` in the `ci` lint job,
   installed by the kit's `setup-lint-toolchain` action, with a minimal committed `deny.toml`
   (advisories only).

## Acceptance

1. AC-0 on this fence throughout; at the end, the whole-tree lint exits 0 with the narrowed
   exemptions, and the CI step is green on the three OSes. Planted negatives: a prohibited word
   in a file under `docs/plans/` fails the lint; the maintainer handle in a fixture fails it; a second literal of the legacy prefix outside the compatibility constant fails it.
2. b-7's format golden still matches the v3 DDL plus the extensions; `PRAGMA user_version` is 3.
3. The neutral corpus covers every 0.1 verb, with no loss against the deleted corpus, and its
   bare-name planted negative fails when the rule is dropped; pytest passes with
   `-W error::DeprecationWarning` (REQ-BDC-0812).
4. Health: degraded with `no_sync_row_enabled` when both rows are disabled, `sync_failing` when
   the Python row fails three runs in a row, `sync_stale` when no worker runs for three periods
   (injected clock), and `hook_failing` after three hook failures; each with a planted negative.
5. The outage scenario passes through the process.
6. The contract matches the register; every difference found is raised with the lead and
   recorded in the PR.
7. AC-Q.

## Fix round 1 (QA-1, PR #101)

The 50 findings B9Q1-001..050 (merged review on PR #101) are fixed under the lead's rulings,
except B9Q1-020 (a dated supply-chain waiver) and B9Q1-047 (pinning the CI actions and tools),
which the lead dropped: deliverable 5 above already records the supply-chain deferral.

**Lead-approved annotations (ruling 8, B9Q1-004).** These pointers inside Approved ADR text
change no decision; each points at the amendment that supersedes the sentence it follows:

- ADR-SYNC-0008 (`docs/bd-cache-sync/architecture.md`): lines 623-624 (the harness's `Cache`
  call, superseded by the implementation notes), 637-639 (the `bd-cache-cli` → `sc-cron` edge,
  superseded by "One surface"), 641-642 (the signal handling, superseded by "Shutdown") and
  669-671 (the root `Cargo.toml` entries, superseded by B8Q1-036).
- ADR-PY-0001 (`docs/bd-cache-py/architecture.md`): lines 67-69 (the "only static" sentence,
  superseded by the QA-1 amendment of ADR-PY-0006).
- ADR-PY-0002 (`docs/bd-cache-py/architecture.md`): lines 90-91 (the Mapping table removed by
  ADR-PY-0006); and this round's line 105 (the `config` kind, amended below it).
- ADR-PY-0004 (`docs/bd-cache-py/architecture.md`): lines 198-200 (the mapping test, call log
  and script, superseded by ADR-PY-0006).

**The 0.1 verb lists, before and after (deliverable 1, B9Q1-007).** The comparison is the
docstring of `crates/bd-cache-py/tests/corpus/record.py` ("Before and after"): the sixteen 0.1
verbs plus `path` and `closed`, the same set before and after this sprint, each with a
recorded successful case; every name the deleted call log listed mapped onto one of them.

**Residual lint fixes (deliverable 5).** With the scope inverted, the whole-tree run found only
the stale anchor record `doc/plan/phase-current/` (removed). `CHANGELOG.md` stays out of scope
(ADR-BDC-0010) and its five lines that named the consumer's messaging command or its file are
reworded all the same. The three `src` files of the original commit carry one doc-comment line
each, restated neutrally, and no code: `crates/sc-bd-cli/src/locks.rs:18`,
`crates/sc-bd-cli/src/path.rs:14`, `crates/sc-beads/src/row.rs:15`. This round adds code to
`locks.rs` only for B9Q1-038 (the private `LockKey`).

**Register versus code (acceptance 6).** Two differences were raised with the lead, and both
are ruled and written into the contract as normative text: the 0.2 reads selected a scope from
a legacy-prefixed label (ruling 1: removed; a missing scope is `SCOPE_UNKNOWN`, contract §5.1,
REQ-API-0004 amended), and a cache file with no sync row was not `no_sync_row_enabled`
(ruling 2: REQ-BDC-0732 read literally, contract §6). The `CONTRACT_VERSION` and wheel choice
stays in phase-b-plan.md, Open items.

**Register edits this round (rulings 9, 11, 14).** ADR-BDC-0012 Amendment 2 and ADR-BDC-0015
Amendment 1 (`docs/architecture.md`); ADR-STORE-0006 Amendment 2 and the `compat.rs` pointers
of ADR-STORE-0001's Amendment b-1 and ADR-STORE-0002's consequences
(`docs/bd-cache-store/architecture.md`); the evidence of REQ-STORE-0005 and REQ-STORE-0006
(`docs/bd-cache-store/requirements.md`); REQ-PY-0001 (the `config` kind removed, `errno`),
REQ-PY-0003, REQ-PY-0009 and REQ-PY-0013 evidence (`docs/bd-cache-py/requirements.md`);
ADR-PY-0002's amendment and ADR-PY-0004's neutral packaging bullets
(`docs/bd-cache-py/architecture.md`); REQ-API-0004 (`docs/bd-cache-api/requirements.md`); the
`bdc_legacy_cache_file` clause of `docs/bd-cache-cli/architecture.md` (now a pointer to
ADR-BDC-0015); and the consumer-repository commit hash, removed from every product doc and
plan with no other change.

## Reference

PR #91 (b-8 attempt: #86, neutral corpus and golden, test_additive, contract §10, lint step; QA
and dev gate passed there), #86 and #96 (the lint draft), all REFERENCE ONLY. Reuse their commits
where they match this doc.

## Does not close

Rand's 0.2.0 release go, and PR #82 (integrate/phase-b → develop).
