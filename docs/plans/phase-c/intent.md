# Phase C intent and proposed anchors

The delivered cache should adopt sc-runtime's supported application structure,
not keep a second scheduler/framework implementation. The application must retain
its local-read performance, persistent cache format and existing CLI/Python
contracts while gaining a real generated lifecycle and the existing validation path.

These are **candidates**, not locked ANC records. Live Jev rubric calls now work,
but their results remain inconclusive; no statement or fixture is approved for lock. IDs below continue the phase-B user-anchor sequence; they
are reservations for discussion, not registered anchors. Existing phase-B locked
anchors continue to constrain all changed source, tests and docs.

| Candidate | Proposed testable statement | User provenance (verbatim) | Development proof |
|---|---|---|---|
| ANC-012 | Phase C must adopt sc-runtime application templates as the baseline for its runtime integration. | “if we consume directly, then we would drift from sc-runtime immediately.” | tagged template regeneration plus adopted-path diff and application smoke |
| ANC-013 | The beads-cache rusqlite dependency must use version 0.39. | “info from sc-runtime:  Matching beads-cache to 0.39 is the right move.” | Cargo metadata/tree and locked store/scheduler tests |
| ANC-014 | Phase C sprint docs must follow the repository sprint-planning-guidelines, including disjoint same-wave owned paths and explicit acceptance ownership. | See the complete verbatim planning request below. | frontmatter/path/dependency review of this planning branch |
| ANC-015 | beads-cache must postpone sc-lint work until sc-runtime version 0.2.0 is released. | “don't worry about sc-lint until sc-runtime 0.2.0 lands” | no early lint bootstrap/profile/version change in sprint fences or delivered diff |

Verbatim source for ANC-014:

> ok, we should put together a plan for phase-\<next> on a planning /sc-git-worktree following [sprint-planning-guidelines.md](/Users/randlee/github/beads-cache/.claude/skills/plan-hardening/sprint-planning-guidelines.md)

Proposed fixtures for ANC-012: pass — the sprint adapts the pinned application
scaffold and provides regenerate-and-diff upkeep; fail — it only changes Cargo
versions and keeps independently maintained application assembly.
Proposed fixtures for ANC-013: pass — the lockfile resolves rusqlite 0.39 and one
libsqlite3-sys compatible with upstream; fail — a workspace store remains on 0.40.
Proposed fixtures for ANC-014: pass — each sprint has its closure, path ownership,
REQ/ADR list and positive/negative acceptance; fail — two wave-1 sprints own ci.yml
and no sprint owns actual consumer acceptance.

Proposed scope decisions requiring review: an optional generated foreground daemon,
`db=none,mcp=false`, deferral of sc-lint until sc-runtime 0.2.0, and deferral of #104 until a consumer requires it.
The user explicitly instructed deferral: “don't worry about sc-lint until
sc-runtime 0.2.0 lands”. This replaces the draft's earlier lint-adoption proposal.

Proposed fixtures for ANC-015: pass — the sprint retains existing CI and defers
sc-lint changes until upstream 0.2.0; fail — it bootstraps 0.8.x before that release.

## Live validation resumed

The user authorized TypeSafe/Jev transmission and then verified the credential with
jev-verify. Both environment and Keychain sources produced HTTP 200 with Jev
1.14.0. Subsequent rubric calls use Keychain through an ephemeral credential bridge;
the inherited stale environment variable is removed for that subprocess, and the
credential is passed only to the child environment, never written to disk or logs.
The Python calls use the system CA bundle with TLS verification enabled.

The earlier HTTP 402 is historical, not the current blocker. Its original receipt
is retained in anchors/runs/rubric-lint-deferral-20261102.json.

| Candidate | First live rubric | Narrowed wording recheck | Remaining issue |
|---|---|---|---|
| ANC-012 | inconclusive | not rewritten | fidelity to the conditional quote, observable adoption, self-contained meaning |
| ANC-013 | inconclusive | inconclusive | fidelity and self-contained meaning below the unchanged 0.6 confidence threshold |
| ANC-014 | inconclusive | not rewritten | compound wording, external guideline reference and fidelity; this may belong solely to existing planning-policy enforcement |
| ANC-015 | inconclusive | inconclusive | observation and self-contained meaning below the unchanged threshold |

Full results, both independent calls per rubric, and the unchanged question/role
outcomes are retained under anchors/runs/anc-012..015-rubric-20261102.json and the
anc-013/015-rubric-r2-20261102.json rechecks. The two narrowed candidates remove
extra wording without adding a new user obligation. No confidence threshold,
question, scope or fixture was altered to turn an inconclusive result into a pass.

These are anchor-testability results, not judgments of sprint compliance. None is
a passing planning gate. No anchor is locked and no question override is adopted.
The next capture step is to confirm the exact statements and pass/fail fixtures
with the user; the optional daemon and application surface remain proposed scope.

The governing plan-anchoring capture rule says: “Record only when the user confirms
or says "lock this" or "anchor this".” The rubric rule says to clarify
inconclusive results rather than treating them as passes. Confirmation must not be
misreported as a Jev testability pass; if the unchanged rubric still cannot judge a
confirmed statement, the independent calibration workflow must examine it against
its frozen corpus before any question change is considered.
