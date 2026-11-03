# Service architecture

Application-owned adaptation of the immutable scaffold's service boundary.
`Application::start` consumes Composed. The cache moves behind one shared mutex;
SystemSchedules and the health closure reuse its schedule view and connection.
The mutex is never held across await. No generated demo store/job is retained.

ReadyState clones only admission state. Its semaphore holds one permit and its
std channel has zero capacity. try_send can refuse a probe while the thread is
starting or between evaluations; callers retry. A response receiver has no worker
ownership. Cleanup closes every sender clone and joins the thread, concurrently
with the existing scheduler stop, while runtime still holds daemon.lock.

The sole store budget is C1 HealthReadControl; this service does not add SQL,
thread-local budget logic, a second connection, a scheduler database, or a fork.
Store budget errors keep BD_CACHE.STORE_FAILED. Refused admission/cache mutex
contention map to daemon-specific unavailable envelopes at the daemon boundary.
