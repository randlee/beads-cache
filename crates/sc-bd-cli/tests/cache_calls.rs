//! The cache calls of `BdCli`: recorded argv, error mapping, deadlines and metadata.
//!
//! Most tests run `BdCli` over `RecordingInvoker`, which records the exact argv of
//! every call and replies from a script; the rest run it over `FakeBd`, which parses
//! that argv as bd does. No test here starts a process (tests/process.rs does).

use std::sync::Arc;
use std::time::Duration;

use sc_bd_cli::bounded::{OutputStream, MAX_OUTPUT_BYTES};
use sc_bd_cli::testing::{FakeBd, RecordingInvoker};
use sc_bd_cli::{BdCli, BoundedError, CliInvoker, TimeoutPhase};
use sc_beads::error::BeadsError;
use sc_beads::testing::contract::{P6_INITIAL_BLOCKED, P6_LABEL};
use sc_beads::testing::{actor, bead, id, FakeOp, Injection, ManualClock};
use sc_beads::{
    Assignee, Bead, BeadId, BeadsCacheError, BlockedRow, CacheBackend, CheckStep, CliClient,
    CliOutput, CliProbe, Clock, Deadline, DepType, Guards, MetaValue, Patch, ProjectRef, Refusal,
    ScopeFilter, SyncCursor, BD_SCHEMA_SHA256, BD_VERSION,
};

const ALL_SEVEN: &str = "open,in_progress,blocked,deferred,closed,pinned,hooked";

/// Delegates to a shared `RecordingInvoker`, so the test can hand `BdCli` a
/// `Box<dyn CliInvoker>` and keep reading `calls()` on the same instance.
/// It keeps the provided no-op `set_child_var`: `RecordingInvoker` spawns nothing,
/// so the forwarding rule exempts this wrapper (ADR-BDCLI-0004, lead ruling S52).
struct Shared(Arc<RecordingInvoker>);

impl CliInvoker for Shared {
    fn binary(&self) -> String {
        self.0.binary()
    }
    fn client_info(&self) -> Option<CliProbe> {
        self.0.client_info()
    }
    fn probe(&self) -> Option<CliProbe> {
        self.0.probe()
    }
    fn run_json(
        &self,
        project: &ProjectRef,
        command: &str,
        args: &[String],
    ) -> Result<String, BeadsError> {
        self.0.run_json(project, command, args)
    }
    fn run_raw(&self, project: &ProjectRef, args: &[&str]) -> Result<CliOutput, BeadsError> {
        self.0.run_raw(project, args)
    }
    fn run_bounded(
        &self,
        project: &ProjectRef,
        argv: &[String],
        timeout: Duration,
    ) -> Result<CliOutput, BoundedError> {
        self.0.run_bounded(project, argv, timeout)
    }
}

struct Rig {
    cli: BdCli,
    rec: Arc<RecordingInvoker>,
    clock: Arc<ManualClock>,
}

impl Rig {
    fn new(rec: RecordingInvoker) -> Self {
        let rec = Arc::new(rec);
        let clock = Arc::new(ManualClock::default());
        let cli = BdCli::with_invoker(Box::new(Shared(Arc::clone(&rec))))
            .with_clock(Arc::clone(&clock) as Arc<dyn Clock>);
        Self { cli, rec, clock }
    }

    fn deadline(&self) -> Deadline {
        Deadline::after(&*self.clock, Duration::from_secs(30))
    }

    fn argv(&self) -> Vec<Vec<String>> {
        self.rec.calls()
    }
}

fn p() -> ProjectRef {
    ProjectRef::local(None)
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "the shape of an invoker reply, so scripts read `reply_bounded(ok(..))`"
)]
fn ok(stdout: impl Into<String>) -> Result<CliOutput, BoundedError> {
    Ok(CliOutput {
        status: Some(0),
        success: true,
        stdout: stdout.into(),
        stderr: String::new(),
    })
}

#[expect(
    clippy::unnecessary_wraps,
    reason = "the shape of an invoker reply, so scripts read `reply_bounded(exits(..))`"
)]
fn exits(status: i32, stderr: &str) -> Result<CliOutput, BoundedError> {
    Ok(CliOutput {
        status: Some(status),
        success: false,
        stdout: String::new(),
        stderr: stderr.into(),
    })
}

fn rows_json(rows: &[Bead]) -> String {
    serde_json::to_string(rows).unwrap_or_default()
}

fn strings(words: &[&str]) -> Vec<String> {
    words.iter().map(|w| (*w).to_owned()).collect()
}

fn labelled(name: &str, label: &str) -> Bead {
    let mut b = bead(name);
    b.labels = vec![label.to_owned()];
    b
}

// ---- REQ-BDC-0106 ------------------------------------------------------------

/// The backend check is `bd version` then `bd schema`; `bd context` is never run
/// (bd 1.3.1 refuses it under temp directories), and a `FakeBd` that knows no
/// `context` command still answers the check.
#[test]
fn test_backend_check_does_not_need_bd_context() {
    let schema = include_str!("fixtures/bd-schema-1.3.1.json");
    let rig = Rig::new(
        RecordingInvoker::new(None)
            .reply_bounded(ok("bd version 1.3.1 (dev)\n"))
            .reply_bounded(ok(schema)),
    );
    let fp = rig.cli.fingerprint(&p(), rig.deadline()).unwrap();
    assert_eq!(fp.bd_version, BD_VERSION);
    assert_eq!(fp.schema_sha256.as_str(), BD_SCHEMA_SHA256);
    assert_eq!(fp.schema_version, "1");
    assert_eq!(
        rig.argv(),
        vec![strings(&["version"]), strings(&["schema"])]
    );
    assert!(rig.argv().iter().flatten().all(|w| w != "context"));

    let clock = Arc::new(ManualClock::default());
    let fake = FakeBd::new(Arc::clone(&clock));
    let seen = fake.clone();
    let cli = BdCli::with_invoker(Box::new(fake)).with_clock(Arc::clone(&clock) as Arc<dyn Clock>);
    let d = Deadline::after(&*clock, Duration::from_secs(30));
    cli.fingerprint(&p(), d).unwrap();
    assert_eq!(
        seen.calls(),
        vec![strings(&["version"]), strings(&["schema"])]
    );
}

#[test]
fn a_failing_check_step_is_named() {
    let rig = Rig::new(RecordingInvoker::new(None).reply_bounded(exits(1, "boom")));
    let e = rig.cli.fingerprint(&p(), rig.deadline()).unwrap_err();
    assert!(
        matches!(
            e,
            BeadsCacheError::CheckFailed {
                step: CheckStep::Version,
                code: 1,
                ..
            }
        ),
        "{e:?}"
    );

    let rig = Rig::new(
        RecordingInvoker::new(None)
            .reply_bounded(ok("bd version 1.3.1 (dev)"))
            .reply_bounded(exits(2, "no schema")),
    );
    let e = rig.cli.fingerprint(&p(), rig.deadline()).unwrap_err();
    assert!(
        matches!(
            e,
            BeadsCacheError::CheckFailed {
                step: CheckStep::Schema,
                code: 2,
                ..
            }
        ),
        "{e:?}"
    );
}

#[test]
fn unreadable_check_output_is_a_contract_error() {
    let rig = Rig::new(RecordingInvoker::new(None).reply_bounded(ok("garbage")));
    assert!(matches!(
        rig.cli.fingerprint(&p(), rig.deadline()),
        Err(BeadsCacheError::Contract { .. })
    ));
    let rig = Rig::new(
        RecordingInvoker::new(None)
            .reply_bounded(ok("bd version 1.3.1"))
            .reply_bounded(ok("not json")),
    );
    assert!(matches!(
        rig.cli.fingerprint(&p(), rig.deadline()),
        Err(BeadsCacheError::Contract { .. })
    ));
}

// ---- REQ-BDC-0201: recorded argv ---------------------------------------------

#[test]
fn recorded_argv_of_every_read_matches_the_baseline_forms() {
    let scope = ScopeFilter::Label(P6_LABEL.into());
    let rig = Rig::new(
        RecordingInvoker::new(None)
            .reply_bounded(ok("[]"))
            .reply_bounded(ok("[]"))
            .reply_bounded(ok("[]"))
            .reply_bounded(ok(P6_INITIAL_BLOCKED))
            .reply_bounded(ok("[]")),
    );
    let d = rig.deadline();
    let ids = [id("a-1"), id("b-2")];
    rig.cli.list_by_ids(&p(), &ids, d).unwrap();
    rig.cli.list_scope(&p(), &scope, None, d).unwrap();
    let cursor = SyncCursor::new("2026-10-23T09:05:00Z");
    rig.cli.list_scope(&p(), &scope, Some(&cursor), d).unwrap();
    rig.cli.list_blocked(&p(), &scope, d).unwrap();
    rig.cli.list_ready(&p(), &scope, d).unwrap();
    assert_eq!(
        rig.argv(),
        vec![
            strings(&["list", "--id", "a-1,b-2", "--status", ALL_SEVEN, "-n", "0", "--json"]),
            strings(&["list", "-l", "phase-p6", "--status", ALL_SEVEN, "-n", "0", "--json"]),
            strings(&[
                "list",
                "-l",
                "phase-p6",
                "--status",
                ALL_SEVEN,
                "-n",
                "0",
                "--updated-after",
                "2026-10-23T09:05:00Z",
                "--json"
            ]),
            strings(&["blocked", "-l", "phase-p6", "--json"]),
            strings(&["ready", "-l", "phase-p6", "-n", "0", "--json"]),
        ]
    );
}

#[test]
fn recorded_argv_of_every_write_matches_the_baseline_forms() {
    let rig = Rig::new(RecordingInvoker::new(None));
    let d = rig.deadline();
    let a = actor("san");
    let none = Guards::default();
    rig.cli
        .close_guarded(&p(), &id("b-1"), Some("done"), &none, &a, d)
        .unwrap();
    rig.cli
        .close_guarded(&p(), &id("b-1"), None, &none, &a, d)
        .unwrap();
    let patch = Patch {
        status: Some("in_progress".into()),
        assignee: Some(Assignee::Named("dev".into())),
        add_labels: vec!["new".into()],
        remove_labels: vec!["old".into()],
        set_metadata: [
            ("n".to_owned(), MetaValue::Int(5)),
            ("ok".to_owned(), MetaValue::Bool(true)),
            ("no".to_owned(), MetaValue::Bool(false)),
            ("sha".to_owned(), MetaValue::Str("abc123".into())),
        ]
        .into_iter()
        .collect(),
        unset_metadata: vec!["gone".into()],
        append_notes: Some("more".into()),
    };
    let guards = Guards {
        status: Some("open".into()),
        assignee: Some(Assignee::Unassigned),
    };
    rig.cli
        .update_guarded(&p(), &id("b-1"), &patch, &guards, &a, d)
        .unwrap();
    rig.cli
        .dep_add_typed(&p(), &id("b-1"), &id("b-2"), DepType::ParentChild, &a, d)
        .unwrap();
    assert_eq!(
        rig.argv(),
        vec![
            strings(&["close", "b-1", "--reason", "done", "--actor", "san"]),
            strings(&["close", "b-1", "--actor", "san"]),
            strings(&[
                "update",
                "b-1",
                "--status",
                "in_progress",
                "--assignee",
                "dev",
                "--add-label",
                "new",
                "--remove-label",
                "old",
                "--set-metadata",
                "n=5",
                "--set-metadata",
                "no=false",
                "--set-metadata",
                "ok=true",
                "--set-metadata",
                "sha=abc123",
                "--unset-metadata",
                "gone",
                "--append-notes",
                "more",
                "--if-status",
                "open",
                "--if-assignee",
                "",
                "--actor",
                "san"
            ]),
            strings(&[
                "dep",
                "add",
                "b-1",
                "b-2",
                "--type",
                "parent-child",
                "--actor",
                "san"
            ]),
        ]
    );
}

#[test]
fn list_by_ids_is_chunked_deduplicated_and_names_all_seven_statuses() {
    let many: Vec<_> = (0..250).map(|i| id(&format!("b-{i}"))).collect();
    let mut with_repeats = many.clone();
    with_repeats.extend(many.iter().take(10).cloned());
    let rig = Rig::new(
        RecordingInvoker::new(None)
            .reply_bounded(ok(rows_json(&[bead("b-0")])))
            .reply_bounded(ok(rows_json(&[bead("b-100")])))
            .reply_bounded(ok(rows_json(&[bead("b-200")]))),
    );
    let rows = rig
        .cli
        .list_by_ids(&p(), &with_repeats, rig.deadline())
        .unwrap();
    assert_eq!(rows.len(), 3, "the chunks' rows are concatenated");
    let calls = rig.argv();
    assert_eq!(calls.len(), 3, "100 + 100 + 50 ids");
    let sizes: Vec<usize> = calls.iter().map(|c| c[2].split(',').count()).collect();
    assert_eq!(sizes, [100, 100, 50]);
    for call in &calls {
        let at = call.iter().position(|w| w == "--status").unwrap();
        assert_eq!(
            call[at + 1],
            ALL_SEVEN,
            "closed beads are omitted otherwise"
        );
    }
    let sent: std::collections::BTreeSet<_> = calls.iter().flat_map(|c| c[2].split(',')).collect();
    assert_eq!(sent.len(), 250, "repeated ids are sent once");
}

#[test]
fn list_by_ids_with_no_ids_makes_no_call() {
    let rig = Rig::new(RecordingInvoker::new(None));
    assert!(rig
        .cli
        .list_by_ids(&p(), &[], rig.deadline())
        .unwrap()
        .is_empty());
    assert!(rig.argv().is_empty());
}

#[test]
fn rows_and_blocked_rows_come_back_typed() {
    let rig = Rig::new(
        RecordingInvoker::new(None)
            .reply_bounded(ok(rows_json(&[labelled("r-1", "x"), labelled("r-2", "x")])))
            .reply_bounded(ok(P6_INITIAL_BLOCKED)),
    );
    let scope = ScopeFilter::Label("x".into());
    let ready = rig.cli.list_ready(&p(), &scope, rig.deadline()).unwrap();
    assert_eq!(ready, [id("r-1"), id("r-2")]);
    let blocked: Vec<BlockedRow> = rig.cli.list_blocked(&p(), &scope, rig.deadline()).unwrap();
    assert!(!blocked.is_empty());
}

#[test]
fn a_row_that_breaks_the_contract_fails_the_whole_call_naming_bead_and_field() {
    let mut rows = serde_json::to_value(vec![bead("ok-1"), bead("bad-1")]).unwrap();
    rows[1]["surprise"] = serde_json::json!(true);
    let rig = Rig::new(RecordingInvoker::new(None).reply_bounded(ok(rows.to_string())));
    let e = rig
        .cli
        .list_scope(&p(), &ScopeFilter::Label("x".into()), None, rig.deadline())
        .unwrap_err();
    let BeadsCacheError::Contract { bead, field, .. } = &e else {
        panic!("{e:?}");
    };
    assert_eq!(bead.as_ref().map(BeadId::as_str), Some("bad-1"));
    assert_eq!(field, "surprise");
}

// ---- guards, empty patches, metadata keys ------------------------------------

#[test]
fn a_guarded_close_reads_the_bead_first_and_closes_only_when_the_guards_hold() {
    let mut b = bead("g-1");
    b.status = Some("in_progress".into());
    b.assignee = Some("dev".into());
    let rig = Rig::new(
        RecordingInvoker::new(None)
            .reply_bounded(ok(rows_json(&[b.clone()])))
            .reply_bounded(ok(rows_json(&[b.clone()])))
            .reply_bounded(ok("closed")),
    );
    let a = actor("san");
    let wrong = Guards {
        status: Some("open".into()),
        assignee: None,
    };
    let e = rig
        .cli
        .close_guarded(&p(), &id("g-1"), None, &wrong, &a, rig.deadline())
        .unwrap_err();
    assert!(
        matches!(e, BeadsCacheError::Refused(Refusal::Precondition)),
        "{e:?}"
    );
    assert_eq!(rig.argv().len(), 1, "a refused guard runs no `bd close`");

    let right = Guards {
        status: Some("in_progress".into()),
        assignee: Some(Assignee::Named("dev".into())),
    };
    rig.cli
        .close_guarded(&p(), &id("g-1"), None, &right, &a, rig.deadline())
        .unwrap();
    let calls = rig.argv();
    assert_eq!(calls.len(), 3);
    assert_eq!(calls[1][0], "list");
    assert_eq!(calls[2], strings(&["close", "g-1", "--actor", "san"]));
}

#[test]
fn a_guarded_close_of_an_unknown_bead_is_not_found_and_not_a_refusal() {
    let rig = Rig::new(RecordingInvoker::new(None).reply_bounded(ok("[]")));
    let guards = Guards {
        status: Some("open".into()),
        assignee: None,
    };
    let e = rig
        .cli
        .close_guarded(
            &p(),
            &id("nope"),
            None,
            &guards,
            &actor("san"),
            rig.deadline(),
        )
        .unwrap_err();
    assert!(
        matches!(&e, BeadsCacheError::NotFound { bead } if bead.as_str() == "nope"),
        "{e:?}"
    );
}

#[test]
fn an_empty_patch_asks_bd_nothing() {
    let rig = Rig::new(RecordingInvoker::new(None));
    rig.cli
        .update_guarded(
            &p(),
            &id("b-1"),
            &Patch::default(),
            &Guards::default(),
            &actor("san"),
            rig.deadline(),
        )
        .unwrap();
    assert!(rig.argv().is_empty());
}

#[test]
fn a_metadata_key_bd_cannot_carry_is_refused_before_any_call() {
    for key in ["", "a=b"] {
        let rig = Rig::new(RecordingInvoker::new(None));
        let patch = Patch {
            set_metadata: [(key.to_owned(), MetaValue::Int(1))].into_iter().collect(),
            ..Patch::default()
        };
        let e = rig
            .cli
            .update_guarded(
                &p(),
                &id("b-1"),
                &patch,
                &Guards::default(),
                &actor("san"),
                rig.deadline(),
            )
            .unwrap_err();
        assert!(
            matches!(
                e,
                BeadsCacheError::Btit(BeadsError::Unsupported {
                    client: CliClient::Bd,
                    ..
                })
            ),
            "{key:?}: {e:?}"
        );
        assert!(rig.argv().is_empty());
    }
}

// ---- REQ-BDC-0303 -------------------------------------------------------------

/// Metadata values the validator accepts reach `bd` unchanged and come back from bd's
/// coercion as the same JSON value; bools are `true` / `false`, never `True`.
///
/// Source of truth: `bd_cache_model::op::meta_value`; this restates its accepted set.
/// The accepted set is `bd_cache_model::op::meta_value`'s (an int, a bool, or a string
/// that is not padded with whitespace and does not read as a JSON number, boolean or
/// null; `bd-cache-model` is not a dependency of this crate, see
/// docs/sc-bd-cli/architecture.md). The refused values are listed to show that the
/// guard is what keeps them out: bd would store them as something else.
#[test]
fn test_metadata_values_round_trip_through_bd_coercion() {
    let accepted: Vec<(&str, MetaValue)> = vec![
        ("i_pos", MetaValue::Int(5)),
        ("i_neg", MetaValue::Int(-3)),
        ("i_zero", MetaValue::Int(0)),
        ("b_true", MetaValue::Bool(true)),
        ("b_false", MetaValue::Bool(false)),
        ("s_plain", MetaValue::Str("abc123".into())),
        ("s_sha", MetaValue::Str("deadbeef".into())),
        ("s_list", MetaValue::Str("[1]".into())),
        ("s_obj", MetaValue::Str(r#"{"x":1}"#.into())),
        ("s_quoted", MetaValue::Str(r#""5""#.into())),
        ("s_cap", MetaValue::Str("True".into())),
        ("s_zero_lead", MetaValue::Str("01".into())),
        ("s_digit_tail", MetaValue::Str("5x".into())),
        ("s_empty", MetaValue::Str(String::new())),
    ];
    let clock = Arc::new(ManualClock::default());
    let fake = FakeBd::new(Arc::clone(&clock));
    fake.backend().insert(bead("m-1"));
    let seen = fake.clone();
    let cli = BdCli::with_invoker(Box::new(fake)).with_clock(Arc::clone(&clock) as Arc<dyn Clock>);
    let d = Deadline::after(&*clock, Duration::from_secs(30));

    let patch = Patch {
        set_metadata: accepted
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
        ..Patch::default()
    };
    cli.update_guarded(
        &p(),
        &id("m-1"),
        &patch,
        &Guards::default(),
        &actor("san"),
        d,
    )
    .unwrap();

    let row = cli.list_by_ids(&p(), &[id("m-1")], d).unwrap().remove(0);
    for (key, value) in &accepted {
        assert_eq!(
            row.metadata.get(*key),
            Some(&value.to_json()),
            "{key}: {value:?} must come back from bd unchanged"
        );
    }
    let argv = &seen.calls()[0];
    assert!(argv.contains(&"b_true=true".to_owned()), "{argv:?}");
    assert!(argv.contains(&"b_false=false".to_owned()), "{argv:?}");
    assert!(
        argv.iter()
            .all(|w| w != "b_true=True" && w != "b_false=False"),
        "bools are lower case: {argv:?}"
    );

    // What the guard refuses would not survive: bd stores each as something else.
    for refused in ["5", "-3", "5.0", "1e3", "0", "true", "false", "null", " 7"] {
        let patch = Patch {
            set_metadata: [("r".to_owned(), MetaValue::Str(refused.to_owned()))]
                .into_iter()
                .collect(),
            ..Patch::default()
        };
        cli.update_guarded(
            &p(),
            &id("m-1"),
            &patch,
            &Guards::default(),
            &actor("san"),
            d,
        )
        .unwrap();
        let row = cli.list_by_ids(&p(), &[id("m-1")], d).unwrap().remove(0);
        assert_ne!(
            row.metadata.get("r"),
            Some(&serde_json::json!(refused)),
            "{refused:?} would have been stored as the string it was given"
        );
    }
}

// ---- NFR-BDC-0004: error mapping and deadlines --------------------------------

#[test]
fn exit_13_is_a_precondition_refusal() {
    let rig = Rig::new(RecordingInvoker::new(None).reply_bounded(exits(13, "status mismatch")));
    let patch = Patch {
        status: Some("closed".into()),
        ..Patch::default()
    };
    let guards = Guards {
        status: Some("open".into()),
        assignee: None,
    };
    let e = rig
        .cli
        .update_guarded(
            &p(),
            &id("b-1"),
            &patch,
            &guards,
            &actor("san"),
            rig.deadline(),
        )
        .unwrap_err();
    assert!(
        matches!(e, BeadsCacheError::Refused(Refusal::Precondition)),
        "{e:?}"
    );
    assert!(!e.is_unavailable());
}

#[test]
fn a_refused_connection_is_unavailable_and_not_a_timeout() {
    let stderr = "Error: failed to open database: Dolt server unreachable at 127.0.0.1:1: dial tcp 127.0.0.1:1: connect: connection refused\n\nThe Dolt server may not be running.";
    let scope = ScopeFilter::Label("x".into());
    let rig = Rig::new(RecordingInvoker::new(None).reply_bounded(exits(1, stderr)));
    let e = rig
        .cli
        .list_scope(&p(), &scope, None, rig.deadline())
        .unwrap_err();
    assert!(e.is_unavailable() && !e.is_timeout(), "{e:?}");
    let BeadsCacheError::Unavailable { reason, .. } = &e else {
        panic!("{e:?}");
    };
    assert!(
        reason.starts_with("Error: failed to open database"),
        "{reason}"
    );
}

#[test]
fn a_timeout_is_unavailable_with_timed_out_set() {
    // Each phase is a timeout, and the reason names where the call was.
    for phase in [
        TimeoutPhase::Lock,
        TimeoutPhase::Child,
        TimeoutPhase::Output,
    ] {
        let rig = Rig::new(
            RecordingInvoker::new(None).reply_bounded(Err(BoundedError::TimedOut { phase })),
        );
        let e = rig
            .cli
            .list_ready(&p(), &ScopeFilter::Label("x".into()), rig.deadline())
            .unwrap_err();
        assert!(
            matches!(&e, BeadsCacheError::Unavailable { timed_out: true, reason } if reason.contains(phase.as_str())),
            "{phase}: {e:?}"
        );
        assert!(e.is_timeout());
    }
}

#[test]
fn unreadable_output_is_a_transport_error_not_a_contract_or_spawn_failure() {
    let rig = Rig::new(
        RecordingInvoker::new(None).reply_bounded(Err(BoundedError::Output(
            std::io::Error::other("broken pipe"),
        ))),
    );
    let e = rig
        .cli
        .list_ready(&p(), &ScopeFilter::Label("x".into()), rig.deadline())
        .unwrap_err();
    assert!(
        matches!(&e, BeadsCacheError::Transport { what, .. } if what == "bd ready"),
        "{e:?}"
    );
    let io = std::error::Error::source(&e).expect("the read error is the source");
    assert_eq!(io.to_string(), "broken pipe");
    assert!(!e.is_unavailable());
}

/// PE-RBP-F012: output over the cap is `Transport` whose source names the stream and
/// the cap, with its own error kind, not a generic read error.
#[test]
fn oversized_output_is_transport_naming_the_stream_and_the_cap() {
    let rig = Rig::new(RecordingInvoker::new(None).reply_bounded(Err(
        BoundedError::OutputTooLarge {
            stream: OutputStream::Stdout,
            limit: MAX_OUTPUT_BYTES,
        },
    )));
    let e = rig
        .cli
        .list_ready(&p(), &ScopeFilter::Label("x".into()), rig.deadline())
        .unwrap_err();
    let BeadsCacheError::Transport { what, source } = &e else {
        panic!("expected Transport, got {e:?}");
    };
    assert_eq!(what, "bd ready");
    assert_eq!(
        source.kind(),
        std::io::ErrorKind::FileTooLarge,
        "{source:?}"
    );
    let text = source.to_string();
    assert!(text.contains("stdout"), "{text}");
    assert!(text.contains(&MAX_OUTPUT_BYTES.to_string()), "{text}");
    assert!(!e.is_unavailable());
}

/// PE-RBP-F012: a started bd that could not be waited on is `Transport` keeping the
/// OS error: neither a spawn failure nor an outage, since bd may have run.
#[test]
fn a_wait_failure_is_transport_not_a_spawn_failure() {
    let rig = Rig::new(
        RecordingInvoker::new(None).reply_bounded(Err(BoundedError::Wait(
            std::io::Error::from_raw_os_error(10), // ECHILD on Unix
        ))),
    );
    let e = rig
        .cli
        .list_ready(&p(), &ScopeFilter::Label("x".into()), rig.deadline())
        .unwrap_err();
    assert!(
        matches!(&e, BeadsCacheError::Transport { what, .. } if what == "bd ready"),
        "{e:?}"
    );
    assert!(
        !matches!(&e, BeadsCacheError::Btit(BeadsError::Spawn { .. })),
        "{e:?}"
    );
    assert!(!e.is_unavailable(), "{e:?}");
    let io = std::error::Error::source(&e).expect("the OS error is the source");
    assert_eq!(
        io.downcast_ref::<std::io::Error>()
            .and_then(std::io::Error::raw_os_error),
        Some(10)
    );
}

#[test]
fn a_bd_that_cannot_start_keeps_its_cause() {
    let spawn = BeadsError::Spawn {
        binary: "bd".into(),
        operation: Some("list".into()),
        source: std::io::Error::other("no such file"),
    };
    let rig = Rig::new(RecordingInvoker::new(None).reply_bounded(Err(BoundedError::Failed(spawn))));
    let e = rig
        .cli
        .list_ready(&p(), &ScopeFilter::Label("x".into()), rig.deadline())
        .unwrap_err();
    assert!(e.is_unavailable() && !e.is_timeout(), "{e:?}");
    let cause = std::error::Error::source(&e).expect("BeadsError");
    let io = std::error::Error::source(cause).expect("io::Error");
    assert_eq!(io.to_string(), "no such file");
}

#[test]
fn other_failures_are_failed_with_code_and_stderr() {
    let rig = Rig::new(
        RecordingInvoker::new(None).reply_bounded(exits(1, "resolving ID: no issue found")),
    );
    let e = rig
        .cli
        .dep_add_typed(
            &p(),
            &id("a"),
            &id("b"),
            DepType::Blocks,
            &actor("san"),
            rig.deadline(),
        )
        .unwrap_err();
    assert!(
        matches!(&e, BeadsCacheError::Failed { code: 1, stderr } if stderr.contains("no issue found")),
        "{e:?}"
    );
}

/// An expired deadline stops every call before it starts `bd`; the time handed to a
/// running call is what is left of the deadline.
#[test]
fn every_call_stops_at_the_deadline_it_is_handed() {
    let rig = Rig::new(RecordingInvoker::new(None));
    let d = Deadline::after(&*rig.clock, Duration::ZERO);
    let a = actor("san");
    let g = Guards::default();
    let scope = ScopeFilter::Label("x".into());
    let one = id("b-1");
    let results = [
        rig.cli.fingerprint(&p(), d).map(drop),
        rig.cli
            .list_by_ids(&p(), std::slice::from_ref(&one), d)
            .map(drop),
        rig.cli.list_scope(&p(), &scope, None, d).map(drop),
        rig.cli.list_blocked(&p(), &scope, d).map(drop),
        rig.cli.list_ready(&p(), &scope, d).map(drop),
        rig.cli.close_guarded(&p(), &one, None, &g, &a, d),
        rig.cli.update_guarded(
            &p(),
            &one,
            &Patch {
                status: Some("open".into()),
                ..Patch::default()
            },
            &g,
            &a,
            d,
        ),
        rig.cli
            .dep_add_typed(&p(), &one, &one, DepType::Related, &a, d),
    ];
    for (n, r) in results.into_iter().enumerate() {
        assert!(
            matches!(
                r,
                Err(BeadsCacheError::Unavailable {
                    timed_out: true,
                    ..
                })
            ),
            "call {n}: {r:?}"
        );
    }
    assert!(rig.argv().is_empty(), "no bd was started");

    // A live deadline hands the child the time that is left.
    let d = Deadline::after(&*rig.clock, Duration::from_secs(30));
    rig.clock.advance(Duration::from_secs(10));
    rig.cli.list_ready(&p(), &scope, d).unwrap();
    assert_eq!(rig.rec.timeouts(), [Duration::from_secs(20)]);
}

// ---- FakeBd: the same errors through bd's exit statuses ------------------------

#[test]
fn fake_bd_failures_reach_the_caller_as_the_errors_they_stand_for() {
    let clock = Arc::new(ManualClock::default());
    let fake = FakeBd::new(Arc::clone(&clock));
    fake.backend().insert(labelled("f-1", "x"));
    let backend = fake.backend().clone();
    let cli = BdCli::with_invoker(Box::new(fake)).with_clock(Arc::clone(&clock) as Arc<dyn Clock>);
    let d = || Deadline::after(&*clock, Duration::from_secs(30));
    let scope = ScopeFilter::Label("x".into());

    backend.inject(
        FakeOp::ListScope,
        Injection::Unavailable { timed_out: false },
    );
    let e = cli.list_scope(&p(), &scope, None, d()).unwrap_err();
    assert!(e.is_unavailable() && !e.is_timeout(), "{e:?}");

    backend.inject(FakeOp::ListScope, Injection::Delay(Duration::from_secs(60)));
    let e = cli.list_scope(&p(), &scope, None, d()).unwrap_err();
    assert!(e.is_timeout(), "{e:?}");

    backend.inject(FakeOp::Update, Injection::Refused(Refusal::Precondition));
    let patch = Patch {
        status: Some("closed".into()),
        ..Patch::default()
    };
    let e = cli
        .update_guarded(
            &p(),
            &id("f-1"),
            &patch,
            &Guards::default(),
            &actor("san"),
            d(),
        )
        .unwrap_err();
    assert!(
        matches!(e, BeadsCacheError::Refused(Refusal::Precondition)),
        "{e:?}"
    );

    backend.inject(FakeOp::Close, Injection::Failed(7));
    let e = cli
        .close_guarded(
            &p(),
            &id("f-1"),
            None,
            &Guards::default(),
            &actor("san"),
            d(),
        )
        .unwrap_err();
    assert!(
        matches!(e, BeadsCacheError::Failed { code: 7, .. }),
        "{e:?}"
    );
}
