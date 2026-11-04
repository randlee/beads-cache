C1 Windows job126408082388 completed SUCCESS for exactbcfcae7021522454e85be964661b9525bb15e58c. Raw log: 200363bytes, SHA2565df1bb652fdb692b44c7e0bab4ee1927b5ee594725efcade10d78df1bb2f277b, matches parent hash.

Actual store result34/34, whole group15.81s (line1282); all5 shutdown tests pass (lines1263-1266,1280). Source interpretation: the passing stalled supervisor satisfied original post-call/stall and500ms failure markers, kill/reap receipts and watchdog assertions for all3 child paths in this occurrence. Raw individual phase timings/results are absent.

ZERO RQA9, FTQ-009, post-call-stall or500ms diagnostic strings occur in this successful job log. The outer CI cargo test harness captures and suppresses successful parent output; inner child --nocapture does not disable outer capture. Developer overlooked this outer capture when printing captured child traces on pass. No phase timing or result has been invented.

The original C2Windows33/34 failure is still unexplained. Its missing marker cannot show whether setup, real operation/cancellation/restoration or marker recording consumed the original5s. Current pass is one qualified occurrence, not evidence of cause or correction. C2e86d9da terminal observation remains parent-owned.

To obtain actual raw Windows phases, a parent-approved bounded exact-source invocation must pass --nocapture/--show-output to the outer exactly filtered test. Existing successful scratch/output was removed by normal ownership, so omitted output cannot be retrieved from the current job log. No new probe, source/Cargo/workflow change or polling performed. All5 findings OPEN; Cargo RELEASED, zero active commands.

Actual checkout recorded at raw lines111/114 is synthetic PR merge42683df11e5c0999f28eb87703c13a4b7fc17d48 of headbcfcae7 into base63ebc1fa. Head already consumes that base; source equivalence is expected, but hosted blob comparison was not independently fetched in this continuation.
