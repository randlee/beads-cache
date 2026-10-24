# Installation and troubleshooting

## Check First

```bash
which gh git ssh && gh --version | head -n 1 && git --version
gh auth status            # ambient account; never pass account flags
ssh -o BatchMode=yes -o ConnectTimeout=10 atmbench@rand-m5 true && echo testbed-ok
```

## Find Existing Install

```bash
for p in "$(brew --prefix 2>/dev/null)/bin/gh" "$HOME/.local/bin/gh"; do
  [ -x "$p" ] && echo "Found at: $p" && break
done
```

## Install

- `gh`: `brew install gh` on macOS. On Linux, see https://cli.github.com.
- `git` and `ssh`: these ship with macOS (Xcode command line tools) and with Linux distributions.
- `bd` and `dolt` are not installed locally. The skill installs them on the
  testbed with `scripts/bd-compat/install-bd.sh` and `scripts/bd-compat/install-dolt.sh`.
- `atm` is optional and only needed to send the report. Without it, give
  the report in chat.

## Minimum Version

`gh` must support `gh workflow run -f` and `gh run watch --exit-status`. Version
2.98.0 is known to work.

## PATH Troubleshooting

Claude Code's bash may not load the shell's init files. If `gh` is found only
at a full path, run `export PATH="$(brew --prefix)/bin:$PATH"`.

On the testbed, a non-login ssh shell adds only `~/.cargo/bin` to the system PATH (no Homebrew), and the
Homebrew `bd` and `dolt` there are a dev build of 1.3.2 and 2.3.1. The runner
script therefore puts its own `~/bd-<X>` and `~/dolt-<v>` first on PATH and sets
`SC_BD_BIN`.

## Validation

`gh workflow list` shows `bd-compat` as active. On the testbed,
`~/bd-<X>/bd version` prints `bd version <X>`.

## Known Issues

- `gh workflow run bd-compat.yml` without `--ref` fails with
  `HTTP 422: Workflow does not have 'workflow_dispatch' trigger` until `main` has
  the file. A `repository_dispatch` sent before then is accepted but starts nothing.
- Inline multi-line `ssh` commands broke on quoting. Copy a script with `scp` and
  run it with plain arguments.
- `ssh` with `BatchMode=yes` fails rather than prompting. If the key is refused,
  use the `ssh-connect` skill and stop.
