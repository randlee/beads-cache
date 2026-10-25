---
id: b-5
phase: b
status: ready
closure_type: integration
target_boundary: release manifest and publish channels
track: release
wave: 3
branch: sprint/b-5-release-channels
worktree: sprint/b-5-release-channels
recommended_agent: rust-developer
recommended_model: sonnet
must_follow: [b-3, b-4]
parallel_safe: []
requirements:
  owns: [REQ-BDC-0813]
  constrained_by: [REQ-BDC-0805, REQ-BDC-0812, REQ-PY-0010]
adrs: [ADR-BDC-0009, ADR-CLI-0001]
owned_paths:
  - "release/**"
  - "scripts/prerelease_tag.py"
  - "scripts/test_prerelease_tag.py"
  - ".github/workflows/{release,release-preflight,release-candidate,prerelease-archive,homebrew-publish,scoop-publish,winget-publish,crates-publish,pypi-publish,npm-publish}.yml"  # through install.py only
  - "README.sc-publish.md"   # through install.py only
---

# Sprint b-5 — binaries on GitHub, Homebrew, Scoop and WinGet

## Outcome

`bd-cache` and `bdc` ship as release binaries for Linux, macOS (arm and
intel) and Windows. They are published to the GitHub Release, Homebrew,
Scoop and WinGet, and a GitHub prerelease carries them. This follows
sc-publish's getting-started guide, Destinations and Checklist sections. The
crates.io and PyPI legs are unchanged.

## Deliverables

1. `release/sc-publish-consumer-input.json`:
   - **`release_binaries`**: `[{"name": "bd-cache"}, {"name": "bdc"}]`. These
     are Cargo `[[bin]]` names in the `publish = false` crate `bd-cache-cli`.
     `fake-bd` is not listed.
   - **`release_targets`**: `x86_64-unknown-linux-gnu` (tar.gz),
     `aarch64-apple-darwin` and `x86_64-apple-darwin` (tar.gz), and
     `x86_64-pc-windows-msvc` (zip).
   - **`project.renderer_archive_path`**: `bin/bd-cache`.
   - **`channels.homebrew`**: tap `randlee/homebrew-tap`; formula
     `Formula/bd-cache.rb`, class `BdCache`, binaries `["bd-cache", "bdc"]`;
     test `bdc --version`, which must print `bd-cache`. Uses the shipped
     template.
   - **`channels.scoop`**: bucket `randlee/scoop-bucket`, manifest
     `bucket/bd-cache.json`. The template is a copy at
     `release/scoop/bd-cache.json.j2` whose `bin` lists both
     `bin/bd-cache.exe` and `bin/bdc.exe`, because the shipped starter takes
     one binary.
   - **`channels.winget`**: identifier `randlee.bd-cache`, installer target
     `x86_64-pc-windows-msvc`.
   - **`prerelease`**: `binaries: ["bd-cache", "bdc"]`. `post_install` keeps
     the wheel install and adds `{selector_dir}/bdc --version`; `verify` runs
     `{selector_dir}/bdc --version`.
2. Re-run `install.py`, and then `install.py --dry-run`, which must exit 0.
   Commit the rendered `release/publish-*.toml` and any kit file that
   changes. Never edit a kit file by hand. The pin stays at `16457e0`,
   unless P-4's fix needs a newer pin.
3. File the P-4 staging issue on randlee/sc-publish, and link it in the PR.
4. A prerelease cut with `/prerelease` from the sprint branch, carrying both
   binaries for every target and the 0.2.0 wheels.
5. Secrets and external repositories, for P-6:
   - `HOMEBREW_TAP_TOKEN` for randlee/homebrew-tap;
   - `SCOOP_BUCKET_TOKEN` for randlee/scoop-bucket;
   - `WINGET_GITHUB_TOKEN` for the winget-pkgs fork, plus the first
     `randlee.bd-cache` submission made by hand;
   - the crates.io and PyPI environments, which are unchanged;
   - immutable releases enabled.

   Before dispatch, `preflight-secret-plan` lists exactly these.

## Acceptance criteria

| # | Root | Positive observable | Planted negative |
|---|---|---|---|
| 1 | REQ-BDC-0813 | `validate-manifest`, `verify-version-lockstep` and `install.py --dry-run` exit 0. | List `bd_cache`, which is not a `[[bin]]` name → `validate-manifest` fails. |
| 2 | REQ-BDC-0813 | `preflight-secret-plan` names `HOMEBREW_TAP_TOKEN`, `SCOOP_BUCKET_TOKEN` and `WINGET_GITHUB_TOKEN`, besides the existing crates.io and PyPI ones. | Run Release Preflight with `HOMEBREW_TAP_TOKEN` unset → it fails, and names the secret. |
| 3 | REQ-BDC-0805 / 0813 | The prerelease's assets include, for all four targets, archives that contain `bin/bd-cache` and `bin/bdc` (`.exe` on Windows), plus `checksums.txt`. `/prerelease --install` and then `bdc --version` print the envelope with `contract_version`. | Leave `bdc` out of `release_binaries` → the asset listing check fails. |
| 4 | Scoop template | The Scoop manifest rendered from the copy lists both executables in `bin`. | Use the shipped starter → only one executable is listed, and the render check fails. |

## Closure while P-4 is open

The sprint closes on deliverables 1 to 5 and criteria 1 to 4. No stable
`release.yml` run starts until the kit pin contains the P-4 fix. The PR says
which legs ran (the prerelease and the dry run) and which wait (the stable
release and the channel dispatches).

## Dependencies

b-3 (the binaries) and b-4 (the 0.2.0 wheel, which the prerelease carries).
External: P-3, P-4, P-6.
