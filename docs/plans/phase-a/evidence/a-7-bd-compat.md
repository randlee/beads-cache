# a-7: the real bd-compat workflow runs

Lead ruling (bc-lead, 2026-10-24): the real `workflow_dispatch` and
`repository_dispatch` runs of `.github/workflows/bd-compat.yml`, deferred from
a-2, belong to a-7. a-2's interim evidence is the push-triggered proof branch
(runs 40073075092 and 40073178515, `docs/plans/phase-a/sprint-a-2-sc-bd-cli.md`).

## What can run before the workflow is on `main`, and what cannot

| Trigger | Before `main` has the file | Why |
|---|---|---|
| `workflow_dispatch` on the default branch | cannot run | the default branch has no `bd-compat.yml`; `gh workflow run bd-compat.yml` answers `HTTP 422: Workflow does not have 'workflow_dispatch' trigger` |
| `workflow_dispatch` on a branch ref | **can run** | GitHub has the workflow registered (it has run on pushed branches); `gh workflow run bd-compat.yml --ref <branch>` dispatches the branch's file as a real `workflow_dispatch` event |
| `repository_dispatch` (`beads-release`) | cannot run | the event always runs the default branch's copy of the workflow, and `main` has none; a dispatch would be accepted and start nothing |

a-2 recorded the first row as `HTTP 404`; today's answer is the 422 above.
The second row is new: it was not known when the ruling was made.

## Runs in a-7

Both on `sprint/a-7-sync-integration` at `984978b` (the file is unchanged by a-7),
real `workflow_dispatch` events, ubuntu-latest, pinned bd 1.3.1:

| Run | Scenario | Result |
|---|---|---|
| [40083096528](https://github.com/randlee/beads-cache/actions/runs/40083096528) | no inputs (the pinned version) | success: fingerprint, `cargo test -p sc-beads -p sc-bd-cli --all-features`, 7 live probes |
| [40083225647](https://github.com/randlee/beads-cache/actions/runs/40083225647) | failure path: `expect_schema_sha` test hook set to a wrong sha | failed as intended; opened issue [#12](https://github.com/randlee/beads-cache/issues/12) with the report; the fingerprint step failed and the other two steps passed |

Issue #12 was closed with a note that it came from this verification, as a-2
did with #8. The schema-diff block is absent from #12: the hook changes the
expected sha, not the schema, so the diff against the pinned fixture is empty.
A diff is produced when the schema really changes, which a-2's proof showed by
also editing the fixture (run 40073178515, issue #8).

## What remains for close-out

- `repository_dispatch` with a different bd version: after `develop` is merged to
  `main` (phase-a close-out, team-lead), send
  `gh api repos/randlee/beads-cache/dispatches -f event_type=beads-release -f 'client_payload[version]=<v>'`
  and check the run, as P-5 in `docs/plans/phase-a/phase-a-plan.md` describes.
- A failing run caused by a real bd release is not reproducible today: the
  locally installed bd 1.3.0 has exactly the pinned schema (sha256
  `caf8fe18...`), and the workflow passes a bd version with the pinned schema
  ("the model is current, the pin is just older"). Only a bd release that
  changes `bd schema` fails it, which is the case the workflow exists for.
- `workflow_dispatch` on `main` at the pinned version: after the same merge.
