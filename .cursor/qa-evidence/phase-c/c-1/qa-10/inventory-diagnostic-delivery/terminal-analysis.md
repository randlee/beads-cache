C1 macOS job 126447058941 passed with the instrumented source ec68ecf: cron_hurry 19/19, 15.63 s. Successful outer libtest capture suppressed all RQA10 stage receipts. This does not establish a historical correction.

C2 macOS job 126448419348 failed the **same recovery test**, not the store or daemon: recovered initial drain, then generic owned inventory timeout; cron_hurry 17/18, exit101 (raw lines1718-1729). Its helper blob 123a5ce3 matches 8c exactly and lacks ec68 stage diagnostics. The actual failed stage and root/census outcome remain unknown. Both jobs checked out synthetic PR merges; exact checkout IDs, source limits and raw hashes are in terminal-analysis.json.

Source analysis confirms callback 3 is phase-dependent. Its acknowledged-stop 80 ms delay has no reservation against remaining time. A late acknowledgement can exhaust a phase, but no actual late acknowledgement is evidenced here. Initial-drain failure may recover; quiescence failure clears incomplete census history and cannot be rescued by a successful final drain. The independent audit has a separate unchanged 250 ms deadline. Existing strict recovery semantics prevent incomplete evidence from qualifying; they do not locate the historical failure.

No meaningful new counterexample was found without arbitrarily forcing lateness or holding work, which would only recreate a generic timeout already covered by the owned controls. No tests/builds or source changes occurred. The useful next evidence is an actual stage/call/remaining/PID/root/census trace in the failing consumer context, routed by parent and owner; no ordinary green rerun or speculative patch is proposed.

All six findings remain OPEN. Cargo is RELEASED, zero active commands; stopped for parent routing.
