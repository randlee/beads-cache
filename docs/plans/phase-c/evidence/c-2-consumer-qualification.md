# C2 actual scratch consumer qualification

**Result: PASS for the supported workflow on the designated macOS arm64 scratch
testbed.** Tested application source:
`623218021d27ef247b389233650314896b69cbc1`, branch
`sprint/c-2-framework-application`. This receipt adds documentation only; it does
not change or rebuild that source. Parent QA, hosted CI, parent PR merges and the
unsuppressed advisory gate remain separate completion requirements.

The owner approved the bounded transfer and scratch workflow. The parent chat
performed transport; the developer analyzed the returned raw log and asserted
each required result. The final driver reported `qualification_result: PASS`,
the wrapper reported `driver_pass`, transport returned 0, and marked teardown
reported `scratch_removed` with `exists: false`.

## Source and installed artifacts

The immutable framework remains sc-runtime 0.1.1 and template commit
`5af24c862fe7ed8dae8710b7bd64a84a29506b51`, with recorded answers and release
registry dependencies. Parent heads in the tested source are C1
`4d27523d2919b8e0c0e6d0f9287c5e4fc9736e56` and C3
`ef181f69944cad0356bb5f3ee4a04f7c92656730`.

Artifacts were built at the exact clean, pushed source commit above:

```sh
cargo build --locked --offline --release -p bd-cache-cli -p bd-cache-daemon --bins
maturin build --locked --offline --release \
  --manifest-path crates/bd-cache-py/Cargo.toml --out /tmp/c2-qualified-artifacts
```

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| bd-cache | 10196176 | 97b7879e6f33202bfc31e4e4f2c1a346ea79900a72bf14e1978e89899758152d |
| bdc | 10196144 | 7423f41d4c3392e093cd32cd304f12c1c3f3cf4c902b9635482c0c9799066d36 |
| bd-cache-daemon | 11791744 | 27a768fb8b7ceba5a05d6f9aca2843042d2b35d0f7271c968b10cd0db6cc6c8f |
| bd_cache_rs-0.2.0-cp39-abi3-macosx_11_0_arm64.whl | 3031634 | af86d7b1f4eae8e814aa7ae414003304af0243ae417d4f31df4ddf14c4160039 |

The remote wrapper verified all 18 payload-file hashes before execution. Payload
archive SHA-256:
`bdc8259d3c9373d46bbb79003c6143d182a2d5f44d96d38e8b0791eec8364806`.
The wheel was force-installed into an owned Python environment and imported from
its installed site-packages. The workflow executed bdc, the daemon and native
Cache. The alias artifact was transferred and hash-verified; alias parity remains
covered by the local CLI suite rather than a separate live alias invocation.

Observed versions: bd **1.3.1 (c1c4b642a)**, Dolt **2.3.1**, Python **3.11.16**,
installed native package **0.2.0**, sc-compose **1.7.0**, PyYAML **6.0.3**,
pydantic **2.14.2**, pydantic-core **2.50.2**. The host's pre-existing bd 1.3.2 dev
was not used. The existing installer downloaded and checksum-verified 1.3.1 into
owned temporary storage. No permissive fingerprint override was used.

## Exact bounded procedure and external bridge

The approved parent command was:

```sh
python3 /tmp/c2-qualification-transport.py
```

It transferred only the approved modules, native artifacts and bounded drivers
to the designated input directory, verified the archive, then invoked:

```sh
/opt/homebrew/bin/python3.11 /tmp/c2-qualification/input/c2-execute-qualification.py
```

The wrapper installed the exact wheel and public import prerequisites, ran the
existing `setup-scratch.sh`, and required both `.sc-bd-live-root` and
`.beads/sc-bd-live-scratch` before live calls. The resulting disposable Git/bd
workspace used its own loopback Dolt server, data directory and configuration.
Only fresh scratch beads and scratch consumer configuration were created.

The actual existing consumer source remained unchanged at external commit
`d21a1b60de465b2f5abde8b36f8b2cbf2155011c`; the source module SHA-256 was
`921fd84be0c2e9fab97c5b90245d1d6455fd98efc213f6ce84b3224dd5e9e32f`.
Its supported existing write method was invoked with its real constructor and
an explicitly supplied provider. Full orchestration and messaging were refused.
Create-dependent product behavior from issue 104 was excluded; the direct bd
create command only seeded a new disposable task for the supported writes.

Direct native substitution was tested and failed honestly: native write requires
`precondition`, and native drain requires two budget arguments. The owner-approved
external bridge supplies `precondition=None`, drain start gate 25 seconds and hard
deadline 28 seconds, and translates the native exception class to the caller's
existing exception class. It forwards business operations and native returned
dictionaries unchanged. It contains no replacement cache, scheduler, success
fabrication or business-result reshaping. The product's native 0.1 signatures
remain unchanged. This qualifies the existing method **through that explicit
bridge**, not direct drop-in compatibility or full consumer adoption.

Representative actual commands, using the disposable repository and the pinned
binary, were:

```sh
/tmp/c2-qualification/bd-pinned/bd create 'Native qualification task' --type task --json
/tmp/c2-qualification/input/bd-cache-daemon --repo "$SCRATCH_REPO" --config "$SCRATCH_ROOT/daemon.json"
/tmp/c2-qualification/input/bdc --repo "$SCRATCH_REPO" \
  --bd /tmp/c2-qualification/bd-pinned/bd issues get live-bi0 --miss-timeout-ms 0
/tmp/c2-qualification/bd-pinned/bd show live-bi0 --json
```

The exact scratch root was
`/private/tmp/c2-qualification/scratch-runs/sc-bd-live.QNebwc`; its repository was
`ws/`. Native public calls configured the every-second built-in schedule, selected
Python replacement sync and inspected journal entries and schedule state. The
outage stopped only the PID recorded by this scratch setup and restarted the
same server/data directory. Marked teardown removed the complete scratch root.

Setup was bounded at 180 seconds, environment installation at 240 seconds,
the live driver at 600 seconds, teardown at 40 seconds, and overall transport at
1500 seconds. Each external command also had a finite timeout. Actual driver
execution was **7.829 seconds**, excluding installation and scratch setup.

## Observed outcomes

| Required behavior | Actual observation |
|---|---|
| Scheduled built-in sync and local read | By elapsed 1.6121 s, the persisted built-in row had run successfully, the native/CLI cache contained the scratch task, and `/ready` was 200 with `probe:null`. |
| Existing consumer supported write | The unchanged method updated status; journal entry 1 was applied and independent bd read-back was `in_progress`. The method took 0.3546 s. |
| Close and drain/read-back | Entry 2 was applied; independent bd read-back was closed with the requested reason. |
| Outage and pending preservation | With scratch Dolt stopped, the caller reported pending rather than applied. Entry 3 stayed pending with `applied_at:null`; local cached reads retained the row and pending assignee overlay. |
| Liveness/readiness separation | During the recorded outage, `/health` stayed 200 while `/ready` was 503, state down, with one pending entry and the recorded backend error; `probe` remained null. |
| Post-sync outage delivery | The real hook received a scope result `outage`, `BD_CACHE.BD_UNAVAILABLE`, `outage:true`, and the pending count. |
| Recovery | Restarting the same scratch server and retrying the same idempotent operation applied entry 3; independent bd read-back contained the requested assignee, pending became zero, and `/ready` returned 200. |
| Hook failure | A deliberate hook exception produced one consecutive hook failure and the recorded native error. |
| Python replacement | Installed Python sync returned exit 0, ingested its declared snapshot, enabled the Python row and disabled the built-in row. A separate daemon-worker run ID showed that the scheduled Python row also ran. |
| Built-in refusal during Python mode | Public tick returned `BD_CACHE.PRECONDITION` with `reason:python_sync_enabled`. |
| Python error rollback | A deliberately failing immediate Python switch returned `BD_CACHE.TICK_RUN_FAILED`, reason exit, status 1 and `restored:true`; built-in mode was restored. |
| Blocked child cleanup | A blocked real Python hook was stopped with the daemon. Shutdown took 1.0046 s with worker grace T=1 s. A subsequent ps query for its PID returned status 1 and no row: the child no longer existed. |
| Restart and daemon-free reads | A new daemon reached readiness 200; a second stop took 0.0143 s. A native cached read after stop retained the closed task and had no pending entries. |
| Cleanup | Transport returned 0, driver PASS and wrapper driver_pass were present, and marked scratch teardown reported `exists:false`. |

The record named `post_sync_success` repeats the successfully delivered outage
request; it is not presented as a separate healthy-scope result or captured hook
return value. The live blocked hook had no grandchild. The local FakeBd process
suite separately verifies child and grandchild reap and ownership retention.
The short live stop did not exhaust the runtime's 31-second HTTP drain maximum;
that maximum and the admitted-read budget remain covered by the pinned runtime
contract and local lifecycle/store tests.

## Preserved evidence and limits

Raw attempt 3 is preserved outside the product tree at
`/tmp/c2-qualification-attempt3.log`: 25295 bytes, SHA-256
`df1541ad2a18c26e42fbf8dd7f318f1bc081007e0991f99ba90f1f059b0cf43a`.
External source paths, consumer configuration and identifiers remain outside the
product tree. The structured neutral receipt is
[c-2-consumer-qualification.json](c-2-consumer-qualification.json).

Attempts 1 and 2 were retained, not counted as passes. Attempt 1 used an unsupported
update field; native validation correctly rejected it. Attempt 2 set allowlisted
environment values after daemon startup; the existing process could not inherit
them. A local causal check reproduced the missing-value hook error and succeeded
with values present before spawn. Both failed scratch workspaces were removed.
The final run used a supported assignee update and inherited hook values before
startup; no product API or business behavior was changed for these corrections.

All five C2 implementation deliverables are now present. This does **not** assert
hosted OS/Python CI or QA completion, merge eligibility, a full consumer migration,
create support, or publication. RUSTSEC-2024-0436 remains unsuppressed under the
immutable 0.1.1 runtime pin. The owner's instruction is to leave that external
blocker until sc-runtime 0.2.0, followed by an explicitly approved baseline change
and revalidation; no workaround or advisory ignore is adopted here.
