# Phase C intent and proposed anchors

The delivered cache should adopt sc-runtime's supported application structure,
not keep a second scheduler/framework implementation. The application must retain
its local-read performance, persistent cache format and existing CLI/Python
contracts while gaining a real generated lifecycle and the existing validation path.

These are **candidates**, not locked ANC records. No Jev confirmation or fixture
approval has occurred. IDs below continue the phase-B user-anchor sequence; they
are reservations for discussion, not registered anchors. Existing phase-B locked
anchors continue to constrain all changed source, tests and docs.

| Candidate | Proposed testable statement | User provenance (verbatim) | Development proof |
|---|---|---|---|
| ANC-012 | Phase C must adopt sc-runtime application templates as the baseline for its runtime integration. | “if we consume directly, then we would drift from sc-runtime immediately.” | tagged template regeneration plus adopted-path diff and application smoke |
| ANC-013 | Every beads-cache workspace crate using rusqlite must resolve the 0.39 release line shared with sc-runtime. | “info from sc-runtime:  Matching beads-cache to 0.39 is the right move.” | Cargo metadata/tree and locked store/scheduler tests |
| ANC-014 | Phase C sprint docs must follow the repository sprint-planning-guidelines, including disjoint same-wave owned paths and explicit acceptance ownership. | See the complete verbatim planning request below. | frontmatter/path/dependency review of this planning branch |
| ANC-015 | Phase C must defer sc-lint installation, bootstrap and upgrades until sc-runtime 0.2.0 has been released. | “don't worry about sc-lint until sc-runtime 0.2.0 lands” | no early lint bootstrap/profile/version change in sprint fences or delivered diff |

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

The initial rubric attempt on ANC-012 had no Jev answer: sandbox networking failed, then
automatic approval review rejected external transmission of planning text and the
user quote. Stored allow_remote_judge=true does not override that rejection.
The user then answered “Approve TypeSafe/Jev validation”, authorizing the retry.
Anchor statements/fixtures themselves still require approval before lock.


After explicit egress approval, the first retries encountered Python CA configuration
errors. Retrying with the system CA bundle and TLS verification enabled reached
Jev, which returned HTTP 402 without a judgment. The saved rubric result is
`anchors/runs/rubric-lint-deferral-20261102.json`. No candidate passed, no anchor is
locked, and no planning gate has been run. The service/account response must be
resolved before live validation can complete; no payment or account change was
attempted.
