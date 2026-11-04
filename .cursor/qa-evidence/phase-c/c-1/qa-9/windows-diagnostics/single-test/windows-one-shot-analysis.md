One-shot Windows126415769693 SUCCESS, immutablee86d9da92a8c8c9b787c780e29bc54e417680d71 explicitly checked out/verified, source/lock/helper blobs match local identities. Selected store SQLite features match prior relevant workspace set. Test1/1 PASS,33filtered,15.02s.

| Mode | Wait observed ms | Cancel installed ms | Worker returned ms | Completion Timeout ms | Root kill/reap s |
|---|---:|---:|---:|---:|---:|
| read-only mutex | 50.0683 | 51.6464 | 83.2285 | 552.4373 | 5.0064097 |
| busy persistence | 65.7693 | 65.8551 | 96.6554 | 567.7216 | 5.0039503 |
| running SQL | 332.6606 | 332.7163 | 362.8003 | 837.367 | 5.0059945 |

Actual results: read-only shutdown-deadline error; busyDatabaseBusy(code5),30.8003ms after cancellation; SQL OperationInterrupted(code9). Each records original post-call stall and500ms timeout, then worker-join-start; busy also rolls back the contender. Worker join deliberately never returns in these negative modes; actual owner root kill/wait/reap is supported by unchanged source, successful receipt assertions and raw FTQ009 summaries. Do not conflate worker-thread join with root-process reap.

Actions UTC timestamps publish captured output after cleanup. Worker phases use fixture-body monotonic clock; scratch/open phases use a separate helper origin. JSON retains every raw phase, result, line and derived duration.

This occurrence only confirms the normal intended negative path. OriginalC2b7de store33/34 failure cause remains unknown; no fix or closure. Exact full-workspace test artifact differs from selected-package artifact, and the full-store parent concurrency is absent. Precise missing observation is a failing in-context phase trace.

Recommended next bounded probe, only after parent authorization: one existing full34-test store suite on immutablee86d9da with same package/features and outer--nocapture:

    cargo test --locked -p bd-cache-store --lib --all-features -- --nocapture

This removes only the supervisor filter and restores existing store libtest concurrency; no synthetic stress, repeated runs, thread-count override, deadline changes or full1171 matrix. A pass remains an observation, not cause proof. Broader workspace feature/artifact context remains separately qualified. Nothing was executed/prepared/pushed for this next probe.

All5 findings remain OPEN; source unchanged, Cargo RELEASED, zero active commands. STOP for parent-supported next action.
