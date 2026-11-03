# Scope R1 developer corrections

Input: [scope-r1.md](scope-r1.md), formal `SCOPE-R1` FAIL at
`e3ef2d259e774c937962e8c7d7f87e84d96f3b24`, fingerprint
`3c178b5c4e5de8f2af585907853de0b48ca96c902b74258c860dd1a3a311c3fb`.
[Scope finding screen](scope-r1-screen.md) retained all five corrections; none
was rejected. This is developer correction evidence, not reviewer approval.

| Finding | Correction | Developer check |
|---|---|---|
| PLAN-SCOPE-001 | c-1 owns the Python architecture document and its existing exception deliverable explicitly amends it while preserving isolated no-clap/no-server restrictions. | Governed document has an implementation owner; c-1/c-3 fences stay disjoint. |
| PLAN-SCOPE-002 | c-3 defines the existing adoption mapping's TOML input, exact source/destination semantics, added/deleted/unmapped-path handling and invalid/stale entry failures. c-2 references that single definition. | TOML example parses; fixture acceptance names the contract; no new artifact or sprint added. |
| PLAN-SCOPE-003 | c-2 and c-4 dependency sections state parent development push, merge-forward before each child dev/fix round, no parent-QA wait, and parent-PR merge before child completion. | Artifact dependencies and execution triggers are both explicit; critical path three and width two are unchanged. |
| PLAN-SCOPE-M1 | Phase CI note records the already-applied Ubuntu 24.04 repair separately from its validation and subsequent migration requalification. | No premature green-CI claim or claim that the repair remains unapplied. |
| PLAN-SCOPE-M2 | Preservation behavior has c-1 as acceptance owner; c-2 qualifies the preserved surface with its daemon present/absent. | Ownership table distinguishes migration closure from application qualification. |

Validation: `git diff --check`; existing consumer-boundary lint on the five
changed plan/sprint documents; mapping example parsed with Python `tomllib`;
assertions checked finding-specific text and non-intersecting wave-one ownership.
The notes file is untracked until the coordinator checkpoints it; the lint's
tracked-file scan does not cover an untracked file. No anchor statement, scope,
question or applicability decision changed in this correction pass. No product
implementation, extra sprint, commit or push was performed by the developer.

All five input findings are resolved in the developer's current working-tree
assessment. Formal scope rerun, critical review, subsequent anchor gates and
quality review remain separate coordinator steps.
