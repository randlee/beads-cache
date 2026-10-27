# bd 1.3.1 scope-filter recordings (ADR-BDC-0013, sprint b-7)

Recorded 2026-10-26 for sprint b-7 with the repository's pinned bd release, bd 1.3.1
(`c1c4b642a`, installed by `scripts/bd-compat/install-bd.sh 1.3.1`), in a throwaway
workspace (`bd init --prefix sf`, embedded Dolt, no shared database). The only edit: the
`owner` e-mail is replaced with `randlee@users.noreply.github.com`.

Before recording, the flags of ADR-BDC-0013's argv table were checked against
`bd list --help`, `bd blocked --help` and `bd ready --help` of bd 1.3.1: `bd list` has
`-l` (all of), `--label-any`, `-t`, `-a`, `--all`, `--status`, `-n` and
`--updated-after`; `bd blocked` has `-l` and `--label-any`; `bd ready` has `-l`,
`--label-any`, `-t`, `-a` and `-n`. None is missing.

Beads:

| Id | Role |
|---|---|
| sf-1vh | a: task, labels `area-x`, `team-y`, assignee `dev`, open |
| sf-t7g | b: bug, label `area-x`, `blocks` on a |
| sf-40r | c: task, label `team-y`, assignee `dev`, in_progress |
| sf-2v4 | d: feature, no label, open |
| sf-j91 | e: task, labels `area-x`, `team-y`, closed |
| sf-dge | f: task, label `area-x`, `blocks` on d (a blocker with no label) |

Each `<name>.json` is the stdout of `bd <argv>`, where `argv.json` gives the exact argv of
`<name>` (the argv `sc-bd-cli` builds for that filter variant, `crates/sc-bd-cli/src/argv.rs`).
`list-all.json` is every row of the workspace and is the world the contract rule
`scope_lists_answer_every_filter_variant` (`src/testing/contract.rs`) loads; the rule checks
each backend's answer for each variant against the recorded one. The argv test
`every_filter_variant_has_the_recorded_argv` (`crates/sc-bd-cli/tests/scope_filters.rs`)
checks the builders against `argv.json`.

Do not edit these files; re-record them for a new bd release.
