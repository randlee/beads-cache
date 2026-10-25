# bd pin locations

Checked on `origin/integrate/phase-a` at `71084a5`, pin `1.3.1`. Re-check before
each bump, since this list goes stale:

```bash
git grep -n "1\.3\.1" origin/integrate/phase-a -- . ':!*.json'
git ls-tree -r --name-only origin/integrate/phase-a | grep "1\.3\.1"
```

## Canonical pin

`crates/sc-beads/src/row/generated.rs`: `BD_VERSION`, `BD_SCHEMA_VERSION` and
`BD_SCHEMA_SHA256`. The file is generated, so do not edit it by hand.
`scripts/bd-compat/lib.sh` (`bdc_pinned`) reads the pin from this file, and so
do the workflow and `fingerprint-check.sh`. The generator is
`crates/sc-beads/tests/strict_rows_generated.rs`, which holds
`PINNED_BD_VERSION` (line 27) and `FIXTURE` (line 28). Regenerate with:

```bash
SC_BEADS_REGENERATE=1 cargo test -p sc-beads --test strict_rows_generated
```

## Fixtures (directory or file name carries the version)

| Path | Notes |
|---|---|
| `crates/sc-beads/tests/fixtures/bd-1.3.1/` (`bd-schema.json`, `list-rows.json`, `PROVENANCE.md`) | `fingerprint-check.sh` reads `bd-<pinned>/bd-schema.json`, so the new directory must exist |
| `crates/sc-bd-cli/tests/fixtures/bd-schema-1.3.1.json` | byte copy of the schema; `schema_copy_is_the_pinned_schema` checks it |
| `crates/bd-cache-py/tests/fixtures/bd-schema-1.3.1.json` | Python test copy |

## Code that names the pinned version or fixture path

- `crates/sc-beads/src/testing/fake.rs:22`, which uses `include_str!` on the fixture
- `crates/sc-beads/tests/strict_rows.rs:16-17`, which uses `include_str!` on the fixtures
- `crates/bd-cache/tests/common/mod.rs:31`, which uses `include_str!` on the sc-beads fixture
- `crates/bd-cache/tests/helpers_backend_check.rs:98`, which asserts `bd_version == "1.3.1"`
- `crates/sc-bd-cli/src/testing/fake_bd.rs:36-40`, a doc comment plus `include_str!`
- `crates/sc-bd-cli/tests/cache_calls.rs:133,136` (the fixture plus a fake `bd version`
  line that is asserted equal to `BD_VERSION` at line 140). Lines 179 and 205 hold fake
  version strings in error-path tests and are optional
- `crates/sc-bd-cli/tests/process.rs:35`, which uses `include_str!`
- `crates/bd-cache-py/tests/conftest.py:22-23` and `crates/bd-cache-py/tests/test_verbs.py:214`
- `crates/bd-cache-model/src/testing/contract.rs:156`, a literal in a test fingerprint.
  TODO: confirm whether it must track the pin

## Prose (update to name the new pin)

`crates/sc-beads/README.md:12`, `crates/sc-beads/src/lib.rs:27`,
`crates/sc-beads/src/row.rs:4`, `docs/architecture.md:24`, `docs/project-plan.md:18`,
`docs/requirements.md:33`, `docs/sc-bd-cli/architecture.md:163`,
`docs/sc-beads/requirements.md:44`. Historical records (sprint docs, evidence,
`docs/baseline/`, `docs/research/`) stay as they are.

## Fixture capture

No capture script exists in the repo.

- `bd-schema.json`: the full stdout of `bd schema` from bd X. Run
  `"$(scripts/bd-compat/install-bd.sh X)/bd" schema`, or take `bd-schema.json` from
  the hosted run's `bd-compat-X` artifact. Keep it LF, as `.gitattributes` requires.
- `list-rows.json`: three rows recorded from `bd list --json` (database `ct`). TODO:
  there is no recipe or script in the repo. Recapture by hand in a scratch workspace
  (`scripts/bd-compat/setup-scratch.sh`) and record how in `PROVENANCE.md`. Until
  then, carry the 1.3.1 rows forward and say so in `PROVENANCE.md`.
