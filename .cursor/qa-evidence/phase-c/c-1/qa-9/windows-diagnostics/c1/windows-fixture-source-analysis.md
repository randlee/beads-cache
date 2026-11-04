Frozen source remains bcfcae7021522454e85be964661b9525bb15e58c, clean; no new code or probes. Cargo RELEASED, zero builds/tests or active commands in this continuation.

Two bounded offline Windows-target feature queries prove the same rusqlite features and locked SQLite/fixture versions across C1/C2; manifests/locks are unchanged from original failing/source-correlated heads. Child exact1test execution isolates registry/TLS/control/channel state from parent tests. No supported feature divergence or cross-test in-memory lock trigger found.

The absolute5s spans process spawn/startup, private temp/schema/WAL setup, the actual2s observer wait, cancellation and join; the500ms observation starts after the wait receipt. Source places the controlled SQLite contender after setup/create, and cancellation is installed even after wait observation failure. No actual setup-I/O, SQLite wait, lost-receipt or session-restoration fault was demonstrated.

Three source-compatible locations remain: before worker entry (need setup phase timestamps), during real operation/cancellation/session restoration (need wait/cancel/call-return result), or before recording the post-call marker (need returned/stall/completion/rollback/join sequence). These are phase hypotheses, not root-cause assertions. Existing diagnostic output already observes those phases; another forced timeout now would not reproduce the original mechanism.

All5 findings remain OPEN; production, reviewed RSH/FTQ011 source, ownership/observer semantics and30ms/500ms/5s unchanged. Parent monitors the existing exact-source Windows handles; no duplicate poll or green-rerun closure. Full source references and limits are in windows-fixture-source-analysis.json.
