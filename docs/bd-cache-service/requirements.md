# Service requirements

REQ-SERVICE-0001 refines REQ-BDC-1001: own one existing Composed cache and
SystemSchedules over its cache-file schedule store. Cached CLI/Python readers
remain independent local consumers; readiness never invokes bd.

REQ-SERVICE-0002 refines REQ-BDC-1001/NFR-BDC-0004: admit one health evaluation
using try-acquire and a zero-capacity rendezvous. Its one dedicated thread owns
the permit until real completion, including disconnect/timeout. Use existing
execute_api health defaults, probe=false and HealthReadControl with an absolute
one-second deadline. Busy/closing are explicit adapter refusals.

REQ-SERVICE-0003 refines REQ-BDC-0730: after HTTP drain, close admission, set
StopFlag, await existing worker shutdown and join the actual readiness thread
before instance unlock. Preserve supervised child termination and the scheduler's
at-most twice shutdown-timeout bound.
