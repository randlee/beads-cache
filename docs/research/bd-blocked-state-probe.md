# Probe: how bd exposes blocked state (P-6)

Date: 2026-10-24. bd 1.3.0 (dev 3c4dd8cecb5d, no CGO), dolt 2.3.5. A
throwaway workspace (`bd init --server --external`) against a scratch dolt
sql-server on 127.0.0.1, no shared database. Run by bc-lead.

## Results

| Question | Result |
|---|---|
| Does `bd list --json` emit `is_blocked`? | **No.** Not for a blocked bead, not after its blocker closes or reopens. The key is absent from every row. |
| Does `bd show --json` emit `is_blocked`? | **No.** |
| Does `bd schema` declare it? | Yes, as an optional property of `types.issue`; the bd-1.3.1 `list-rows.json` fixture has no row with it either. |
| Does closing or reopening a blocker bump the blocker's own `updated_at`? | Yes (close and reopen each bumped it). |
| Does it bump the depender's `updated_at`? | No (BlockedStateInvariant; also confirmed on a live cache: 18 of 1279 `blocks` edges). |
| Do `--id` and `--updated-after` combine as an intersection? | Yes: after closing A, `bd list --id A,B --updated-after T1` returned only A. |
| Does `bd list --id <closed bead>` return it without `--status`? | No, it returns `[]`; an explicit status list is required (also noted by fenix). |
| Which edge types block? | `blocks` blocks. A child of an open, unblocked parent is ready; a child of a blocked parent is blocked (inherits through `parent-child`). `related` does not block. Other types (`until`, `validates`, gates) were not probed. |
| Is there a bd query for the blocked set? | `bd blocked [-l <label>...] --json` returns each blocked bead with `blocked_by` (ids) and `blocked_by_count`, including blockers outside the label filter and inherited parent-child blocking. A child of a blocked parent lists the **parent** as its blocker (`pr-9b1` blocked_by `[pr-llw]`, its parent), not the parent's blockers. Row keys: `blocked_by`, `blocked_by_count`, `created_at`, `created_by`, `id`, `issue_type`, `labels`, `owner`, `priority`, `status`, `title`, `updated_at`. `bd ready` takes the same label filters. |

## Consequence

A cache cannot get blocked state from list rows at all, so ADR-BDC-0002's
earlier design (read `is_blocked` from cached rows and re-read dependers when
it flips) does not work. bd's own `bd blocked` answers the question directly,
so the cache asks it once per scope per sync instead of reimplementing bd's
blocking rules. See ADR-BDC-0002.
