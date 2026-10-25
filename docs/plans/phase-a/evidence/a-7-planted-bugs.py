"""a-7 planted-bug runner (NFR-BDC-0005), committed as evidence.

Usage: python3 docs/plans/phase-a/evidence/a-7-planted-bugs.py [name-filter]
Each mutant is one literal replacement checked to match exactly once; the suite
of the crates it touches runs, the file is restored. Results: a-7-planted-bugs.json.
A mutant is killed by a failing test, or by a compile-time assertion (`const_assert`).
"""
"""Plants each bug (one literal replacement, checked to match exactly once),
runs the bd-cache-sync suite, records the failing tests, restores the file."""
import json, re, subprocess, sys, pathlib

REPO = pathlib.Path(__file__).resolve().parents[4]
S = "crates/bd-cache-sync/src/"
M = "crates/bd-cache-model/src/testing/store.rs"

MUTANTS = [('unannounced filter dropped',
  'crates/bd-cache-model/src/testing/store.rs',
  'entries(|e| e.state == JournalState::Escalated && e.announced_at.is_none())',
  'entries(|e| e.state == JournalState::Escalated)'),
 ('open-work ignores announcement',
  'crates/bd-cache-model/src/testing/store.rs',
  '                e.state == JournalState::Escalated && e.announced_at.is_none()\n            }),',
  '                e.state == JournalState::Escalated\n            }),'),
 ('refusal dropped',
  'crates/bd-cache-sync/src/announce.rs',
  '            Some(Err(e)) => reply.refused.push(Refusal::undelivered(\n'
  '                RefusalCode::EscalationUndelivered,\n'
  '                &format!("escalation #{}", entry.seq),\n'
  '                &e,\n'
  '            )),',
  '            Some(Err(_)) => {}'),
 ('team check dropped',
  'crates/bd-cache-sync/src/phases.rs',
  'let team = member(config.team.as_deref(), "team", ConfigRefusal::MissingTeam)?;',
  'let team = MemberName::new(config.team.clone().unwrap_or_else(|| "t".to_owned())).map_err(|e| '
  'ConfigRefusal::BadMember { key: "team", value: e.value })?;'),
 ('closed roots active',
  'crates/bd-cache-sync/src/active.rs',
  'status == Some(bd_cache::CLOSED_STATUS)',
  'status == Some("never-closed")'),
 ('stop ignores open work',
  'crates/bd-cache-sync/src/tick.rs',
  '    if work.any() {\n        return Ok(None);\n    }\n',
  ''),
 ('fallback lead dropped for entries with no scope',
  'crates/bd-cache-sync/src/announce.rs',
  '            None => Some(settings.fallback_lead()),',
  '            None => None,'),
 ('escalation not marked announced',
  'crates/bd-cache-sync/src/announce.rs',
  '                cache\n'
  '                    .mark_announced_within(entry.seq, budget.write_wait())\n'
  '                    .inspect_err(|e| {\n'
  '                        unmarked_after_send(Unmarked::Escalation(entry.seq), lead, e);\n'
  '                    })?;\n',
  ''),
 ('escalation message without the resolve command',
  'crates/bd-cache-sync/src/announce.rs',
  '         bd-cache resolve {seq} --note',
  '         bd-cache {seq} --note'),
 ('sync failure announced every tick (alert not compared)',
  'crates/bd-cache-sync/src/announce.rs',
  '        .is_some_and(|stored| stored.covers(cause))\n    {\n        return Ok(());\n    }',
  '        .is_some_and(|stored| stored.covers(cause) && false)\n'
  '    {\n'
  '        return Ok(());\n'
  '    }'),
 ('sync failure alert not recorded',
  'crates/bd-cache-sync/src/announce.rs',
  '            cache\n'
  '                .set_sync_alert_within(&scope, Some(cause.key()), budget.write_wait())\n'
  '                .inspect_err(|e| {\n'
  '                    unmarked_after_send(Unmarked::SyncFailure(&scope), lead, e);\n'
  '                })?;\n',
  ''),
 ('sync success does not clear the alert',
  'crates/bd-cache-sync/src/announce.rs',
  '        clear_alert(cache, budget, scope, ClearReason::SyncSucceeded);\n',
  ''),
 ('outage alert not recorded after the send',
  'crates/bd-cache-sync/src/announce.rs',
  '                for phase in &phases {\n'
  '                    let scope = phase.scope();\n'
  '                    cache\n'
  '                        .set_sync_alert_within(\n'
  '                            &scope,\n'
  '                            Some(AlertCause::BackendUnavailable.key()),\n'
  '                            budget.write_wait(),\n'
  '                        )\n'
  '                        .inspect_err(|e| {\n'
  '                            unmarked_after_send(Unmarked::Outage(&scope), lead, e);\n'
  '                        })?;\n'
  '                }\n'
  '                reply.sent.push(Announcement {\n'
  '                    kind: AnnouncementKind::Outage {',
  '                reply.sent.push(Announcement {\n'
  '                    kind: AnnouncementKind::Outage {'),
 ('outage alert not reset when it ends',
  'crates/bd-cache-sync/src/announce.rs',
  '                clear_alert(cache, budget, &scope, ClearReason::OutageOver);\n',
  ''),
 ('outage: no lead for a scope without one',
  'crates/bd-cache-sync/src/announce.rs',
  '    phase\n'
  '        .lead\n'
  '        .as_ref()\n'
  '        .unwrap_or_else(|| settings.fallback_lead())',
  '    phase.lead.as_ref().unwrap_or_else(|| settings.team())'),
 ('outage not told to every configured lead when no root is readable',
  'crates/bd-cache-sync/src/tick.rs',
  '        .chain(active.unreadable.iter())\n',
  ''),
 ('outage message per phase instead of per lead',
  'crates/bd-cache-sync/src/announce.rs',
  '    for (lead, phases) in by_lead {\n',
  '    for (lead, phases) in by_lead.into_iter().flat_map(|(l, ps)| ps.into_iter().map(move |p| '
  '(l, vec![p]))) {\n'),
 ('undelivered outage message recorded as delivered',
  'crates/bd-cache-sync/src/announce.rs',
  '            Some(Err(e)) => reply.refused.push(Refusal::undelivered(\n'
  '                RefusalCode::OutageUndelivered,\n'
  '                &format!("outage message to {lead}"),\n'
  '                &e,\n'
  '            )),',
  '            Some(Err(_)) => {\n'
  '                for phase in &phases {\n'
  '                    cache.set_sync_alert(&phase.scope(), '
  'Some(AlertCause::BackendUnavailable.key()))?;\n'
  '                }\n'
  '            }'),
 ('sync failure of an unreachable cause announced per phase',
  'crates/bd-cache-sync/src/announce.rs',
  '    if cause == AlertCause::Sync(SyncCause::BdUnavailable) {\n        return Ok(());\n    }\n',
  ''),
 ('drain without the start gate and hard deadline',
  'crates/bd-cache-sync/src/tick.rs',
  'cache.drain(&DrainBudget::before_tick_end(end))?',
  'cache.drain(&DrainBudget { start_gate: end, hard: end })?'),
 ('sync not deferred before T - 2 s',
  'crates/bd-cache-sync/src/tick.rs',
  '    let gate = Deadline::at(\n'
  '        end.instant()\n'
  '            .checked_sub(HARD_MARGIN)\n'
  '            .unwrap_or_else(|| end.instant()),\n'
  '    );',
  '    let gate = end;'),
 ('sync run with no deadline (a day)',
  'crates/bd-cache-sync/src/tick.rs',
  'cache.sync(&phase.scope(), end)',
  'cache.sync(&phase.scope(), Deadline::after(&*clock, std::time::Duration::from_secs(86_400)))'),
 ('custom phase list ignored',
  'crates/bd-cache-sync/src/phases.rs',
  '        Some(only) => Ok(load_named(dir, only)),',
  '        Some(_) => scan(dir, entries).map_err(fail),'),
 ('malformed phase entry skipped instead of refused',
  'crates/bd-cache-sync/src/phases.rs',
  '                    let name = ScopeId::parse(entry)\n'
  '                        .map(|s| s.phase_name().clone())\n'
  '                        .ok_or_else(|| ConfigRefusal::MalformedPhases {\n'
  '                            entry: entry.clone(),\n'
  '                        })?;',
  '                    let Some(name) = ScopeId::parse(entry).map(|s| s.phase_name().clone()) else '
  '{\n'
  '                        continue;\n'
  '                    };'),
 ('unreadable root stops the schedule',
  'crates/bd-cache-sync/src/tick.rs',
  'if active.stopped || !active.active.is_empty() || !active.unreadable.is_empty() {',
  'if active.stopped || !active.active.is_empty() {'),
 ('configuration the tick refuses can stop the schedule',
  'crates/bd-cache-sync/src/tick.rs',
  '    let Ok(settings) = TickSettings::from_config(config) else {\n'
  '        return Ok(None);\n'
  '    };',
  '    let settings = TickSettings::from_config(config).unwrap_or_else(|_| '
  'TickSettings::from_config(&TickConfig { team: Some("t".into()), fallback_lead: '
  'Some("f".into()), phases_dir: config.phases_dir.clone(), phases: None }).unwrap());'),
 ('broken phase file does not hold the stop rule',
  'crates/bd-cache-sync/src/tick.rs',
  '    if !set.problems.is_empty() {\n        return Ok(None);\n    }\n',
  ''),
 ('busy: stop check stops under a running task',
  'crates/bd-cache-sync/src/job.rs',
  '            // Busy: a blocking task is still working; never stop under it.\n'
  '            return StopDecision::Continue;',
  '            return StopDecision::Stop("busy".to_owned());'),
 ('busy tick not reported (runs as an empty pass-through)',
  'crates/bd-cache-sync/src/job.rs',
  '            return TickRun::Done(Box::new(TickReply::busy()));',
  '            return TickRun::Done(Box::new(TickReply::started()));'),
 ('poison not cleared (reported on every tick)',
  'crates/bd-cache-sync/src/job.rs',
  '        self.cache.clear_poison();\n',
  ''),
 ('poisoned mutex not reported',
  'crates/bd-cache-sync/src/job.rs',
  '                if poisoned {\n                    reply.poisoned = true;',
  '                if false && poisoned {\n                    reply.poisoned = true;'),
 ('a refused tick reported as ok to the scheduler',
  'crates/bd-cache-sync/src/job.rs',
  '            TickOutcome::Refused => {\n                let list',
  '            TickOutcome::Refused => RunOutcome::Ok,\n'
  '            TickOutcome::PassThrough if false => {\n'
  '                let list'),
 ('schedule started without a team',
  'crates/bd-cache-sync/src/job.rs',
  '    let settings = TickSettings::from_config(config)?;\n    if let Some(atm)',
  '    let settings = TickSettings::from_config(&TickConfig { team: '
  'Some(config.team.clone().unwrap_or_default()), ..config.clone() }).or_else(|_| '
  'TickSettings::from_config(&TickConfig { team: Some("t".into()), ..config.clone() }))?;\n'
  '    if let Some(atm)'),
 ('atm address not validated',
  'crates/bd-cache-sync/src/phases.rs',
  '        let ok = !name.is_empty()\n'
  "            && !name.starts_with('-')\n"
  '            && !name\n'
  '                .chars()\n'
  "                .any(|c| c.is_whitespace() || c.is_control() || c == '@');",
  '        let ok = !name.is_empty();'),
 ('atm message not on stdin',
  'crates/bd-cache-sync/src/atm.rs',
  '                let _ = stdin.write_all(message.as_bytes());',
  '                let _ = stdin.write_all(b"");'),
 ('compose ignores the [tick] table',
  'crates/bd-cache-sync/src/compose.rs',
  '    let mut tick = repo.tick.unwrap_or_default();',
  '    let mut tick = TickConfig::default();\n'
  '    let _ = repo.tick;\n'
  '    tick.team = Some("atm-dev".into());\n'
  '    tick.fallback_lead = Some("team-lead".into());'),
 ('announcements ignore the tick budget',
  'crates/bd-cache-sync/src/control.rs',
  '        let left = self.end.remaining(self.clock).saturating_sub(SEND_RESERVE);\n'
  '        (left >= MIN_SEND_BUDGET).then_some(left)',
  '        let _ = self.end.remaining(self.clock);\n'
  '        Some(std::time::Duration::from_secs(10))'),
 ('announcements ignore the stop flag',
  'crates/bd-cache-sync/src/control.rs',
  '        if self.stop.is_stopped() {\n            return None;\n        }\n',
  ''),
 ('tick ignores the stop flag between phases',
  'crates/bd-cache-sync/src/tick.rs',
  '        if stop.is_stopped() {\n'
  '            reply.stopped = true;\n'
  '            break;\n'
  '        }\n',
  ''),
 ('atm kills only the direct child',
  'crates/bd-cache-sync/src/atm.rs',
  '            command.process_group(0);',
  '            let _ = &command;'),
 ('atm ignores the time the tick passes',
  'crates/bd-cache-sync/src/atm.rs',
  '        let timeout = timeout.min(self.timeout);',
  '        let timeout = self.timeout;'),
 ('atm ignores the stop flag',
  'crates/bd-cache-sync/src/atm.rs',
  '            if self.stop.is_stopped() {\n                kill_tree(child);',
  '            if false {\n                kill_tree(child);'),
 ('atm invocation without the reference environment',
  'crates/bd-cache-sync/src/atm.rs',
  '            .env("ATM_IDENTITY", &self.identity)\n',
  ''),
 ('atm truncated output not marked',
  'crates/bd-cache-sync/src/atm.rs',
  '        Ok(c) if c.truncated => format!("{} ... (output truncated)", c.text),\n',
  ''),
 ('poison met by the stop check is lost',
  'crates/bd-cache-sync/src/job.rs',
  '        self.poison_unreported.store(true, Ordering::SeqCst);\n',
  '        self.poison_unreported.store(false, Ordering::SeqCst);\n'),
 ('empty phase list accepted',
  'crates/bd-cache-sync/src/phases.rs',
  '            Some(list) if list.is_empty() => return Err(ConfigRefusal::EmptyPhases),\n',
  ''),
 ('phase file size not capped',
  'crates/bd-cache-sync/src/phases.rs',
  'pub const MAX_PHASE_FILE_BYTES: u64 = 64 * 1024;',
  'pub const MAX_PHASE_FILE_BYTES: u64 = u64::MAX / 2;'),
 ('phase file count not capped',
  'crates/bd-cache-sync/src/phases.rs',
  '        if files.len() >= MAX_PHASE_FILES {',
  '        if files.len() >= usize::MAX {'),
 ('compose does not root the phases directory',
  'crates/bd-cache-sync/src/compose.rs',
  '    tick.phases_dir = Some(rooted_phases_dir(tick.phases_dir.as_deref(), repo_root));',
  '    let _ = rooted_phases_dir;'),
 ('embedded error text not clipped',
  'crates/bd-cache-sync/src/text.rs',
  '    if text.len() <= limit {\n        return text.to_owned();\n    }',
  '    if true {\n        return text.to_owned();\n    }'),
 ('recorded outcome not clipped',
  'crates/bd-cache-sync/src/job.rs',
  'const MAX_OUTCOME_BYTES: usize = 1024;',
  'const MAX_OUTCOME_BYTES: usize = usize::MAX / 2;'),
 ('get ignores a pending entry (fast path always)',
  'crates/bd-cache/src/ready.rs',
  '    let untouched = !cache.store.has_entries_since(&entry.fetched_at)?;',
  '    let untouched = true;'),
 ('get loads the whole base again',
  'crates/bd-cache/src/ready.rs',
  '    let Some(entry) = cache.store.base_entry(s, &view.id)? else {\n'
  '        return Ok(None);\n'
  '    };',
  '    let Some(entry) = cache.store.ready_base(s)?.map(|b| bd_cache_model::BaseEntry::of(&b, '
  '&view.id)) else {\n'
  '        return Ok(None);\n'
  '    };'),
 ('store index not replaced with the base',
  'crates/bd-cache-store/src/store.rs',
  '            tx.prepare_cached("DELETE FROM ready_base_blocked WHERE scope = ?1")\n'
  '                .at(st)?\n'
  '                .execute([s.name()])\n'
  '                .at(st)?;\n',
  ''),
 ('store index survives a prune',
  'crates/bd-cache-store/src/store.rs',
  '            "DELETE FROM ready_base_blocked WHERE scope = ?1",\n',
  ''),
 ('store index keeps duplicate blockers',
  'crates/bd-cache-store/src/codec.rs',
  '            if !list.contains(&b) {\n                list.push(b);\n            }',
  '            list.push(b);'),
 ('announcing goes on after the budget is spent (S17)',
  'crates/bd-cache-sync/src/announce.rs',
  '                break;\n'
  '            }\n'
  '            Some(Ok(())) => {\n'
  '                cache\n'
  '                    .mark_announced_within',
  '                continue;\n'
  '            }\n'
  '            Some(Ok(())) => {\n'
  '                cache\n'
  '                    .mark_announced_within'),
 ('tick starts although it was asked to stop (RSH-014)',
  'crates/bd-cache-sync/src/tick.rs',
  '    let mut reply = TickReply::started();\n'
  '    if stop.is_stopped() {\n'
  '        reply.stopped = true;\n'
  '        return Ok(reply.finish());\n'
  '    }\n',
  '    let mut reply = TickReply::started();\n'),
 ('send gets no reserve before T (QA2-007)',
  'crates/bd-cache-sync/src/control.rs',
  '.saturating_sub(SEND_RESERVE);',
  '.saturating_sub(Duration::ZERO);'),
 ('a phase file that is not a regular file is opened (RSH-017)',
  'crates/bd-cache-sync/src/phases.rs',
  '    if !meta.is_file() {',
  '    if false {'),
 ('a cached root is trusted without its scope having synced',
  'crates/bd-cache-sync/src/active.rs',
  'if scope.holds(&reply.row.bead) && cache.scope_synced(&scope)? {',
  'if scope.holds(&reply.row.bead) {'),
 ('base_entry trusts the index of a base another writer replaced',
  'crates/bd-cache-store/src/store.rs',
  '            if indexed != Some(1) {',
  '            if false && indexed != Some(1) {'),
 ('store index not marked on replace',
  'crates/bd-cache-store/src/store.rs',
  'stale, indexed) VALUES (?1, ?2, ?3, ?4, 0, 1)',
  'stale, indexed) VALUES (?1, ?2, ?3, ?4, 0, NULL)'),
 ('a journal index holds a function (S18)',
  'crates/bd-cache-store/src/schema.rs',
  "journal(applied_epoch) WHERE state = 'applied';",
  "journal(abs(applied_epoch)) WHERE state = 'applied';"),
 ('entries bd_cache.py applied are not counted by has_entries_since (S18)',
  'crates/bd-cache-store/src/schema.rs',
  'INDEXED BY journal_applied_unknown \\\n'
  "        WHERE state = 'applied' AND applied_epoch IS NULL \\\n"
  '        AND COALESCE(unixepoch(COALESCE(applied_at, updated_at)) >= ?1, 1))";',
  'INDEXED BY journal_applied_unknown \\\n'
  '        WHERE state = \'applied\' AND applied_epoch IS NULL AND 0)";'),
 ('has_entries_since ignores pending entries',
  'crates/bd-cache-store/src/schema.rs',
  'SELECT \\\n'
  "    EXISTS (SELECT 1 FROM journal WHERE state = 'pending') OR \\\n"
  '    EXISTS (SELECT 1 FROM journal INDEXED BY journal_applied_epoch',
  'SELECT \\\n    0 OR \\\n    EXISTS (SELECT 1 FROM journal INDEXED BY journal_applied_epoch'),
 ('applied_since misses entries bd_cache.py applied (S18)',
  'crates/bd-cache-store/src/schema.rs',
  "journal_applied_unknown \\\n"
  "     WHERE state = 'applied' AND applied_epoch IS NULL \\\n"
  "     AND COALESCE(unixepoch(COALESCE(applied_at, updated_at)) >= ?1, 1) \\\n"
  '     ORDER BY seq"',
  "journal_applied_unknown \\\n"
  "     WHERE state = 'applied' AND applied_epoch IS NULL \\\n"
  "     AND 0 \\\n"
  '     ORDER BY seq"'),
 ('open does not date the entries bd_cache.py applied',
  'crates/bd-cache-store/src/store.rs',
  '    tx.execute_batch(BACKFILL_APPLIED_EPOCH)\n',
  '    tx.execute_batch("SELECT 1")\n'),
 ('the expression index of an earlier build is kept',
  'crates/bd-cache-store/src/schema.rs',
  'DROP INDEX IF EXISTS journal_applied_at;\n',
  ''),
 ('a missing hinted index is an error (QA3-007)',
  'crates/bd-cache-store/src/store.rs',
  'Err(e) if is_missing_index(&e) => {',
  'Err(e) if false && is_missing_index(&e) => {'),
 ('a stored alert is compared as raw text (S21)',
  'crates/bd-cache-sync/src/announce.rs',
  '        Self::from_key(key.trim())',
  '        Self::from_key(value)'),
 ('a generic stored alert does not cover a specific cause (S21)',
  'crates/bd-cache-sync/src/announce.rs',
  'self == new || (self == Self::SyncFailed && matches!(new, Self::Sync(_)))',
  'self == new'),
 ('a file without an execute bit resolves (QA3-008)',
  'crates/bd-cache-sync/src/atm.rs',
  'meta.is_file() && meta.permissions().mode() & 0o111 != 0',
  'meta.is_file()'),
 ('a phase file is opened blocking (RSH-024)',
  'crates/bd-cache-sync/src/phases.rs',
  'options.custom_flags(libc::O_NONBLOCK);',
  'let _ = &options;'),
 ('the tick goes on after the drain although stopped (S20)',
  'crates/bd-cache-sync/src/tick.rs',
  '    if stop.is_stopped() {\n'
  '        // The drain is the long part; do not start reading phases after it.\n'
  '        reply.stopped = true;\n'
  '        return Ok(reply.finish());\n'
  '    }\n',
  ''),
 ("the model's close test uses another status",
  'crates/bd-cache-model/src/op.rs',
  'Op::Close { .. } => row.status.as_deref() == Some(CLOSED_STATUS),',
  'Op::Close { .. } => row.status.as_deref() == Some("nope"),'),
 ('a descendant holding the pipes is waited for in full (QA3-003)',
  'crates/bd-cache-sync/src/atm.rs',
  '    now + READ_GRACE.min(left).max(READ_FLOOR)',
  '    let _ = (left, READ_FLOOR);\n    now + Duration::from_secs(30)'),
 # ---- round-5 fixes (a-7 fix-5/6) ----
 ('S26: threshold dropped, every pass dates (count skipped)',
  'crates/bd-cache-store/src/store.rs',
  'Ok(n) => n >= limit && self.date_now(),',
  'Ok(_) => self.date_now(),'),
 ('S26: dating never runs (reads always walk the undated entries)',
  'crates/bd-cache-store/src/store.rs',
  'Ok(n) => n >= limit && self.date_now(),',
  'Ok(_) => false,'),
 ('S26: threshold off by one (dates at the threshold)',
  'crates/bd-cache-store/src/store.rs',
  'let limit = i64::from(READ_DATING_THRESHOLD) + 1;\n        let waiting',
  'let limit = i64::from(READ_DATING_THRESHOLD);\n        let waiting'),
 ('QA5-002: the undatable floor rises onto the first dateable entry',
  'crates/bd-cache-store/src/store.rs',
  'Some(first) => first.saturating_sub(1),',
  'Some(first) => first,'),
 ('QA5-002: the undatable floor rises past a dateable entry',
  'crates/bd-cache-store/src/store.rs',
  'Some(first) => first.saturating_sub(1),',
  'Some(_) => i64::MAX,'),
 ('QA5-003: the busy timeout is not restored after the no-wait dating',
  'crates/bd-cache-store/src/store.rs',
  'if let Err(e) = self.conn.busy_timeout(BUSY_TIMEOUT) {',
  'if let Err(e) = Ok::<(), rusqlite::Error>(()) {'),
 ('QA5-003: the dating waits for the write lock (BusyWait not at zero)',
  'crates/bd-cache-store/src/store.rs',
  'BusyWait::new(&self.conn, &self.path, Duration::ZERO)',
  'BusyWait::new(&self.conn, &self.path, BUSY_TIMEOUT)'),
 ('S27: the tree kill and the reaping do not share one deadline',
  'crates/bd-cache-sync/src/atm.rs',
  'if !exited_by(&mut child, until) {',
  'if !exited_by(&mut child, Instant::now() + KILL_BOUND) {'),
 ('S27: the kill bound is far too long',
  'crates/bd-cache-sync/src/atm.rs',
  'const KILL_BOUND: Duration = Duration::from_secs(1);',
  'const KILL_BOUND: Duration = Duration::from_secs(30);'),
 ('S27: the tree kill is not given up at the deadline',
  'crates/bd-cache-sync/src/atm.rs',
  '        if Instant::now() >= until {\n            tracing::warn!(\n                pid,\n'
  '                "the atm tree kill did not finish in time; giving up on it"',
  '        if false {\n            tracing::warn!(\n                pid,\n'
  '                "the atm tree kill did not finish in time; giving up on it"'),
 ('S28: a child not reaped in time is dropped, not handed to the reaper',
  'crates/bd-cache-sync/src/atm.rs',
  '        adopt(child);\n    }\n    outcome',
  '        drop(child);\n    }\n    outcome'),
 ('S28: the reaper lets go of every process at once',
  'crates/bd-cache-sync/src/atm.rs',
  'held.retain_mut(|c| matches!(c.try_wait(), Ok(None)));',
  'held.retain_mut(|_| false);'),
 ('S28: the reaper never looks at what it holds',
  'crates/bd-cache-sync/src/atm.rs',
  'held.retain_mut(|c| matches!(c.try_wait(), Ok(None)));',
  'held.retain_mut(|_| true);'),
 ('S27: SEND_RESERVE shrunk below the read grace plus the kill bound',
  'crates/bd-cache-sync/src/control.rs',
  'pub(crate) const SEND_RESERVE: Duration = Duration::from_millis(1500);',
  'pub(crate) const SEND_RESERVE: Duration = Duration::from_millis(500);'),
 ('RBP-F039: a root bd did not return is not refused',
  'crates/bd-cache-sync/src/tick.rs',
  '    reply\n        .refused\n        .extend(active.not_returned.iter().map(root_not_returned));',
  '    let _ = root_not_returned;'),
 ('RBP-F039: a root bd did not return is never recorded',
  'crates/bd-cache-sync/src/active.rs',
  '                set.not_returned.push(phase.clone());',
  '                let _ = phase;'),
 ('QA5-004: scope_synced says every scope has synced',
  'crates/bd-cache/src/lib.rs',
  'Ok(self.store.sync_state(scope)?.last_ok_at.is_some())',
  'Ok(self.store.sync_state(scope).map(|_| true)?)'),
 # ---- round-8 fixes (a-7 fix-8): S33 bounded marker writes, QA6-001 ----
 ('S33: the escalation marker waits the full busy timeout, not the time left',
  'crates/bd-cache-sync/src/announce.rs',
  '.mark_announced_within(entry.seq, budget.write_wait())',
  '.mark_announced_within(entry.seq, std::time::Duration::from_secs(5))'),
 ('S33: the sync-failure alert waits the full busy timeout, not the time left',
  'crates/bd-cache-sync/src/announce.rs',
  '.set_sync_alert_within(&scope, Some(cause.key()), budget.write_wait())',
  '.set_sync_alert_within(&scope, Some(cause.key()), std::time::Duration::from_secs(5))'),
 ('S33: the outage alert waits the full busy timeout, not the time left',
  'crates/bd-cache-sync/src/announce.rs',
  '                            Some(AlertCause::BackendUnavailable.key()),\n'
  '                            budget.write_wait(),',
  '                            Some(AlertCause::BackendUnavailable.key()),\n'
  '                            std::time::Duration::from_secs(5),'),
 ('S33: write_wait is not the time left before T',
  'crates/bd-cache-sync/src/control.rs',
  '    pub fn write_wait(&self) -> Duration {\n        self.end.remaining(self.clock)\n',
  '    pub fn write_wait(&self) -> Duration {\n'
  '        self.end.remaining(self.clock).max(Duration::from_secs(5))\n'),
 ('S33: the store ignores the wait (full busy timeout)',
  'crates/bd-cache-store/src/store.rs',
  'BusyWait::new(&mut self.conn, &self.path, wait.min(BUSY_TIMEOUT))',
  'BusyWait::new(&mut self.conn, &self.path, BUSY_TIMEOUT.max(wait))'),
 ('S33: the store does not cap the wait at BUSY_TIMEOUT',
  'crates/bd-cache-store/src/store.rs',
  'BusyWait::new(&mut self.conn, &self.path, wait.min(BUSY_TIMEOUT))',
  'BusyWait::new(&mut self.conn, &self.path, wait)'),
 ('S33: mark_announced_within is the unbounded write',
  'crates/bd-cache-store/src/store.rs',
  'self.write_within(wait, "mark announced", |tx, st| {',
  'self.write_within(BUSY_TIMEOUT.max(wait), "mark announced", |tx, st| {'),
 ('S33: set_sync_alert_within is the unbounded write',
  'crates/bd-cache-store/src/store.rs',
  'self.write_within(wait, "set a sync alert", |tx, at| {',
  'self.write_within(BUSY_TIMEOUT.max(wait), "set a sync alert", |tx, at| {'),
 # ---- round-10 fixes (a-7 fix-10): S35 bounded clears, marker failure path ----
 ('S35: an alert clear waits the full busy timeout, not the time left',
  'crates/bd-cache-sync/src/announce.rs',
  'cache.set_sync_alert_within(scope, None, budget.write_wait())',
  'cache.set_sync_alert_within(scope, None, std::time::Duration::from_secs(5))'),
 ('S35: a failed alert clear fails the tick',
  'crates/bd-cache-sync/src/announce.rs',
  'if let Err(e) = cache.set_sync_alert_within(scope, None, budget.write_wait()) {',
  'if let Err(e) = cache\n'
  '        .set_sync_alert_within(scope, None, budget.write_wait())\n'
  '        .inspect_err(|e| panic!("{e}"))\n'
  '    {'),
 ('S35: a failed marker write after a delivered send is swallowed',
  'crates/bd-cache-sync/src/announce.rs',
  '                        unmarked_after_send(Unmarked::Escalation(entry.seq), lead, e);\n'
  '                    })?;\n',
  '                        unmarked_after_send(Unmarked::Escalation(entry.seq), lead, e);\n'
  '                    })\n'
  '                    .unwrap_or(());\n'),
 ('S34: ComposeError passes the store code through instead of mapping the variant',
  'crates/bd-cache-sync/src/compose.rs',
  '            Self::Config(e) => config_code(e),',
  '            Self::Config(e) => e.code(),'),
 ('S34: an unreadable configuration is mapped to the invalid-TOML code',
  'crates/bd-cache-sync/src/compose.rs',
  '        ConfigLoadError::Read { .. } => "sync_config_unreadable",',
  '        ConfigLoadError::Read { .. } => "sync_config_invalid_toml",'),
 # ---- round-12 fixes (a-7 fix-12): S38 exhaustive mapping, S40 compose inputs, QA8-001 ----
 ('S38: the exhaustive config mapping swaps the bad-scope and invalid-TOML codes',
  'crates/bd-cache-sync/src/compose.rs',
  '        ConfigLoadError::Scope { .. } => "sync_config_bad_scope",',
  '        ConfigLoadError::Scope { .. } => "sync_config_invalid_toml",'),
 ('S40: compose_with does not give the bd child the resolved BEADS_DIR',
  'crates/bd-cache-sync/src/compose.rs',
  '    let backend = backend(locks)\n'
  '        .with_clock(Arc::clone(&clock))\n'
  '        .with_child_var(\n'
  '            OsStr::new(BEADS_DIR_ENV),\n'
  '            beads_dir.as_ref().map(BeadsDirOverride::as_os_str),\n'
  '        );',
  '    let backend = backend(locks).with_clock(Arc::clone(&clock));'),
 ('S40: the runner does not apply its child environment to run_raw',
  'crates/sc-bd-cli/src/runner.rs',
  'crate::run::run_raw_in(&self.binary, &wd, args, &self.child_env)',
  'crate::run::run_raw(&self.binary, &wd, args)'),
 ('QA8-001: the outage-over clear visits a scope twice in one tick',
  'crates/bd-cache-sync/src/announce.rs',
  '        let mut scopes: BTreeSet<ScopeId> =',
  '        let mut scopes: Vec<ScopeId> ='),
 # ---- round-14 fixes (a-7 fix-14): S44 one normalized BEADS_DIR, S45 outage-over clears stop ----
 ('S44: an empty BEADS_DIR reaches the bd child as an empty value',
  'crates/bd-cache-store/src/location.rs',
  '        let Some(value) = value.filter(|v| !v.is_empty()) else {',
  '        let Some(value) = value else {'),
 ('S44: a relative BEADS_DIR is not made absolute against the working directory',
  'crates/bd-cache-store/src/location.rs',
  '            Ok(abs) => Ok(Some(Self(abs))),',
  '            Ok(_) => Ok(Some(Self(dir.to_path_buf()))),'),
 ('S45: the outage-over clears go on after T',
  'crates/bd-cache-sync/src/announce.rs',
  '            if budget.write_wait().is_zero() {\n                break;\n            }\n',
  ''),
 ('S45: the outage-over clears go on after a stop',
  'crates/bd-cache-sync/src/announce.rs',
  '            if budget.stop.is_stopped() {\n                reply.stopped = true;\n                break;\n            }\n',
  ''),
 # ---- round-15 fixes (a-7 fix-15): S48 fail closed, S49 one BeadsDirOverride ----
 ('S48: a BEADS_DIR that cannot be made absolute falls back to the relative value',
  'crates/bd-cache-store/src/location.rs',
  '            Err(source) => Err(BeadsDirError::new(dir, source)),',
  '            Err(_) => Ok(Some(Self(dir.to_path_buf()))),'),
 ('S48: compose_with ignores an unresolvable BEADS_DIR and opens the store without it',
  'crates/bd-cache-sync/src/compose.rs',
  '    let beads_dir = beads_dir?;',
  '    let beads_dir = beads_dir.unwrap_or(None);'),
 ('S48: an unresolvable BEADS_DIR is reported with the cache code',
  'crates/bd-cache-sync/src/compose.rs',
  '            Self::BeadsDir(_) => BEADS_DIR_CODE,',
  '            Self::BeadsDir(_) => "BD_CACHE.NO_DATABASE",'),
 ('S49: beads_dir ignores the override',
  'crates/bd-cache-store/src/location.rs',
  '        |dir| dir.as_path().to_path_buf(),',
  '        |_| repo_root.join(".beads"),'),
 ('S49: CacheLocation::resolve does not build the override from the environment',
  'crates/bd-cache-store/src/location.rs',
  '        Self::resolve_with(repo_root, config, env_beads_dir()?.as_ref())',
  '        Self::resolve_with(repo_root, config, None)'),
 ('QA6-001: the dating skips entries below the undatable floor',
  'crates/bd-cache-store/src/schema.rs',
  'UPDATE journal INDEXED BY journal_applied_unknown \\\n'
  'SET applied_epoch = unixepoch(COALESCE(applied_at, updated_at)) \\\n'
  "WHERE state = 'applied' AND applied_epoch IS NULL \\\n",
  'UPDATE journal INDEXED BY journal_applied_unknown \\\n'
  'SET applied_epoch = unixepoch(COALESCE(applied_at, updated_at)) \\\n'
  "WHERE state = 'applied' AND applied_epoch IS NULL \\\n"
  "  AND seq > COALESCE((SELECT MAX(seq) FROM journal WHERE state = 'applied' \\\n"
  "  AND applied_epoch IS NULL AND unixepoch(COALESCE(applied_at, updated_at)) IS NULL), 0) \\\n")]

# Mutants that survive the suite, each with the reason (not "equivalent": a gap or a
# case no portable test can reach). The runner copies the reason into the results.
SURVIVORS = {
    'S27: the tree kill and the reaping do not share one deadline':
        'justified: unreachable without an uninterruptible process. The two deadlines differ only when the child is still alive after '
        'SIGKILL (an uninterruptible state), which no portable test can make. The tree-kill '
        'bound, the KILL_BOUND value and SEND_RESERVE are each killed.',
    'S28: a child not reaped in time is dropped, not handed to the reaper':
        'justified: unreachable without an uninterruptible process. The call site runs only for a child that outlives SIGKILL by '
        'KILL_BOUND. adopt() and the reaper are killed (the helper path and the reaper loop).',
}

def run(only=None):
    results = []
    for name, rel, old, new in MUTANTS:
        if only and only not in name:
            continue
        path = REPO / rel
        text = path.read_text()
        n = text.count(old)
        if n != 1:
            results.append({"bug": name, "file": rel, "error": f"pattern matched {n} times"})
            print("SKIP", name, n, flush=True)
            continue
        path.write_text(text.replace(old, new))
        try:
            p = subprocess.run(
                ["cargo", "test"] + sum((["-p", p_] for p_ in (["bd-cache", "bd-cache-model", "bd-cache-store", "bd-cache-sync"] if not rel.startswith("crates/bd-cache-sync") else ["bd-cache-sync"])), []) + ["--all-features", "--no-fail-fast"],
                cwd=REPO, capture_output=True, text=True, timeout=900)
            out = p.stdout + p.stderr
        finally:
            path.write_text(text)
        # A compile-time assertion (`const _: () = assert!(..)`) that the mutant breaks
        # kills it at build time: error[E0080], evaluation of a constant failed.
        const_assert = "error[E0080]" in out
        compile_error = not const_assert and (
            "could not compile" in out
            or (re.search(r"^error(\[E\d+\])?:", out, re.M) is not None
                and p.returncode == 101 and "test result" not in out))
        failed = sorted(set(re.findall(r"^test (\S+) \.\.\. FAILED", out, re.M)))
        # harness=false process test prints a panic instead
        if "tests/atm_process" in out and "panicked" in out and "atm_process" not in "".join(failed):
            if re.search(r"Running tests/atm_process.rs[\s\S]*?panicked", out):
                failed.append("atm_process (process test)")
        res = {"bug": name, "file": rel, "exit": p.returncode, "compile_error": bool(compile_error), "const_assert": const_assert, "failed": failed}
        if not failed and not const_assert and name in SURVIVORS:
            res["survived"] = SURVIVORS[name]
        results.append(res)
        print(("OK  " if (failed or const_assert) and not compile_error else "MISS"), name, p.returncode, failed[:4], flush=True)
    return results

if __name__ == "__main__":
    only = sys.argv[1] if len(sys.argv) > 1 else None
    res = run(only)
    import os
    out = pathlib.Path(os.environ.get("PLANT_OUT", str(pathlib.Path(__file__).with_suffix(".json"))))
    out.write_text(json.dumps(res, indent=1))
