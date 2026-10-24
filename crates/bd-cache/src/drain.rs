//! `drain`: apply journaled writes in seq order under the drain lock (REQ-BDC-0401..0406).
//!
//! One pass (`docs/bd-cache/write-path.md`, REQ-CORE-0102 and ADR-CORE-0101..0103):
//!
//! 1. [`DrainLock::try_acquire`]: held elsewhere, the report is `busy: true`
//!    and nothing else runs. The guard lives until the pass returns.
//! 2. Pending entries in seq order. A bead whose row is stale (a failed
//!    read-back) is held until a sync re-reads it, and a bead whose write
//!    fails is held for the rest of the pass. A held bead's entries stay
//!    pending: `retrying` when the pass reaches them, `deferred` when it
//!    stops first (REQ-CORE-0103). With only stale-held entries the pass
//!    makes no bd call.
//! 3. The start gate: once it has passed, every entry not yet started is
//!    `deferred` (pending, `attempts` unchanged). It is checked before the
//!    backend check and before each entry.
//! 4. `backend_check::run`, then one batched `bd list --id` of every bead
//!    with a runnable entry (REQ-BDC-0105, 0403). Either step failing stops
//!    the pass with no attempt counted, by the BD-CACHE v3 rule
//!    (ADR-BDC-0005, ADR-CORE-0101): `BD_UNAVAILABLE`, `BD_VERSION_FAILED`,
//!    `BD_SCHEMA_FAILED` and `BD_LIST_FAILED` are an outage, recorded, and a
//!    cut is no outage; both put every pending entry in `deferred`. Any other
//!    failure (for example `BD_CONTRACT`) is logged as a `tracing` warning
//!    and every pending entry retries (`retrying`).
//! 5. Per entry (REQ-BDC-0404): a bead absent from the batch escalates
//!    `NOT_FOUND`; an achieved write stores the fresh row and is applied
//!    with no second write (REQ-BDC-0005); an unmet precondition stores the
//!    fresh row and escalates `PRECONDITION`; otherwise the write runs. A bd
//!    guard refusal (exit 13) escalates; any other failure of the write
//!    counts one attempt, holds the bead for the rest of the pass and
//!    escalates at `max_attempts`.
//! 6. Read-back (REQ-BDC-0405): one `bd list --id <bead>`, then
//!    `apply_with_readback` stores the row (forced) and marks the entry
//!    applied in one transaction; the row is the bead's fresh state for its
//!    next entry. A failed read-back marks the entry applied with the error,
//!    sets the stale mark and holds the bead.
//!
//! Every bd call goes through `call` under the hard deadline. A call the
//! hard deadline cuts is neither an outage nor an attempt: its entry stays
//! pending in `deferred_in_flight`, the pass stops and the rest is
//! `deferred`; the next pass's achieved check catches a write that landed
//! (REQ-BDC-0402). Each pending entry appears in exactly one report list.

use std::collections::{BTreeMap, BTreeSet};

use bd_cache_model::{
    CacheError, CacheStore, CallFailure, CallKind, DrainBudget, DrainLock, DrainReport,
    EscalatedItem, JournalEntry, Op, PreconditionReason, unmet,
};
use sc_beads::logging::tracing;
use sc_beads::{Bead, BeadId, CacheBackend, Deadline};

use crate::Cache;
use crate::write::describe_unmet;

/// Applies pending entries in seq order within `budget`; see the module docs.
pub(crate) fn drain<S: CacheStore, L: DrainLock, B: CacheBackend>(
    cache: &mut Cache<S, L, B>,
    budget: &DrainBudget,
) -> Result<DrainReport, CacheError> {
    // Bound for the whole pass: dropping the guard releases the lock on every return path.
    let Some(_guard) = cache.lock.try_acquire()? else {
        return Ok(DrainReport {
            busy: true,
            ..DrainReport::default()
        });
    };
    let mut pass = Pass {
        cache,
        budget: *budget,
        report: DrainReport::default(),
        fresh: BTreeMap::new(),
        held: BTreeSet::new(),
    };
    pass.run()?;
    pass.report.outage = pass.cache.store.outage()?;
    Ok(pass.report)
}

/// Whether the pass goes on to the next entry.
enum Flow {
    Next,
    /// A cut or an outage: every later entry is deferred.
    Stop,
}

/// One drain pass's working state.
struct Pass<'c, S: CacheStore, L: DrainLock, B: CacheBackend> {
    cache: &'c mut Cache<S, L, B>,
    budget: DrainBudget,
    report: DrainReport,
    /// The bead's latest bd row: the pre-read, then each read-back.
    fresh: BTreeMap<BeadId, Bead>,
    /// Beads skipped for the rest of the pass (stale row, or a failed write).
    held: BTreeSet<BeadId>,
}

impl<S: CacheStore, L: DrainLock, B: CacheBackend> Pass<'_, S, L, B> {
    fn gate_passed(&self) -> bool {
        self.budget.start_gate.expired(&*self.cache.clock)
    }

    fn hard(&self) -> Deadline {
        self.budget.hard
    }

    fn run(&mut self) -> Result<(), CacheError> {
        let pending = self.cache.store.pending_entries()?;
        if pending.is_empty() {
            return Ok(());
        }
        self.held = self.cache.store.stale_rows()?.into_iter().collect();
        // Beads with an entry to run: those whose row is not stale.
        let wanted: BTreeSet<BeadId> = pending
            .iter()
            .filter(|e| !self.held.contains(&e.bead))
            .map(|e| e.bead.clone())
            .collect();
        if self.gate_passed() {
            self.defer(&pending);
            return Ok(());
        }
        if wanted.is_empty() {
            // Only stale-held entries: each is reached and held, with no bd call.
            self.report.retrying.extend(pending.iter().map(|e| e.seq));
            return Ok(());
        }
        if let Err(f) = crate::backend_check::run(self.cache, self.hard()) {
            return self.stop_before_entries(f, &pending);
        }
        // Each bead once, in bead-id order.
        let ids: Vec<BeadId> = wanted.iter().cloned().collect();
        let rows = match crate::call(self.cache, CallKind::PreRead, self.hard(), |b, p, d| {
            b.list_by_ids(p, &ids, d)
        }) {
            Ok(rows) => rows,
            Err(f) => return self.stop_before_entries(f, &pending),
        };
        self.fresh = rows
            .into_iter()
            .filter(|r| wanted.contains(&r.id))
            .map(|r| (r.id.clone(), r))
            .collect();
        // Seq order; a held bead's entries go to `retrying` when reached.
        let mut rest = pending.iter();
        while let Some(e) = rest.next() {
            if self.gate_passed() {
                self.report.deferred.push(e.seq);
                self.report.deferred.extend(rest.map(|e| e.seq));
                break;
            }
            if let Flow::Stop = self.entry(e)? {
                self.report.deferred.extend(rest.map(|e| e.seq));
                break;
            }
        }
        Ok(())
    }

    fn defer(&mut self, entries: &[JournalEntry]) {
        self.report.deferred.extend(entries.iter().map(|e| e.seq));
    }

    /// The backend check or the pre-read failed: no entry starts and no attempt is counted.
    ///
    /// `entries` is every pending entry, those held for a stale row included.
    /// The v3 rule (ADR-CORE-0101): an unreachable backend ([`is_unreachable`])
    /// or a cut defers them all; any other failure retries them all.
    fn stop_before_entries(
        &mut self,
        f: CallFailure,
        entries: &[JournalEntry],
    ) -> Result<(), CacheError> {
        let defer_all = match f {
            // A cut is no outage; an `Outage` is already recorded by `call`.
            CallFailure::Cut | CallFailure::Outage(_) => true,
            // A store failure inside the helper (no backend cause) is the drain's error.
            CallFailure::Error(e) if e.backend_cause().is_none() => return Err(e),
            CallFailure::Error(e) if is_unreachable(&e) => {
                crate::outage::note_unavailable(self.cache, &e.to_string())?;
                true
            }
            CallFailure::Error(e) => {
                tracing::warn!(
                    code = e.code(),
                    detail = e.detail(),
                    "bd-cache drain: backend check or pre-read failed; every pending entry retries"
                );
                false
            }
        };
        let seqs = entries.iter().map(|e| e.seq);
        if defer_all {
            self.report.deferred.extend(seqs);
        } else {
            self.report.retrying.extend(seqs);
        }
        Ok(())
    }

    /// Steps 1-5 for one entry.
    fn entry(&mut self, e: &JournalEntry) -> Result<Flow, CacheError> {
        if self.held.contains(&e.bead) {
            self.report.retrying.push(e.seq);
            return Ok(Flow::Next);
        }
        let Some(row) = self.fresh.get(&e.bead).cloned() else {
            let reason = CacheError::NotFound {
                detail: format!("bead {} is absent from bd at drain time", e.bead),
            };
            self.escalate(e, reason.to_string())?;
            return Ok(Flow::Next);
        };
        if e.op.achieved(&row) {
            let at = self.cache.clock.now_utc();
            self.cache
                .store
                .apply_with_readback(e.seq, Some(&row), None, &at)?;
            self.report.applied.push(e.seq);
            return Ok(Flow::Next);
        }
        let unmet = unmet(&row, &e.precondition);
        if !unmet.is_empty() {
            self.cache
                .store
                .upsert_rows(None, std::slice::from_ref(&row), true)?;
            let reason = CacheError::Precondition {
                detail: format!(
                    "precondition no longer holds for {} at drain time: {}; not written",
                    e.bead,
                    describe_unmet(&unmet)
                ),
                unmet,
                reason: PreconditionReason::PreconditionFailed,
                source: None,
            };
            self.escalate(e, reason.to_string())?;
            return Ok(Flow::Next);
        }
        match self.run_write(e) {
            Ok(()) => self.read_back(e),
            Err(CallFailure::Cut) => {
                self.report.deferred_in_flight.push(e.seq);
                Ok(Flow::Stop)
            }
            // Not produced for writes by `call`; stop as for any outage.
            Err(CallFailure::Outage(_)) => {
                self.report.deferred.push(e.seq);
                Ok(Flow::Stop)
            }
            Err(CallFailure::Error(err)) => self.write_failed(e, err),
        }
    }

    /// Runs the entry's bd command with its precondition as bd guards.
    fn run_write(&mut self, e: &JournalEntry) -> Result<(), CallFailure> {
        let hard = self.hard();
        crate::call(self.cache, CallKind::Write, hard, |b, p, d| match &e.op {
            Op::Close { reason } => {
                b.close_guarded(p, &e.bead, reason.as_deref(), &e.precondition, &e.actor, d)
            }
            Op::Update(patch) => b.update_guarded(p, &e.bead, patch, &e.precondition, &e.actor, d),
            // bd's `dep add` takes no guards: the precondition was checked on the fresh row.
            Op::DepAdd { depends_on, kind } => {
                b.dep_add_typed(p, &e.bead, depends_on, *kind, &e.actor, d)
            }
        })
    }

    /// Step 4's failure branch: a refusal escalates, anything else counts an attempt.
    fn write_failed(&mut self, e: &JournalEntry, err: CacheError) -> Result<Flow, CacheError> {
        if err.backend_cause().is_none() {
            return Err(err);
        }
        if let CacheError::Precondition { .. } = err {
            // bd's guard refused (exit 13, or a /v0 refusal): never retried.
            self.escalate(e, err.to_string())?;
            return Ok(Flow::Next);
        }
        let at = self.cache.clock.now_utc();
        let attempts = self
            .cache
            .store
            .record_attempt(e.seq, &err.to_string(), &at)?;
        self.held.insert(e.bead.clone());
        if attempts >= self.cache.config.max_attempts {
            self.escalate(
                e,
                format!("retries exhausted after {attempts} attempts; last error {err}"),
            )?;
        } else {
            self.report.retrying.push(e.seq);
        }
        Ok(Flow::Next)
    }

    /// Step 5: read the bead back and mark the entry applied.
    fn read_back(&mut self, e: &JournalEntry) -> Result<Flow, CacheError> {
        let hard = self.hard();
        let bead = &e.bead;
        let answer = crate::call(self.cache, CallKind::ReadBack, hard, |b, p, d| {
            b.list_by_ids(p, std::slice::from_ref(bead), d)
        });
        let err = match answer {
            Ok(rows) => match rows.into_iter().find(|r| r.id == *bead) {
                Some(row) => {
                    let at = self.cache.clock.now_utc();
                    self.cache
                        .store
                        .apply_with_readback(e.seq, Some(&row), None, &at)?;
                    self.fresh.insert(bead.clone(), row);
                    self.report.applied.push(e.seq);
                    return Ok(Flow::Next);
                }
                None => CacheError::NotFound {
                    detail: format!("read-back: bead {bead} is absent from bd after the write"),
                },
            },
            Err(CallFailure::Cut) => {
                // The write ran; the next pass's achieved check finds it.
                self.report.deferred_in_flight.push(e.seq);
                return Ok(Flow::Stop);
            }
            Err(CallFailure::Error(err)) if err.backend_cause().is_none() => return Err(err),
            // ADR-CORE-0101, stop table, read-back row: an unreachable bd (any
            // code) is a failed read-back, applied with the error, stale, bead
            // held, and the pass goes on. `call` classifies an unreachable
            // read-back as `Error(BD_UNAVAILABLE)`, never `Outage`.
            Err(CallFailure::Outage(err) | CallFailure::Error(err)) => err,
        };
        let at = self.cache.clock.now_utc();
        self.cache
            .store
            .apply_with_readback(e.seq, None, Some(&err.to_string()), &at)?;
        self.fresh.remove(bead);
        self.held.insert(bead.clone());
        self.report.applied.push(e.seq);
        Ok(Flow::Next)
    }

    fn escalate(&mut self, e: &JournalEntry, reason: String) -> Result<(), CacheError> {
        let at = self.cache.clock.now_utc();
        self.cache.store.escalate(e.seq, &reason, &at)?;
        self.report.escalated.push(EscalatedItem {
            seq: e.seq,
            bead: e.bead.clone(),
            op: e.op.name().to_owned(),
            actor: e.actor.clone(),
            phase: e.scope.as_ref().map(|s| s.phase_name().clone()),
            reason,
        });
        Ok(())
    }
}

/// The codes BD-CACHE v3 counts as an unreachable backend at the backend
/// check and the pre-read (`UNREACHABLE`; ADR-CORE-0101), matched on the
/// typed error, never on its text.
fn is_unreachable(e: &CacheError) -> bool {
    matches!(
        e,
        CacheError::BdUnavailable { .. }
            | CacheError::BdVersionFailed { .. }
            | CacheError::BdSchemaFailed { .. }
            | CacheError::BdListFailed { .. }
    )
}
