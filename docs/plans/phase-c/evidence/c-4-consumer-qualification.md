# C4 installed candidate consumer qualification

**PASS for the supported scratch workflow at tested source
`65250a9ee6de25cf591e85dd2ee5ec4dbc4c584c`.** The parent performed the approved
transport and execution on the designated macOS arm64 testbed. This receipt
records that source only. Later test-only merges and this documentation commit
were not rebuilt or executed by the live run.

## Provenance and installed artifacts

The source includes reviewed C2 parent
`63cb429eb1a65ab25a17923fded8de5ecf390a5a`. Package version is **0.2.0**;
sc-runtime remains **0.1.1**, template source remains
`5af24c862fe7ed8dae8710b7bd64a84a29506b51`, and the canonical sc-publish kit
remains `5fd4d0db1b7cbb4ebf9316588047391a8e4be496`.

All three executable inputs were extracted from the verified candidate archive.
The wheel hash matches the same candidate receipt. The fresh environment imported
the installed package under its venv site-packages. A source-tree Python import
or build-tree executable was not substituted for these installed inputs.

| Artifact | Bytes | SHA-256 |
|---|---:|---|
| bd-cache | 10263600 | 9004db986a5d6d1d6391f2664478ae325e98294cf74eaa9e84cb51e50cb40002 |
| bdc | 10263584 | 939f63d6fac11b5624271b33b28d4a388857e23b9762923a7d066f7efe8689d6 |
| bd-cache-daemon | 11884624 | 0bc375f54145e16225d469d197bf2eda3af2dc074f591632979f91cb73eb3f07 |
| bd_cache_rs-0.2.0-cp39-abi3-macosx_11_0_arm64.whl | 3036395 | fe33046471481010edc086fb790c2f2014c1c2ea807cffbfb80501073e3d5a8b |
| beads-cache-v0.2.0-aarch64-apple-darwin.tar.gz | 12229014 | a6b45b8baa02847fe5f10fd1ef90307e9182090013b1104353437da2ccfb27a5 |

The remote wrapper verified all **22** payload files before execution. Payload
archive SHA-256 is `cc197f52c6d057a40008a80acc25654ace11bbe4389c6e62547e4c6afac52d6f`;
the transfer manifest SHA-256 is
`5e85ad154ca7a988a3357bccac7cc4678621e7abbe14bc26c406b701d2a2f963`.
The parent retains the immutable external modules, exact commands and raw log
outside the product repository. Raw log: **23561 bytes**, SHA-256
`0569280f71b67a24051c1804361381e27d53be1e50a4a026b016a32dfc352cc3`.

Observed tools were bd **1.3.1 (c1c4b642a)**, Dolt **2.3.1**, Python
**3.11.16**, sc-compose **1.7.0**, PyYAML **6.0.3** and pydantic **2.14.2**.
The setup used its checksum-verified pinned bd and a fresh marker-qualified
Git/bd workspace with an owned Dolt server, data and configuration. The external
source remained unchanged and hash-verified. Full consumer orchestration and
messaging were not invoked.

## Actual outcomes

| Procedure | Observed result |
|---|---|
| Built-in schedule and cached access | Scheduled sync completed; installed CLI/Python reads returned cached data and readiness was 200. |
| Existing supported write method | Update journal entry 1 applied; independent backend read-back was in_progress. Close entry 2 applied; read-back was closed. |
| Backend outage | Entry 3 stayed pending with no applied timestamp; cached data and its pending overlay survived. Health stayed 200, readiness became 503 and its probe remained null. |
| Post-sync outage | The hook received the pending count and an outage result with BD_CACHE.BD_UNAVAILABLE. |
| Recovery | Retrying the same operation applied entry 3; independent backend read-back matched the requested value, pending cleared and readiness returned 200. |
| Hook failure | A genuine post-sync exception was persisted as a hook failure. |
| Python replacement | Immediate and scheduled replacement sync succeeded, populated a cached snapshot row and disabled built-in ticking. |
| Failed Python switch | BD_CACHE.TICK_RUN_FAILED reported restored=true; built-in mode was restored. |
| Blocked hook stop | Daemon exited 0; the recorded hook PID was absent after shutdown. |
| Restart and daemon-free read | Restart became healthy; the second stop exited 0; a local cached read still succeeded afterward. |
| Final result and cleanup | Driver PASS, wrapper driver_pass, transport return code 0 and marked scratch_removed with exists=false. |

The driver completed in **9.9981 seconds**. The supported update method took
**0.394278 seconds**. Shutdown with the blocked hook took **1.004024 seconds**;
the second shutdown took **0.001307 seconds**. These are observed durations,
not broader shutdown bounds. Approved outer limits and driver behavior were
retained; only the fresh owned scratch-base literal differed from the previous
approved wrapper/driver pair.

Direct native substitution still raises the expected signature incompatibility.
The explicitly approved external provider bridge supplies precondition=None,
drain start gate **25 seconds** and hard deadline **28 seconds**, and translates
exception classes. It returns native business dictionaries unchanged. This
qualifies the supported workflow through that bridge, not direct drop-in
compatibility or a complete consumer migration.

## Later parent and limits

The later committed C2 test-only correction
`8bcd47fa364055879e06392a53cb250ebfe564f7` is included before QA1. It changes the
readiness test module and adds its Unix lifecycle test child; the parent module
is under cfg(test). All other tracked native, configuration, dependency,
packaging, installer and script input blobs match the tested source. Their
projection fingerprint and scope are recorded in the JSON receipt. This is
source-input comparison, not a claim that the later head was built or that new
Rust tests ran. Initial Rust QA owns an appropriate independent build; the
previous full-workspace attempt aborted during compilation with ENOSPC.

The live procedure invoked bdc and the daemon. The original CLI executable was
transferred and hash-verified; the separate local exact-source installed smoke
exercised both CLI names. The live blocked hook had no grandchild; the local
FakeBd suite provides separate grandchild-cleanup evidence. This positive run
did not contend store work or exhaust the pinned HTTP-drain maximum and does not
qualify Windows CTRL_BREAK or all supported hosted platform/Python combinations.

C3-owned cleanup CI, published runtime/logging/graph/stop dependencies, the
unsuppressed paste advisory, current-head hosted matrix and full preflight,
parent merges and independent zero-finding QA remain separate gates. Missing
channel setup and the deliberately absent release-candidate tag remain truthful
preflight outcomes. No publication, tag, registry/channel write, baseline change,
guard suppression or test skip was used to obtain this result.
