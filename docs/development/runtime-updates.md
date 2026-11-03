# Reviewing runtime template updates

REQ-BDC-1003 uses a pristine generated baseline to review framework drift.
`runtime/pin.toml` records the repository, immutable source tag and full commit,
cargo-generate 0.25.1 and sc-compose 1.7.0. `runtime/answers.json` records the
approved answers: `project_name=bd-cache`, `db=none`, `mcp=false`, `licence=MIT`.
The helper requires Python 3.11+ and the `jsonschema` package with Draft 2020-12
support. The tagged upstream generation requirements use jsonschema 4.26.0.

The helper verifies the tag against the SHA and exports committed `template/`
and `wizard/answers.schema.json` Git objects into temporary storage. A local
`--source` supplies Git objects only: its uncommitted files are never a baseline.
Omit `--source` to clone the recorded repository into temporary storage.
Answers are validated against the exported schema before rendering. Generation
requires the exact recorded cargo-generate and sc-compose versions.

```sh
# Choose an absent output; mktemp -d creates only its parent.
scratch=$(mktemp -d)
python3 -I scripts/runtime_templates.py generate \
  --source /absolute/path/to/sc-runtime --output "$scratch/old"
python3 -I scripts/runtime_templates.py check "$scratch/old"
```

Generation runs the tagged renderer with a values file, `--name bd-cache`,
`--init --silent --allow-commands --force --no-workspace`, then `cargo fmt --all`
inside its owned scratch project and separate sc-compose renders for
`AGENTS.md.j2` and `CLAUDE.md.j2`. The `.j2` inputs are removed after rendering.
The baseline check rejects dependency patches/replacements and path/Git-sourced
framework dependencies; internal application crate paths remain valid.

There is a destination detail in upstream v0.1.1: `docs/generation.md`, ordered
step 5, says to create the absent destination before invoking cargo-generate.
`scripts/new_project.py:cargo_generate` implements that `mkdir`. The installed
0.25.1 renderer fails with `Error: No such file or directory (os error 2)` when
`--destination` is literally absent, including with `--init`. Therefore the
helper refuses any existing **caller output**, creates one empty **owned scratch
render directory**, and only copies the checked result to an exclusively created
new caller output. Existing files, directories and dangling symlinks are refused;
the caller output must be outside this application in temporary storage. Failed
generation removes scratch, leaves caller output absent and preserves source and
application files. Upstream stdout, stderr and exit status accompany failures.

Do not run `scripts/new_project.py` or generated `just setup`, `lint`, `test`,
or build recipes for upkeep. That driver bootstraps sc-lint, which remains
deferred until sc-runtime 0.2.0. The pristine scratch tree includes upstream lint
configuration as comparison data; it is never installed into beads-cache.

For an upgrade, retain the old pin/answers, review and record a new immutable
tag/SHA and tool versions in another pin file, then generate a second pristine
baseline with `generate --pin /path/to/new-pin.toml --output "$scratch/new"`.
Use the same answers unless an approved change calls for different ones. Compare:

```sh
python3 -I scripts/runtime_templates.py diff "$scratch/old" "$scratch/new"
# Or select an adoption mapping explicitly:
python3 -I scripts/runtime_templates.py diff "$scratch/old" "$scratch/new" \
  --mapping runtime/adoption.toml
```

The default mapping is `runtime/adoption.toml` when present. Before c-2 adopts
the application it may be absent; every change is then explicitly unadopted.
c-2 owns that file and records exactly which generated files informed each
consumer-owned destination:

```toml
version = 1

[[paths]]
generated = "crates/daemon/src/composition.rs"
adopted = "crates/bd-cache-daemon/src/composition.rs"

[[paths]]
generated = "crates/daemon/src/routes.rs"
adopted = "crates/bd-cache-daemon/src/routes.rs"
```

Each side is one exact relative file path using `/`. Absolute paths, `..`,
globs, directories, duplicate generated/adopted paths and unsupported versions
are rejected. A source absent from both baselines is stale and fails comparison.
A deleted old file retains its mapping; a newly generated file can have a new
mapping. No destination file has to exist or byte-match upstream: mapping assigns
review ownership and never directs a write.

JSON output lists every added, modified or deleted path in sorted order, including
dotfiles and executable-bit changes. Each row includes `generated`, `change`,
`adopted` (a destination or null) and `ownership` (`adopted` or `unadopted`).
Changes exit 0; invalid inputs or generation failures exit 1. Review every mapped
delta against the adapted application code and its requirements, then decide
which upstream changes to port and test. Review unadopted changes for newly useful
files. Update mapping when adoption changes; remove a deleted source's entry only
after reviewing its deletion against the old baseline. Preserve the old baseline
until this review is complete. The helper never overwrites or merges application
code, creates a framework fork or runs an installer. Retire it when a supported
upstream template updater replaces this local maintenance step.

```sh
python3 -I -m unittest discover -s scripts -p 'test_runtime_templates.py' -v
RUNTIME_TEMPLATE_SMOKE_SOURCE=/absolute/path/to/sc-runtime \
  python3 -I -m unittest discover -s scripts -p 'test_runtime_templates.py' -v
```

The first command runs small local fixture tests (no bd, Dolt or network). The
second additionally runs the real pinned generator and document renderer in an
absent scratch output. No application build or generated recipe is invoked.
