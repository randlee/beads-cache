# Phase C intent

Phase C proposes replacing the local scheduler prototype with the published
sc-runtime scheduler, adapting its application scaffold around the existing cache,
and qualifying the result with an actual consumer and installed artifacts.

The work is bounded by the existing cache responsibilities. It must not turn the
cache into a backend database client, replace the v3 file format, make cached reads
start bd, break the existing Python contract, or discard the sync-replacement,
post-sync-hook and deadline guarantees while changing runtime assembly.

Relevant previously locked constraints are inherited unchanged from phase B in
[anchors/anchors.md](anchors/anchors.md). They constrain the implementation rather
than treating discussion remarks as phase goals. IDs, statements, provenance,
status, question links and fixtures retain their original authority. No new
conversation-derived anchor is proposed or locked by this plan.

The optional generated foreground daemon and its narrow readiness surface remain
an architectural proposal to review; using the template does not by itself approve
new product operations. MCP, arbitrary HTTP operations and journaled create remain
outside the proposed phase. The full phase plan and sprint docs define the work
fences; inherited requirements constrain what those changes may do.

Planning context is recorded separately from anchors: rusqlite 0.39 is the current
upstream compatibility choice, the sprint-planning guidelines govern plan shape,
and sc-lint work waits for sc-runtime 0.2.0. Those remarks are not a phase anchor
packet. Jev service diagnostics and the rejected candidate rubric results are not
phase-plan acceptance criteria.
