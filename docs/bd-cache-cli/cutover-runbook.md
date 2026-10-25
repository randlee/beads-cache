# Cutover runbook: atm-bd from `bd_cache.py` to bd-cache 0.2

One runbook for moving atm-bd's cache from `bd_cache.py` to bd-cache 0.2
(`bd-cache-rs`, `bdc`). The contract is [`contract.md`](contract.md); §10 there
lists what differs for a `bd_cache.py` caller. This page is the order of
operations.

## Owners

| Owner | Owns |
|---|---|
| atm-bd | The script switch: `atm-task-close.py`, `sanity_task_close_hook.py`, `close-vars.py`, `close_fields_backfill.py`, `phase_beads.py`, the cron registration, the wrapper `bd` executable, and the rollback decision. |
| beads-cache | The wheel `bd-cache-rs`, the `bdc` binary, and the ATM cron shim `python -m bd_cache_rs.cron`. |

## Names that land in the fix/fen-tick layer

Two names in this runbook are implemented by the fix/fen-tick layer and are
not in a build until it merges. Do not run the steps that use them before the
release that contains it.

| Name | Where it is used |
|---|---|
| `python -m bd_cache_rs.cron` | step 4, the cron module |
| `[tick] identity` (default `atm-bd`) and `bdc cache tick --identity ID` | step 1, the sender of the tick's messages |

Everything else below is in `crates/bd-cache-cli/src/args.rs` and
`crates/bd-cache-cli/src/commands/` today.

## Rules that hold throughout

- **One drainer per cache file.** `bd_cache.py` and bd-cache share the same
  drain lock and the same file, but they must never drain the same journal at
  the same time. The ATM cron of `bd_cache_cron.py` is stopped (step 3) before
  anything of bd-cache drains.
- The file stays at store `user_version` 3 and bd-cache's columns are
  additive and nullable, so the Python hooks can still open it. That is what
  makes rollback possible.
- bdc never searches upward for `.beads`; run it with `--repo DIR` or from the
  repository root.

## 1. Install

1. Install the binary, one of:

   ```sh
   brew install randlee/tap/bd-cache
   scoop bucket add randlee https://github.com/randlee/scoop-bucket && scoop install bd-cache
   winget install randlee.bd-cache
   ```

   or the `bd-cache` / `bdc` archive from the GitHub Releases page. Check it:

   ```sh
   bdc --version
   ```

2. Install the wheel into the Python that runs the atm-bd scripts and the
   cron:

   ```sh
   pip install bd-cache-rs        # import bd_cache_rs
   ```

   Both need the 0.2.0 release; until it is published, neither channel has
   them (a source build needs the pinned Rust toolchain on every host).

3. Optional, once fix/fen-tick has landed: in `.beads/bd-cache.toml` set the
   tick's sender. `atm-bd` is the default, and ATM refuses a message from
   `cron@tool`:

   ```toml
   [tick]
   identity = "atm-bd"
   ```

   or `bdc cache tick --identity atm-bd`.

## 2. Point bd-cache at atm-bd's existing file

The default file is `.bd-cache/<dolt_database>.bd-cache.sqlite3`. atm-bd's is
`.atm-bd/<db>.bd-cache.sqlite3`. Without a `cache_path`, bd-cache would create
a second file with its own journal and its own drain lock, and the entries
still pending in the `.atm-bd` file would be stranded. In
`<repo>/.beads/bd-cache.toml`:

```toml
cache_path = ".atm-bd/<db>.bd-cache.sqlite3"
```

(`<db>` is `dolt_database` from `.beads/metadata.json`; a relative path is
resolved against `--repo`.) `bdc` refuses with `CONFIG_INVALID` while no
`cache_path` is set, the default file does not exist and the `.atm-bd` file
does, so a missing line is an error and not a silent fork. Check what bdc
opens:

```sh
bdc --repo <repo> cache status
```

`data.cache_path` must be the `.atm-bd` file, and `data.journal` the counts
you expect. `bdc --repo <repo> --cache-path .atm-bd/<db>.bd-cache.sqlite3 cache status`
does the same without the TOML.

Record the numbers you will compare against in step 5:

```sh
bdc --repo <repo> cache journal list --unannounced
bdc --repo <repo> cache journal list --state pending
bdc --repo <repo> cache health
```

## 3. Stop the `bd_cache_cron` cron

```sh
atm cron stop <the bd_cache_cron registration>
```

(atm-bd's command; use the name the cron is registered under in ATM.) Wait
for a tick that is running to finish: a drain in flight holds the drain lock,
and `bdc cache flush` reports `busy: true` while it does.

Then drain what is left from one place:

```sh
bdc --repo <repo> cache flush
```

`data.busy` must be `false`. Entries that stay `retrying` or `deferred` are
fine; they go on the next tick.

## 4. Switch the scripts and the cron

atm-bd switches these, in this order, in one change. After each, the old
`bd_cache.py` import is gone from that script.

| Script | Switch to |
|---|---|
| `atm-task-close.py` | `bd_cache_rs.Cache(repo, bd_binary=, cache_path=)`; the writes go through `cache.beads.issues_close` / `issues_update` (or the unchanged 0.1 `write`) with an idempotency key. |
| `sanity_task_close_hook.py` | the same; its detached drain becomes a detached `bdc cache flush` **whose stdout is logged to a file**, never sent to `DEVNULL`: the envelope is where an error is reported (contract §10, I6). |
| `close-vars.py` | one `Cache` per process, reused (contract §10, M1: a second handle in one process is `STORE_IN_USE`). |
| `close_fields_backfill.py` | `cache.beads.issues_list(scope=, all=True)` or the unchanged 0.1 `list`: 0.2 `issues_list` drops closed, template and gate rows by default (contract §5.3). |
| `phase_beads.py` | the same as `close_fields_backfill.py`; and check `outage` on a sync report, since a sync that cannot reach bd no longer raises `SYNC_FAILED` (contract §10, I1). |
| the cron | register `python -m bd_cache_rs.cron` (lands in the fix/fen-tick layer) in place of `bd_cache_cron.py`. It speaks ATM's `cron.tick`, `cron.stop` and `doctor` protocol and runs the `bdc cache tick` job: drain, sync the active phases, announce. |

Where a script passed `runner=` (environment, retries, the attempt log),
there is no 0.2 equivalent: put those in a wrapper executable and pass it as
`bd_binary` (or `bdc --bd PATH`; contract §10, I3).

Then start the new cron (atm-bd's command).

## 5. Verify

After the first tick of the new cron:

```sh
bdc --repo <repo> cache journal list --unannounced
bdc --repo <repo> cache status
bdc --repo <repo> cache health
```

- `cache journal list --unannounced` lists the escalations nobody has been
  told about. Expect the escalations that were open before the cutover to be
  announced once now. An outage that was current at cutover is announced
  **once more** (the dedupe is per scope, and `outage_announced_at` is not
  carried over; contract §10, I4).
- The pending counts: `data.journal.pending` of `cache status`, and
  `data.open_work.pending` of `cache health`, fall from the step-2 numbers to
  `0`, or to the entries written since.
- `cache health` is `ok`. `degraded` has a `reasons[]` (contract §6):
  `pending_too_old` means the drainer is not running (is the new cron
  registered, and is the old one stopped?), and `unresolved_escalations`
  means a lead has to settle an entry:

  ```sh
  bdc --repo <repo> cache journal list
  bdc --repo <repo> cache journal resolve <seq> --note "<what was done>" --actor <who>
  ```

  `oldest_pending_age_s` and `last_drain_at` show how long since the last
  drain.
- `bdc --repo <repo> cache health --probe` checks that bd answers (it records
  bd's fingerprint, and a changed schema rebuilds the cache; the journal is
  kept).

## Rollback

The file stays readable by `bd_cache.py`, so rollback is the cutover in
reverse, with the same rule: one drainer.

1. Stop the new cron (`python -m bd_cache_rs.cron`) and wait for a running
   tick to end.
2. `bdc --repo <repo> cache flush`, and `data.busy` is `false`, so nothing of
   bd-cache is mid-drain.
3. Restore the scripts (revert atm-bd's switch commit) and re-register
   `bd_cache_cron.py`.
4. Leave `cache_path` as it is; `bd_cache.py` reads the same `.atm-bd` file.
   An entry that bd-cache resolved shows without its note in `bd_cache.py`'s
   `entry()` (the note is its own column; contract §10, M2).
5. Run `bdc --repo <repo> cache journal list --state pending` once: nothing
   is lost, because the journal is in the file.
