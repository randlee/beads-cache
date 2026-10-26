# Intent - beads-cache phase B

Written 2026-10-25 by bc-lead, from Rand's phase-B direction; the boundary extension is
`docs/plans/phase-b/phase-b-boundary.md` (r3 with Amendments A1–A6). One page at most. This is the "why"
that frames every anchor beneath it; Jev questions are judged against anchors, not against this
document.

## What this plan is for

beads-cache is a cache for the beads issue tracker (`bd`), with a CLI (`bdc`) for configuring and
managing it and a Python API exposed through PyO3. Phase B delivers that tool to its consumers. The
first consumer is atm-bd, but nothing in beads-cache is built for it in particular. The original
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

- No ATM concern remains in beads-cache source, tests, scripts or product docs, and the no-ATM lint
  enforces this in CI.
- The built-in bd sync runs from a schedule row in `sc-cron`, with at most one such row per
  repository. Its scopes are bd filters, not phases.
- A consumer can hand scopes to its own registered Python sync method, which feeds the cache
  through the same validation, act on the sync results and errors in a post-sync hook, and pause
  the tick with the cron row's `enabled` flag. One tick runs at a time, and registered code runs
  only as an isolated subprocess from a cache file the user owns.
- `sc-cron` supports Rust and Python hook targets, an `enabled` flag per row, and a stop hook that
  returns continue, disable or delete.
- The bd-cache-rs wheel and the `bdc` binaries ship for the consumer through the release channels.

## What is deliberately out of scope

- Sending messages of any kind from beads-cache. That is the consumer's job, done in its hooks.
- Migrating a consumer's existing files and running its compatibility tests. That is the consumer's
  job, done against the published wheel.

## Anchors

Anchors for this plan live in `anchors.json` (record of truth) and are rendered to `anchors.md`.
Requirements and ADRs that govern this plan are listed there too, under their own IDs.
