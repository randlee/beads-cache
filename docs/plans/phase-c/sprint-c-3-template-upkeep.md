---
phase: c
status: complete
recommended_agent: rust-developer
recommended_model: gpt-6.1-sol
id: c-3
closure_type: boundary
target_boundary: framework template upkeep tooling
track: templates
wave: 1
branch: sprint/c-3-template-upkeep
worktree: /Users/randlee/github/beads-cache-worktrees/sprint/c-3-template-upkeep
must_follow: []
parallel_safe:
- c-1
requirements:
  owns: []
  constrained_by:
  - REQ-BDC-1003
  - REQ-BDC-0006
  - NFR-BDC-0005
adrs:
- ADR-BDC-0010
- ADR-BDC-0012
- ADR-BDC-0016
owned_paths:
- runtime/pin.toml
- runtime/answers.json
- scripts/runtime_templates.py
- scripts/test_runtime_templates.py
- docs/development/runtime-updates.md
inherited_bounds:
- ANC-001
---

# Sprint c-3 — Repeatable framework template upkeep

## Outcome

The pinned application template can be regenerated and compared without touching
working source. This sprint closes the upkeep-tool boundary; c-2 adopts the actual
application and owns REQ-BDC-1003's feature-level upgrade proof.

## Recorded owner approval

The project owner's explicit 2026-11-02 instruction to execute all approved
Phase C sprints satisfies this sprint's dispatch prerequisite. See the phase
plan's [recorded approval](phase-c-plan.md#implementation-dispatch-gate) and
ADR-BDC-0016 for provenance and the unchanged bounds. ADR-BDC-0016 is now in the
binding `adrs` list. c-1 applies the approved CLI/API/CRON amendments in its owned
files; no other existing rule is waived. Independent implementation QA, applicable
anchor gates and green CI remain required before merge.

## Deliverables

1. REQ-BDC-1003: commit the immutable source tag/full SHA and generation-tool
   version in runtime/pin.toml, plus runtime/answers.json using the exact answers
   in the phase plan. Verify the pinned tag matches the recorded commit. Use
   plain cargo-generate registry dependencies, not development checkout patches.
   Do not run new_project.py or its just setup/lint/test recipe pipeline: that
   driver would bootstrap the sc-lint work the user deferred.
2. REQ-BDC-1003: implement a small regenerate-and-diff helper around the documented
   `cargo generate --path template` renderer at the pinned source. Validate answers
   against the tagged wizard schema, supply a values file, `--name bd-cache`,
   `--allow-commands` and an absent destination; run the required sc-compose agent
   document rendering separately. Do not invoke generated setup/lint recipes or
   install their lint assets in beads-cache. Render only into absent temporary destinations. Compare
   old/new pristine baselines and report every changed path. Preserve the original
   workspace on success or failure; refuse an existing output destination.
   This helper is consumed by maintainers during framework upgrades to address the
   user's drift concern. It retires when a supported upstream updater replaces it;
   it is not an automatic merge engine, framework fork or new installer product.
3. REQ-BDC-1003: document how c-2 records its adopted-path mapping and uses the
   baseline delta to review application-specific adaptations. The helper consumes
   that mapping when present, but can compare pristine baselines before adoption.
   Tests use small local fixture trees for error/change detection, plus one real
   pinned-template generation smoke; no new app or product behavior lives here.

## Adopted-path mapping contract

c-3 defines the helper input; c-2 writes `runtime/adoption.toml` using this format:

```toml
version = 1

[[paths]]
generated = "crates/daemon/src/composition.rs"
adopted = "crates/bd-cache-daemon/src/composition.rs"

[[paths]]
generated = "crates/daemon/src/routes.rs"
adopted = "crates/bd-cache-daemon/src/routes.rs"
```

`generated` is one exact file path relative to a pristine generated project;
`adopted` is its consumer-owned destination relative to the beads-cache root.
No glob or directory mapping is implied. Paths use `/`, must be relative and
contain no `..` component; duplicate generated or adopted paths and unsupported
versions are errors. The mapping identifies review ownership; it never directs
an overwrite or requires adapted code to byte-match the template.

Compare the union of old/new generated file paths. Report every addition,
modification and deletion, attaching the adopted destination when mapped.
A deleted source still retains its mapping through the old baseline; a newly
added source may use a mapping once present in the new baseline. Unmapped changed
paths are reported explicitly as unadopted, including new upstream files, so a
maintainer can decide whether to adopt them. A mapping whose generated path is
absent from both baselines is an error, not a silently ignored stale entry.
Without a mapping, report all changes as unadopted. Fixture tests cover these
rules and reject a report that suppresses a mapped change.

## Acceptance criteria

| Root | Positive observable | Planted negative |
|---|---|---|
| boundary:template-upkeep-tooling | recorded pin/answers regenerate a pristine baseline with registry dependencies | replace tag SHA or insert a checkout patch; baseline check fails |
| boundary:template-upkeep-tooling | an old/new fixture comparison obeys the mapping contract above for modified, added, deleted and unadopted paths and rejects invalid/stale mappings | suppress a changed adopted file; helper test fails |
| boundary:template-upkeep-tooling | existing destinations are refused and the original worktree remains byte-identical on generator failure | render over the worktree or swallow a failed generator exit; failure-path test fails |
| boundary:template-upkeep-tooling | the pinned upstream generation path runs in an absent scratch destination; tool errors retain upstream diagnostics | ignore a generation failure and report success; smoke rejects the result |

## Required validation

Unit tests here exercise template/packaging helpers, not sync jobs, hooks, ingest
or CLI behavior. Installed-artifact smoke checks remain integration procedures.

`python3 -I -m unittest discover -s scripts -p 'test_runtime_templates.py' -v`,
helper positive/negative fixtures, and the pinned real generator smoke. Validate
pin/answers against the tagged wizard schema. Do not use a mutable checkout as the
source baseline. Existing no-consumer lint still applies to every new tracked file.

## Dependencies and limits

c-1 is parallel-safe: this sprint owns no Cargo, workflow, runtime domain or
scheduler path. c-2 consumes the pin/answers/helper and proves actual adoption and
upkeep against the installed application. sc-lint bootstrap, upgrades, profiles and
boundary-manifest migration are expressly deferred until sc-runtime 0.2.0 lands;
this sprint must not take that work early.

## Implementation evidence (2026-11-02)

The c-3 owned paths now provide the immutable v0.1.1 pin, exact approved answers,
scratch-only generator, baseline check, exhaustive mapped/unadopted JSON delta,
local fixtures and maintainer procedure. Feature-level application adoption and
upgrade proof remain c-2's responsibility; independent QA, the development anchor
gate and CI are still required before merge.

- `python3 -I -m unittest discover -s scripts -p 'test_runtime_templates.py' -v`:
  18 local tests passed; the explicitly enabled real smoke was skipped.
- With `RUNTIME_TEMPLATE_SMOKE_SOURCE=/Users/randlee/github/sc-runtime`, the same
  command passed all 19 tests, including real pinned generation.
- The standalone `generate --source /Users/randlee/github/sc-runtime --output
  /tmp/beads-cache-c3-first-baseline` succeeded with 39 files, both rendered agent
  documents and registry dependencies. Its source tag resolved exactly to
  `5af24c862fe7ed8dae8710b7bd64a84a29506b51`. `check` passed and self-`diff`
  reported zero changed paths. Tools: cargo-generate 0.25.1, sc-compose 1.7.0,
  Python 3.11.9, jsonschema 4.25.1 (Draft 2020-12).
- Planted negatives cover mismatched source SHA, checkout patches/path/Git
  dependencies, invalid/stale/duplicate mappings, mapped-change suppression via
  exact expected-report assertions, existing/concurrently-created outputs and
  generator/document failure with byte-identical original fixture trees.
- `cargo fmt --all` and `cargo clippy --all-targets -- -D warnings` exited 0;
  rustfmt printed a pre-existing v0.1.0 dependency checkout's unrendered-template
  parse diagnostic. No Rust or Cargo file changed in this sprint. c-1 owns the
  dependency migration that removes that inherited source.

Upstream renderer semantics: a literally absent cargo-generate `--destination`
failed with `Error: No such file or directory (os error 2)`. Tagged
`docs/generation.md` step 5 and `scripts/new_project.py:cargo_generate` require
creating that directory first. The helper therefore keeps caller output absent,
creates only an empty owned scratch render directory, then publishes the checked
result to an exclusively created new output. Existing caller destinations are
always refused. This distinction is documented in the maintainer procedure; no
driver/setup/lint recipe or lint-asset installation was run.
