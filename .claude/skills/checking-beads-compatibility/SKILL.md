---
name: checking-beads-compatibility
version: 0.1.0
description: >
  Check a new beads/bd release against beads-cache from start to finish: a hosted
  bd-compat workflow run, live runs on the atmbench@rand-m5 testbed, a COMPATIBLE
  or BREAKING verdict, then a pin-bump PR or a break report. Use when a bd release
  is announced ("run the beads-compatibility skill for 1.3.2", a `beads-release`
  repository_dispatch, a release note from grecon@hermes).
---

# Beads Compatibility

Input: bd version `X`, a semantic version such as `1.3.2`. Strip any leading `v`.
The person running the skill decides the gates. Ask before opening a PR or sending
a message. This is an interactive skill and delegates to no agents.

Rules:
- Never publish anything: no crates.io, no PyPI, no release tags.
- Never push directly to `main`, `develop` or `integrate/*`. A ruleset protects them, so every change goes through a PR.
- Confirm with the user before opening a PR or sending a message.
- Use the ambient `gh` and `git` with no `--user`, `--hostname` or account flags.
- Change no code on a BREAKING verdict.

Paths are relative to the repo root unless they start with `~`, which is the
testbed home. The machinery lives on `integrate/phase-a` until phase-a merges to
`develop`. The a-7 live suite (`crates/bd-cache-sync/tests/run-live.sh`) is still on
the `sprint/a-7-*` PR stack.

## Step 1: Verify the CLIs and the testbed

```bash
which gh git ssh && gh --version | head -n 1 && gh auth status
ssh -o BatchMode=yes -o ConnectTimeout=10 atmbench@rand-m5 true && echo testbed-ok
```

If any check fails, read `references/installation-and-troubleshooting.md` and stop.

## Step 2: Pick the ref and check the pin

```bash
git fetch origin
has() { git cat-file -e "origin/$1:.github/workflows/bd-compat.yml" 2>/dev/null; }
if has main; then REF=main; elif has develop; then REF=develop; else REF=integrate/phase-a; fi
SHA=$(git rev-parse "origin/$REF"); SHA7=${SHA:0:7}
PIN=$(git show "origin/$REF:crates/sc-beads/src/row/generated.rs" | tr -d '\n' \
  | grep -oE 'pub const BD_VERSION: &str =[[:space:]]*"[^"]*"' | sed -E 's/.*"([^"]*)"$/\1/')
echo "ref=$REF sha=$SHA7 pin=$PIN"
```

`scripts/bd-compat/lib.sh` (`bdc_pinned`) reads the pin with the same logic. If
`X == PIN`, report "bd X is already pinned" and stop. Also reject an `X` that does
not match `^[0-9]+\.[0-9]+\.[0-9]+(-[0-9A-Za-z.]+)?$`.

## Step 3: Hosted check (bd-compat workflow)

The workflow has two inputs: `version` and `expect_schema_sha`. Leave
`expect_schema_sha` empty, because it is a test hook. `repository_dispatch`
(`beads-release`) and dispatches without `--ref` work only once `main` has the
workflow. Until then, dispatch with `--ref`.

```bash
gh workflow run bd-compat.yml --ref "$REF" -f version="$X"
gh run list --workflow bd-compat.yml --branch "$REF" --event workflow_dispatch \
  --limit 1 --json databaseId,url,headSha,createdAt
```

Repeat the list command until a run created after the dispatch appears, up to
10 tries a few seconds apart. Check that its `headSha` equals `$SHA`. Then:

```bash
gh run watch "$RUN" --exit-status --interval 30   # the job is capped at 60 min
gh run view "$RUN" --log | grep -E -- '- (fingerprint|contract suite and crate tests|live probes): '
```

The workflow runs a fingerprint check (`scripts/bd-compat/fingerprint-check.sh`),
then `cargo test -p sc-beads -p sc-bd-cli --all-features`, then the live probes with
`SC_BD_PINNED_ANY=1`. If the schema is unchanged, the fingerprint step passes with a
notice even though the version differs. If the schema changed, it fails and attaches
`schema.diff`. Any failure opens or comments on an issue titled `bd-compat: bd X`.
Record the run URL, the three outcomes, and the issue number if there is one.

## Step 4: Live check on the testbed

Ship the exact commit and the runner script (`scripts/run-bd-compat.sh` in this
skill's folder), then run the script with plain arguments. Do not send inline
multi-line ssh commands.

```bash
D="github/beads-cache-compat-$X-$SHA7"
git archive --format=tar "$SHA" | ssh -o BatchMode=yes atmbench@rand-m5 "mkdir -p $D && tar -xf - -C $D"
scp -o BatchMode=yes .claude/skills/checking-beads-compatibility/scripts/run-bd-compat.sh "atmbench@rand-m5:run-bd-compat-$X.sh"
ssh -o BatchMode=yes atmbench@rand-m5 "chmod +x run-bd-compat-$X.sh && ./run-bd-compat-$X.sh $X $SHA7"
```

The script installs or confirms `~/bd-X` (`scripts/bd-compat/install-bd.sh`) and
`~/dolt-2.3.5` (`scripts/bd-compat/install-dolt.sh`, the workflow's `DOLT_VERSION`),
puts both first on PATH, and runs:

1. `scripts/bd-compat/run-live.sh` (pinned)
2. `SC_BD_PINNED_ANY=1 scripts/bd-compat/run-live.sh`
3. `crates/bd-cache-sync/tests/run-live.sh`, the a-7 live suite. It prints
   `NFR-BDC-0003: ... limit 1s: PASS|OVER` and reports the time without asserting it.

The summary is `~/bd-compat-X-SHA7-summary.txt`, and the logs are
`~/bd-compat-X-SHA7-{pinned,pinnedany,a7}.log`. Read the summary with
`ssh atmbench@rand-m5 cat bd-compat-$X-$SHA7-summary.txt`. A run can take 30
minutes, so run it in the background.

## Step 5: Verdict

**COMPATIBLE** requires all of these:
- Hosted: the contract suite and crate tests passed, and the live probes passed.
- Testbed: the pinned run either passed or failed only on
  `fingerprint_reports_what_bd_prints`.
- Testbed: the PINNED_ANY run passed every probe, and the a-7 run passed.

**BREAKING** is anything else. A failed hosted fingerprint step means the schema
moved (see `schema.diff`). It does not make the release BREAKING by itself, but the
bump must regenerate the strict rows. ADR-BDC-0007 expects that fix to ship as a
semver-major version, which is the maintainer's decision. An `OVER` on NFR-BDC-0003
is a finding to report, not a pass. If the a-7 suite is `absent` at `$SHA`, say so,
because the verdict then covers only the a-2 probes.

## Step 6a: COMPATIBLE, prepare the pin bump

Branch `chore/bd-pin-X` from `origin/$REF`. Use `integrate/phase-a` until phase-a has
merged, then `develop`. Make the changes in `references/pin-locations.md`:

1. Create `crates/sc-beads/tests/fixtures/bd-X/` with `bd-schema.json` as the
   `bd schema` stdout of bd X. Get it from the hosted `bd-compat-X` artifact or from
   `"$(scripts/bd-compat/install-bd.sh X)/bd" schema`. Move `list-rows.json` and
   `PROVENANCE.md` into it, and update the provenance. Recapturing `list-rows.json`
   is a TODO, because the repo has no script for it.
2. Set `PINNED_BD_VERSION` and `FIXTURE` in `crates/sc-beads/tests/strict_rows_generated.rs`,
   then run `SC_BEADS_REGENERATE=1 cargo test -p sc-beads --test strict_rows_generated`.
3. Rename the schema copies (`bd-schema-X.json` in `sc-bd-cli` and `bd-cache-py`; these
   two are the only copies, since sprint b-9 removed the store's compat fixtures),
   then update every include path and version literal in the reference file.
4. Run the gates in `.claude/project/quality-policy.md`:
   `cargo fmt --all --check`,
   `cargo clippy --all-targets --all-features -- -D warnings` and
   `cargo test --all-features`.
5. Commit and push `chore/bd-pin-X` only. After the user confirms, open a PR to the
   integration branch with
   `gh pr create --base "$REF" --head chore/bd-pin-X`.

## Step 6b: BREAKING, write the report

Name each failing probe or test and its log line. Name the bd change behind it,
using `schema.diff` and the bd release notes. Name the affected crate, usually
`sc-bd-cli`, or `sc-beads` for a schema or strict-row break. Change no code.

## Step 7: Report

Give the report to Rand in chat:

```json
{"bd_version": "X", "pin": "PIN", "ref": "REF", "sha": "SHA7", "verdict": "COMPATIBLE|BREAKING",
 "hosted_run": "<url>", "hosted": {"fingerprint": "", "tests": "", "live": ""}, "issue": null,
 "testbed": {"pinned": "", "pinnedany": "", "a7": "", "nfr_bdc_0003": "<ms> PASS|OVER"},
 "logs": "~/bd-compat-X-SHA7-*.log on atmbench@rand-m5", "pr": null, "breaking_report": null}
```

If the user names an address, offer to send the report from this machine and send
it only after the user confirms. Rand is reached through `grecon@hermes.rand-m5` for
now. Write the report to a file, then run
`atm send <address> --file <report> --summary "bd X: <verdict>"`. Try `--dry-run`
first.
