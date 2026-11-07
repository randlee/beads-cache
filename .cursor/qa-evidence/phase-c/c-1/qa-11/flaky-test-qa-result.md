```json
{
  "success": true,
  "data": {
    "status": "findings",
    "reviewed_branch": "sprint/c-1-runtime-migration",
    "reviewed_commit": "d8242dbe6e246a7c69a2d9a892451c3080364050",
    "review_mode": "sprint_review",
    "round_limit": false,
    "qa_pass": "qa-11",
    "model": "actual gpt-6.1-sol high fallback; configured Grok/Composer unavailable",
    "exact_input_branch_HEAD_clean_verified_before_and_after": true,
    "cm_context_executed": true,
    "scope": {
      "input": "/tmp/phase-c-execution/c-1/qa-11/flaky-test-qa-input.json",
      "full_targets_count": 93,
      "method": "Formal full rendered scope reviewed using full changed-body/source reads, current risk scan/contextual reads, preserved independent QA10/owner evidence and independently recomputed current-source equivalence. Not limited to changed-file hint.",
      "current_pattern_scan_matches": 172,
      "deleted_targets": "Listed localcrates/sc-cron targets are intentionally deleted by reviewed published-scheduler migration. Root Cargo pins publishedsc-cron0.1.1. Not missing executable review input.",
      "own_execution": "Source/evidence only; no Cargo/test/binary executed by this reviewer. Independent RustQA native proof assessed as evidence."
    },
    "findings": [
      {
        "id": "RQA9-001",
        "severity": "important",
        "file": "crates/bd-cache-store/src/schedule.rs",
        "line": 1151,
        "test": "shutdown_fixture_supervisor_kills_and_reaps_each_stalled_cleanup",
        "mechanism": "timing_assertion",
        "issue": "Original required Windows store negative qualification remains open; a new exact-head occurrence is now localized. Busy child variable database setup/contender preparation consumed nearly its entire absolute5s lifetime; worker began at4.975s and cancellation was installed at5.480s, so the owned timeout/reap occurred before required real-return/stall/500ms qualification. No underlying correction delivered.",
        "recommendation": "Route the now-observed fixture setup versus qualification lifetime mechanism to developer for a concrete deterministic fixture design/coverage ledger under the original complete5s ownership,30ms cancellation and500ms observation. Do not reset/start a fresh5s budget after setup, pad timers, skip receipt assertions, detach ownership or rerun unchanged for green. Preserve original failure and current phase trace. No product SQLite/host scheduling mechanism is established.",
        "evidence": "Exactd824 nativeWindows job126474512170 rawparent-current-hosted-windows.log1308-1356 contains actual busyPID5192 protocol. Mainclock open-return1.934422s,view-return3.699318s,setupcomplete4.785357s,contenderacquired4.974569s,workerstart4.975143s,actualwaitOk5.479958s,canceldeadline5.510357s installed5.480373s. Ownerreported5.493828s,rootcleanedtrue; missingreturn/stall/500msreceipts;store36/37exit101. Source is unchanged validatedprotocol/owner.",
        "limits": "This actual occurrence proves worker entry and locates setup/budget exhaustion. It does not prove each earlier uninstrumented Windows failure had identical phase, or explain low-level setup/observer costs. Childmonotonic phases and ownerreceipts have differentorigins; source reports separatepre-spawn offsets. Root wait/reap remains credited; no orphan or production cancellation regression inferred.",
        "correlation": "Same actual occurrence as independent rust-qa-agent RQA9-001; retain this ID, not a duplicate FTQ ID.",
        "original_line": 1084,
        "status": "open",
        "new_finding": false
      },
      {
        "id": "RQA11-001",
        "severity": "important",
        "category": "tests",
        "file": "crates/bd-cache-sync/tests/common/session_fixture.rs",
        "line": 723,
        "test": "session_fixture::inventory_delay_uses_the_existing_preflight_budget_and_reaps_its_command",
        "mechanism": "timing_assertion",
        "status": "open_new_actual_hosted_delayed_inventory_positive_occurrence",
        "new_finding": true,
        "issue": "Actual exact-head macOS required CI fails the retained separate delayed-inventory positive. Its owned command exits0 and is waited, but the caller250ms output-stage check finds the deadline exhausted, so cron_hurry reports18passed1failed/exit101. The corrected transient-recovery case passes; this is a distinct actual failed qualification.",
        "recommendation": "Preserve the failed raw command/deadline receipt and route this separate delayed-positive proof to its owner. Investigate or redesign the removed50ms-cap regression with explicit causal readiness/clock/test seams and an old/new coverage ledger, retaining a meaningful delayed-command success proof, complete actual inventory, original250ms caller budget and actual wait/reap. Do not accept arbitrary timeout as success, widen timers, retry-to-green, guess hostload or remove the delayed obligation under the RQA10 resolution.",
        "evidence": "Exactd824 run41800238075 macOSjob126474511866 parent-current-hosted-macos.log1562/1575-1586: pid12479 spawn250.75us;RQA10_COMMAND_EXIT command_elapsed257.978625ms actual_waittrue exit0 remaining0;Errsessioninventoryoutputbudgetexhausted;18/19exit101. Source723-751 stillrequiresactualSTOPack/80ms/CONT,fullparsedinventory,selfpresence,<250ms andPIDabsence. This occurrence returns atprocess_ids_using? before finaldelay/PID assertions.",
        "limits": "The successful exit was observed after deadline; exact OS exit time, actual STOP/sleep/CONT elapsed and output-read cost are not all separately recorded. Do not infer ps itself executed for257ms, a kernel/hostload cause, missing reap or reinstated50ms cap. Source-backed try_wait/Drop waits support ownedcommandreap; finalPIDabsence assertion was not reached. Same canonicalID as independentRustQA; no duplicate."
      }
    ],
    "summary": {
      "total_findings": 2,
      "critical": 0,
      "important": 2,
      "minor": 0,
      "new_findings": 1,
      "resolved_findings_this_review": [
        "RQA10-001",
        "CI-C2-RUNNER-001"
      ],
      "sprint_verdict": "FAIL: existing WindowsRQA9 now has a localized current occurrence; new separate macOS delayed-inventory positive RQA11-001 remainsopen. RQA10 composition and sharedrunner correction resolved in exactC1scope. Current15checks12SUCCESS3FAILURE; graph/SDK/requiredCI gaps remain separately unwaived."
    },
    "RQA10_owner_resolution": {
      "id": "RQA10-001",
      "status": "resolved_verified_for_exact_C1_candidate",
      "owner": "flaky-test-qa",
      "reviewed_commit": "d8242dbe6e246a7c69a2d9a892451c3080364050",
      "owning_adjudication_complete": true,
      "canonical_disposition": "RESOLVED for the supported mixed recovery/artificial-delay fixture composition; sole manager aggregates this actual owner disposition. No pending-owning-reviewer condition remains.",
      "supported_mechanism": "Actual instrumented consumer observation7fb/run41796715401/job126462431901 reached quiescence call3 afterSTOP/80ms/CONT, timed out its actual inventory under original250ms window, reaped command/root, completed final drain and correctly retained false cleanup. Positive recovery mixed transient-error recovery and additional artificially delayed inventory in one two-stable-census window. Current split removes that extra positive timing assumption without relaxing deadlines or census proof.",
      "why_not_pass_only": "Direct source diff changes only two existing private test bodies and independently verified old/new ledger retains every intended proof. Independent current native full19 evidence supplies real scan/STOP/CONT/wait/error/receipt/census outcomes, including required negative controls. This satisfies the supported correction direction; no identical historical recurrence or low-level host cause is required.",
      "scope_limits": "Only exact C1 candidate fixture-composition mechanism. This does not identify all historical uninstrumented stages or detailed ps/signal/observer costs, guarantee kernel scheduling, qualify wholeC2, or claim C2 consumed this helper. Original failure logs and distinct RQA9/CI/SDK findings remain.",
      "further_code_or_unchanged_observation_required_for_this_mechanism": false,
      "additional_current_hosted_evidence": "Exactd824 macOS raw1558 reports corrected recovery-positivePASS. Separate retained delayed-positive fails and is newRQA11-001; source-split mechanism remains resolved. Resolution does not claim wholeinventoryfamily/currentplatformgreen or waive that separate failure."
    },
    "coverage_ledger_independently_assessed": [
      {
        "obligation": "Transient recovery positive",
        "old": "First transient error plus global call3 STOP/80ms/CONT under same quiescence deadline.",
        "current": "First transient error preserved; all later callbacks perform real fresh inventory.>=5scans, two stable quiet censuses, original failed root status/output, actual rootwait, final empty census, independent250ms audit and rootabsence still required. Any timeout propagates as failure.",
        "actual": "native raw30-81 session45808: injectedErr; successful initial2,quiescent3+4,rootwaitexit101 at95.130209ms,final5,cleanuptrue121.399792ms,auditOk0 at26.961917ms."
      },
      {
        "obligation": "Delayed real owned inventory",
        "old": "Duplicated in recovery and independent existing250ms preflight regression.",
        "current": "Existing separate regression retains acknowledged real STOP,requested80ms thenCONT,complete real inventory and owningprocess presence,actual wait/reap and ESRCH; explicit measured delay>=80ms strengthened. Still must complete under original250ms.",
        "actual": "native raw62 pid45798 STOPacktrue,requested80ms,actual87.911459ms,CONTreturnedOk,actualwait/reap/PIDabsence;total126.580083ms. Intentional delay is adversarial input, not sleep-based readiness synchronization; STOPack is causal.",
        "current_hosted_limit": "Exactd824 macOS separate delayed-positiveFAIL with outputbudgetexhausted; newRQA11-001 retains this live success obligation. Native87.911459ms delay/126.580083ms completion is credited as oneactual meaningful proof, not universal scheduling guarantee."
      },
      {
        "obligation": "Incomplete quiescence rejected",
        "current": "Byte-identical real held-command control; quiescence timeout retains false receipt and original failed fixture,rootwait/finalindependentcensus; secondcleanup refused without newbudget.",
        "actual": "native raw108 post_auditfalse,actual_root_waittrue,held_inventory_reapedtrue,completeindependentcensustrue,receiptfalse,originalfailedtrue beforefallback."
      },
      {
        "obligation": "Independent audit timeout distinct",
        "current": "Byte-identical real held audit after successful actual cleanup; audit error cannot become empty success and does not rewrite prior receipt.",
        "actual": "native raw156/159 post_audittrue;actual owned commandtimeout+wait,auditErr,priorreceipttrue,originalfailedrootretained,complete separate verification beforefallback."
      },
      {
        "obligation": "Noncooperation/one attempt/unwind/preregistration",
        "current": "Implementations and controls unchanged; original100ms initialdrain/250ms absolutequiescence/1s cleanup/250ms audit/5s fixture,root/sessionownership and false incomplete receipts retained. No retries/reset/reservation/threadoverride.",
        "actual": "native full19 includes three noncooperative inventory command reaps/false receipt, stopped command timeout/unwind, empty kernel session preregistration timeout/unwind,actual PythonPID reaps and stalled-worker/management/second-start negatives. Source-backed real waits; no syscall audit or impossible kernelprogress guarantee."
      }
    ],
    "source_equivalence": {
      "own_receipt": "/tmp/phase-c-execution/c-1/qa-11/flaky-test-qa-source-receipt.json",
      "all159_src_rs_equal_ec68": true,
      "helper_current_blob": "d8db8c954d53eb33aa629e409328cfd8a6e53685",
      "helper_outside_two_reviewed_bodies_equal_ec68": true,
      "only_Rust_path_changed_since_ec68": "crates/bd-cache-sync/tests/common/session_fixture.rs",
      "test_count_added": 0,
      "test_count_deleted": 0,
      "unchanged_wholebytes": [
        "crates/bd-cache-store/tests/support/fixture_child.rs",
        "crates/bd-cache-store/tests/support/schedule_protocol.rs",
        "crates/bd-cache-sync/tests/cron_hurry.rs",
        "crates/bd-cache-sync/tests/check_consumer_graph.py",
        "Cargo.toml",
        "Cargo.lock",
        ".github/workflows/ci.yml"
      ],
      "workflow_exact_three_label_replacements": [
        40,
        71,
        113
      ],
      "limits": "No wholehelper-file equality claimed; two existing test bodies changed. Whole159src.rs comparison usesec68, not a generic claim all historical source is unchanged."
    },
    "independent_Rust_evidence": {
      "final_fence": "/tmp/phase-c-execution/c-1/qa-11/rust-qa-agent-result.json",
      "native_receipt": "/tmp/phase-c-execution/c-1/qa-11/rust-qa-native-test-receipt.json",
      "native_log": "/tmp/phase-c-execution/c-1/qa-11/rust-qa-tests-native.log",
      "command": "cargo test --locked -p bd-cache-sync --test cron_hurry --all-features -- --nocapture",
      "result": "19passed0failed0ignored0filtered;15.51s,exit0",
      "environment": "CARGO_INCREMENTAL=0; existing sharedtarget; RUST_TEST_THREADS unset/default. Native authorized process inventory visibility.",
      "initial_environment_failure_retained": {
        "path": "/tmp/phase-c-execution/c-1/qa-11/rust-qa-tests.log",
        "result": "10pass9fail,exit101; raw /bin/ps Operationnotpermitted and PermissionDenied",
        "disposition": "Not a valid native process-inventory qualification and not silently converted to a pass. Separate completed native command provides required proof. No source/timer/guard relaxation or claimed fixture rootcause from EPERM."
      },
      "checks": "Independent fmt and strict sync alltarget/allfeature Clippy pass; producer proof secondary. Original runtime failures and current exacthead graph logs retained.",
      "owner_pending_language": "RustQA properly deferred RQA10 to this owner; this completed independent owner adjudication supersedes that procedural pending-owner text for this scoped mechanism. No circular wait on manager or another reviewer."
    },
    "runner_assessment": {
      "configuration": "Exactly three supportedarm64macos15 label replacements, every otherC1workflow byte retained,11jobs=3Rust+2wheel+6pytest,Python3.9/3.11/3.13,default native wheel target and matching matrix upload/download wheel-${{matrix.os}} preserved. C1-specific commands not overwritten byC2workflow.",
      "ownership": "Currentconsumerworkflowownedpaths/C3 contract permitslocal3labels; no regeneration,pin,answers,adoptionmap,fingerprint or upstreamwork. Existingparent singlewriter coordination preserved.",
      "remaining_missing_evidence": "None for prescribedC1runner/wheel/Pythoncompatibilityscope: all11PythonchecksSUCCESS plus raw4runnerproof logs independentlyread. Separate currentnativeCI failures remain.",
      "official_support": "Prior independent owner primaryretirement/runnerimages evidence establishes retiredmacos14 and supportedarm64macos15. Someoldlabeljobs executed, so exactoldqueuecause remains unproven.",
      "actual_raw_proof": {
        "map": "/tmp/phase-c-execution/c-1/qa-11/runner-proof-jobs.json",
        "workflow_run": 41800237943,
        "macos_wheel": {
          "job": 126474418830,
          "image": "macos-15-arm64 line17",
          "toolchain": "1.98.1-aarch64-apple-darwin lines119/127",
          "Python": "3.12.10/arm64 line214",
          "wheel": "bd_cache_rs-0.2.0-cp39-abi3-macosx_11_0_arm64.whl line543"
        },
        "installed_pytest": [
          {
            "python": "3.9.13/arm64",
            "job": 126477230329,
            "install_line": 250,
            "result_line": 305,
            "result": "226passed65.89s"
          },
          {
            "python": "3.11.9/arm64",
            "job": 126477230590,
            "install_line": 204,
            "result_line": 262,
            "result": "226passed68.50s"
          },
          {
            "python": "3.13.17/arm64",
            "job": 126477230372,
            "install_line": 204,
            "result_line": 262,
            "result": "226passed59.24s"
          }
        ],
        "checkout": "Allrawrunnerproofs show synthetic268476b merge ofd824 intof1; source associationverified. No literalheadhostedcheckout or independentlyfetchedsyntheticblob equalityclaimed.",
        "qualification": "Original11jobs,Pythonversions,arm64wheel/installedtests andmatchingartifactwiringpreserved. NoqueuecauseorwholeCIclaim."
      }
    },
    "hosted_evidence": {
      "source": "d8242dbe6e246a7c69a2d9a892451c3080364050",
      "native_run": 41800238075,
      "python_run": 41800237943,
      "reviewed_snapshots": [
        "/tmp/phase-c-execution/c-1/qa-11/rust-qa-native-ci.json",
        "/tmp/phase-c-execution/c-1/qa-11/rust-qa-python-ci.json",
        "/tmp/phase-c-execution/c-1/qa-11/parent-current-pr108-ci.json"
      ],
      "snapshot_status": "Superseded by authoritative later exactheadterminalreceipt:15checks12SUCCESS3FAILURE. All11PythonSUCCESS,nativeLinuxSUCCESS;nativeboundary/macOS/WindowsFAIL.",
      "actual_boundary_raw": "/tmp/phase-c-execution/c-1/qa-11/rust-qa-hosted-boundary-raw.log",
      "actual_boundary_line": 561,
      "actual_boundary": "bd-cache-sync->sc-command onlypublishedsc-cronmayintroduceexception,exit1. Separate current required graph failure.",
      "once_only_refresh": "Attempted read-only gh metadata refresh for both existing runs; both failed connectingapi.github.com. No networkescalation,retry,jobexecution or inferred new status. Existing raw snapshots used with explicit limits.",
      "terminal_receipt": "/tmp/phase-c-execution/c-1/qa-11/parent-current-pr108-ci-later.json",
      "terminal_native_failures": {
        "macOS": {
          "job": 126474511866,
          "test": "session_fixture::inventory_delay_uses_the_existing_preflight_budget_and_reaps_its_command",
          "group": "cron_hurry18/19exit101",
          "raw": "/tmp/phase-c-execution/c-1/qa-11/parent-current-hosted-macos.log"
        },
        "Windows": {
          "job": 126474512170,
          "test": "shutdown_fixture_supervisor_kills_and_reaps_each_stalled_cleanup",
          "group": "store36/37exit101",
          "raw": "/tmp/phase-c-execution/c-1/qa-11/parent-current-hosted-windows.log"
        },
        "boundary": {
          "job": 126474512121,
          "cause": "Actualdirectsync->sc-commandisolatedgraphguardfail"
        }
      }
    },
    "dispositions": [
      {
        "id": "RQA10-001",
        "status": "resolved_verified_for_exact_C1_candidate",
        "owner": "flaky-test-qa",
        "reviewed_commit": "d8242dbe6e246a7c69a2d9a892451c3080364050",
        "owning_adjudication_complete": true,
        "canonical_disposition": "RESOLVED for the supported mixed recovery/artificial-delay fixture composition; sole manager aggregates this actual owner disposition. No pending-owning-reviewer condition remains.",
        "supported_mechanism": "Actual instrumented consumer observation7fb/run41796715401/job126462431901 reached quiescence call3 afterSTOP/80ms/CONT, timed out its actual inventory under original250ms window, reaped command/root, completed final drain and correctly retained false cleanup. Positive recovery mixed transient-error recovery and additional artificially delayed inventory in one two-stable-census window. Current split removes that extra positive timing assumption without relaxing deadlines or census proof.",
        "why_not_pass_only": "Direct source diff changes only two existing private test bodies and independently verified old/new ledger retains every intended proof. Independent current native full19 evidence supplies real scan/STOP/CONT/wait/error/receipt/census outcomes, including required negative controls. This satisfies the supported correction direction; no identical historical recurrence or low-level host cause is required.",
        "scope_limits": "Only exact C1 candidate fixture-composition mechanism. This does not identify all historical uninstrumented stages or detailed ps/signal/observer costs, guarantee kernel scheduling, qualify wholeC2, or claim C2 consumed this helper. Original failure logs and distinct RQA9/CI/SDK findings remain.",
        "further_code_or_unchanged_observation_required_for_this_mechanism": false
      },
      {
        "id": "RQA9-001",
        "status": "open_current_occurrence_localized_setup_vs_absolute_lifetime",
        "reason": "CurrentWindowsactualstructuredreceiptshowslatebusysetupandcancelafterownerdeadline;originalhistoricalunderlyingcostnotallknown. Workerentry/reapcredited;no correctiondelivered."
      },
      {
        "id": "CI-C2-RUNNER-001",
        "status": "resolved_verified_for_exact_C1_candidate",
        "owner": "flaky-test-qa",
        "reviewed_commit": "d8242dbe6e246a7c69a2d9a892451c3080364050",
        "owning_adjudication_complete": true,
        "reason": "Exactthree-labelsourcecorrection plus direct actualsupportedmacOS15arm64/toolchain/wheel/installedPython3version receipts complete prescribedC1qualification; no pendingownerplaceholder. All11PythonchecksSUCCESS.",
        "scope_limits": "C1d824 candidate only; does not assert C2currentheadindependentmatrixacceptance, exactoldqueuecause, or overallCI/signal/SDKacceptance. Originalretiredconfigurationandqueuecounterevidencepreserved."
      },
      {
        "id": "FTQ-009",
        "status": "closed_preserved_for_original_unbounded_lifetime",
        "reason": "Exactlyfilteredwholefixtureowner,actualrootwait/reap and unchanged protocol/FixtureChild retain prior acceptedmechanism; RQA9 separate."
      },
      {
        "id": "FTQ-010",
        "status": "closed_preserved_for_original_session_orphan_inventory_mechanisms",
        "reason": "Cleanupimplementation unchanged;full19independent actual timeout/unwind/preregistration/Pythonownership controls qualified; no speculativekernelprogress finding."
      },
      {
        "id": "FTQ-011",
        "status": "closed_preserved_for_independent50ms_cap_mechanism",
        "reason": "Callerabsoluteinventorydeadline unchanged;separate actualdelayedSTOP80CONTregression strengthened and passes; fullcensus/actualwait/falseincompletereceipts/oneattempt unchanged. RQA10 supportedcomposition correction separately resolved."
      },
      {
        "id": "RSH-001",
        "status": "resolved_preserved_in_prior_supported_C1_and_paired_public_C2_scope",
        "reason": "All159src.rs exactlyec68;prioractualownedstartup/publiccaller250.876416mswithin500ms and service7scope retained. No newstockHTTP/wholeC2 or universaldeadlineclaim."
      },
      {
        "id": "ARCH-001",
        "status": "open",
        "reason": "Publishedcanonicalreceivercomposition/directsyncsc-commandgap unchanged;architectureownerretainsownseverity."
      },
      {
        "id": "RBQA-F001",
        "status": "open",
        "reason": "Distinctcarriedboundary/publicreceiverfinding unchanged;no waiver or newRBP review."
      },
      {
        "id": "QA2-001",
        "status": "open",
        "reason": "Actual current isolatedgraph failure independentlyread in rawboundarylog561; production/manifest/guard unchanged."
      },
      {
        "id": "REQ-QA-004",
        "status": "open",
        "reason": "Actualcurrent12/15SUCCESS includesall11Python butnativeboundary/macOS/WindowsFAIL. No requiredCI/SDK/sprintacceptance; no forcedrequirementscompletioncount."
      },
      {
        "id": "C2-FTQ-006",
        "status": "retracted_preserved",
        "reason": "No impossiblehealthy-kernel/sameUIDprogressfindingrevived."
      },
      {
        "id": "RQA11-001",
        "status": "open_new_actual_macOS_delayed_inventory_positive_failure",
        "reason": "DistinctexistingtestobligationfailscurrentrequiredCI;samemacraw/canonicalIDasRustQA;correctedRQA10recoveryitselfpasses."
      }
    ],
    "limits": [
      "RQA10 resolution covers removed extra artificial-delay composition and preserved proof obligations, not every historical low-level latency stage. All historical failed runs/raw diagnostics preserved.",
      "No initial sandboxEPERM is claimed as a natural codefailure or skipped from evidence; separate meaningful native environment proof is used.",
      "Livecompleteinventories must still meet unchangeddeadlines. Delayedpositive still fails honestly if it cannot finish; no paddedtimeout,retry,reservation,forcedload,clockwaiver or minimumsuccessmargin.",
      "Root wait and independentcensus receipts are source-backed actual control evidence, not external syscalldumps. Intentional negative child status remains failed.",
      "Formal leaf review only; no fullsprintacceptance,101requirements/34deliverablepercentage,anchor/manager/mergeverdict inferred. OthercurrentC2signal/endpoint/securityhistory remains separatelyowned.",
      "LaterauthoritativeterminalCI/rawproofsupersedesinitialsnapshotsandfailedrefresh. C1runnerresolutionisnowcomplete; RQA10scopedcorrectioncreditdoesnotwaivenewRQA11separatetestfailure orcurrentRQA9."
    ],
    "actions": "Read-only source/diff/risk/evidence review and bounded failedCI metadata reads. Wrote only own source/result receipts underqa11. No source/triage edits,Cargo/build/tests/rerun,binaryexecution,agents,PRreport/merge or SDK/publisher/auth/tokenwork. STOP for solecursor-quality-mgr handoff.",
    "active_owned_commands": 0,
    "state": "STOP",
    "evidence_files": [
      {
        "path": "/tmp/phase-c-execution/c-1/qa-10/mac-observation-runner-correction/observation-owner-assessment.json",
        "bytes": 18083,
        "sha256": "e3c482b2d0e7a87429663b4e3261ac8449d984f38f7954988dc4652fb063f685"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-10/recovery-composition-result.json",
        "bytes": 10779,
        "sha256": "4b1fa4d11bc7873d1d5c3f501640575262a578e5b6398f61bf820df11e4acdef"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-10/shared-python-runner-fix-result.json",
        "bytes": 11090,
        "sha256": "c93c001bc198af5399ce829a0535589a3cb4df6a84be4a00ee06a554abe90b91"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/flaky-test-qa-input.json",
        "bytes": 21154,
        "sha256": "a4413dd07cabf84bd6be22dd1610bf867f473bf7b7f780845a249aa83bcb307d"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/flaky-test-qa-source-receipt.json",
        "bytes": 1733,
        "sha256": "2913f512fb2baaefbc828f0c9b42a7b15752c3634d31ab90fb5b5657889675a1"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/parent-current-hosted-macos.log",
        "bytes": 140992,
        "sha256": "3dab9ded0ca461c57856526e674d81fd94dfaabb2c4b07dc0a7f96a3e9ca03b4"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/parent-current-hosted-windows.log",
        "bytes": 129443,
        "sha256": "a7791c07633ca09f39c7458a215a0ec8b75a397d74187618f6638f5494fbc1e5"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/parent-current-pr108-ci-later.json",
        "bytes": 4588,
        "sha256": "c098f35ef3279e5ad2dabfec29345353770b879c340ed9f8a39e3e3bb8e87338"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/runner-proof-job-126474418830.log",
        "bytes": 60644,
        "sha256": "680964c3115ef166f3c32fa1bb6b519ea1bcfa2f04667606f373c29e9af5a7d7"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/runner-proof-job-126477230329.log",
        "bytes": 27703,
        "sha256": "a35f7163596cc591cbac9e9d89d8ee999feb298694ae8a0ba6ea1ee872a807e4"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/runner-proof-job-126477230372.log",
        "bytes": 24044,
        "sha256": "0e47374ef2362587cdbac7ffa442256141211189332c67c0e60d06a53da1d055"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/runner-proof-job-126477230590.log",
        "bytes": 24022,
        "sha256": "49aa7b72a996f8fa1aba22b64b2cf50b7ac429cf4e96d30e36963c1b45d9aa0f"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/runner-proof-jobs.json",
        "bytes": 648,
        "sha256": "9ddcb0c1927ff550d824de67c05319fa81187052c8031360f73592f7f3423874"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/rust-qa-agent-result.json",
        "bytes": 22424,
        "sha256": "28070142c23f21de9ee7968c6d2103b52200219f2b6ef4d16ba2f037bef961b2"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/rust-qa-hosted-boundary-raw.log",
        "bytes": 44661,
        "sha256": "a4f9119ebe1acc084eb3816178eaa1d1aedf25e89013820b061b50394f0decad"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/rust-qa-native-ci.json",
        "bytes": 9673,
        "sha256": "5a35011602e7cd51616cf11f6a71aa3eb7df0e48276b66c8619e5eda7c6bc19b"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/rust-qa-native-test-receipt.json",
        "bytes": 294,
        "sha256": "f39c16cf3c3ac253aad61f002dee829493b5ad02f31cf8560f4ea4cd1b501a64"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/rust-qa-python-ci.json",
        "bytes": 11269,
        "sha256": "0a929069586f4d76622295d9153ebab5f18e35895054cfaec3602299d4c53060"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/rust-qa-source-receipt.json",
        "bytes": 2008,
        "sha256": "609e752a7965028d93862a11aaff508207f3a47f0ea65e587a1525777cfbf378"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/rust-qa-tests-native.log",
        "bytes": 39938,
        "sha256": "c00b1fda0ef9fca65833242788b8dc1e1d75cacb294faabcabc870d3dfaecd88"
      },
      {
        "path": "/tmp/phase-c-execution/c-1/qa-11/rust-qa-tests.log",
        "bytes": 4917,
        "sha256": "588f821c01c0890f070789e0acc3ba14f88b66b031a23705b8d1a45470390a60"
      }
    ],
    "timestamp_utc": "2026-11-04T08:52:50.623188+00:00",
    "provisional_result_preserved": {
      "json": "/tmp/phase-c-execution/c-1/qa-11/flaky-test-qa-result-provisional-before-terminal.json",
      "markdown": "/tmp/phase-c-execution/c-1/qa-11/flaky-test-qa-result-provisional-before-terminal.md",
      "reason": "Superseded pre-terminal snapshots; preserved before current failures and rawrunnerproof were supplied."
    },
    "runner_owner_resolution": {
      "id": "CI-C2-RUNNER-001",
      "status": "resolved_verified_for_exact_C1_candidate",
      "owner": "flaky-test-qa",
      "reviewed_commit": "d8242dbe6e246a7c69a2d9a892451c3080364050",
      "owning_adjudication_complete": true,
      "reason": "Exactthree-labelsourcecorrection plus direct actualsupportedmacOS15arm64/toolchain/wheel/installedPython3version receipts complete prescribedC1qualification; no pendingownerplaceholder. All11PythonchecksSUCCESS.",
      "scope_limits": "C1d824 candidate only; does not assert C2currentheadindependentmatrixacceptance, exactoldqueuecause, or overallCI/signal/SDKacceptance. Originalretiredconfigurationandqueuecounterevidencepreserved."
    },
    "RQA9_current_phase_assessment": {
      "packet": {
        "absolute_lifetime_ns": 5000000000,
        "classification": "worker_return_unobserved_phase_unresolved",
        "clock_limits": "receive is not execution; wall correlation may move; all deadlines use original owner Instant",
        "mode": "stalled",
        "outer_to_owner_ns": 839200,
        "owner_origin": "Instant before Command.spawn",
        "owner_reported_ns": 5493828300,
        "owner_unix_ns": 1793781525817593900,
        "receipts": [
          {
            "execution": {
              "child_elapsed_ns": 31000,
              "data": null,
              "phase": "fixture-body-entered",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 6509400
            },
            "owner_received_ns": 7790400
          },
          {
            "execution": {
              "child_elapsed_ns": 480500,
              "data": null,
              "phase": "scratch-create-start",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 6968900
            },
            "owner_received_ns": 7817900
          },
          {
            "execution": {
              "child_elapsed_ns": 1043800,
              "data": null,
              "phase": "store-open-start",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 7537900
            },
            "owner_received_ns": 13590900
          },
          {
            "execution": {
              "child_elapsed_ns": 1934422000,
              "data": null,
              "phase": "store-open-returned",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 1940909800
            },
            "owner_received_ns": 2872065300
          },
          {
            "execution": {
              "child_elapsed_ns": 1934740300,
              "data": null,
              "phase": "schedule-view-start",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 1941233000
            },
            "owner_received_ns": 2872085000
          },
          {
            "execution": {
              "child_elapsed_ns": 3699317800,
              "data": null,
              "phase": "schedule-view-returned",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 3705806100
            },
            "owner_received_ns": 3708223200
          },
          {
            "execution": {
              "child_elapsed_ns": 3699651000,
              "data": null,
              "phase": "schedule-create-start",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 3706142400
            },
            "owner_received_ns": 3708237200
          },
          {
            "execution": {
              "child_elapsed_ns": 4785357200,
              "data": null,
              "phase": "schedule-setup-complete",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 4791839600
            },
            "owner_received_ns": 4792901500
          },
          {
            "execution": {
              "child_elapsed_ns": 4785693500,
              "data": null,
              "phase": "contender-open-start",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 4792183300
            },
            "owner_received_ns": 4792918300
          },
          {
            "execution": {
              "child_elapsed_ns": 4974038100,
              "data": null,
              "phase": "contender-lock-start",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 4980522000
            },
            "owner_received_ns": 5486362300
          },
          {
            "execution": {
              "child_elapsed_ns": 4974568600,
              "data": null,
              "phase": "contender-lock-acquired",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 4981056600
            },
            "owner_received_ns": 5486385700
          },
          {
            "execution": {
              "child_elapsed_ns": 4974755000,
              "data": null,
              "phase": "worker-spawn-start",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 4981245100
            },
            "owner_received_ns": 5486403000
          },
          {
            "execution": {
              "child_elapsed_ns": 5479957600,
              "data": {
                "ok": true
              },
              "phase": "wait-result",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 5486443200
            },
            "owner_received_ns": 5493682300
          },
          {
            "execution": {
              "child_elapsed_ns": 5480372600,
              "data": {
                "child_deadline_ns": 5510356600
              },
              "phase": "cancellation-deadline-installed",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 5486862100
            },
            "owner_received_ns": 5493698900
          },
          {
            "execution": {
              "child_elapsed_ns": 5480623000,
              "data": null,
              "phase": "cancellation-installed",
              "pid": 5192,
              "stream": "main",
              "wall_since_owner_ns": 5487111800
            },
            "owner_received_ns": 5493710000
          },
          {
            "execution": {
              "child_elapsed_ns": 4975143400,
              "data": null,
              "phase": "worker-call-start",
              "pid": 5192,
              "stream": "worker",
              "wall_since_owner_ns": 4981631800
            },
            "owner_received_ns": 5486514000
          }
        ],
        "root_joined_and_cleaned": true,
        "spawn_returned_ns": 2181100,
        "test": "schedule::tests::shutdown_interrupts_busy_persistence_and_restores_the_shared_session"
      },
      "supported_mechanism": "Completefixturelifetimecontainsvariablesetupplusfixednegativeobservation. Atworkerstart4.975s onlyabout25ms remainoforiginal5s; the required500ms completiontimeout cannotfit withinthatremainingtime evenwithoutworker/cancelcost. Actualcancellationinstalled5.480s alreadybeyondownerdeadline; no new return/stallnegativequalification possiblebeforethedeadline. This is fixturebudgetpartitionevidence, not a productshutdownproof.",
      "receipt_interpretation": "Worker-call-start is directlyrecorded. Itsreturnisnotobserved; silence doesnotproveitnevercouldreturnafterkill. Ownercleanedtrue and <7s receiptpasses. Parentreceipttimeofsomephaseslagsexecution; do not inferalllagisprocessruntime.",
      "next_action": "Concretefixture-redesignreviewandpreservationledger; no additionalunchangedprobe orunsupportedproductfix. Originalabsolute5slifetimemuststillownsetupandfailurepaths; nofreshpost-readinessbudget."
    }
  },
  "error": null
}
```
