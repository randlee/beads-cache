```json
{
  "success": true,
  "data": {
    "status": "findings",
    "reviewed_branch": "sprint/c-1-runtime-migration",
    "reviewed_commit": "8c1aefafd7fd269c82c7974e610e848816d026d8",
    "review_mode": "sprint_review",
    "round_limit": false,
    "qa_pass": "qa-10",
    "model": "gpt-6.1-sol high fallback; Composer unavailable",
    "input_contract_verified": true,
    "exact_branch_HEAD_clean_verified_before_and_after": true,
    "scope": {
      "assignment": "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-input.json",
      "review_targets_count": 90,
      "review_targets": "Entire rendered scope reviewed using risk-pattern scans, contextual reads, previous independent QA9 evidence and explicit current-source equivalence; all changed Rust files read in full. No full-review claim based solely on producer assertions.",
      "intentional_deleted_targets": "Listed crates/sc-cron source/test paths are absent as the previously reviewed migration removed the local crate; workspace pins published sc-cron=0.1.1 with default-features=false. Their absence is not new missing review input.",
      "changes_since_QA9": [
        "crates/bd-cache-store/src/schedule.rs (cfg(test) only)",
        "crates/bd-cache-store/tests/support/fixture_child.rs",
        "crates/bd-cache-store/tests/support/schedule_protocol.rs (new private include)"
      ]
    },
    "procedural_memory": {
      "context_executed": true,
      "rule_application": "No new code or test execution by this reviewer. Returned cass maintenance rule irrelevant; cross-platform CI rule is respected by retaining platform qualification limits, not inventing current all-platform PASS. CASS search degraded; no fabricated rules."
    },
    "source_equivalence": {
      "comparison": "a753d2799183dd4e341c854c4515bf046447eb6c..8c1aefafd7fd269c82c7974e610e848816d026d8",
      "tracked_crates_src_rs": 159,
      "wholebyte_equal_src_rs": 158,
      "changed_src_rs": [
        "crates/bd-cache-store/src/schedule.rs"
      ],
      "schedule_production_prefix_equal": true,
      "schedule_production_prefix_sha256": "49438f00b7310fb4d385355dbc887ab3c353264eed25e05df9f824e5872da05a",
      "same_QA9_wholebytes": [
        "crates/bd-cache-sync/src/cron.rs",
        "crates/bd-cache-sync/src/schedule.rs",
        "crates/bd-cache-sync/tests/common/session_fixture.rs",
        "crates/bd-cache-sync/tests/cron_hurry.rs",
        "crates/bd-cache-sync/Cargo.toml",
        "crates/bd-cache-sync/src/cron_log.rs",
        ".github/workflows/ci.yml"
      ],
      "limits": "Do not call the entire schedule.rs file unchanged. Preserved startup/service qualification is carried only for identical production and the same supported scope, never stock HTTP or wholeC2 acceptance."
    },
    "changed_scope_assessment": {
      "status": "no_new_concrete_phase_protocol_defect; separate_actual_lifecycle_inventory_failure_reported",
      "private_protocol": "Included only under schedule cfg(test). Separate per-child owned scratch files have one fixed main writer and one worker writer, with no shared logging mutex. Fixed callsites emit a finite phase set. Record is no-op when no fixture root exists; old public consumers require no hook or protocol.",
      "bounded_collection": "Each stream read is capped at64KiB+1 and rejects oversize. Only complete lines are parsed live; final collection after root wait rejects remaining partial line. PID/stream/phase/elapsed type are checked; changed contents at already received index are rejected. Malformed/partial data propagates error and cannot fabricate a positive receipt. Complete files are regular files in owned unique scratch, not pipes/read tasks; no unsupported hostile filesystem or kernel-progress guarantee asserted.",
      "qualification": "Normal qualification still requires timed-out child status, true completed cleanup receipt, original<7s watchdog, original post-call stall text and original500ms expiry text. It additionally requires real actual-waitOk, cancellation, real error return, completionTimeout and join-start checkpoints plus marker-write completion. Structured worker return never substitutes for missing original text.",
      "planted_control": "Private post-return-gap mode parks only after actual busy call return and successful independent return checkpoint, before result/stall text. Outer regression catches and requires normal qualify panic, checks DatabaseBusy/missing marker/actual wait/cancel/Timeout/reap, and optional explicit direct-negative environment resumes that exact panic for realexit101. No production call skipped or healthy negative relabeled.",
      "budgets": "Original2s actual-wait receive, finish_by(now+30ms),500ms completion receive, whole child absolute5s and supervisor<7s remain. Parent Instant starts before Command.spawn as before; no lifetime reset, retry, timeout widening, synthetic load or early-stop selection change.",
      "clock_provenance": "All child phases now use fixture-body Instant, including setup. Parent owner_receive uses original pre-spawn Instant. Separate outer_to_owner and spawn_return offsets are reported. Unix wall correlation is diagnostic, may move and never controls decisions. File timestamp values are captured before their write/receipt; parent receive bounds observation rather than exact child execution. No cross-process monotonic subtraction or perfect wall-clock claim.",
      "shared_optional_hook": "finish delegates to finish_observing with a no-op callback. Observer executes inline under same original deadline; elapsed observation costs are not fresh budget. Live observer errors or panics in protocol::observe unwind/drop the retained owner; final observer runs only after actual cleanup. Existing SessionFixture still explicitly takes ownership and sets cleanup_on_drop=false, using its unchanged own cleanup path.",
      "ownership": "kill_and_reap marks one attempt and disables retry, captures registry errors while still killing/waiting root, sets cleaned only after wait and descendant checks succeed. An incomplete receipt stays false; Drop does not create another cleanup budget. DefaultDrop and assertion unwind retain actual kill/wait. Final collection/output errors occur after reap; early observer failures drop the owner. No new callback before ownership construction or detached reader task.",
      "controls_verified": "Independent RustQA raw controls show observerErr and observerPanic actual kill/wait receipt; malformed registered PID retains cleanup receiptfalse, root try_waitSome, repeat attempt refused. Source prevents Drop retry. All recorded controls complete before outer fallback, and RustQA driver reports fallbackfalse.",
      "coverage_precision": "Health2/2 uses a distinct health::MutexChild, so it is additional unchanged health evidence, not shared FixtureChild default-path proof. Three normal schedule children in independent8/8 actually exercise shared default finish/no-op behavior. Strict sync cron_hurry Clippy independently covers changed helper inclusion; unchanged SessionFixture behavior is carried via source equivalence, not newly rerun lifecycle18."
    },
    "independent_evidence_used": {
      "executor": "/root/c1_rustqa10 owns Cargo; this reviewer ran no Cargo/build/test or binary",
      "commands": [
        {
          "name": "fmt",
          "exit": 0,
          "elapsed_seconds": 0.7256027907133102,
          "driver_fallback_used": false,
          "log": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-fmt.log"
        },
        {
          "name": "private-fmt",
          "exit": 0,
          "elapsed_seconds": 0.04024250013753772,
          "driver_fallback_used": false,
          "log": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-private-fmt.log"
        },
        {
          "name": "clippy",
          "exit": 0,
          "elapsed_seconds": 0.33904141606763005,
          "driver_fallback_used": false,
          "log": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-clippy.log"
        },
        {
          "name": "clippy-default",
          "exit": 0,
          "elapsed_seconds": 0.7781849172897637,
          "driver_fallback_used": false,
          "log": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-clippy-default.log"
        },
        {
          "name": "clippy-helper-compat",
          "exit": 0,
          "elapsed_seconds": 0.18456237483769655,
          "driver_fallback_used": false,
          "log": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-clippy-helper-compat.log"
        },
        {
          "name": "store",
          "exit": 0,
          "elapsed_seconds": 15.179670332930982,
          "driver_fallback_used": false,
          "log": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-store.log"
        },
        {
          "name": "health-default",
          "exit": 0,
          "elapsed_seconds": 5.146079292055219,
          "driver_fallback_used": false,
          "log": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-health-default.log"
        }
      ],
      "affected_store": "8passed0failed29filtered;15.03s",
      "health_additional": "2passed0failed35filtered;5.01s",
      "direct_negative": {
        "source": "8c1aefafd7fd269c82c7974e610e848816d026d8",
        "binary_sha256": "e648e86788ee0e67368364c9ff126fd92b74270889e69845d9f56b8ae77298ac",
        "exit": 101,
        "elapsed_seconds": 5.06444974988699,
        "driver_fallback_used": false,
        "qualification_failed": true
      },
      "direct_negative_interpretation": "Expected actual failed qualification101; not a healthy test PASS. Packet shows root_joined_and_cleaned=true before output/failure and no driver fallback. No independent syscall/kernel trace claimed. Intentionally parked worker threads do not join; owned root process does.",
      "packets": [
        {
          "test": "schedule::tests::shutdown_interrupts_busy_persistence_and_restores_the_shared_session",
          "mode": "post-return-gap",
          "classification": "worker_returned_required_marker_missing",
          "root_joined_and_cleaned": true,
          "owner_reported_ns": 5005009000,
          "spawn_returned_ns": 1437625,
          "receipt_count": 22,
          "actual_wait": {
            "ok": true
          },
          "actual_worker_return": {
            "debug": "Err(Store(StoreError { kind: Busy, context: \"failed to update sc_cron_schedule id 1\", source: Some(SqliteFailure(Error { code: DatabaseBusy, extended_code: 5 }, Some(\"database is locked\"))) }))",
            "is_error": true,
            "sqlite_code": "DatabaseBusy"
          },
          "completion": {
            "timeout": true
          },
          "stall_marker_written": false,
          "worker_join_started": true,
          "planted_gap": true
        },
        {
          "test": "schedule::tests::shutdown_interrupts_an_already_contended_worker_read_only",
          "mode": "stalled",
          "classification": "worker_return_and_stall_marker_observed",
          "root_joined_and_cleaned": true,
          "owner_reported_ns": 5005967000,
          "spawn_returned_ns": 1470125,
          "receipt_count": 16,
          "actual_wait": {
            "ok": true
          },
          "actual_worker_return": {
            "debug": "Err(Store(StoreError { kind: Other, context: \"bounded cache schedule session\", source: Some(StoreFailed { detail: \"schedule worker shutdown: store deadline expired\", source: Some(StoreCause(Custom { kind: TimedOut, error: \"schedule store deadline expired\" })) }) }))",
            "is_error": true,
            "sqlite_code": null
          },
          "completion": {
            "timeout": true
          },
          "stall_marker_written": true,
          "worker_join_started": true,
          "planted_gap": false
        },
        {
          "test": "schedule::tests::shutdown_interrupts_busy_persistence_and_restores_the_shared_session",
          "mode": "stalled",
          "classification": "worker_return_and_stall_marker_observed",
          "root_joined_and_cleaned": true,
          "owner_reported_ns": 5004628792,
          "spawn_returned_ns": 246125,
          "receipt_count": 22,
          "actual_wait": {
            "ok": true
          },
          "actual_worker_return": {
            "debug": "Err(Store(StoreError { kind: Busy, context: \"failed to update sc_cron_schedule id 1\", source: Some(SqliteFailure(Error { code: DatabaseBusy, extended_code: 5 }, Some(\"database is locked\"))) }))",
            "is_error": true,
            "sqlite_code": "DatabaseBusy"
          },
          "completion": {
            "timeout": true
          },
          "stall_marker_written": true,
          "worker_join_started": true,
          "planted_gap": false
        },
        {
          "test": "schedule::tests::shutdown_interrupts_a_running_sql_statement_and_restores_progress",
          "mode": "stalled",
          "classification": "worker_return_and_stall_marker_observed",
          "root_joined_and_cleaned": true,
          "owner_reported_ns": 5004841375,
          "spawn_returned_ns": 497791,
          "receipt_count": 17,
          "actual_wait": {
            "ok": true
          },
          "actual_worker_return": {
            "debug": "Err(Store(StoreError { kind: Other, context: \"failed to update sc_cron_schedule id 1\", source: Some(SqliteFailure(Error { code: OperationInterrupted, extended_code: 9 }, Some(\"interrupted\"))) }))",
            "is_error": true,
            "sqlite_code": "OperationInterrupted"
          },
          "completion": {
            "timeout": true
          },
          "stall_marker_written": true,
          "worker_join_started": true,
          "planted_gap": false
        }
      ],
      "producer_evidence": "Final qa9/phase-protocol-result.json and raw postpush8+2/directnegative checked as secondary evidence; actual independent QA10 raw logs/command receipts supply fresh execution evidence. Initial developer Clippy errors retained; no suppression inferred."
    },
    "findings": [
      {
        "id": "RQA9-001",
        "severity": "important",
        "file": "crates/bd-cache-store/src/schedule.rs",
        "line": 1151,
        "test": "shutdown_fixture_supervisor_kills_and_reaps_each_stalled_cleanup",
        "mechanism": "timing_assertion",
        "issue": "The original actual C2 Windows busy-negative child missed the required post-worker/stall receipt at its absolute five-second lifetime. New private receipts and planted classification control are independently verified, but no historical failing phase, trigger or underlying correction has been established. This remains an unresolved required-test failure, with observational timing classification only.",
        "recommendation": "Retain RQA9-001 open with preserved original raw evidence. Use the new per-child phase/actual-result evidence to localize a real failing occurrence if obtained; route only a supported mechanism for correction or evidence-based owner reclassification. Do not rerun-to-green, widen budgets, infer worker non-entry from missing text, or waive the original failure because the diagnostic/control passes.",
        "evidence": "Original QA9 raw Windows job126397662727 lines1950-1965 and original owner evidence retained. Independently read current schedule.rs test module, schedule_protocol.rs and shared FixtureChild. Current RustQA source8c1aefa affected8/8 passes; direct planted actualexit101 in5.06444975s, realDatabaseBusy worker return, required marker absent, completionTimeout, actual owned root cleanup before driverfallback(false). This is a controlled negative classification, not historical reproduction.",
        "limits": "Timing classification describes the observed watchdog/receipt failure only. Logs do not locate the child at timeout or prove host scheduling, SQLite malfunction, missing product cancellation, orphaning, or a newly introduced source regression. Missing receipt does not prove the operation never began or returned. This is separate from the earlier Windows CTRL_BREAK issue, startup shutdown correction, and the repaired FTQ009 unbounded fixture lifetime.",
        "correlation": "Same actual occurrence as independent rust-qa-agent RQA9-001; retain this ID, not a duplicate FTQ ID.",
        "original_line": 1084,
        "status": "open",
        "new_finding": false
      },
      {
        "id": "RQA10-001",
        "severity": "important",
        "file": "crates/bd-cache-sync/tests/common/session_fixture.rs",
        "line": 631,
        "test": "session_fixture::cleanup_inventory_recovery_preserves_the_original_fixture_failure",
        "mechanism": "timing_assertion",
        "status": "open",
        "new_finding": true,
        "issue": "Actual current C1 hosted macOS required test fails with an owned session inventory timeout after logging recovery from the injected preliminary inventory failure. cron_hurry reports17passed1failed andexit101. The failing inventory stage and cause are unresolved; current qualification cannot be accepted from local passes.",
        "recommendation": "Preserve the actual failed log and route this distinct lifecycle-inventory occurrence to its owner. Add or collect bounded stage/call/deadline receipts distinguishing the third injected80ms stopped inventory, subsequent quiescence/final cleanup scans and the independent final250ms session audit. Record actual owned inventory exit/reap and remaining caller deadline before attributing cause. Preserve existing100ms drain, absolute250ms quiescence,1s cleanup and5s fixture budgets, original fixture failure, false incomplete receipts and one cleanup attempt. No retry-to-green, timeout widening, host-load guess or automatic FTQ011 reopening.",
        "evidence": "Current run41786865484 macOS job126430892802: raw rust-qa-hosted-macos.log1557/1572-1583, Error: Custom{kind:TimedOut,error:owned session inventory timed out}; prior line1573 session drain recovered inventory before deadline: injected transient inventory failure. Actual test group17/18exit101. Source session_fixture.rs631-690/119-182/219-244 directly reviewed and byte-identicalQA9a753d279.",
        "limits": "Concrete observed test failure, not a proven fixture race, production fault, orphan, returned50ms-cap defect or new shared FixtureChild observer regression. Inventory.finish emits this same timeout for any supplied absolute deadline. The raw log does not identify whether the timeout arose in injected scan3, a later cleanup scan or separate post-cleanup audit at678. It does not prove final cleanup success or failure, actual empty census, or survivors.",
        "correlation": "Same actual occurrence and canonical ID RQA10-001 as independent RustQA; separate from original Windows store RQA9-001 and previously fixed FTQ011 independent50ms cap."
      }
    ],
    "summary": {
      "total_findings": 2,
      "critical": 0,
      "important": 2,
      "minor": 0,
      "new_findings": 1,
      "sprint_verdict": "FAIL: original RQA9-001 remains open and actual distinct current macOS lifecycle-inventory failure RQA10-001 is newly open. Phase protocol/control itself has no new identified defect."
    },
    "dispositions": [
      {
        "id": "RQA9-001",
        "status": "open",
        "reason": "Original Windows occurrence remains unlocalized. Independent diagnostic and planted failed-qualification evidence support new receipt design, not repair or false-positive reclassification. Actual busy-negative sibling overlap was not reproduced by earlier successful full34 context run; no host-load/interfering resource cause established."
      },
      {
        "id": "FTQ-009",
        "status": "closed_preserved_for_original_complete_lifetime_mechanism",
        "reason": "Current source and independent8/8 retain exactly filtered owned store child, original5s lifetime, actual kill/wait, normal/restoration paths, all3 post-call stall negatives and assertion-unwind reap; new observer/error/panic/failedreceipt controls do not show an ownership regression. RQA9 remains distinct."
      },
      {
        "id": "FTQ-010",
        "status": "closed_preserved_for_original_session_orphan_inventory_mechanisms",
        "reason": "SessionFixture and cron_hurry byte-identical QA9; explicit cleanup handoff preserved. No reopening based on speculative kernel progress/credential behavior. Changed shared include compiles with strict current sync lint."
      },
      {
        "id": "FTQ-011",
        "status": "closed_preserved_for_exact_C1_candidate_mechanism",
        "reason": "Current SessionFixture blob123a5ce3a4c14895b2013f1cb74093f982129be9 is byte-identical acceptedQA9: caller absolute inventory deadline, completeness/identity/quiescence checks, actual root wait and one-attempt cleanup remain. Do not carry stale pending-owner wording or imply closure for unconsumed old C2 source. New actual current macOS recovery test failure is retained separately as RQA10-001; it does not establish recurrence of removed50ms cap. No broad current-platform qualification claimed."
      },
      {
        "id": "RSH-001",
        "status": "resolved_preserved_in_supported_C1_and_paired_public_C2_scope",
        "reason": "Current production cron/schedule exactQA9 wholebytes and schedule.rs productionprefix match accepted owner evidence; prior lifecycle18, actual rebuilt supported public Application250.876416ms within original500ms and service7 causal/actual joins remain credited. No new stock HTTP/wholeC2 qualification or all-path mathematical guarantee."
      },
      {
        "id": "REQ-QA-004",
        "status": "open",
        "reason": "Actual current run41786865484 core LinuxSUCCESS, macOSFAIL(17/18cron_hurry), Windows terminalSUCCESS includingTest/Latency, and boundaryFAIL on unchanged direct sync->sc-command. Required current CI acceptance remains unsatisfied; no waiver from local/current store37/37 passes."
      },
      {
        "id": "ARCH-001",
        "status": "open",
        "reason": "Published public canonical-event receiver composition gap/direct sync->sc-command source unchanged; private diagnostics do not supply SDK seam."
      },
      {
        "id": "RBQA-F001",
        "status": "open",
        "reason": "Distinct carried boundary/public receiver finding remains; unchanged direct edge and strict guard not waived."
      },
      {
        "id": "QA2-001",
        "status": "open",
        "reason": "Actual current boundary job126430892553 fails isolated transitive consumers: bd-cache-sync -> sc-command only published sc-cron may introduce exception (raw560). API boundary step succeeds. Guard and dependency source unchanged; preserve finding."
      },
      {
        "id": "RQA10-001",
        "status": "open_new_actual_hosted_lifecycle_inventory_occurrence",
        "reason": "Actual macOS17/18exit101; inventory call/stage and trigger unresolved, own evidence needed. Canonical ID matches RustQA; preserve raw current occurrence and original scoped FTQ011 correction."
      }
    ],
    "canonical_open_ids_preserved": [
      "REQ-QA-004",
      "ARCH-001",
      "RBQA-F001",
      "QA2-001",
      "RQA9-001",
      "RQA10-001"
    ],
    "qualification_limits": [
      "No new underlying RQA9 mechanism claimed; planted case is control, not historical reproduction.",
      "No healthy-kernel/same-UID impossible guarantee; prior C2FTQ006 retraction retained.",
      "No whole1171/performance/repeated lifecycle18/App7/publicprobe matrix executed by this reviewer.",
      "Whole sprint101 requirements/34deliverables/anchors/CI acceptance belongs to requirements owner and sole enforcer; no inferred29/34 or forced completion percentage."
    ],
    "actions": "Read-only source/diff/scoped risk/evidence review; wrote only this result pair. No source/triage edits, Cargo/build/test execution, agents, PR posting, merge, auth/SDK/publisher/token work. Prior investigation files unchanged.",
    "active_owned_commands": 0,
    "evidence_files": [
      {
        "path": "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-input.json",
        "bytes": 16206,
        "sha256": "6ec01d0fe1fe20ecaf0261e6d183513ab684f922d490e5e943205eddd654a465"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-9/phase-protocol-result.json",
        "bytes": 26206,
        "sha256": "6e8342115348d44640b3cf54e54088244e007301fd19580bb816c0b34f034a83"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-commands.json",
        "bytes": 2628,
        "sha256": "2157cde475d006bfcbb70776a8656d96cf537198d289cc92dc71fd0514260b28"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-store.log",
        "bytes": 26508,
        "sha256": "1efa0814d4f05d70b9c6a3c11ce69c93abd03b6df02125883fde29e5067d0588"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-direct-negative.json",
        "bytes": 8472,
        "sha256": "89071e8eacc2888352d90026fed3a210f4479880ea175fafb074ec0e81380eed"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-direct-negative.log",
        "bytes": 8096,
        "sha256": "b7bbb5fb7063c46c6ef2085f85a216d5a70be7cd0d6ea748740e7eb11db1538c"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-9/rust-qa-hosted-c2-windows.log",
        "bytes": 168109,
        "sha256": "9353fe77341ad929becd2f0e923c2ec02c369afa93fa66a275a5c4d4d3acbc4b"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-hosted-macos.log",
        "bytes": 209785,
        "sha256": "4069ae433235e2214dea82cab62edb8026a2d5168a8d9d45b3056d03dc02993c"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-hosted-boundary.log",
        "bytes": 87281,
        "sha256": "dd5fb0d5f10a9d3912e6e9568338388f24c15ced5ec76f384f27cd2ad06db675"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-current-run.json",
        "bytes": 12632,
        "sha256": "7dd3cd6d8323e51f2a73e3231217cbb5a49a59bbfca221fe0fb51a07c0c6ef2e"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-windows-job-final.json",
        "bytes": 3793,
        "sha256": "fd2941fb563abf550cfecedc6fe6bacb2bf08f979e96c448a411fc167cd1dfd1"
      }
    ],
    "timestamp_utc": "2026-11-04T06:50:24.660132+00:00",
    "provisional_history": {
      "artifacts": [
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-provisional-result.json",
        "/tmp/phase-c-execution/c-1/qa-10/flaky-test-qa-provisional-result.md"
      ],
      "reason": "Preserved pre-hosted-failure assessment; superseded by this current fullQA10 result after independently reading new macOS raw failure."
    },
    "hosted_current_evidence": {
      "run_id": 41786865484,
      "head_source": "8c1aefafd7fd269c82c7974e610e848816d026d8",
      "macOS": {
        "job_id": 126430892802,
        "result": "FAILURE",
        "raw_log": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-hosted-macos.log",
        "actual_checkout": "9033fad7d2b7da7c3b2a3154f177eda344d0acf9 synthetic PR merge of8c1aefa into b731c09e",
        "checkout_limits": "Actual raw checkout is synthetic merge, not literal head. Synthetic object not locally present and no hosted blob equality asserted. Local8c1aefa already consumes b731c09e; current run is associated with exact candidate head.",
        "store": "37passed0failed fullstore, including all new protocol/owner controls (raw1278-1299).",
        "lifecycle": "cron_hurry17passed1failed; failure raw1557/1572-1583.",
        "causal_source_map": "Preliminary recovered-drain message establishes successful recovery from injected first census error. Test then calls more inventory via finish_using and, after successful cleanup assertions if reached, a separate250ms session_members audit at678. Same owned-inventory timeout string can arise in either path; no stage-specific receipt available.",
        "source_invariant": "process_ids_using passes caller absolute deadline to owned Inventory.finish; old independent50ms cap absent. Cleanup still attempts actual root kill/wait before final drain and never sets positive receipt on retained error. Returned io::Error alone does not prove which cleanup/audit obligation failed."
      },
      "Linux": {
        "job_id": 126430892843,
        "result": "SUCCESS",
        "evidence": "rust-qa-current-run.json snapshot"
      },
      "Windows": {
        "job_id": 126430893018,
        "result": "COMPLETED SUCCESS",
        "completed_at": "2026-11-04T06:46:35Z",
        "Test": "SUCCESS",
        "Latency": "SUCCESS",
        "evidence": "rust-qa-windows-job-final.json"
      },
      "boundary": {
        "job_id": 126430892553,
        "result": "FAILURE",
        "raw_log": "/tmp/phase-c-execution/c-1/qa-10/rust-qa-hosted-boundary.log",
        "line": 560,
        "cause": "bd-cache-sync -> sc-command: only published sc-cron may introduce contract/logging exception"
      },
      "qualification": "No cause inferred from a platform job conclusion; actual macOS occurrence and boundary text read independently. Current core success on twoOS cannot waive oneOS or graph failure."
    }
  },
  "error": null
}
```
