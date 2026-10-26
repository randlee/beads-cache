# Intent - beads-cache phase B

Written 2026-10-25 by bc-lead, from Rand's phase-B direction; the boundary extension is
`docs/plans/phase-b/phase-b-boundary.md` (r3 with Amendments A1–A8: A7, of 2026-10-26, makes the sync
switch two cron rows with a CLI convenience; A8, of 2026-10-26, settles the PR #97 QA round). One page at most. This is the "why"
that frames every anchor beneath it; Jev questions are judged against anchors, not against this
document.

## What this plan is for

beads-cache is a cache for the beads issue tracker (`bd`), with a CLI (`bdc`) for configuring and
managing it and a Python API exposed through PyO3. Phase B delivers that tool to its consumers. There
is a first consumer, but nothing in beads-cache is built for it in particular, and its name is a
prohibited word (on the no-consumer lint's prohibited list, `scripts/lint_no_atm.py`). The original
phase-B scope is built and merged:
- the `bdc` contract, whose two surfaces are a beads surface shaped like bd's own interface and a
  cache-management surface;
- the `bd-cache-api` crate and the `bdc` CLI;
- Python wheel 0.2;
- the release channels (PyPI, GitHub, Homebrew, Scoop, WinGet).

The consumer review of that work found the bd sync ("tick") still carries the consumer's concerns:
messaging, phase files, team and lead configuration, and a stop rule. Phase B finishes by making
beads-cache general:
- the cache syncs bd itself on a schedule held in the SQLite cron table of `sc-cron`;
- a consumer can replace the built-in sync with its own Python method, and receive the sync results
  and errors in a post-sync hook;
- `sc-cron` stays a general-purpose job scheduler that sc-runtime can copy as it is.

## What success looks like

- No consumer concern remains in beads-cache source, tests, scripts or product docs, and no
  prohibited word appears in the repository outside the lint, its tests and `docs/team-protocol.md`:
  the no-consumer lint, whose prohibited list is the one place those words are written, enforces
  this in CI on the whole tree.
- The built-in bd sync runs from a schedule row in `sc-cron`, with at most one such row per
  repository. Its scopes are bd filters, not phases.
- A consumer can switch from the built-in sync row to its own Python sync row with one command,
  which runs the script at once so any error is seen; the script feeds the cache through the
  same validation. The built-in sync gives a post-sync hook the results and errors. One sync
  runs at a time, and the consumer's code runs only as an isolated subprocess from a cache file
  the user owns.
- `sc-cron` supports Rust and Python hook targets chosen per row, an `enabled` flag per row, and a
  stop hook that returns continue, disable or delete; it stays a general-purpose cron assembly.
- The bd-cache-rs wheel and the `bdc` binaries ship for the consumer through the release channels.

## What is deliberately out of scope

- Sending messages of any kind from beads-cache. That is the consumer's job, done in its hooks.
- Migrating a consumer's existing files and running its compatibility tests. That is the consumer's
  job, done against the published wheel.

## Anchors

Anchors for this plan live in `anchors.json` (record of truth) and are rendered to `anchors.md`.
Requirements and ADRs that govern this plan are listed there too, under their own IDs.
