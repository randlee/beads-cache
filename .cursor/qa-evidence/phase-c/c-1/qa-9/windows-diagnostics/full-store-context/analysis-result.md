Full-store Windows126420131075 SUCCESS: actual34/34,0filtered,15.97s. Literal immutablee86d9da/source/lock/helper verified. RUST_TEST_THREADS=<unset> (line316), no outer thread override; effective numeric count not logged. Same package-feature set/named artifact13fa as prior selected probe, not a claimed binary hash or whole-workspace0736 artifact identity.

| Mode | Wait ms | Cancel ms | Return ms | Cancel→return ms | Completion timeout ms | Root kill/reap s |
|---|---:|---:|---:|---:|---:|---:|
| read-only mutex | 176.486 | 186.2565 | 217.1601 | 30.9036 | 686.5521 | 5.0067212 |
| busy persistence | 66.7376 | 66.7871 | 98.093 | 31.3059 | 581.0009 | 5.0072666 |
| running SQL | 65.9342 | 65.9814 | 96.0475 | 30.0661 | 571.5166 | 5.0061447 |

Actual worker results remain shutdown-deadline/read-only, DatabaseBusy5/busy, OperationInterrupted9/SQL. All3 original stall+500ms markers occur; main enters actual join, which deliberately never returns because worker parks. Outer owner actually kills/waits/reaps root under original5s receipt assertions. Busy rollback581.0636→581.1294ms precedes join-start581.154ms. Original normal3 fixture tests and assertion-unwind test pass; injected caught panics are expected.

Selected versus full-store: busy cancellation→return30.8003→31.3059ms; read31.5821→30.9036ms; SQL30.0840→30.0661ms. Existing parent store concurrency did not reproduce missing receipt in this occurrence. It is not disproven as an intermittent interaction or established as cause. Setup phases vary, but none exhausts original lifetime. Diagnostic timing/I/O perturbation is retained as a limit.

Originalb7de store33/34 remains valid/unexplained; no accidental substantive correction in diagnostic diff per independent owner. The successful full34 trace does not reconstruct historical failure or closeRQA9.

Specific observation gap: owner5s begins before Command.spawn, while child phase clocks begin inside body/helper. No recorded common-origin timestamp maps original child entry to remaining owner lifetime; successful local phase durations cannot establish historical spawn/startup/setup allocation. This is an accounting gap, not evidence of a delay or a kernel-progress defect. A separate remaining source question is whether a reachable whole-workspace feature/cfg delta (0736 versus13fa artifact) affects initializer/worker/observer behavior; relevantSQLite features already match. Bounded read-only reachable-delta/lifetime accounting review can guide owner adjudication; another green rerun, stress or matrix is not justified. No concrete new executable probe or patch selected.

All5 findings OPEN, original30ms/500ms/5s assertions/ownership unchanged. No source/Cargo/workflow changes or further executions; Cargo RELEASED, STOP for independent parent owner assessment. Full phases/lines/results/source identities/hashes are in analysis-result.json.
