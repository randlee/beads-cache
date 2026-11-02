---
phase: c
status: planned
recommended_agent: rust-developer
recommended_model: gpt-6.1-sol
id: c-3
closure_type: boundary
target_boundary: framework template upkeep tooling
track: templates
wave: 1
branch: sprint/c-3-template-upkeep
worktree: sprint/c-3-template-upkeep
must_follow: []
parallel_safe:
- c-1
requirements:
  owns: []
  constrained_by:
  - REQ-BDC-1003
  - REQ-BDC-1004
  - REQ-BDC-0006
  - NFR-BDC-0005
adrs:
- ADR-BDC-0010
- ADR-BDC-0012
- ADR-BDC-0016 (proposed)
owned_paths:
- runtime/pin.toml
- runtime/answers.json
- scripts/runtime_templates.py
- scripts/test_runtime_templates.py
- docs/development/runtime-updates.md
---

# Sprint c-3 — Repeatable framework template upkeep

## Outcome

The pinned application template can be regenerated and compared without touching
working source. This sprint closes the upkeep-tool boundary; c-2 adopts the actual
application and owns REQ-BDC-1003's feature-level upgrade proof.

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

## Acceptance criteria

| Root | Positive observable | Planted negative |
|---|---|---|
| boundary:template-upkeep-tooling | recorded pin/answers regenerate a pristine baseline with registry dependencies | replace tag SHA or insert a checkout patch; baseline check fails |
| boundary:template-upkeep-tooling | an old/new fixture comparison reports modified, added and deleted paths; adopted paths are identified when mapping is supplied | suppress a changed adopted file; helper test fails |
| boundary:template-upkeep-tooling | existing destinations are refused and the original worktree remains byte-identical on generator failure | render over the worktree or swallow a failed generator exit; failure-path test fails |
| boundary:template-upkeep-tooling | the pinned upstream generation path runs in an absent scratch destination; tool errors retain upstream diagnostics | ignore a generation failure and report success; smoke rejects the result |

## Required validation

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
