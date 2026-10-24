# P-6 probe recording: `bd blocked`, `bd ready` and `bd list`

Recorded 2026-10-23 for sprint a-1 by repeating the P-6 probe
(`docs/research/bd-blocked-state-probe.md`) with bd 1.3.0
(dev 3c4dd8cecb5d) and dolt 2.3.5: a throwaway workspace
(`bd init --server --external --prefix pr`) against a scratch
`dolt sql-server` on 127.0.0.1, no shared database. The only edit: the
`owner` e-mail is replaced with `randlee@users.noreply.github.com`.

Beads (label `phase-p6` unless noted):

| Id | Role |
|---|---|
| pr-tqm | x: outside blocker (no label) |
| pr-axf | y: closed blocker (no label) |
| pr-bnv | a: `blocks` on x (blocker outside the label) |
| pr-wqb | b: `blocks` on a |
| pr-zbr | p: parent, `blocks` on a |
| pr-cwo | c: `parent-child` child of the blocked parent p |
| pr-t29 | q: parent, unblocked |
| pr-bgt | d: `parent-child` child of the unblocked parent q |
| pr-762 | r: `related` to x |
| pr-4pb | t: `blocks` on the closed y |

States, each with `bd list --id <all ten> --status <all seven> -n 0 --json`
(`*-list.json`), `bd blocked -l phase-p6 --json` (`*-blocked.json`) and
`bd ready -l phase-p6 -n 0 --json` (`*-ready.json`):

1. `1-initial`: as built.
2. `2-x-closed`: after `bd close pr-tqm`.
3. `3-x-reopened`: after `bd reopen pr-tqm`.

Findings match the P-6 probe: no row carries `is_blocked`; the child of a
blocked parent lists the parent (`pr-cwo` blocked by `pr-zbr`); `related` and
a closed blocker do not block; closing x unblocks only a (b, p and c stay
blocked by the still-open a); reopening x blocks a again.
