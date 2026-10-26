//! [`TickBudget`]: the built-in sync row's time budget (REQ-BDC-0703 as
//! amended, REQ-BDC-0720).

use std::time::Duration;

use sc_beads::InvalidValue;

/// The literals of the range constants, so each refusal states the range it
/// checks from the one value (`concat!` takes literals only).
macro_rules! min_budget_ms {
    () => {
        1000
    };
}
macro_rules! max_budget_ms {
    () => {
        600000
    };
}
macro_rules! min_work_ms {
    () => {
        1000
    };
}
macro_rules! min_hook_reserve_ms_with_hook {
    () => {
        1000
    };
}

/// The default `budget_ms` of the built-in row.
pub const DEFAULT_BUDGET_MS: u64 = 25_000;
/// The smallest `budget_ms`.
pub(crate) const MIN_BUDGET_MS: u64 = min_budget_ms!();
/// The largest `budget_ms`.
pub(crate) const MAX_BUDGET_MS: u64 = max_budget_ms!();
/// The default `hook_reserve_ms`.
pub const DEFAULT_HOOK_RESERVE_MS: u64 = 5_000;
/// The smallest `hook_reserve_ms` when a post-sync hook is named.
pub(crate) const MIN_HOOK_RESERVE_MS_WITH_HOOK: u64 = min_hook_reserve_ms_with_hook!();
/// How much of the budget the drain and sync always leave: the reserve is at
/// most `budget_ms` minus this.
pub(crate) const MIN_WORK_MS: u64 = min_work_ms!();
/// How far past the budget the built-in row's sc-cron timeout sits, so
/// sc-cron never aborts a run before its hook and its lease release.
///
/// It is also how long after a run's deadline its sync lease expires
/// (bd-cache-sync's `LEASE_MARGIN` is this value): a lease taken for the
/// longest run, `budget_ms` at its most, then lasts exactly
/// [`MAX_LEASE_TTL`], the most the store accepts.
pub const ROW_TIMEOUT_MARGIN: Duration = Duration::from_secs(10);

// The defaults are in range, so `TickBudget::default` needs no check.
const _: () = assert!(DEFAULT_BUDGET_MS >= MIN_BUDGET_MS && DEFAULT_BUDGET_MS <= MAX_BUDGET_MS);
const _: () = assert!(DEFAULT_HOOK_RESERVE_MS <= DEFAULT_BUDGET_MS - MIN_WORK_MS);

/// The longest a sync lease may be taken for: the largest row timeout
/// (`budget_ms` at its most, 600 s, plus the 10 s row-timeout margin).
///
/// A run holds the lease at most its row's timeout; a holder that asks for
/// longer and dies would block every later run until the file is edited by
/// hand, so the store refuses a lease past `now` plus this
/// ([`check_lease_expiry`]).
pub const MAX_LEASE_TTL: Duration =
    Duration::from_millis(MAX_BUDGET_MS).saturating_add(ROW_TIMEOUT_MARGIN);

/// Checks a lease's `expires_at` against `now` (REQ-BDC-0722): it must be
/// after `now` and at most [`MAX_LEASE_TTL`] past it. Both stores call it in
/// `lease_take` before anything is written.
///
/// # Errors
///
/// `BAD_ARGS` naming the bound the expiry breaks.
pub fn check_lease_expiry(
    expires_at: &sc_beads::Timestamp,
    now: &sc_beads::Timestamp,
) -> Result<(), crate::CacheError> {
    let ttl = expires_at.unix().saturating_sub(now.unix());
    let most = i64::try_from(MAX_LEASE_TTL.as_secs()).unwrap_or(i64::MAX);
    if ttl <= 0 {
        return Err(crate::CacheError::BadArgs {
            detail: format!("lease expires_at {expires_at} is not after now ({now})"),
        });
    }
    if ttl > most {
        return Err(crate::CacheError::BadArgs {
            detail: format!(
                "lease expires_at {expires_at} is {ttl}s after now; a lease lasts at most {most}s"
            ),
        });
    }
    Ok(())
}

/// The built-in row's budget and the part of it kept for the post-sync hook.
///
/// The drain and the sync use at most [`TickBudget::work`], so the hook always
/// has at least [`TickBudget::hook_reserve`] (REQ-BDC-0703 as amended). Built
/// only by [`TickBudget::new`], so every value is in range.
///
/// # Examples
///
/// ```
/// use std::time::Duration;
/// use bd_cache_model::TickBudget;
/// let b = TickBudget::new(None, None, true)?;
/// assert_eq!(b.budget(), Duration::from_millis(25_000));
/// assert_eq!(b.hook_reserve(), Duration::from_millis(5_000));
/// assert_eq!(b.work(), Duration::from_millis(20_000));
/// assert_eq!(b.row_timeout(), Duration::from_millis(35_000));
/// assert!(TickBudget::new(Some(999), None, false).is_err());
/// assert!(TickBudget::new(Some(2_000), Some(1_001), false).is_err());
/// assert!(TickBudget::new(Some(2_000), Some(0), true).is_err());
/// assert!(TickBudget::new(Some(2_000), Some(0), false).is_ok());
/// # Ok::<(), sc_beads::InvalidValue>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TickBudget {
    budget: Duration,
    hook_reserve: Duration,
}

impl TickBudget {
    /// The budget of `budget_ms` (default [`DEFAULT_BUDGET_MS`], range 1000
    /// to 600000) with a hook reserve of `hook_reserve_ms` (default
    /// [`DEFAULT_HOOK_RESERVE_MS`], range 0 to `budget_ms` − 1000, and at
    /// least 1000 when `hook_named`). Each refusal states its range.
    ///
    /// # Errors
    ///
    /// [`InvalidValue`] naming `budget_ms` or `hook_reserve_ms` (its `kind`)
    /// for a value out of range, the default reserve included.
    pub fn new(
        budget_ms: Option<u64>,
        hook_reserve_ms: Option<u64>,
        hook_named: bool,
    ) -> Result<Self, InvalidValue> {
        let budget = budget_ms.unwrap_or(DEFAULT_BUDGET_MS);
        if !(MIN_BUDGET_MS..=MAX_BUDGET_MS).contains(&budget) {
            return Err(InvalidValue {
                kind: "budget_ms",
                value: budget.to_string(),
                reason: concat!(
                    "out of range: budget_ms is ",
                    min_budget_ms!(),
                    " to ",
                    max_budget_ms!()
                ),
            });
        }
        let reserve = hook_reserve_ms.unwrap_or(DEFAULT_HOOK_RESERVE_MS);
        if reserve > budget.saturating_sub(MIN_WORK_MS) {
            return Err(InvalidValue {
                kind: "hook_reserve_ms",
                value: reserve.to_string(),
                reason: concat!(
                    "out of range: hook_reserve_ms is 0 to budget_ms - ",
                    min_work_ms!()
                ),
            });
        }
        if hook_named && reserve < MIN_HOOK_RESERVE_MS_WITH_HOOK {
            return Err(InvalidValue {
                kind: "hook_reserve_ms",
                value: reserve.to_string(),
                reason: concat!(
                    "out of range: hook_reserve_ms is at least ",
                    min_hook_reserve_ms_with_hook!(),
                    " when a hook is named"
                ),
            });
        }
        Ok(Self {
            budget: Duration::from_millis(budget),
            hook_reserve: Duration::from_millis(reserve),
        })
    }

    /// The whole budget of one run.
    #[must_use]
    pub fn budget(&self) -> Duration {
        self.budget
    }

    /// The part the drain and the sync may not use.
    #[must_use]
    pub fn hook_reserve(&self) -> Duration {
        self.hook_reserve
    }

    /// The part the drain and the sync may use: the budget minus the reserve.
    #[must_use]
    pub fn work(&self) -> Duration {
        self.budget.saturating_sub(self.hook_reserve)
    }

    /// The built-in row's sc-cron timeout: the budget plus 10 s, so sc-cron
    /// never aborts a run before its hook and its lease release.
    #[must_use]
    pub fn row_timeout(&self) -> Duration {
        self.budget.saturating_add(ROW_TIMEOUT_MARGIN)
    }
}

impl Default for TickBudget {
    /// The defaults, [`DEFAULT_BUDGET_MS`] with [`DEFAULT_HOOK_RESERVE_MS`]
    /// and no hook named: the budget of an on-demand tick with no built-in
    /// row. Both are in range (checked when this crate compiles), so this is
    /// `TickBudget::new(None, None, false)` with no refusal to handle.
    fn default() -> Self {
        Self {
            budget: Duration::from_millis(DEFAULT_BUDGET_MS),
            hook_reserve: Duration::from_millis(DEFAULT_HOOK_RESERVE_MS),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The default is exactly `new(None, None, false)`; planted negative: a
    /// budget with a hook named is not the default one (its reserve check differs).
    #[test]
    fn the_default_budget_is_the_unchecked_new() {
        assert_eq!(
            Ok(TickBudget::default()),
            TickBudget::new(None, None, false)
        );
        assert_eq!(TickBudget::default().row_timeout(), Duration::from_secs(35));
        assert_ne!(
            Ok(TickBudget::default()),
            TickBudget::new(Some(DEFAULT_BUDGET_MS + 1), None, false)
        );
    }
}
