C1 qualified C5 pin delivered and pushed; independent QA13 pending.

- Branch: sprint/c-1-runtime-migration / PR108.
- Before: 00d8243778a4bada2adbff5e9567e359eade32b0
- After: 478812a83adb04ea38c93a6d4bc83a17233a9cf4 (clean; commits c457707 then478812a).
- SDK: sc-cron/sc-command0.2.0 at exact Git c3ce3d1b271b54fbb47efaef57ee1563a390cd9e. Only those two lock records change; registry observability1.5, rusqlite0.39 and one SQLite provider unchanged. Default features off; one nominal contract identity.
- Real public cron logging adapter and every production Rust byte unchanged. One test assertion adopts retained Error sources after SDK Debug redaction; original36/37 failure preserved in store-tests-initial-failure.log, exact correction/full37/37 now pass.
- Actual isolated API/Python graphs PASS. Guard accepts only published crates.io0.1.1 OR exact candidate URL/rev/resolvedcommit/version; wrong source/rev/path/movingbranch/mixedidentity/introducer/features fail. Eight unit groups pass; real candidate/server negative exits1 for server+axum+rmcp and forbidden introducer.
- Final local proof: store37/37; sync132 passed/4 existing ignored, including logging1/1 and cron_hurry19/19; CLI16/16; strict workspace all-target/all-feature and default-lib Clippy, affected follow-up Clippy, fmt, exact direct/isolated boundary commands, no-backend and strict consumer lint PASS. Raw logs and exact command receipts are adjacent.
- Native CI registered42206105621 and Python42206105712 for 478812a83adb04ea38c93a6d4bc83a17233a9cf4; pending. No all-green or literal upstream merge-head CI claim. PR118 sourcehead e8a0755a with17/17successful PRchecks and merge c3ce3d1b separately recorded.

Authorization: user's exact words: “ok, sc-runtime team has a ci failure on pr115. once this is resolved, you can pin that release to use before the 0.2.0 release is finalized.” Parent interprets the producer-qualified PR118 merge as satisfying this condition. ADR-BDC-0016/REQ source clauses and current plan rows now record only that exact exception. Parent anchor review must adjudicate historical published-only wording versus this source pair; template v0.1.1 provenance and every ANC/domain bound remain unchanged. No anchor waiver or self-approval.

ARCH-001, RBQA-F001, REQ-QA-004 and QA2-001 remain OPEN pending independent QA13 and actual current-head hosted gates. Cargo RELEASED, no active local command. STOP; no selfmerge, release, SDK edits, new agents or later-sprint work.
