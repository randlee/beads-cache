---
id: b-3
phase: b
status: complete
closure_type: integration
target_boundary: bd-cache-cli
track: cli
wave: 2
branch: sprint/b-3-cli
worktree: sprint/b-3-cli
recommended_agent: rust-developer
recommended_model: opus
must_follow: [b-1, b-2, b-3a]
parallel_safe: [b-4]
requirements:
  owns: [REQ-BDC-0801, REQ-BDC-0802, REQ-BDC-0805, REQ-BDC-0810, NFR-BDC-0002, "REQ-CLI-* (behaviour)"]
  constrained_by: [REQ-BDC-0004, REQ-BDC-0101, REQ-BDC-0102, REQ-BDC-0104, REQ-BDC-0402, REQ-BDC-0803, REQ-BDC-0807, REQ-BDC-0808, REQ-BDC-0809, REQ-BDC-0811, REQ-BDC-0902, REQ-BDC-0907, REQ-BDC-0912, NFR-BDC-0005]
adrs: [ADR-BDC-0002, ADR-BDC-0003, ADR-BDC-0005, ADR-BDC-0006, ADR-BDC-0009, ADR-STORE-0003, "ADR-CLI-*"]
owned_paths:
  - "crates/bd-cache-cli/**"
  - "docs/bd-cache-cli/requirements.md"
  - "docs/bd-cache-cli/architecture.md"
  - "Cargo.lock"
  - ".github/workflows/ci.yml"   # one step: the release-profile latency test
  - "docs/architecture.md"       # the bd-cache-cli rows and the ADR-BDC-0009 implementation status only
---

# Sprint b-3 — the bdc CLI

## Outcome

`bd-cache` / `bdc` implements contract §1–§8 over `bd-cache-api`: both
command groups, every reply in one envelope, the exit codes and the
configuration rules. Both binaries are tested in process through `run()` and
as real processes against `fake-bd`.

## Deliverables

1. **Argv.** clap argv is turned into the JSON request and passed to
   `bd-cache-api`'s decoder (contract §1 rule 6). `--input FILE|-` sends the
   request as given, and mixing it with request flags is `USAGE`. Long options
   must be spelled out in full; any other argv-shape error is `USAGE` with
   `did_you_mean` / `valid` (contract §3).
   - Files: `src/args.rs`, `src/request.rs`.
2. **Configuration** (REQ-BDC-0810, contract §2).
   - Precedence: flag > `BD_CACHE__*` env > TOML > default.
   - `--bd` is never read from TOML. `--format` comes from the flag only.
   - No upward search.
   - `.beads/bd-cache.toml` is read from `<repo>`, and `metadata.json` from
     `$BEADS_DIR` when set.
   - The legacy-file `CONFIG_INVALID` rule.
   - The merged config is opened through `bd_cache_sync::compose_with_config`.
     `ConfigLoadError` and `ComposeError` map through `ApiError::config_invalid`.
   - File: `src/config.rs`.
3. **Commands.** Surface (a) and surface (b) call the `bd-cache-api`
   operations. `cache tick` calls `bd_cache_sync::tick`, and each `refused[]`
   item maps through `ApiError::tick_refusal`.
   - Files: `src/commands/{beads,manage,tick}.rs`.
4. **Output.**
   - The sc-command `Envelope` on stdout, and `--format text` (contract §1
     rule 1, §3, §4).
   - A caught panic is `INTERNAL`.
   - Logs go to stderr at `-v` / `-q` / `BD_CACHE__LOG`.
   - `--help` is text on a TTY. Otherwise, or with `--format json`, it is the
     envelope with `{command, synopsis, arguments}`. `USAGE` is always JSON
     (contract §8).
   - Files: `src/render/{json,text}.rs`, `src/help.rs`.
5. **Tests.**
   - `tests/fixtures.rs` replays b-2's shared fixtures through `run()`. The
     envelopes must equal `ok`/`data` or the error the fixture expects.
   - `tests/process.rs` covers the real-process cases.
   - `tests/scenarios.rs` holds one test per named negative scenario
     (contract §8, Testing).
6. **Latency** (NFR-BDC-0002). `tests/latency.rs` is `#[ignore]` in debug
   builds and runs in a new CI step,
   `cargo test --release -p bd-cache-cli --test latency -- --ignored`.
   - Gated: the median and p95 of 50 new-process `bdc issues get` cache hits,
     with nothing pending on the scope, are under 50 ms.
   - Reported, not gated: the same with 10 pending entries in a 1,000-row
     scope (contract §2).
7. **Docs.** `docs/bd-cache-cli/{requirements,architecture}.md`, and the
   `bd-cache-cli` rows in `docs/architecture.md`.

## Acceptance criteria

| # | Root | Positive observable | Planted negative |
|---|---|---|---|
| 1 | REQ-BDC-0801 | Every command of contract §5.2 and §6 replays its fixture through `run()` and gets the expected envelope. | Route `cache evict` to `refresh` → its fixture fails. |
| 2 | REQ-BDC-0802 | Exit 0 is `ok: true`; exit 2 is `USAGE` (an unknown flag, an abbreviated option, `--input` mixed with flags, a bare `cache refresh`); exit 3 is any other typed error. stdout is always exactly one JSON document. | Exit 1 on `USAGE` → the exit test fails; or print an error on stderr only → the parse of stdout fails. |
| 3 | REQ-BDC-0810 | A matrix test sets one key in TOML, env and flag, and the flag wins, then the env, then the TOML. `bd = "…"` in TOML is ignored. Running from a subdirectory does not find the parent `.beads`. The legacy `.atm-bd/` file gives `CONFIG_INVALID` with `details.legacy_path`. | Read `bd` from TOML → the planted fake binary named in TOML runs, and the test fails. |
| 4 | contract §8 `--help` | Under a pseudo-TTY, `--help` is text; piped, it is the envelope; both exit 0. | Print text help when piped → the envelope parse fails. |
| 5 | scenario: outage during a write | `fake-bd` is unavailable; `issues update` exits 0 with `x_cache.state: "pending"`; a later `cache flush` after recovery applies it, and `journal get` shows `applied`. | Report `applied` at journal time → the state assertion fails. |
| 6 | scenario: escalation at flush | A `not_closable` refusal and a guard miss each escalate; `cache flush` lists them in `escalated[]`, and `journal get` shows `last_error.code` with no note. | Store the reason only as free text → `last_error.code` is null, and the test fails. |
| 7 | scenario: partial `refresh scopes` | One scope fails and one passes; exit 0, `data.failed: 1`, `results[]` in request order. | Exit 3 on a partial failure → the test fails. |
| 8 | scenario: `fresh_within` runs out | `fake-bd` is slow by 2 s; `ready --fresh-within-ms 200` returns within 400 ms with `read_fallback: "timed_out"` and `basis.kind: "cached"`. | Wait for bd → the 400 ms bound fails. |
| 9 | scenario: schema change | The scenario changes the bd schema fingerprint; `cache health --probe` reports `rebuilt: true`, the rows are dropped and the journal is kept. | Skip the rebuild → the stale-row assertion fails. |
| 10 | scenario: `WRONG_DATABASE` | A cache file of another project is refused with exit 3 and `WRONG_DATABASE`. | Skip the project-id check → the command succeeds, and the test fails. |
| 11 | scenario: lost reply | The same key is sent twice; the second reply is `duplicate: true` with the same `seq`, and `journal get --idempotency-key K` returns that entry. | Journal the retry as a new entry → the seq assertion fails. |
| 12 | NFR-BDC-0002 | The CI latency step is green, and its log reports both the gated and the reported case. | Run a backend check on open, with the latency scenario giving `fake-bd` a 60 ms startup delay → the median goes over 50 ms, and the step fails. |

## Dependencies

b-1 (`compose_with_config`, the core verbs), b-2 (`bd-cache-api` and its
fixtures) and b-3a (the crate and the harness).
