# Repository Quality Policy

This file contains repository-specific QA policy. Reusable agents and skills
must read this file rather than embedding repository names, commands,
interfaces, approval authorities, or temporary architectural exceptions in
their own prompts.

## Repository Baseline

- Default comparison branch: `develop`
- Requirements index: `docs/requirements.md`
- Architecture index: `docs/architecture.md`
- Project plan: `docs/project-plan.md`
- Team protocol: `docs/team-protocol.md`
- Per-crate docs: none declared. When this section declares them, each crate
  in scope carries `docs/<crate>/requirements.md` and
  `docs/<crate>/architecture.md`, indexed from the baseline docs above.

## Reviewer Policy

- Initial implementation review: `req-qa`, `arch-qa`,
  `ruthless-boundary-qa`, `rust-qa-agent`,
  `rust-best-practices-agent`, and `rust-service-hardening-agent`
- Fix verification: `req-qa`, `arch-qa`, and `rust-qa-agent`; dispatch an
  additional reviewer only when rechecking a carry-forward finding owned by
  that reviewer
- Plan review QA-1: `req-qa`, `arch-qa`, `ruthless-boundary-qa`,
  `rust-best-practices-agent`, `rust-service-hardening-agent`, and
  `ceremony-qa`
- Plan review QA-2 and later: `req-qa` and `arch-qa` scoped to the
  dispatched findings; dispatch another reviewer only when rechecking a
  carry-forward finding it owns. Plan QA is capped at 3 rounds
  (`plan_qa_cycle_limit`); a round that leaves only minor findings reports
  `PASS — minor fixes required, no re-QA`
- `ceremony-finding-screen`: every sprint or plan QA round that has
  findings, over all of them, before the report is posted
- Phase-end review: the initial implementation set plus `flaky-test-qa`
- Run `flaky-test-qa` earlier when tests changed or instability is suspected
- Run `schema-reviewer` only when the governed-interface section below names
  an interface relevant to the assigned scope
- Every QA round, plan or sprint, requires an open PR; the rendered report
  is posted to it every round

## Validation Policy

No repository-wide phase-end artifact command is configured yet. An
implementation phase must add its authoritative command here before phase-end
QA can pass. Reviewer assignments may provide narrower commands for earlier
rounds.

## Lint and Gates

Required on every implementation PR once the Cargo workspace exists:

| Check | Command |
| --- | --- |
| Formatting | `cargo fmt --all --check` |
| Lints | `cargo clippy --all-targets --all-features -- -D warnings` |
| Tests | `cargo test --all-features` |

Optional tooling (for example `sc-lint` or `just` recipes) applies only where
the repository configures it; when configured, list its commands here.

Boundary manifests: none declared. When the repository declares boundary
manifests, list their paths here; until then, crate boundaries are reviewed
against `docs/architecture.md`, and any relaxation needs a lead ruling
recorded as an ADR.

## Plan Naming

One convention for every phase. Everything is lower case.

| Thing | Form | Example |
|---|---|---|
| Phase id | next unused letter (then letter pairs once single letters run out), lower case | `a` |
| Sprint id | `<phase>-<n>`, `n` from 1 | `a-1` |
| Plan directory | `docs/plans/phase-<phase>/` | `docs/plans/phase-a/` |
| Phase plan | `docs/plans/phase-<phase>/phase-<phase>-plan.md` | `phase-a-plan.md` |
| Sprint doc | `docs/plans/phase-<phase>/sprint-<phase>-<n>-<slug>.md` | `sprint-a-1-<slug>.md` |
| Plan branch | `plan/phase-<phase>`, PR to `develop` | `plan/phase-a` |
| Phase branch | `integrate/phase-<phase>`, cut from `develop` | `integrate/phase-a` |
| Sprint branch | `sprint/<phase>-<n>-<slug>` | `sprint/a-1-<slug>` |
| Fix layer | `fix/<phase>-<n>-qa<k>`; phase-level: `fix/<phase>-<slug>` | `fix/a-1-qa1` |

- Phase plans never live outside `docs/plans/phase-<phase>/`.
- Every sprint doc declares its branch explicitly. Tools read the declared
  branch and never infer it from a file name.

## Governed Interfaces

No governed interface policy is currently defined. Until this section names
an interface, its compatibility rule, evidence paths, and approval authority,
`schema-reviewer` returns `SKIPPED`.

## Repository Exceptions

None.
