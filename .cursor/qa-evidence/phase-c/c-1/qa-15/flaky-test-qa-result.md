```json
{
  "success": true,
  "data": {
    "status": "pass",
    "role_owned_verdict": "PASS",
    "owner": "flaky-test-qa",
    "qa_pass": "qa-15",
    "review_mode": "full_sprint_review",
    "round_limit": false,
    "reviewed_branch": "sprint/c-1-runtime-migration",
    "reviewed_commit": "c4006f8d218b743aeabeddcfac9bb5c1c0af8907",
    "reviewed_at_utc": "2026-11-06T23:53:39.210977+00:00",
    "model_disclosure": "Explicit assigned gpt-6.1-sol high fallback; configured Composer/Grok unavailable. No unavailable model execution claimed.",
    "findings": [],
    "new_role_findings": [],
    "summary": {
      "total_findings": 0,
      "critical": 0,
      "important": 0,
      "minor": 0,
      "active_role_owned_findings": 0,
      "new_flaky_findings": 0
    },
    "review_targets": [
      ".github/workflows/ci.yml",
      ".github/workflows/python.yml",
      ".triage/c/findings/ARCH-001.ttl",
      ".triage/c/findings/ARCH13-001.ttl",
      ".triage/c/findings/CI-C2-RUNNER-001.ttl",
      ".triage/c/findings/FTQ-009.ttl",
      ".triage/c/findings/FTQ-010.ttl",
      ".triage/c/findings/QA2-001.ttl",
      ".triage/c/findings/RBQA-F001.ttl",
      ".triage/c/findings/REQ-QA-004.ttl",
      ".triage/c/findings/RQA10-001.ttl",
      ".triage/c/findings/RQA11-001.ttl",
      ".triage/c/findings/RQA9-001.ttl",
      "Cargo.lock",
      "Cargo.toml",
      "crates/bd-cache-cli/Cargo.toml",
      "crates/bd-cache-store/Cargo.toml",
      "crates/bd-cache-store/src/codec.rs",
      "crates/bd-cache-store/src/health.rs",
      "crates/bd-cache-store/src/lib.rs",
      "crates/bd-cache-store/src/schedule.rs",
      "crates/bd-cache-store/src/schedule_budget.rs",
      "crates/bd-cache-store/src/schema.rs",
      "crates/bd-cache-store/src/store.rs",
      "crates/bd-cache-store/tests/schedule_store.rs",
      "crates/bd-cache-store/tests/support/fixture_child.rs",
      "crates/bd-cache-store/tests/support/schedule_protocol.rs",
      "crates/bd-cache-sync/Cargo.toml",
      "crates/bd-cache-sync/src/compose.rs",
      "crates/bd-cache-sync/src/control.rs",
      "crates/bd-cache-sync/src/cron.rs",
      "crates/bd-cache-sync/src/cron_log.rs",
      "crates/bd-cache-sync/src/history.rs",
      "crates/bd-cache-sync/src/job.rs",
      "crates/bd-cache-sync/src/launch.rs",
      "crates/bd-cache-sync/src/lib.rs",
      "crates/bd-cache-sync/src/mode_lock.rs",
      "crates/bd-cache-sync/src/module_hook.rs",
      "crates/bd-cache-sync/src/payload.rs",
      "crates/bd-cache-sync/src/python_job.rs",
      "crates/bd-cache-sync/src/run.rs",
      "crates/bd-cache-sync/src/schedule.rs",
      "crates/bd-cache-sync/tests/check_consumer_graph.py",
      "crates/bd-cache-sync/tests/common/mod.rs",
      "crates/bd-cache-sync/tests/common/repo.rs",
      "crates/bd-cache-sync/tests/common/session_fixture.rs",
      "crates/bd-cache-sync/tests/cron_hurry.rs",
      "crates/bd-cache-sync/tests/cron_rows.rs",
      "crates/bd-cache-sync/tests/e2e.rs",
      "crates/bd-cache-sync/tests/hook_runner.rs",
      "crates/bd-cache-sync/tests/job_threading.rs",
      "crates/bd-cache-sync/tests/perf.rs",
      "crates/bd-cache-sync/tests/schedule_switch.rs",
      "crates/bd-cache-sync/tests/test_consumer_graph.py",
      "crates/sc-cron/Cargo.toml",
      "crates/sc-cron/LICENSE",
      "crates/sc-cron/README.md",
      "crates/sc-cron/clippy.toml",
      "crates/sc-cron/src/clock.rs",
      "crates/sc-cron/src/error.rs",
      "crates/sc-cron/src/expr.rs",
      "crates/sc-cron/src/hook.rs",
      "crates/sc-cron/src/jitter.rs",
      "crates/sc-cron/src/job.rs",
      "crates/sc-cron/src/lib.rs",
      "crates/sc-cron/src/limits.rs",
      "crates/sc-cron/src/process.rs",
      "crates/sc-cron/src/scheduler.rs",
      "crates/sc-cron/src/sqlite.rs",
      "crates/sc-cron/src/store.rs",
      "crates/sc-cron/src/timestamp.rs",
      "crates/sc-cron/src/types.rs",
      "crates/sc-cron/src/worker.rs",
      "crates/sc-cron/tests/common/mod.rs",
      "crates/sc-cron/tests/enabled.rs",
      "crates/sc-cron/tests/expr.rs",
      "crates/sc-cron/tests/process_api.rs",
      "crates/sc-cron/tests/python_hook.rs",
      "crates/sc-cron/tests/reload_poll.rs",
      "crates/sc-cron/tests/run_due_abort.rs",
      "crates/sc-cron/tests/sqlite_store.rs",
      "crates/sc-cron/tests/stop_hook.rs",
      "crates/sc-cron/tests/target_resolver.rs",
      "crates/sc-cron/tests/two_process.rs",
      "crates/sc-cron/tests/worker.rs",
      "docs/architecture.md",
      "docs/bd-cache-api/requirements.md",
      "docs/bd-cache-cli/architecture.md",
      "docs/bd-cache-cli/requirements.md",
      "docs/bd-cache-py/architecture.md",
      "docs/bd-cache-store/architecture.md",
      "docs/bd-cache-sync/architecture.md",
      "docs/plans/phase-c/anchors/anchors.json",
      "docs/plans/phase-c/anchors/anchors.md",
      "docs/plans/phase-c/phase-c-plan.md",
      "docs/plans/phase-c/sprint-c-1-runtime-migration.md",
      "docs/requirements.md",
      "docs/sc-cron/architecture.md",
      "docs/sc-cron/requirements.md"
    ],
    "review_assessments": {
      "scope": {
        "mode": "full assignment scope, round_limit=false",
        "requested_targets": 99,
        "present_targets": 68,
        "intentional_deleted_local_sc_cron_targets": 31,
        "all_current_Rust_files": 262,
        "unchanged_Rust_since_QA14": 258,
        "changed_Rust_since_QA14": [
          "crates/bd-cache-store/src/health.rs",
          "crates/bd-cache-sync/tests/common/repo.rs",
          "crates/bd-cache-sync/tests/cron_hurry.rs",
          "crates/bd-cache-sync/tests/schedule_switch.rs"
        ],
        "all_Rust_equal_0c": true,
        "workflows_equal_QA14": true,
        "method": "Inventoried every requested target, current risk scan, reviewed every changed test/helper/guard context and current production-health prefix, checked retained ownership/deadline/code sources against QA14 exact review. Read full actual developer JSON/MD, QA14 manager FAIL38/40, Windows original and qualification evidence; retained prior full target coverage instead of pretending deleted in-tree scheduler sources still execute."
      },
      "cached_read_causality": {
        "status": "PASS",
        "source": "crates/bd-cache-sync/tests/cron_hurry.rs:1067-1232",
        "same_production_route": "Prewarm and concurrent read use decode(Command::IssuesGet)->execute_api through the same Repo.cache/ScheduleTable, real Cache/SqliteStore/FlockDrainLock/BdCli composition. Expected cached id/title and cold id are asserted. No rawSQL, second cache, mock read answer or generic missing-bead error.",
        "real_active_job": "Actual SystemSchedules::system constructs the migrated scheduler and cron_run_hurried. PyStub managed_hang writes actual run.pid and owned-child receipt before readiness, then execs sleep3600; it cannot ordinarily finish before supervisor cancellation. Readiness timeout plus actual PID existence and nonfinished run handle are required before/after the read; cold falsifier must also pass liveness before reaching its required extra-lookup rejection.",
        "real_no_backend_probe": "Repo.bd_invoker is a FakeBd clone sharing Arc<Mutex<State>> with the actual Switched/BdCli invoker; FakeBd records actual argv calls. Existing backend-free architecture replaces physical host bd/dolt intentionally. Positive supplied current log1->1 retains only prewarm list --id cached; cold supplied current log1->2 adds list --id cold and the identical concurrent_read_guard returns additional BdCli lookup request. Physical bd spawning is not inferred.",
        "read_deadline": "Read runs in an owned spawn_blocking handle. The existing500ms observation completes before stop/hurry, so releasing the job cannot turn a blocked read into a claimed concurrent success. Snapshot after read occurs while job still active. No fixed sleep decides success."
      },
      "cached_read_failure_ownership": {
        "status": "PASS",
        "whole_owner": "Existing SessionFixture/FixtureChild establishes original5s pre-spawn lifetime and temp/output ownership. Exactly filtered child enters setsid before runtime/store/stub/process creation. Existing one-attempt session cleanup quiesces complete kernel membership, kills descendants, actually waits the root, and verifies final census; pre-registration descendants are owned.",
        "join_paths": "Stop/hurry precedes result assertions; run is actually joined and observed PID must be absent. Read timeout retains reader handle and awaits owned completion rather than silently dropping it. Error/panic/runtime-drop/blocking-thread paths remain inside the physically supervised5s child; SessionFixture owns cleanup on every exit. Inner helper30s await cap cannot extend the external5s owner. Successful receipt is emitted only after actual joins/reap and data assertions; outer requires successful child and causal receipt.",
        "falsifiability": "Cold backend-present/cache-absent read returns correct data through identical route while the same real job stays active, but the same guard rejects its actual additional argv. No assertfalse, fabricated liveness, negative-only missing key, retry, added cap renewal, timeout widening, thread detachment or new production instrumentation."
      },
      "health_constructor": {
        "status": "justified_test_scope_narrowing_no_introduced_finding",
        "source": "crates/bd-cache-store/src/health.rs:331-334; store.rs:925-968",
        "preservation": "Only cfg(test) store constructor switches open_at to existing open_shutdown_fixture. Real physical SQLite connection, canonical file registration, complete initialize/schema/extensions/backfill/version/identity, same shared connection/mutex, busy timeout and checked WAL remain. Only crash-durability synchronization is unnecessary for these cancellation/read-only fixtures; normal production open path and consumer durability remain unchanged.",
        "budgets": "No thread count, fault placement, read/answer/deadline/assertion change. Health remains50ms operation/200ms answer/150ms elapsed and existing5s supervisor. Health clock starts in MutexChild.finish after spawn returns; it is not falsely described as schedule FixtureChild pre-spawn clock. Positive/stalled reader real-holder channels and bounded parent kill/wait remain; schedule30ms cancellation/500ms observation/pre-spawn5s owner unchanged.",
        "causal_limit": "This is a justified fixture-purpose correction, not a proved causal repair of all three original Windows failures. SQL negative already used the same nondurable constructor when it failed. No exact fsync, Defender, CPU, host, scheduler, signal or production cancellation cause is inferred. Subsequent actual candidate-associated native qualification closes the missing execution receipt, not the unknown historical cause."
      },
      "historical_windows_failure": {
        "status": "preserved_not_rewritten_or_claimed_repaired_by_one_pass",
        "failed_candidate": "68f64e594728279c6c3da21d175521a362322d8d",
        "raw": "/tmp/phase-c-execution/c-1/qa-15/initial-windows-failure.log",
        "raw_sha256": "c0efe7ac531b8eaa980e131fb5b07d7146ab6d2482487830068c533e858b8f7c",
        "observation": "Actual store34passed3failed: health positive exceeded owner; health stalled reader lacked required failure marker, both with test-name-prefix-only output; SQL stalled negative lacked completed worker-return/stall qualification. Separate mutex/busy schedule negatives supplied real-return/stall receipts and actual cleanup. Health phase remains unlocalized; missing cancellation receipt does not prove cancellation never installed.",
        "known_SQL_timeline": "Structured SQL setup-complete3.4457985s, wait-result(ok=true)4.5981187s; owner ultimately killed/reaped and reported5.2215925s. These are execution timestamps, not receive timestamps; no later worker return observed. Exact operation between checkpoints/diagnostic writes is not proven.",
        "limits": "No qualification claimed for original failed run, no arbitrary timer/parallelism adjustment, no retry-to-green diagnosis, no universal OS progress promise. Actual pass is useful current qualification but does not prove a permanent repair of every historical intermittent mechanism."
      },
      "native_windows_qualification": {
        "status": "PASS_receipt_verification",
        "candidate_associated": "0c5893a5a9fa6496ba8d84e7190fab16ebe3eb09",
        "actual_checkout": "d7b5056943928ff11a9adc057eae452dc1ea2e62 PR merge of0c intof1e0756",
        "job": "127901365448",
        "run": "42220594503",
        "receipt_validation": "Independent SHA256 of raw corrected-windows-pass.log matches receipt;11 exact line/value receipts checked:4 checkout lines,6 named health/schedule positive/stalled outcome lines,1 store37/37 suite result.",
        "intended_negative_semantics": "Health stalled named test requires timeout, actual reap and contended-read failure marker; schedule stalled-cleanup supervisor unconditionally loops all three STORE_FIXTURES and protocol::qualify requires real call return/error/cancellation/timeout/join-start/stall receipt/root cleanup. Their actual pass establishes intended controls by these unchanged contracts. Successful capture has no inner RQA9_PROTOCOL stream; no fabricated individual timing/result-code evidence.",
        "current_transfer_limit": "All262 Rust files atc400 equal0c; all49 registry SDK Rust match earlier C5. Current c400 check snapshot separately15SUCCESS, including native Windows. This is source-equivalence credit plus current check evidence; no claim that supplied0c raw was captured from c400 or checkout exactly0c."
      },
      "guard_and_registry": {
        "status": "PASS",
        "fresh_independent_execution": "python3 -I -m unittest discover -s crates/bd-cache-sync/tests -p test_consumer_graph.py -v; exit0,8/8 in0.054s; no Cargo invoked.",
        "source_scope": "Exact published registry0.2.0 replaces old registry0.1.1/Git tuple. Guard rejects retired Git tuple, other revisions/URLs/paths/versions, split nominal identity/direct normal-build introducers, SDK features including server/client, assemblies and multiple SQLite providers. Existing root normal/build traversal/dev exclusion, temporary independent roots and owned120s metadata child timeout unchanged.",
        "test_nondeterminism": "Each table case constructs fresh metadata; no cross-case mutable fixture, retry, extra sleeps or widened timeout. Existing real Python sleep30s timeout negative is deliberately killed/reaped at0.05s, not synchronization padding.",
        "independent_local_source_proof": {
          "changed_lock_records": [
            [
              "sc-command",
              "0.2.0"
            ],
            [
              "sc-cron",
              "0.2.0"
            ]
          ],
          "registry_SDK_source_equivalence": [
            {
              "package": "sc-cron",
              "registry_root": "/Users/randlee/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sc-cron-0.2.0",
              "archive": "/Users/randlee/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f/sc-cron-0.2.0.crate",
              "prior_source": "/Users/randlee/.cargo/git/checkouts/sc-runtime-9bd6cb97de8223e7/c3ce3d1/crates/sc-cron",
              "Rust_count": 41,
              "Rust_differences": [],
              "archive_sha256": "997daaabbc7a27862b14281bdc051cfa74fd230b605e16885fee7e29cdb1c17a",
              "lock_checksum": "997daaabbc7a27862b14281bdc051cfa74fd230b605e16885fee7e29cdb1c17a",
              "checksum_matches": true
            },
            {
              "package": "sc-command",
              "registry_root": "/Users/randlee/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/sc-command-0.2.0",
              "archive": "/Users/randlee/.cargo/registry/cache/index.crates.io-1949cf8c6b5b557f/sc-command-0.2.0.crate",
              "prior_source": "/Users/randlee/.cargo/git/checkouts/sc-runtime-9bd6cb97de8223e7/c3ce3d1/crates/sc-command",
              "Rust_count": 8,
              "Rust_differences": [],
              "archive_sha256": "45a55653c6fd1e98d842c2b99fd9cc59aecb94b058647df393566f5c4ea0584f",
              "lock_checksum": "45a55653c6fd1e98d842c2b99fd9cc59aecb94b058647df393566f5c4ea0584f",
              "checksum_matches": true
            }
          ],
          "structural_guard": "Current published-only guard+tests independently read; source predicate narrows to exact registry0.2.0; introducer/features/identities/SQLite traversal remain unchanged.",
          "production_health_prefix_equal": true
        }
      },
      "retained_full_scope": {
        "status": "preserved",
        "evidence": "Current fixture_child, schedule_protocol, session_fixture, schedule/store production logic, schedule_store observers, remaining cron_rows/e2e/hook_runner/job_threading/perf/schedule_switch behavior and workflows equal QA14 except the four explicitly reviewed test changes. Retained channel/gate/notify/injected-clock readiness, per-test temp isolation, child-scoped env, owned poll/observer joins, whole-fixture stalled/unwind/invalid-receipt controls remain. No new fixed shared path/global env race/order assumption, unbounded tested-worker wait escaping the owned child, masking retry or timeout widening identified. No impossible healthy-kernel progress finding revived."
      },
      "plan_and_metadata_preservation": {
        "status": "PASS_preservation_only",
        "evidence": "Current plan49000bytes retains byte-identical QA14 frontmatter101 requirement/NFR bindings, ADR/anchor bindings, all8AC rows, required validation commands/platforms and QA11 no-standing-ledger instruction. Full prior audited reduction proof preserved. Only approved exact published baseline/current proof and scoped owner-state prose changes plus new registry-baseline paragraph; no lowered budgets or deleted fixture obligation. Recursive anchor JSON diff changes only REQ0007 deterministic_check/version/history length; statements, quotations, questions, scopes, exclusions/fixtures and other16 records unchanged. New check names real strict-row BD_CACHE.BD_CONTRACT and separate atomic ingest INGEST_INVALID tests per approvedREQ0726/0729. Metadata consistency is not a judge/gate PASS."
      }
    },
    "finding_dispositions": [
      {
        "id": "ARCH-001",
        "disposition": "resolved_preserved",
        "evidence": "Actual QA14 owner/enforcer result retains this resolution; current exact registry0.2.0 source-only policy narrows away the former Git exception. Original unauthorized direct sync dependency remains absent and genuine public receiver/structural restrictions remain. No fictional pending-owner carry."
      },
      {
        "id": "QA2-001",
        "disposition": "resolved_preserved",
        "evidence": "Actual QA14 owner/enforcer result retains this resolution; current exact registry0.2.0 source-only policy narrows away the former Git exception. Original unauthorized direct sync dependency remains absent and genuine public receiver/structural restrictions remain. No fictional pending-owner carry."
      },
      {
        "id": "ARCH13-001",
        "disposition": "resolved_preserved",
        "evidence": "Actual QA14 owner/enforcer result retains this resolution; current exact registry0.2.0 source-only policy narrows away the former Git exception. Original unauthorized direct sync dependency remains absent and genuine public receiver/structural restrictions remain. No fictional pending-owner carry."
      },
      {
        "id": "RBQA-F001",
        "disposition": "resolved_preserved",
        "evidence": "Actual QA14 owner/enforcer result retains this resolution; current exact registry0.2.0 source-only policy narrows away the former Git exception. Original unauthorized direct sync dependency remains absent and genuine public receiver/structural restrictions remain. No fictional pending-owner carry."
      },
      {
        "id": "REQ-QA-004",
        "disposition": "concurrent_read_subgap_verified_parent_global_acceptance_pending",
        "owner": "req-qa / independent anchor gate / parent enforcer",
        "evidence": "New public same-cache concurrent cached-read control and real cold-read falsifier verified from current source and actual c400 native logs. Current c400 hosted snapshot15SUCCESS. Full v3 development-anchor gate/paired deterministic-check acceptance and final closeout remain parent-owned and pending at this review snapshot. No sprint completion denominator, anchor waiver or self-closure assigned."
      }
    ],
    "preserved_dispositions": [
      {
        "id": "RQA10-001",
        "status": "resolved_preserved_verified_through_exact_C1_candidate",
        "owner": "flaky-test-qa",
        "reviewed_commit": "da94641038f6745366a240c308b211c796d4cbb6",
        "owning_adjudication_complete": true,
        "canonical_disposition": "RESOLVED for the supported mixed recovery/artificial-delay fixture composition; sole manager aggregates this actual owner disposition. No pending-owning-reviewer condition remains.",
        "supported_mechanism": "Actual instrumented consumer observation7fb/run41796715401/job126462431901 reached quiescence call3 afterSTOP/80ms/CONT, timed out its actual inventory under original250ms window, reaped command/root, completed final drain and correctly retained false cleanup. Positive recovery mixed transient-error recovery and additional artificially delayed inventory in one two-stable-census window. Current split removes that extra positive timing assumption without relaxing deadlines or census proof.",
        "why_not_pass_only": "Direct source diff changes only two existing private test bodies and independently verified old/new ledger retains every intended proof. Independent current native full19 evidence supplies real scan/STOP/CONT/wait/error/receipt/census outcomes, including required negative controls. This satisfies the supported correction direction; no identical historical recurrence or low-level host cause is required.",
        "scope_limits": "Only exact C1 candidate fixture-composition mechanism. This does not identify all historical uninstrumented stages or detailed ps/signal/observer costs, guarantee kernel scheduling, qualify wholeC2, or claim C2 consumed this helper. Original failure logs and distinct RQA9/CI/SDK findings remain.",
        "further_code_or_unchanged_observation_required_for_this_mechanism": false,
        "qa12_preservation": "Relevant recovery body, ordinary inventory helper and workflows unchanged versus accepted d824; independent affected full suites and current supported-runner CI pass. Historical/scope limits preserved; no C2 or global acceptance.",
        "qa13_preservation": "Preserve prior scoped owner disposition and original failure limits. Relevant fixture/source/workflow bytes independently match QA12 except the one schedule test assertion adopting typed source inspection; current developer store37/37 and full cron_hurry19/19 logs support continued fixture qualification. No new evidence reopens this mechanism; no whole-C2 or current hosted matrix claim.",
        "qa13_reviewed_commit": "478812a83adb04ea38c93a6d4bc83a17233a9cf4",
        "qa14_reviewed_commit": "60f89200fe337d57ede24ecaa55e36171a1c4305",
        "qa14_preservation": "Preserved within the original supported mechanism and candidate scope. All 262 current tracked Rust files, both graph guard sources and both workflows are independently byte-identical to QA13. Prior native raw 37/37 store and 19/19 cron_hurry, meaningful failure controls, and actual current 15/15 hosted checks corroborate preservation. Historical failures remain historical; no new timing cause or whole-C2 acceptance inferred.",
        "qa15_reviewed_commit": "c4006f8d218b743aeabeddcfac9bb5c1c0af8907",
        "qa15_preservation": "Original accepted mechanism and historical limits preserved. Current fixture_child, schedule_protocol, session_fixture, schedule/store production logic and workflows equal QA14. Health helper changes only cfg(test) constructor; new causal cached-read cases reuse existing kernel-session owner. All 262 Rust files equal0c; all49 registry SDK Rust files equal C5. Actual current c400 CI15SUCCESS and independently checked0c-associated native Windows intended-case receipts corroborate current qualification; no permanent all-historical-failure repair or whole-C2 approval inferred."
      },
      {
        "id": "RQA9-001",
        "severity": "important",
        "status": "resolved_verified_for_exact_C1_candidate",
        "owner": "flaky-test-qa",
        "reviewed_commit": "da94641038f6745366a240c308b211c796d4cbb6",
        "owning_adjudication_complete": true,
        "canonical_disposition": "RESOLVED for supported disposable-fixture setup versus original absolute lifetime mechanism. No pending-owner condition remains.",
        "correction": "Actual cfg(test) open_shutdown_fixture sets synchronous=OFF only on disposable real connection before SAME initialize/schema/extensions/backfill/identity/claim/WAL. Public read-only mutex, external BEGIN IMMEDIATE busy and executing SQL paths remain. Production open_at and all production behavior are byte-equivalent; helper also covers management/error/unwind/health.",
        "qualification": "Unchanged pre-spawn whole5s owner,30ms cancellation,500ms observation, real-return/stall receipts, rollback/no-write and normal-session restoration remain. Independent QA12 local37/37; required Windows37/37 and macOS37/37. Retained detailed corrected raw supplies actual read-only deadline error, DatabaseBusy5 and OperationInterrupted9 returns/stall markers/root reap; planted post-return marker gap still fails normal qualification and cleans root; observer error/panic/bad-receipt controls retained.",
        "why_not_pass_only": "QA11 actual trace localized setup consuming4.785s and worker-entry4.975s before cancellation5.480s under original5s owner. Candidate removes irrelevant crash-durability work while retaining complete real cancellation conditions and meaningful failing controls. This source-supported scoped redesign plus independent affected/platform qualification supports disposition, without demanding identical historical recurrence.",
        "limits": "Original QA9 and QA11 failures remain retained. Exact fsync/OS cost and every earlier uninstrumented stage remain unknown; no universal timing or production durable-commit/cancellation guarantee, whole-C2 acceptance or assertion that original failed case qualified. Captured hosted/local PASS logs omit detailed phases; retained developer raw and source assertions supply those semantics.",
        "qa13_preservation": "Preserve prior scoped owner disposition and original failure limits. Relevant fixture/source/workflow bytes independently match QA12 except the one schedule test assertion adopting typed source inspection; current developer store37/37 and full cron_hurry19/19 logs support continued fixture qualification. No new evidence reopens this mechanism; no whole-C2 or current hosted matrix claim.",
        "qa13_reviewed_commit": "478812a83adb04ea38c93a6d4bc83a17233a9cf4",
        "qa14_reviewed_commit": "60f89200fe337d57ede24ecaa55e36171a1c4305",
        "qa14_preservation": "Preserved within the original supported mechanism and candidate scope. All 262 current tracked Rust files, both graph guard sources and both workflows are independently byte-identical to QA13. Prior native raw 37/37 store and 19/19 cron_hurry, meaningful failure controls, and actual current 15/15 hosted checks corroborate preservation. Historical failures remain historical; no new timing cause or whole-C2 acceptance inferred.",
        "qa15_reviewed_commit": "c4006f8d218b743aeabeddcfac9bb5c1c0af8907",
        "qa15_preservation": "Original accepted mechanism and historical limits preserved. Current fixture_child, schedule_protocol, session_fixture, schedule/store production logic and workflows equal QA14. Health helper changes only cfg(test) constructor; new causal cached-read cases reuse existing kernel-session owner. All 262 Rust files equal0c; all49 registry SDK Rust files equal C5. Actual current c400 CI15SUCCESS and independently checked0c-associated native Windows intended-case receipts corroborate current qualification; no permanent all-historical-failure repair or whole-C2 approval inferred."
      },
      {
        "id": "CI-C2-RUNNER-001",
        "status": "resolved_preserved_verified_through_exact_C1_candidate",
        "owner": "flaky-test-qa",
        "reviewed_commit": "da94641038f6745366a240c308b211c796d4cbb6",
        "owning_adjudication_complete": true,
        "reason": "Exactthree-labelsourcecorrection plus direct actualsupportedmacOS15arm64/toolchain/wheel/installedPython3version receipts complete prescribedC1qualification; no pendingownerplaceholder. All11PythonchecksSUCCESS.",
        "scope_limits": "C1d824 candidate only; does not assert C2currentheadindependentmatrixacceptance, exactoldqueuecause, or overallCI/signal/SDKacceptance. Originalretiredconfigurationandqueuecounterevidencepreserved.",
        "qa12_preservation": "Relevant recovery body, ordinary inventory helper and workflows unchanged versus accepted d824; independent affected full suites and current supported-runner CI pass. Historical/scope limits preserved; no C2 or global acceptance.",
        "qa13_preservation": "Preserve prior scoped owner disposition and original failure limits. Relevant fixture/source/workflow bytes independently match QA12 except the one schedule test assertion adopting typed source inspection; current developer store37/37 and full cron_hurry19/19 logs support continued fixture qualification. No new evidence reopens this mechanism; no whole-C2 or current hosted matrix claim.",
        "qa13_reviewed_commit": "478812a83adb04ea38c93a6d4bc83a17233a9cf4",
        "qa14_reviewed_commit": "60f89200fe337d57ede24ecaa55e36171a1c4305",
        "qa14_preservation": "Preserved within the original supported mechanism and candidate scope. All 262 current tracked Rust files, both graph guard sources and both workflows are independently byte-identical to QA13. Prior native raw 37/37 store and 19/19 cron_hurry, meaningful failure controls, and actual current 15/15 hosted checks corroborate preservation. Historical failures remain historical; no new timing cause or whole-C2 acceptance inferred.",
        "qa15_reviewed_commit": "c4006f8d218b743aeabeddcfac9bb5c1c0af8907",
        "qa15_preservation": "Original accepted mechanism and historical limits preserved. Current fixture_child, schedule_protocol, session_fixture, schedule/store production logic and workflows equal QA14. Health helper changes only cfg(test) constructor; new causal cached-read cases reuse existing kernel-session owner. All 262 Rust files equal0c; all49 registry SDK Rust files equal C5. Actual current c400 CI15SUCCESS and independently checked0c-associated native Windows intended-case receipts corroborate current qualification; no permanent all-historical-failure repair or whole-C2 approval inferred."
      },
      {
        "id": "FTQ-009",
        "status": "closed_preserved_for_original_unbounded_lifetime",
        "reason": "Exactlyfilteredwholefixtureowner,actualrootwait/reap and unchanged protocol/FixtureChild retain prior acceptedmechanism; RQA9 separate.",
        "qa13_preservation": "Preserve prior scoped owner disposition and original failure limits. Relevant fixture/source/workflow bytes independently match QA12 except the one schedule test assertion adopting typed source inspection; current developer store37/37 and full cron_hurry19/19 logs support continued fixture qualification. No new evidence reopens this mechanism; no whole-C2 or current hosted matrix claim.",
        "qa13_reviewed_commit": "478812a83adb04ea38c93a6d4bc83a17233a9cf4",
        "qa14_reviewed_commit": "60f89200fe337d57ede24ecaa55e36171a1c4305",
        "qa14_preservation": "Preserved within the original supported mechanism and candidate scope. All 262 current tracked Rust files, both graph guard sources and both workflows are independently byte-identical to QA13. Prior native raw 37/37 store and 19/19 cron_hurry, meaningful failure controls, and actual current 15/15 hosted checks corroborate preservation. Historical failures remain historical; no new timing cause or whole-C2 acceptance inferred.",
        "qa15_reviewed_commit": "c4006f8d218b743aeabeddcfac9bb5c1c0af8907",
        "qa15_preservation": "Original accepted mechanism and historical limits preserved. Current fixture_child, schedule_protocol, session_fixture, schedule/store production logic and workflows equal QA14. Health helper changes only cfg(test) constructor; new causal cached-read cases reuse existing kernel-session owner. All 262 Rust files equal0c; all49 registry SDK Rust files equal C5. Actual current c400 CI15SUCCESS and independently checked0c-associated native Windows intended-case receipts corroborate current qualification; no permanent all-historical-failure repair or whole-C2 approval inferred."
      },
      {
        "id": "FTQ-010",
        "status": "closed_preserved_for_original_session_orphan_inventory_mechanisms",
        "reason": "Cleanupimplementation unchanged;full19independent actual timeout/unwind/preregistration/Pythonownership controls qualified; no speculativekernelprogress finding.",
        "qa13_preservation": "Preserve prior scoped owner disposition and original failure limits. Relevant fixture/source/workflow bytes independently match QA12 except the one schedule test assertion adopting typed source inspection; current developer store37/37 and full cron_hurry19/19 logs support continued fixture qualification. No new evidence reopens this mechanism; no whole-C2 or current hosted matrix claim.",
        "qa13_reviewed_commit": "478812a83adb04ea38c93a6d4bc83a17233a9cf4",
        "qa14_reviewed_commit": "60f89200fe337d57ede24ecaa55e36171a1c4305",
        "qa14_preservation": "Preserved within the original supported mechanism and candidate scope. All 262 current tracked Rust files, both graph guard sources and both workflows are independently byte-identical to QA13. Prior native raw 37/37 store and 19/19 cron_hurry, meaningful failure controls, and actual current 15/15 hosted checks corroborate preservation. Historical failures remain historical; no new timing cause or whole-C2 acceptance inferred.",
        "qa15_reviewed_commit": "c4006f8d218b743aeabeddcfac9bb5c1c0af8907",
        "qa15_preservation": "Original accepted mechanism and historical limits preserved. Current fixture_child, schedule_protocol, session_fixture, schedule/store production logic and workflows equal QA14. Health helper changes only cfg(test) constructor; new causal cached-read cases reuse existing kernel-session owner. All 262 Rust files equal0c; all49 registry SDK Rust files equal C5. Actual current c400 CI15SUCCESS and independently checked0c-associated native Windows intended-case receipts corroborate current qualification; no permanent all-historical-failure repair or whole-C2 approval inferred."
      },
      {
        "id": "FTQ-011",
        "status": "closed_preserved_for_independent50ms_cap_mechanism",
        "reason": "Prior removed independent50ms cap correction preserved; explicit bounded-ready regression and actual old50 mutation retain meaningful delay/success/error/ownership proof. Ordinary caller deadlines unchanged; no RQA11 pending-owner condition.",
        "qa13_preservation": "Preserve prior scoped owner disposition and original failure limits. Relevant fixture/source/workflow bytes independently match QA12 except the one schedule test assertion adopting typed source inspection; current developer store37/37 and full cron_hurry19/19 logs support continued fixture qualification. No new evidence reopens this mechanism; no whole-C2 or current hosted matrix claim.",
        "qa13_reviewed_commit": "478812a83adb04ea38c93a6d4bc83a17233a9cf4",
        "qa14_reviewed_commit": "60f89200fe337d57ede24ecaa55e36171a1c4305",
        "qa14_preservation": "Preserved within the original supported mechanism and candidate scope. All 262 current tracked Rust files, both graph guard sources and both workflows are independently byte-identical to QA13. Prior native raw 37/37 store and 19/19 cron_hurry, meaningful failure controls, and actual current 15/15 hosted checks corroborate preservation. Historical failures remain historical; no new timing cause or whole-C2 acceptance inferred.",
        "qa15_reviewed_commit": "c4006f8d218b743aeabeddcfac9bb5c1c0af8907",
        "qa15_preservation": "Original accepted mechanism and historical limits preserved. Current fixture_child, schedule_protocol, session_fixture, schedule/store production logic and workflows equal QA14. Health helper changes only cfg(test) constructor; new causal cached-read cases reuse existing kernel-session owner. All 262 Rust files equal0c; all49 registry SDK Rust files equal C5. Actual current c400 CI15SUCCESS and independently checked0c-associated native Windows intended-case receipts corroborate current qualification; no permanent all-historical-failure repair or whole-C2 approval inferred."
      },
      {
        "id": "RSH-001",
        "status": "resolved_preserved_in_prior_supported_C1_and_paired_public_C2_scope",
        "reason": "Prior accepted C1 and paired public C2 startup scope retained. Current production source equivalence independently verified despite two physically changed cfg(test) source files; prior publiccaller250.876416mswithin500ms and service7 evidence remains scoped. No new stock HTTP/wholeC2/universaldeadline claim.",
        "qa13_preservation": "Preserve prior scoped owner disposition and original failure limits. Relevant fixture/source/workflow bytes independently match QA12 except the one schedule test assertion adopting typed source inspection; current developer store37/37 and full cron_hurry19/19 logs support continued fixture qualification. No new evidence reopens this mechanism; no whole-C2 or current hosted matrix claim.",
        "qa13_reviewed_commit": "478812a83adb04ea38c93a6d4bc83a17233a9cf4",
        "qa14_reviewed_commit": "60f89200fe337d57ede24ecaa55e36171a1c4305",
        "qa14_preservation": "Preserved within the original supported mechanism and candidate scope. All 262 current tracked Rust files, both graph guard sources and both workflows are independently byte-identical to QA13. Prior native raw 37/37 store and 19/19 cron_hurry, meaningful failure controls, and actual current 15/15 hosted checks corroborate preservation. Historical failures remain historical; no new timing cause or whole-C2 acceptance inferred.",
        "qa15_reviewed_commit": "c4006f8d218b743aeabeddcfac9bb5c1c0af8907",
        "qa15_preservation": "Original accepted mechanism and historical limits preserved. Current fixture_child, schedule_protocol, session_fixture, schedule/store production logic and workflows equal QA14. Health helper changes only cfg(test) constructor; new causal cached-read cases reuse existing kernel-session owner. All 262 Rust files equal0c; all49 registry SDK Rust files equal C5. Actual current c400 CI15SUCCESS and independently checked0c-associated native Windows intended-case receipts corroborate current qualification; no permanent all-historical-failure repair or whole-C2 approval inferred."
      },
      {
        "id": "C2-FTQ-006",
        "status": "retracted_preserved",
        "reason": "No impossiblehealthy-kernel/sameUIDprogressfindingrevived.",
        "qa13_preservation": "Preserve prior scoped owner disposition and original failure limits. Relevant fixture/source/workflow bytes independently match QA12 except the one schedule test assertion adopting typed source inspection; current developer store37/37 and full cron_hurry19/19 logs support continued fixture qualification. No new evidence reopens this mechanism; no whole-C2 or current hosted matrix claim.",
        "qa13_reviewed_commit": "478812a83adb04ea38c93a6d4bc83a17233a9cf4",
        "qa14_reviewed_commit": "60f89200fe337d57ede24ecaa55e36171a1c4305",
        "qa14_preservation": "Preserved within the original supported mechanism and candidate scope. All 262 current tracked Rust files, both graph guard sources and both workflows are independently byte-identical to QA13. Prior native raw 37/37 store and 19/19 cron_hurry, meaningful failure controls, and actual current 15/15 hosted checks corroborate preservation. Historical failures remain historical; no new timing cause or whole-C2 acceptance inferred.",
        "qa15_reviewed_commit": "c4006f8d218b743aeabeddcfac9bb5c1c0af8907",
        "qa15_preservation": "Original accepted mechanism and historical limits preserved. Current fixture_child, schedule_protocol, session_fixture, schedule/store production logic and workflows equal QA14. Health helper changes only cfg(test) constructor; new causal cached-read cases reuse existing kernel-session owner. All 262 Rust files equal0c; all49 registry SDK Rust files equal C5. Actual current c400 CI15SUCCESS and independently checked0c-associated native Windows intended-case receipts corroborate current qualification; no permanent all-historical-failure repair or whole-C2 approval inferred."
      },
      {
        "id": "RQA11-001",
        "severity": "important",
        "status": "resolved_verified_for_exact_C1_candidate",
        "owner": "flaky-test-qa",
        "reviewed_commit": "da94641038f6745366a240c308b211c796d4cbb6",
        "owning_adjudication_complete": true,
        "canonical_disposition": "RESOLVED for isolated delayed-positive preparation versus caller-budget composition. No pending-owner condition remains.",
        "correction": "Existing same test now runs exactly filtered in complete SessionFixture5s from pre-spawn with actual setsid. Real stopped command is acknowledged inside that owner before creating ONE250ms caller deadline immediately before process_ids_using. Measured>=80ms hold,CONT,same Inventory finish/output/full parse/self-presence,actual wait/reap and PID absence remain inside caller qualification. No earlier supplied caller or outer deadline is renewed.",
        "qualification": "Independent QA12 local full19/19 and current required macOS full19/19. Retained corrected raw measures85.854834ms hold/114.615834ms caller; postpush81.869792ms/117.949916ms. Old50ms-before-closure mutant actually fails child+outer101 after84.020583ms hold, PID89442 actual kill/wait at89.354083ms; outer cleanuptrue does not waive caller error. Restored helperSHA72345e0132cfaca90daf3ba689d403319fd26c8ac6277929c23563ab3d49c9ac independently matches current source.",
        "why_not_pass_only": "Explicit acknowledged-readiness split removes incidental shell/temp/STOP-ack preparation from this isolated regression measurement while retaining the intentional delay exceeding the former cap. Ordinary preflight/cleanup/audit whole-call absolute bounds and stopped timeout/unwind/three incomplete-scan false-receipt/no-retry/distinct quiescence-audit controls are byte-equivalent and exercised. No arbitrary timeout acceptance, padding, cap renewal, fake clock, host tuning or missing census.",
        "limits": "Original macOS waited-exit0 at257.978625ms then output-deadline failure remains retained. Isolated positive intentionally no longer measures preparation within250ms; outer5s owns preparation. This does not change real preflight spawning budget or identify unrecorded STOP/CONT/OS costs. Mutant is inspected retained execution/provenance, not independently re-executed by this owner; no C2 consumption inferred.",
        "qa13_preservation": "Preserve prior scoped owner disposition and original failure limits. Relevant fixture/source/workflow bytes independently match QA12 except the one schedule test assertion adopting typed source inspection; current developer store37/37 and full cron_hurry19/19 logs support continued fixture qualification. No new evidence reopens this mechanism; no whole-C2 or current hosted matrix claim.",
        "qa13_reviewed_commit": "478812a83adb04ea38c93a6d4bc83a17233a9cf4",
        "qa14_reviewed_commit": "60f89200fe337d57ede24ecaa55e36171a1c4305",
        "qa14_preservation": "Preserved within the original supported mechanism and candidate scope. All 262 current tracked Rust files, both graph guard sources and both workflows are independently byte-identical to QA13. Prior native raw 37/37 store and 19/19 cron_hurry, meaningful failure controls, and actual current 15/15 hosted checks corroborate preservation. Historical failures remain historical; no new timing cause or whole-C2 acceptance inferred.",
        "qa15_reviewed_commit": "c4006f8d218b743aeabeddcfac9bb5c1c0af8907",
        "qa15_preservation": "Original accepted mechanism and historical limits preserved. Current fixture_child, schedule_protocol, session_fixture, schedule/store production logic and workflows equal QA14. Health helper changes only cfg(test) constructor; new causal cached-read cases reuse existing kernel-session owner. All 262 Rust files equal0c; all49 registry SDK Rust files equal C5. Actual current c400 CI15SUCCESS and independently checked0c-associated native Windows intended-case receipts corroborate current qualification; no permanent all-historical-failure repair or whole-C2 approval inferred."
      }
    ],
    "execution_and_limits": [
      "Ran absolute cm context before analysis and saved additional final context receipt; relevant rule b-mr7cp6x0-ipdnhs applied by checking actual Windows independently of local Darwin success. Cass history unavailable; proceeded.",
      "Only fresh execution by this reviewer was eight Python graph unit tests and read-only source/git/JSON/hash checks. No Cargo, physical backend process, external Jev, native Rust test rerun, child agents, source/triage/anchor/bead changes, PR posts, resets or merge.",
      "Current developer native full workspace/affected proof and original sandbox tree_kill failures inspected with exact receipts. Two initial sandbox tree_kill readiness failures remain retained; one identical authorized native command succeeds, with no source/timer/fault changes recorded. No lower-level sandbox, ps, scheduling or host cause inferred.",
      "Current supplied c400 CI snapshot is15SUCCESS. Prior Windows0c raw exactness is PR-merge checkout, not candidate checkout or current-c400 raw. SDK checksum/local cache source equivalence does not replace external publication policy/whole-consumer acceptance.",
      "Current full v3 development-anchor gate and paired deterministic-check acceptance remain pending per parent. No whole-sprint PASS, completion count, gate waiver, new standing ledger or C2/C4 acceptance inferred. No unresolved external gate is manufactured into a new flaky-test finding.",
      "Health constructor purpose correction plus a subsequent pass is not claimed as deterministic causal repair of all historical Windows failures. Historical observations, original severity/provenance and uncertainty remain intact.",
      "QA14 full report/MD preserved; original JSON hash f05176cdf8d4192573a6e709a828d954e136a7bbdfe9e2e96eecd4e72e775141 verified unchanged."
    ],
    "evidence_manifest": [
      {
        "path": "/tmp/phase-c-execution/c-1/qa-15/flaky-test-qa-input.json",
        "bytes": 12913,
        "sha256": "95c7b53aa7479eb7ab883ffe3115e98a2a98a0df1da83b8932959967aa3d1721"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-15/flaky-test-qa-source-receipt.json",
        "bytes": 20795,
        "sha256": "bbf1f25e8c01e220a215f7eb36e09f2b150aa6a71a21fdc3d006524fd49dd5eb"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-15/flaky-test-qa-risk-scan.txt",
        "bytes": 193775,
        "sha256": "f5359273c833fb4b8f849030271928bbe529e229d22cea137115ab6fc3941036"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-15/flaky-test-qa-graph-unit.log",
        "bytes": 1242,
        "sha256": "9890a4679fcae2a8c9c97d664b4098a45c1be6865eebfa7be06c0b47b0675ed5"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-15/flaky-test-qa-sdk-proof.json",
        "bytes": 1695,
        "sha256": "5677a0cec6b5a550ff41e5026286190ab55220e235049fa98c218a2dfdf3e51e"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-15/flaky-test-qa-windows-receipt-proof.json",
        "bytes": 3526,
        "sha256": "fa6c4587759283df5c71af37aee2607d3dd3e12d66835d179810a53866fc4cfb"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-15/flaky-test-qa-preservation-proof.json",
        "bytes": 626,
        "sha256": "fa32b8e577cc188451c2517abb17e9664690a237c4fe0d958db781fdf84144a2"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-15/flaky-test-qa-cm-context.json",
        "bytes": 2316,
        "sha256": "aac9ebe4c553b9eb0dddd2b3542abda1c609920a7a91ad5d0039e65c6215555c"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-15/current-ci.json",
        "bytes": 4588,
        "sha256": "96ae03ec1c61aa277d0a580ac9e5f5d583a2bd368cfe7c73596661d1846a31d7"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-15/initial-windows-failure.log",
        "bytes": 205604,
        "sha256": "c0efe7ac531b8eaa980e131fb5b07d7146ab6d2482487830068c533e858b8f7c"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-15/corrected-windows-pass.log",
        "bytes": 308204,
        "sha256": "b025cf43fe60cd56b70f93084a8acce6740fa5e6e61bffedcdbef9f131940a47"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-15/windows-causal-review/review.json",
        "bytes": 19208,
        "sha256": "07bcc428d0056d8cd8417dc19b25a960c515eb35f5c3277d9ee341ee8386af07"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-15/windows-causal-review/qualification-receipt.json",
        "bytes": 8266,
        "sha256": "e916f580b4eed2772bc004eff409baa2f9e5b7ad2d51d096d9c9e54aec081f10"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-14/flaky-test-qa-result.json",
        "bytes": 38272,
        "sha256": "f05176cdf8d4192573a6e709a828d954e136a7bbdfe9e2e96eecd4e72e775141"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-14/manager-result.md",
        "bytes": 11395,
        "sha256": "bda682050d3156a1f927ee9c388ba2a3217f19ccc32042239f5b8a47e7bdd540"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-14/manager-machine-status.json",
        "bytes": 4256,
        "sha256": "262570cad12aba073700ef009662f344ff4148c494c7c06aaa83151eaa620971"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-14-fix/developer-result.json",
        "bytes": 25476,
        "sha256": "6ea69d171a62982ae72cf6ced294c2ccb7420d466abdeb40d08d4906ae1042d1"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-14-fix/ci-repair-result.json",
        "bytes": 9754,
        "sha256": "75dee44db6c02c327c334b64f38463bd363558732b6f163b2b9536513ed5e21f"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-14-fix/windows-ci-diagnosis.json",
        "bytes": 10338,
        "sha256": "036945de8ed8cef330865410caae4df22130a7adc91570870a2c8f0ed560dd08"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/registry020-adoption/developer-result.json",
        "bytes": 24536,
        "sha256": "3042dc505debfa71a7c5f7225dcaa2620c154524a375958db1f5280e5213a2d0"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/registry020-adoption/developer-result.md",
        "bytes": 4391,
        "sha256": "a367fd7705e25e0f11249195703b96a8242127bb7d60291ab243545c23d3d0c8"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/registry020-adoption/validation-receipts.json",
        "bytes": 8700,
        "sha256": "59a30a07f074d07d91debd53897ccc4696c17a81cb50646b4d26200b6bcdc599"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/registry020-adoption/source-preservation.json",
        "bytes": 1327,
        "sha256": "c61dd9fe0278228418ee81df86644a2125a407276f628479e9084999c033bcae"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/registry020-adoption/source-provenance.json",
        "bytes": 2546,
        "sha256": "9547d6b3622a77295d99573811dd90e060404593108a92061b749e849cd4c7b9"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/registry020-adoption/workspace-tests-native.log",
        "bytes": 105614,
        "sha256": "b19dcdc4ea4c480518713d8a7a004478bb84e13dc5841bd2e5ffb7d4753be1c1"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/registry020-adoption/workspace-tests-sandbox.log",
        "bytes": 36448,
        "sha256": "e7199b8ca4cbd0105a26933d00e65d07a078a93a51bfb13439035fd5c67dad60"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/registry020-adoption/causal-cached-read-controls.log",
        "bytes": 6365,
        "sha256": "084f96dec0527c55d523d7d067d447209918fe7df220a61c910959f0aee643da"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/registry020-adoption/isolated-graph.log",
        "bytes": 177,
        "sha256": "2ac4a90b105294e03599c55756b1dd047e19c6bcf5c9bab85cf7c0941509b02d"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/registry020-adoption/negative-server.log",
        "bytes": 261,
        "sha256": "3d57fb694660469e96e9104eef68bb32afbac8cc47aafa23d224d352a5aa6c0e"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/registry020-adoption/backend-free-path.log",
        "bytes": 22,
        "sha256": "2d16fc7fbdd8a2783ff108dabc83fd41419efda0faf9f8688faba828a391740d"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/registry020-adoption/postpush-backend-free-path.log",
        "bytes": 22,
        "sha256": "2d16fc7fbdd8a2783ff108dabc83fd41419efda0faf9f8688faba828a391740d"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/registry020-adoption/python-rust-tests.log",
        "bytes": 3816,
        "sha256": "a69019107d07b50bbad028691667e22011e11512e0a0ec949f17a0728b373e34"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/registry020-adoption/python-pytest.log",
        "bytes": 2809,
        "sha256": "2cb498fc5f739963d18b651d859b36c2aabcbb8361a77bd23a2f15d8c612e3ef"
      }
    ],
    "artifacts": {
      "json": "/tmp/phase-c-execution/c-1/qa-15/flaky-test-qa-result.json",
      "markdown": "/tmp/phase-c-execution/c-1/qa-15/flaky-test-qa-result.md",
      "projection": "/tmp/phase-c-execution/c-1/qa-15/flaky-test-qa-return-projection.json"
    }
  },
  "error": null
}
```
