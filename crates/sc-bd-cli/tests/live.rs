//! Live probes against a real `bd` (NFR-BDC-0006, REQ-BDC-0905).
//!
//! Every test here is `#[ignore]`d and also returns at once unless `SC_BD_LIVE=1`, so
//! neither `cargo test` nor `cargo test -- --ignored` touches a database by accident.
//! They are run by the team-lead on the `m5-atmbench` testbed account and by the
//! bd-compatibility workflow (`.github/workflows/bd-compat.yml`), always through
//! `scripts/bd-compat/run-live.sh`, which starts a scratch Dolt server, runs
//! `bd init` in a scratch directory and removes both afterwards.
//!
//! Environment:
//!
//! - `SC_BD_LIVE=1`: run the probes.
//! - `SC_BD_LIVE_DIR`: the scratch workspace. It must contain
//!   `.beads/sc-bd-live-scratch`, the marker `scripts/bd-compat/setup-scratch.sh`
//!   writes; a workspace without it is refused, so a shared database cannot be used
//!   by mistake.
//! - `SC_BD_BIN`: the `bd` to test (default `bd` on the extended `PATH`).
//!
//! The probes repeat the P-6 manual probe (`docs/research/bd-blocked-state-probe.md`)
//! as code and check the other facts the cache relies on. a-7 records a full run.

use std::collections::BTreeSet;
use std::path::Path;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use sc_bd_cli::{BdCli, ProjectLocks};
use sc_beads::{
    parse_bd_rows, schema_sha256, Actor, Assignee, Bead, BeadId, BeadsCacheError, CacheBackend,
    CliBackend, Clock, Deadline, Guards, MetaValue, Patch, ProjectRef, Refusal, ScopeFilter,
    SystemClock, Timestamp, BD_SCHEMA_SHA256, BD_VERSION,
};

const MARKER: &str = ".beads/sc-bd-live-scratch";
/// bd stamps `updated_at` with second precision; a probe waits this long between two
/// writes so that a bump is visible.
const TICK: Duration = Duration::from_millis(1_200);
/// Each live call may take this long (a loaded testbed can be slow; the cache's own
/// cap is 30 s).
const CALL: Duration = Duration::from_secs(60);

struct Live {
    cli: BdCli,
    project: ProjectRef,
    /// A label no other probe or run uses, so scopes never overlap.
    label: String,
    seq: std::cell::Cell<u32>,
}

impl Live {
    /// `None` unless `SC_BD_LIVE=1`; an error when it is set but the workspace is not a scratch one.
    fn open() -> Result<Option<Self>, String> {
        if std::env::var("SC_BD_LIVE").as_deref() != Ok("1") {
            eprintln!("skipped: SC_BD_LIVE is not 1");
            return Ok(None);
        }
        let dir = std::env::var("SC_BD_LIVE_DIR").map_err(|e| {
            format!("SC_BD_LIVE_DIR is not usable ({e}); see scripts/bd-compat/run-live.sh")
        })?;
        if !Path::new(&dir).join(MARKER).is_file() {
            return Err(format!(
                "{dir} has no {MARKER}: live probes only run in a scratch workspace made by \
                 scripts/bd-compat/setup-scratch.sh, never in a shared database"
            ));
        }
        let bin = std::env::var("SC_BD_BIN").unwrap_or_else(|_| "bd".to_owned());
        let clock: Arc<dyn Clock> = Arc::new(SystemClock);
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        Ok(Some(Self {
            cli: BdCli::new_with_clock(bin, Arc::clone(shared_locks()), clock),
            project: ProjectRef::local(Some(dir)),
            label: format!("live-{}-{nanos}", std::process::id()),
            seq: std::cell::Cell::new(0),
        }))
    }

    fn scope(&self) -> ScopeFilter {
        ScopeFilter::Label(self.label.clone())
    }

    fn actor() -> Result<Actor, String> {
        Actor::new("sc-bd-live").map_err(|e| e.to_string())
    }

    /// Runs `bd <args>` outside the cache surface (setup and raw observations).
    fn raw(&self, args: &[&str]) -> Result<String, String> {
        let out = self
            .cli
            .run_raw(&self.project, args)
            .map_err(|e| format!("bd {args:?}: {e}"))?;
        if out.success {
            Ok(out.stdout)
        } else {
            Err(format!(
                "bd {args:?} exited {:?}: {}",
                out.status, out.stderr
            ))
        }
    }

    /// Creates a bead, in this probe's scope unless `outside_scope`, and returns its id.
    fn create(&self, title: &str, outside_scope: bool) -> Result<BeadId, String> {
        self.seq.set(self.seq.get() + 1);
        let title = format!("{title} #{}", self.seq.get());
        let mut args = vec!["create", title.as_str()];
        if !outside_scope {
            args.extend(["--labels", self.label.as_str()]);
        }
        args.push("--json");
        let out = self.raw(&args)?;
        let value: serde_json::Value =
            serde_json::from_str(&out).map_err(|e| format!("bd create output: {e}: {out}"))?;
        let id = value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .ok_or_else(|| format!("bd create printed no id: {out}"))?;
        BeadId::new(id).map_err(|e| e.to_string())
    }

    fn dep(&self, from: &BeadId, on: &BeadId, kind: &str) -> Result<(), String> {
        self.raw(&["dep", "add", from.as_str(), on.as_str(), "--type", kind])
            .map(drop)
    }

    fn read(&self, id: &BeadId) -> Result<Bead, String> {
        let rows = self
            .cli
            .list_by_ids(&self.project, std::slice::from_ref(id), deadline())
            .map_err(|e| e.to_string())?;
        rows.into_iter()
            .next()
            .ok_or_else(|| format!("{id} is not in bd"))
    }
}

/// The deadline of one live call.
fn deadline() -> Deadline {
    Deadline::after(&SystemClock, CALL)
}

/// The one lock table of the process: every probe's `BdCli` shares it, so two probes
/// on one workspace serialize as the cache's callers do.
fn shared_locks() -> &'static Arc<ProjectLocks> {
    static LOCKS: OnceLock<Arc<ProjectLocks>> = OnceLock::new();
    LOCKS.get_or_init(|| Arc::new(ProjectLocks::new()))
}

fn instant(row: &Bead) -> Result<i64, String> {
    Timestamp::parse(&row.updated_at)
        .map(|t| t.unix())
        .map_err(|e| e.to_string())
}

fn ids(list: impl IntoIterator<Item = BeadId>) -> BTreeSet<String> {
    list.into_iter().map(BeadId::into_string).collect()
}

/// `bd close` and `bd reopen` bump the bead's own `updated_at` and never its
/// depender's (bd's `BlockedStateInvariant`; P-6).
#[test]
#[ignore = "live bd probe: run through scripts/bd-compat/run-live.sh (SC_BD_LIVE=1, SC_BD_LIVE_DIR)"]
fn close_and_reopen_bump_the_beads_own_updated_at() -> Result<(), String> {
    let Some(live) = Live::open()? else {
        return Ok(());
    };
    let blocker = live.create("blocker", false)?;
    let depender = live.create("depender", false)?;
    live.dep(&depender, &blocker, "blocks")?;
    let (blocker_0, depender_0) = (live.read(&blocker)?, live.read(&depender)?);

    std::thread::sleep(TICK);
    let actor = Live::actor()?;
    live.cli
        .close_guarded(
            &live.project,
            &blocker,
            Some("probe"),
            &Guards::default(),
            &actor,
            deadline(),
        )
        .map_err(|e| e.to_string())?;
    let closed = live.read(&blocker)?;
    assert_eq!(closed.status.as_deref(), Some("closed"));
    assert!(
        instant(&closed)? > instant(&blocker_0)?,
        "close bumps the bead"
    );

    std::thread::sleep(TICK);
    let reopen = Patch {
        status: Some("open".into()),
        ..Patch::default()
    };
    live.cli
        .update_guarded(
            &live.project,
            &blocker,
            &reopen,
            &Guards::default(),
            &actor,
            deadline(),
        )
        .map_err(|e| e.to_string())?;
    let reopened = live.read(&blocker)?;
    assert_eq!(reopened.status.as_deref(), Some("open"));
    assert!(
        instant(&reopened)? > instant(&closed)?,
        "reopen bumps the bead"
    );
    assert_eq!(
        instant(&live.read(&depender)?)?,
        instant(&depender_0)?,
        "the depender's updated_at never moves"
    );
    Ok(())
}

/// `bd list --id` returns a closed bead only when the status list names `closed`.
#[test]
#[ignore = "live bd probe: run through scripts/bd-compat/run-live.sh (SC_BD_LIVE=1, SC_BD_LIVE_DIR)"]
fn list_by_id_returns_a_closed_bead_only_with_the_status_list() -> Result<(), String> {
    let Some(live) = Live::open()? else {
        return Ok(());
    };
    let bead = live.create("to close", false)?;
    live.cli
        .close_guarded(
            &live.project,
            &bead,
            None,
            &Guards::default(),
            &Live::actor()?,
            deadline(),
        )
        .map_err(|e| e.to_string())?;

    let without = live.raw(&["list", "--id", bead.as_str(), "--json"])?;
    let without = parse_bd_rows(&without).map_err(|e| e.to_string())?;
    assert!(
        without.is_empty(),
        "bd omits a closed bead without --status: {without:?}"
    );

    let with = live
        .cli
        .list_by_ids(&live.project, std::slice::from_ref(&bead), deadline())
        .map_err(|e| e.to_string())?;
    assert_eq!(with.len(), 1, "the cache's call names all seven statuses");
    assert_eq!(with[0].status.as_deref(), Some("closed"));
    Ok(())
}

/// The beads of the P-6 probe, created in the live scope.
struct P6 {
    /// An open bead that blocks `b` and `parent`.
    a: BeadId,
    b: BeadId,
    parent: BeadId,
    /// A child of `parent` (inherits its blocked state).
    child: BeadId,
    /// Related to `a`, which does not block.
    related: BeadId,
    /// A blocker outside the label.
    outside: BeadId,
    /// Blocked by `outside`.
    guarded: BeadId,
    /// Depends on a closed bead, so it is ready.
    free: BeadId,
    /// Deferred into the future.
    deferred: BeadId,
}

impl P6 {
    fn build(live: &Live) -> Result<Self, String> {
        let a = live.create("open blocker", false)?;
        let b = live.create("blocked by a", false)?;
        live.dep(&b, &a, "blocks")?;
        let parent = live.create("parent blocked by a", false)?;
        live.dep(&parent, &a, "blocks")?;
        let child = live.create("child of a blocked parent", false)?;
        live.dep(&child, &parent, "parent-child")?;
        let related = live.create("related to a", false)?;
        live.dep(&related, &a, "related")?;
        let outside = live.create("blocker outside the label", true)?;
        let guarded = live.create("blocked by an outside bead", false)?;
        live.dep(&guarded, &outside, "blocks")?;
        let done = live.create("closed blocker", false)?;
        let free = live.create("depends on a closed bead", false)?;
        live.dep(&free, &done, "blocks")?;
        live.cli
            .close_guarded(
                &live.project,
                &done,
                None,
                &Guards::default(),
                &Live::actor()?,
                deadline(),
            )
            .map_err(|e| e.to_string())?;
        let deferred = live.create("deferred into the future", false)?;
        live.raw(&["update", deferred.as_str(), "--defer", "2099-01-01"])?;

        Ok(Self {
            a,
            b,
            parent,
            child,
            related,
            outside,
            guarded,
            free,
            deferred,
        })
    }
}

/// The cache's `ready` predicate over the scope: open, no future `defer_until`, and
/// not in `bd blocked` (ADR-BDC-0002).
fn ready_predicate(
    live: &Live,
    blocked: &[sc_beads::BlockedRow],
) -> Result<BTreeSet<String>, String> {
    let scope_rows = live
        .cli
        .list_scope(&live.project, &live.scope(), None, deadline())
        .map_err(|e| e.to_string())?;
    let now = SystemClock.now_utc();
    let blocked_ids = ids(blocked.iter().map(|r| r.id.clone()));
    let mut predicate = BTreeSet::new();
    for row in &scope_rows {
        let open = row.status.as_deref() == Some("open");
        let deferred_ahead = match &row.defer_until {
            Some(t) => Timestamp::parse(t).map_err(|e| e.to_string())? > now,
            None => false,
        };
        if open && !deferred_ahead && !blocked_ids.contains(row.id.as_str()) {
            predicate.insert(row.id.as_str().to_owned());
        }
    }
    Ok(predicate)
}

/// The P-6 probe as code: list rows carry no `is_blocked`; `bd blocked` has the shape
/// and the inheritance the cache reads; and the cache's `ready` predicate (open, no
/// future `defer_until`, not in `bd blocked`) equals `bd ready` for the scope.
#[test]
#[ignore = "live bd probe: run through scripts/bd-compat/run-live.sh (SC_BD_LIVE=1, SC_BD_LIVE_DIR)"]
fn blocked_state_matches_the_p6_probe_and_the_ready_predicate() -> Result<(), String> {
    let Some(live) = Live::open()? else {
        return Ok(());
    };
    let P6 {
        a,
        b,
        parent,
        child,
        related,
        outside,
        guarded,
        free,
        deferred,
    } = P6::build(&live)?;

    // No list row carries `is_blocked`.
    let listed = live.raw(&[
        "list",
        "-l",
        &live.label,
        "--status",
        "open,in_progress,blocked,deferred,closed,pinned,hooked",
        "-n",
        "0",
        "--json",
    ])?;
    let raw: serde_json::Value = serde_json::from_str(&listed).map_err(|e| e.to_string())?;
    let rows = raw.as_array().ok_or("bd list did not print an array")?;
    assert!(!rows.is_empty());
    assert!(
        rows.iter().all(|r| r.get("is_blocked").is_none()),
        "bd list rows must carry no is_blocked"
    );

    // `bd blocked`: shape, outside blockers, parent-child inheritance, `related` and a
    // closed blocker do not block.
    let blocked = live
        .cli
        .list_blocked(&live.project, &live.scope(), deadline())
        .map_err(|e| e.to_string())?;
    let blockers = |of: &BeadId| -> Option<BTreeSet<String>> {
        blocked
            .iter()
            .find(|r| &r.id == of)
            .map(|r| ids(r.blocked_by.iter().cloned()))
    };
    assert_eq!(blockers(&b), Some(ids([a.clone()])), "blocks");
    assert_eq!(blockers(&parent), Some(ids([a.clone()])), "blocks (parent)");
    assert_eq!(
        blockers(&child),
        Some(ids([parent.clone()])),
        "a child of a blocked parent lists the parent, not the parent's blockers"
    );
    assert_eq!(
        blockers(&guarded),
        Some(ids([outside.clone()])),
        "a blocker outside the label filter is listed"
    );
    for not_blocked in [&a, &related, &free, &deferred] {
        assert!(
            blockers(not_blocked).is_none(),
            "{not_blocked} must not be blocked"
        );
    }

    // The cache's ready predicate equals `bd ready`.
    let predicate = ready_predicate(&live, &blocked)?;
    let ready = ids(live
        .cli
        .list_ready(&live.project, &live.scope(), deadline())
        .map_err(|e| e.to_string())?);
    assert_eq!(predicate, ready, "cache ready predicate vs bd ready");
    assert!(ready.contains(a.as_str()) && ready.contains(free.as_str()));
    assert!(
        !ready.contains(deferred.as_str()),
        "a future defer_until is not ready"
    );

    // Closing the outside blocker unblocks the bead it blocked.
    live.cli
        .close_guarded(
            &live.project,
            &outside,
            None,
            &Guards::default(),
            &Live::actor()?,
            deadline(),
        )
        .map_err(|e| e.to_string())?;
    let after = live
        .cli
        .list_blocked(&live.project, &live.scope(), deadline())
        .map_err(|e| e.to_string())?;
    assert!(
        after.iter().all(|r| r.id != guarded),
        "closing the blocker unblocks"
    );
    Ok(())
}

/// A stale `--if-status` / `--if-assignee` exits 13 and writes nothing; `""` means
/// unassigned.
#[test]
#[ignore = "live bd probe: run through scripts/bd-compat/run-live.sh (SC_BD_LIVE=1, SC_BD_LIVE_DIR)"]
fn unmet_guards_exit_13_and_write_nothing() -> Result<(), String> {
    let Some(live) = Live::open()? else {
        return Ok(());
    };
    let bead = live.create("guarded", false)?;
    let actor = Live::actor()?;
    let claim = Patch {
        status: Some("in_progress".into()),
        ..Patch::default()
    };
    let stale_status = Guards {
        status: Some("closed".into()),
        assignee: None,
    };
    let r = live.cli.update_guarded(
        &live.project,
        &bead,
        &claim,
        &stale_status,
        &actor,
        deadline(),
    );
    assert!(
        matches!(r, Err(BeadsCacheError::Refused(Refusal::Precondition))),
        "{r:?}"
    );
    let r = live.cli.close_guarded(
        &live.project,
        &bead,
        None,
        &stale_status,
        &actor,
        deadline(),
    );
    assert!(
        matches!(r, Err(BeadsCacheError::Refused(Refusal::Precondition))),
        "{r:?}"
    );
    assert_eq!(
        live.read(&bead)?.status.as_deref(),
        Some("open"),
        "nothing was written"
    );

    let held = Guards {
        status: None,
        assignee: Some(Assignee::Named("someone".into())),
    };
    let r = live
        .cli
        .update_guarded(&live.project, &bead, &claim, &held, &actor, deadline());
    assert!(
        matches!(r, Err(BeadsCacheError::Refused(Refusal::Precondition))),
        "{r:?}"
    );
    let unassigned = Guards {
        status: Some("open".into()),
        assignee: Some(Assignee::Unassigned),
    };
    live.cli
        .update_guarded(
            &live.project,
            &bead,
            &claim,
            &unassigned,
            &actor,
            deadline(),
        )
        .map_err(|e| e.to_string())?;
    assert_eq!(live.read(&bead)?.status.as_deref(), Some("in_progress"));
    Ok(())
}

/// Metadata values the validator accepts come back from bd's coercion unchanged
/// (REQ-BDC-0303), including bools sent as `true` / `false`.
#[test]
#[ignore = "live bd probe: run through scripts/bd-compat/run-live.sh (SC_BD_LIVE=1, SC_BD_LIVE_DIR)"]
fn metadata_values_round_trip_through_real_bd() -> Result<(), String> {
    let Some(live) = Live::open()? else {
        return Ok(());
    };
    let bead = live.create("metadata", false)?;
    let accepted: Vec<(&str, MetaValue)> = vec![
        ("i_pos", MetaValue::Int(5)),
        ("i_neg", MetaValue::Int(-3)),
        ("i_zero", MetaValue::Int(0)),
        ("b_true", MetaValue::Bool(true)),
        ("b_false", MetaValue::Bool(false)),
        ("s_plain", MetaValue::Str("abc123".into())),
        ("s_list", MetaValue::Str("[1]".into())),
        ("s_obj", MetaValue::Str(r#"{"x":1}"#.into())),
        ("s_quoted", MetaValue::Str(r#""5""#.into())),
        ("s_cap", MetaValue::Str("True".into())),
        ("s_zero_lead", MetaValue::Str("01".into())),
    ];
    let patch = Patch {
        set_metadata: accepted
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
        ..Patch::default()
    };
    live.cli
        .update_guarded(
            &live.project,
            &bead,
            &patch,
            &Guards::default(),
            &Live::actor()?,
            deadline(),
        )
        .map_err(|e| e.to_string())?;
    let row = live.read(&bead)?;
    for (key, value) in &accepted {
        assert_eq!(
            row.metadata.get(*key),
            Some(&value.to_json()),
            "{key}: {value:?} must come back from bd unchanged"
        );
    }
    Ok(())
}

/// A database server that is not there is `Unavailable`, not `Failed` and not a timeout.
#[test]
#[ignore = "live bd probe: run through scripts/bd-compat/run-live.sh (SC_BD_LIVE=1, SC_BD_LIVE_DIR)"]
fn unreachable_server_is_unavailable() -> Result<(), String> {
    let Some(live) = Live::open()? else {
        return Ok(());
    };
    // A scratch workspace whose metadata points at a closed port; `bd` is run there
    // and nowhere near the live database.
    let dir = std::env::temp_dir().join(format!("sc-bd-live-down-{}", std::process::id()));
    let beads = dir.join(".beads");
    std::fs::create_dir_all(&beads).map_err(|e| e.to_string())?;
    std::fs::write(
        beads.join("metadata.json"),
        r#"{"database":"dolt","backend":"dolt","dolt_mode":"server","dolt_server_host":"127.0.0.1","dolt_server_port":1,"dolt_server_user":"root","dolt_database":"down","project_id":"sc-bd-live-down"}"#,
    )
    .map_err(|e| e.to_string())?;
    let down = ProjectRef::local(Some(dir.to_string_lossy().into_owned()));
    let r = live.cli.list_scope(&down, &live.scope(), None, deadline());
    let _ = std::fs::remove_dir_all(&dir);
    let e = r.err().ok_or("a list against a closed port succeeded")?;
    assert!(e.is_unavailable() && !e.is_timeout(), "{e:?}");
    Ok(())
}

/// `fingerprint` reports what `bd version` and `bd schema` print, and the pinned
/// fingerprint is the one this bd presents (the compat workflow reports a difference
/// with the schema diff; this probe makes the failure local too).
#[test]
#[ignore = "live bd probe: run through scripts/bd-compat/run-live.sh (SC_BD_LIVE=1, SC_BD_LIVE_DIR)"]
fn fingerprint_reports_what_bd_prints() -> Result<(), String> {
    let Some(live) = Live::open()? else {
        return Ok(());
    };
    let fp = live
        .cli
        .fingerprint(&live.project, deadline())
        .map_err(|e| e.to_string())?;
    let schema = live.raw(&["schema"])?;
    assert_eq!(fp.schema_sha256, schema_sha256(&schema));
    let version = live.raw(&["version"])?;
    assert!(
        version.contains(&fp.bd_version),
        "{version:?} vs {}",
        fp.bd_version
    );
    // The bd under test is the pinned one, unless SC_BD_PINNED_ANY=1 (a bd release
    // being checked for compatibility: the workflow reports the difference itself).
    if std::env::var("SC_BD_PINNED_ANY").as_deref() != Ok("1") {
        assert_eq!(
            fp.schema_sha256.as_str(),
            BD_SCHEMA_SHA256,
            "schema moved: regenerate the rows"
        );
        assert_eq!(fp.bd_version, BD_VERSION);
    }
    Ok(())
}
