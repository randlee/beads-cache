---
id: b-3a
phase: b
status: complete
closure_type: boundary
target_boundary: bd-cache-cli skeleton and fake-bd
track: cli
wave: 1
branch: sprint/b-3a-cli-harness
worktree: sprint/b-3a-cli-harness
recommended_agent: rust-developer
recommended_model: sonnet
must_follow: []
parallel_safe: [b-1, b-2]
requirements:
  owns: ["REQ-CLI-* (harness, created here)"]
  constrained_by: [REQ-BDC-0805, NFR-BDC-0005, NFR-BDC-0006]
adrs: [ADR-BDC-0007, ADR-BDC-0009, ADR-BDCLI-0004, "ADR-CLI-0001 (created here: the sc-command git pin, publish = false)"]
owned_paths:
  - "crates/bd-cache-cli/**"   # the whole crate until b-3 starts; b-3 owns it afterwards
  - "docs/bd-cache-cli/requirements.md"
  - "docs/bd-cache-cli/architecture.md"
  - "Cargo.toml"               # the bd-cache-cli member, sc-command (git, tag prerelease/v0.1.0), clap
  - "Cargo.lock"
---

# Sprint b-3a — CLI skeleton and the fake-bd harness

## Outcome

The `bd-cache-cli` crate exists on all three CI OSes, with sc-command pinned
(P-7) and the binaries `bd-cache` and `bdc`. It also has the real-process
test harness of contract §8 "Testing": a `fake-bd` executable over
`sc-bd-cli`'s stateful `FakeBd`, with its state seeded from a scenario file
and a fixed clock. b-3 then adds behaviour only.

## Deliverables

1. `crates/bd-cache-cli/Cargo.toml`:
   - `publish = false`;
   - `sc-command` as a git dependency on `https://github.com/randlee/sc-runtime`,
     tag `prerelease/v0.1.0`, and `clap`;
   - `[[bin]]` `bd-cache` and `bdc`;
   - `[[bin]]` `fake-bd` with `required-features = ["fake-bd"]`, where the
     feature `fake-bd` enables `sc-bd-cli/test-support`. A self dev-dependency
     with that feature lets the integration tests use `CARGO_BIN_EXE_fake-bd`.
2. `src/lib.rs` with the in-process entry point at its final signature (contract
   §8): `pub fn run(argv, env, invoker, clock) -> (i32, String)`. In this sprint
   it answers `--version` and returns a `USAGE` envelope for everything else.
   `src/bin/{bd-cache,bdc}.rs` call it.
3. `src/bin/fake-bd.rs`: runs `FakeBd` behind the `bd` argv. It reads the
   state from `$FAKE_BD_SCENARIO`, a JSON file with the rows, the blocked set,
   the version and the schema fingerprint, plus injected faults: unavailable,
   slow by N ms, a refused write with an exit code. It writes the state back
   after each mutating call, so a sequence of processes sees one consistent bd.
   It reads the time from the scenario's fixed clock.
4. `tests/support/mod.rs`: build a scenario, run `bdc` with `--bd <fake-bd>`
   in a temp repo with `.beads/metadata.json`, and parse the envelope.
5. `docs/bd-cache-cli/requirements.md` and `architecture.md`: ADR-CLI-0001
   (why it uses git dependencies and `publish = false`) and the scenario file
   format.

## Acceptance criteria

| # | Root | Positive observable | Planted negative |
|---|---|---|---|
| 1 | P-7 / ADR-CLI-0001 | CI is green on ubuntu, macOS and Windows. `bdc --version` prints one sc-command envelope with `ok: true` and `data.name`. | Print `--version` as plain text → the envelope parse in the test fails. |
| 2 | REQ-BDC-0805 | `bd-cache` and `bdc` give identical output for the same argv. | Build `bdc` from a different entry point that drops `--version` → the identity test fails. |
| 3 | harness parity | A scripted sequence (list, then update, then list) gives the same rows through `BdCli` over the `fake-bd` process as through in-process `FakeBd`. | Do not persist state between `fake-bd` calls → the second list differs, and the test fails. |
| 4 | harness faults | A scenario marked unavailable makes `BdCli` return the unreachable error; slow by 500 ms with a 100 ms call timeout times out. | Ignore the fault fields → both fault tests pass bd's answer through, and the tests fail. |
| 5 | release hygiene | `cargo build --release --bin bd-cache --bin bdc` does not build `fake-bd`, and the release artifacts contain no `test-support` code. | Make `fake-bd` an unconditional bin → the check (`cargo metadata` lists `fake-bd` without `required-features`) fails. |

## Dependencies

None inbound. b-3 builds on this crate.
