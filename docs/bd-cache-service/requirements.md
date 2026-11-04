# Service requirements

Approved refinements of the owner-approved Phase C application contract.

## REQ-SERVICE-0001: One composed cache

**Status:** Approved (owner Phase C execution approval, 2026-11-02)

The application owns one existing Composed cache and
SystemSchedules over its cache-file schedule store. Cached CLI/Python readers
remain independent local consumers; readiness never invokes bd.

**Refines:** REQ-BDC-1001
**Acceptance evidence:** `health_is_cached_and_cleanup_releases_the_single_connection`; FakeBd process acceptance in `scripts/consumer_checks.py`.

## REQ-SERVICE-0002: Finite owned readiness

**Status:** Approved (owner Phase C execution approval, 2026-11-02)

The service admits one health evaluation
using try-acquire and a zero-capacity rendezvous. Its one dedicated thread owns
the permit until real completion, including disconnect/timeout. Use existing
execute_api health defaults, probe=false and HealthReadControl with an absolute
one-second deadline. Busy/closing are explicit adapter refusals.

**Refines:** REQ-BDC-1001/NFR-BDC-0004
**Acceptance evidence:** `canceled_response_retains_capacity_and_cleanup_joins_real_work` and daemon `canceled_http_keeps_actual_read_and_cache_claim_until_cleanup`; the latter proves actual HTTP timeout/BUSY and cache-claim cleanup, not runtime instance-lock ordering.

## REQ-SERVICE-0003: Joined cleanup before unlock

**Status:** Approved (owner Phase C execution approval, 2026-11-02)

After HTTP drain, close admission, set
StopFlag, await existing worker shutdown and join the actual readiness thread
before instance unlock. Preserve supervised child termination and the scheduler's
at-most twice shutdown-timeout bound.


**Refines:** REQ-BDC-0730
**Acceptance evidence:** `health_is_cached_and_cleanup_releases_the_single_connection` and blocked-hook process acceptance. Exact qualified C1 source5255610 supplies the bounded worker/store path consumed by SystemSchedules::system. Actual Application writer-contention and zero-timeout regressions request independent C2 QA5; owning RSH-001/C2-REQ-004 remain open / implemented_pending_C2_QA. Happy-path timing does not close the contention requirement.
