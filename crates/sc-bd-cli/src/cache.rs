//! [`CacheBackend`] for [`BdCli`]: the cache's calls over the `bd` CLI.
//!
//! Every call builds its argv with [`argv`], runs it through
//! [`CliInvoker::run_bounded`](crate::CliInvoker::run_bounded) with the time left before the [`Deadline`] it was
//! handed, and classifies the outcome (ADR-BDCLI-0001):
//!
//! | Outcome | Result |
//! |---|---|
//! | the deadline has already passed, or runs out during the call | `Unavailable { timed_out: true }`, the reason naming the phase (lock, child, output) |
//! | the child's output cannot be read | `Transport`, the read error as its `source` (PE-RBP-F001) |
//! | a stream exceeds `MAX_OUTPUT_BYTES` | `Transport`, its `source` an `io::ErrorKind::FileTooLarge` naming the stream and the cap (PE-RBP-F012) |
//! | `bd` started but could not be polled for its exit or reaped | `Transport`, the OS error as its `source`: bd may have run, so this is not `Unavailable` (PE-RBP-F012) |
//! | a guarded close whose guard check finds no row for the bead | `NotFound` (PE-RBP-F013) |
//! | `bd` cannot be started (or the project is not local) | `Btit(Spawn)` / `Btit(Unsupported)`: [`BeadsCacheError::is_unavailable`] is true for a spawn failure, and the `io::Error` stays in the `source` chain |
//! | exit 13 | `Refused(Refusal::Precondition)` |
//! | any other nonzero exit whose stderr says the server is unreachable (see [`UNREACHABLE_MARKERS`]) | `Unavailable { timed_out: false }` |
//! | stderr names bd 0.49.4's `no such column: spec_id` | `Btit(SchemaMigration)` |
//! | any other nonzero exit | `Failed { code, stderr }`; for `fingerprint`, `CheckFailed { step, .. }` |
//! | output that breaks the strict row contract | `Contract`, from `sc_beads::parse_bd_rows` |
//!
//! No call has a cap of its own: the cache owns the per-call cap and passes the
//! earlier of it and its caller's deadline (NFR-BDC-0004). A call that finds its
//! deadline already passed does not start `bd`.

use std::collections::BTreeSet;
use std::time::Duration;

use sc_beads::error::BeadsError;
use sc_beads::{log_error, log_info};
use sc_beads::{
    parse_bd_rows, parse_blocked_rows, schema_sha256, Actor, Bead, BeadId, BeadsCacheError,
    BlockedRow, CacheBackend, CheckStep, CliClient, CliOutput, Deadline, DepType, Fingerprint,
    Guards, Patch, ProjectRef, Refusal, ScopeFilter, SyncCursor,
};

use crate::argv::{self, IDS_PER_CALL};
use crate::backend::BdCli;
use crate::bounded::{BoundedError, TimeoutPhase};

/// bd's exit status for a refused `--if-status` / `--if-assignee` guard
/// (`bd update --help`: "a mismatch writes nothing and exits 13").
const EXIT_PRECONDITION: i32 = 13;

/// The exit code reported when a failed `bd` was ended by a signal and has none.
const EXIT_SIGNALLED: i32 = -1;

/// Fragments of bd's stderr (lower-case) that mean the database server could not be
/// reached.
///
/// bd exits 1 for every failure, so for "unreachable" there is no typed signal to
/// read; this is the one place the classification reads stderr text. The set is the
/// connection errors of Go's `net` package and bd's own wording, checked against bd
/// 1.3.0 ("Dolt server unreachable at 127.0.0.1:38317: dial tcp ...: connect:
/// connection refused"). The live probe `unreachable_server_is_unavailable` re-checks
/// it on every bd release (REQ-BDC-0905); a wording change shows up there, and the
/// failure then degrades to `Failed`, never to a silent success.
pub const UNREACHABLE_MARKERS: &[&str] = &[
    "connection refused",
    "server unreachable",
    "i/o timeout",
    "no such host",
    "connection reset by peer",
    "network is unreachable",
    "no route to host",
];

/// bd 0.49.4's migration failure on stderr, the same marker btit's `spawn_json` uses.
const SCHEMA_MIGRATION_MARKER: &str = "no such column: spec_id";

/// The timeout error; `when` says where the call was (a [`TimeoutPhase`] phrase, or
/// that the deadline had passed before the call started).
fn timed_out(what: &str, when: &str) -> BeadsCacheError {
    BeadsCacheError::Unavailable {
        reason: format!("{what} did not finish before its deadline ({when})"),
        timed_out: true,
    }
}

/// Maps a finished process to its stdout, or to the error its exit status means.
///
/// `binary` names the program in a schema-migration error.
fn check_exit(binary: &str, out: CliOutput) -> Result<String, BeadsCacheError> {
    if out.success {
        return Ok(out.stdout);
    }
    if out.status == Some(EXIT_PRECONDITION) {
        return Err(BeadsCacheError::Refused(Refusal::Precondition));
    }
    let lower = out.stderr.to_lowercase();
    if UNREACHABLE_MARKERS.iter().any(|m| lower.contains(m)) {
        let reason = out
            .stderr
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .unwrap_or("the database server is unreachable");
        return Err(BeadsCacheError::Unavailable {
            reason: reason.to_owned(),
            timed_out: false,
        });
    }
    if lower.contains(SCHEMA_MIGRATION_MARKER) {
        return Err(BeadsCacheError::Btit(BeadsError::SchemaMigration {
            binary: binary.to_owned(),
        }));
    }
    Err(BeadsCacheError::Failed {
        code: out.status.unwrap_or(EXIT_SIGNALLED),
        stderr: out.stderr,
    })
}

impl BdCli {
    /// The time `d` leaves, or the timeout error when none is left.
    fn time_left(&self, d: Deadline, what: &str) -> Result<Duration, BeadsCacheError> {
        let left = d.remaining(self.clock.as_ref());
        if left.is_zero() {
            Err(timed_out(what, "the deadline had passed before it started"))
        } else {
            Ok(left)
        }
    }

    /// Runs one `bd` command under `d` and returns its stdout.
    fn run(&self, p: &ProjectRef, argv: &[String], d: Deadline) -> Result<String, BeadsCacheError> {
        let what = format!("bd {}", argv.first().map_or("", String::as_str));
        let left = self.time_left(d, &what)?;
        log_info!("[bd] {what} | bounded to {} ms", left.as_millis());
        let out = match self.inv.run_bounded(p, argv, left) {
            Ok(out) => out,
            Err(BoundedError::TimedOut { phase }) => {
                log_error!(
                    "[bd] {what} timed out {phase} after {} ms",
                    left.as_millis()
                );
                return Err(timed_out(&what, phase.as_str()));
            }
            Err(BoundedError::Failed(e)) => return Err(BeadsCacheError::Btit(e)),
            // Output that cannot be read whole is a transport failure, not a
            // row-contract break: nothing was parsed. The read error stays the source.
            Err(BoundedError::Output(source)) => {
                log_error!("[bd] {what}: reading the output failed: {source}");
                return Err(BeadsCacheError::Transport { what, source });
            }
            // Output over the cap is a transport failure too, but not a read error:
            // the source names the stream and the cap, with its own kind
            // (PE-RBP-F012).
            Err(BoundedError::OutputTooLarge { stream, limit }) => {
                log_error!("[bd] {what}: {stream} exceeds the cap of {limit} bytes");
                return Err(BeadsCacheError::Transport {
                    what,
                    source: std::io::Error::new(
                        std::io::ErrorKind::FileTooLarge,
                        format!("{stream} exceeds the cap of {limit} bytes"),
                    ),
                });
            }
            // A child that was started but could not be polled or reaped is a
            // transport failure, not `Unavailable` (PE-RBP-F012). `Unavailable` (and
            // btit's `Spawn`, which `is_unavailable` counts as one) says bd or its
            // server could not be reached, so nothing ran and a retry later is safe.
            // Here bd did start, and may have finished and written: what it did is
            // unknown, as with output that could not be read. So it takes the same
            // path as an unreadable output: `Transport`, mapped by call kind
            // (`BD_WRITE_FAILED` for a write), with the OS error as the source.
            Err(BoundedError::Wait(source)) => {
                log_error!("[bd] {what}: waiting for bd failed: {source}");
                return Err(BeadsCacheError::Transport { what, source });
            }
            // A shutdown before bd started (RSH-001): nothing ran, so it is an
            // outage that a later call may retry, not a timeout.
            Err(
                e @ BoundedError::Interrupted {
                    phase: TimeoutPhase::Lock,
                    ..
                },
            ) => {
                log_error!("[bd] {what}: {e}");
                return Err(BeadsCacheError::Unavailable {
                    reason: format!("{what}: {e}"),
                    timed_out: false,
                });
            }
            // A shutdown while bd ran: it was killed, so what it did is unknown, as
            // with a child that could not be waited for (`Transport`).
            Err(e @ BoundedError::Interrupted { .. }) => {
                log_error!("[bd] {what}: {e}");
                return Err(BeadsCacheError::Transport {
                    what,
                    source: std::io::Error::new(std::io::ErrorKind::Interrupted, e.to_string()),
                });
            }
        };
        check_exit(&self.inv.binary(), out)
    }

    /// Reads the bead's row and refuses unless it still holds the guards; a
    /// bead bd does not return is [`BeadsCacheError::NotFound`].
    ///
    /// `bd close` has no `--if-*` flags (bd 1.3.0), so a guarded close checks the
    /// guards against a fresh row first. Unlike `bd update --if-status` this is two
    /// calls, so another writer can change the bead between them; the cache's drain
    /// already reads the row it writes against, which bounds the window.
    fn check_guards(
        &self,
        p: &ProjectRef,
        id: &BeadId,
        g: &Guards,
        d: Deadline,
    ) -> Result<(), BeadsCacheError> {
        let rows = self.list_by_ids(p, std::slice::from_ref(id), d)?;
        let Some(row) = rows.iter().find(|r| &r.id == id) else {
            return Err(BeadsCacheError::NotFound { bead: id.clone() });
        };
        let status_ok = g
            .status
            .as_deref()
            .is_none_or(|want| row.status.as_deref() == Some(want));
        let assignee_ok = g
            .assignee
            .as_ref()
            .is_none_or(|want| want.matches(row.assignee.as_deref()));
        if status_ok && assignee_ok {
            Ok(())
        } else {
            Err(BeadsCacheError::Refused(Refusal::Precondition))
        }
    }

    /// The version token of `bd version` output (`bd version 1.3.0 (dev: ...)` gives `1.3.0`).
    fn parse_version(stdout: &str) -> Result<String, BeadsCacheError> {
        let mut words = stdout.split_whitespace();
        while let Some(word) = words.next() {
            if word == "version" {
                if let Some(version) = words.next() {
                    return Ok(version.to_owned());
                }
            }
        }
        Err(BeadsCacheError::contract(
            None,
            "version",
            format!(
                "unrecognized `bd version` output: {:?}",
                stdout.lines().next().unwrap_or("")
            ),
        ))
    }

    /// `schema_version` of `bd schema` output, as text.
    fn parse_schema_version(stdout: &str) -> Result<String, BeadsCacheError> {
        let value: serde_json::Value = serde_json::from_str(stdout)
            .map_err(|e| BeadsCacheError::contract(None, "schema", format!("not JSON: {e}")))?;
        match value.get("schema_version") {
            Some(serde_json::Value::Number(n)) => Ok(n.to_string()),
            Some(serde_json::Value::String(s)) => Ok(s.clone()),
            _ => Err(BeadsCacheError::contract(
                None,
                "schema_version",
                "missing or neither a number nor a string",
            )),
        }
    }
}

/// A failed fingerprint step is `CheckFailed` naming it; anything else is unchanged.
fn in_step(step: CheckStep, e: BeadsCacheError) -> BeadsCacheError {
    match e {
        BeadsCacheError::Failed { code, stderr } => {
            BeadsCacheError::CheckFailed { step, code, stderr }
        }
        other => other,
    }
}

/// Rejects a metadata key `--set-metadata key=value` cannot carry.
fn check_metadata_keys(patch: &Patch) -> Result<(), BeadsCacheError> {
    match patch
        .set_metadata
        .keys()
        .find(|k| k.is_empty() || k.contains('='))
    {
        Some(_) => Err(BeadsCacheError::Btit(BeadsError::Unsupported {
            operation: "set_metadata with an empty key or a key containing '='",
            client: CliClient::Bd,
        })),
        None => Ok(()),
    }
}

impl CacheBackend for BdCli {
    fn fingerprint(&self, p: &ProjectRef, d: Deadline) -> Result<Fingerprint, BeadsCacheError> {
        let version = self
            .run(p, &argv::version(), d)
            .map_err(|e| in_step(CheckStep::Version, e))?;
        let bd_version = Self::parse_version(&version)?;
        let schema = self
            .run(p, &argv::schema(), d)
            .map_err(|e| in_step(CheckStep::Schema, e))?;
        Ok(Fingerprint {
            bd_version,
            schema_sha256: schema_sha256(&schema),
            schema_version: Self::parse_schema_version(&schema)?,
        })
    }

    fn list_by_ids(
        &self,
        p: &ProjectRef,
        ids: &[BeadId],
        d: Deadline,
    ) -> Result<Vec<Bead>, BeadsCacheError> {
        // A repeated id would be listed twice in one chunk or across chunks.
        let mut seen = BTreeSet::new();
        let unique: Vec<&BeadId> = ids.iter().filter(|id| seen.insert(*id)).collect();
        let mut rows = Vec::with_capacity(unique.len());
        for chunk in unique.chunks(IDS_PER_CALL) {
            let stdout = self.run(p, &argv::list_by_ids(chunk), d)?;
            rows.extend(parse_bd_rows(&stdout)?);
        }
        Ok(rows)
    }

    fn list_scope(
        &self,
        p: &ProjectRef,
        f: &ScopeFilter,
        since: Option<&SyncCursor>,
        d: Deadline,
    ) -> Result<Vec<Bead>, BeadsCacheError> {
        f.validate().map_err(BeadsCacheError::Invalid)?;
        let stdout = self.run(p, &argv::list_scope(f, since), d)?;
        parse_bd_rows(&stdout)
    }

    fn list_blocked(
        &self,
        p: &ProjectRef,
        f: &ScopeFilter,
        d: Deadline,
    ) -> Result<Vec<BlockedRow>, BeadsCacheError> {
        f.validate().map_err(BeadsCacheError::Invalid)?;
        let stdout = self.run(p, &argv::blocked(f), d)?;
        parse_blocked_rows(&stdout)
    }

    fn list_ready(
        &self,
        p: &ProjectRef,
        f: &ScopeFilter,
        d: Deadline,
    ) -> Result<Vec<BeadId>, BeadsCacheError> {
        f.validate().map_err(BeadsCacheError::Invalid)?;
        let stdout = self.run(p, &argv::ready(f), d)?;
        Ok(parse_bd_rows(&stdout)?.into_iter().map(|b| b.id).collect())
    }

    fn close_guarded(
        &self,
        p: &ProjectRef,
        id: &BeadId,
        reason: Option<&str>,
        g: &Guards,
        a: &Actor,
        d: Deadline,
    ) -> Result<(), BeadsCacheError> {
        if !g.is_empty() {
            self.check_guards(p, id, g, d)?;
        }
        self.run(p, &argv::close(id, reason, a), d).map(drop)
    }

    fn update_guarded(
        &self,
        p: &ProjectRef,
        id: &BeadId,
        patch: &Patch,
        g: &Guards,
        a: &Actor,
        d: Deadline,
    ) -> Result<(), BeadsCacheError> {
        // `bd update` with no field prints "No updates specified" and exits 0 without
        // looking at the guards, so there is nothing to ask it.
        if patch.is_empty() {
            self.time_left(d, "bd update")?;
            return Ok(());
        }
        check_metadata_keys(patch)?;
        self.run(p, &argv::update(id, patch, g, a), d).map(drop)
    }

    fn dep_add_typed(
        &self,
        p: &ProjectRef,
        id: &BeadId,
        on: &BeadId,
        kind: DepType,
        a: &Actor,
        d: Deadline,
    ) -> Result<(), BeadsCacheError> {
        self.run(p, &argv::dep_add(id, on, kind, a), d).map(drop)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn out(status: Option<i32>, stderr: &str) -> CliOutput {
        CliOutput {
            status,
            success: status == Some(0),
            stdout: "out".into(),
            stderr: stderr.into(),
        }
    }

    fn scripted(reply: Result<CliOutput, BoundedError>) -> BdCli {
        BdCli::with_invoker(Box::new(
            crate::testing::RecordingInvoker::new(None).reply_bounded(reply),
        ))
    }

    fn deadline() -> Deadline {
        Deadline::after(&sc_beads::SystemClock, Duration::from_secs(30))
    }

    fn bead_id(s: &str) -> BeadId {
        BeadId::new(s).unwrap_or_else(|e| panic!("{e}"))
    }

    /// PE-RBP-F001: unreadable output is a transport failure that keeps the read
    /// error, never a row-contract break.
    #[test]
    fn unreadable_output_is_transport_with_the_read_error_as_source() {
        let cli = scripted(Err(BoundedError::Output(std::io::Error::other(
            "the pipe broke",
        ))));
        let e = cli
            .list_by_ids(&ProjectRef::local(None), &[bead_id("b-1")], deadline())
            .unwrap_err();
        assert!(
            matches!(&e, BeadsCacheError::Transport { what, .. } if what == "bd list"),
            "{e:?}"
        );
        let io = std::error::Error::source(&e).expect("the read error is kept");
        assert_eq!(io.to_string(), "the pipe broke");
        assert!(!e.is_unavailable());
    }

    /// PE-RBP-F013: a guarded close whose guard check finds no row is a typed
    /// not-found, and `bd close` never runs.
    #[test]
    fn a_guarded_close_of_a_missing_bead_is_not_found() {
        let rec = std::sync::Arc::new(crate::testing::RecordingInvoker::new(None).reply_bounded(
            Ok(CliOutput {
                status: Some(0),
                success: true,
                stdout: "[]".into(),
                stderr: String::new(),
            }),
        ));
        let cli = BdCli::with_invoker(Box::new(SharedInvoker(std::sync::Arc::clone(&rec))));
        let guards = Guards {
            status: Some("open".into()),
            assignee: None,
        };
        let actor = Actor::new("san").unwrap_or_else(|e| panic!("{e}"));
        let e = cli
            .close_guarded(
                &ProjectRef::local(None),
                &bead_id("gone-1"),
                None,
                &guards,
                &actor,
                deadline(),
            )
            .unwrap_err();
        assert!(
            matches!(&e, BeadsCacheError::NotFound { bead } if bead.as_str() == "gone-1"),
            "{e:?}"
        );
        let calls = rec.calls();
        assert_eq!(calls.len(), 1, "only the guard check ran: {calls:?}");
        assert_eq!(calls[0].first().map(String::as_str), Some("list"));
    }

    /// Lets a test keep reading the calls of the invoker it hands to `BdCli`.
    struct SharedInvoker(std::sync::Arc<crate::testing::RecordingInvoker>);

    impl crate::CliInvoker for SharedInvoker {
        fn binary(&self) -> String {
            self.0.binary()
        }
        fn client_info(&self) -> Option<sc_beads::CliProbe> {
            self.0.client_info()
        }
        fn probe(&self) -> Option<sc_beads::CliProbe> {
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

    #[test]
    fn exit_zero_is_the_stdout() {
        assert_eq!(check_exit("bd", out(Some(0), "warn")).unwrap(), "out");
    }

    #[test]
    fn exit_13_is_a_precondition_refusal_whatever_stderr_says() {
        for stderr in ["", "status mismatch", "connection refused"] {
            assert!(matches!(
                check_exit("bd", out(Some(13), stderr)),
                Err(BeadsCacheError::Refused(Refusal::Precondition))
            ));
        }
    }

    #[test]
    fn unreachable_server_is_unavailable_not_timed_out() {
        let e = check_exit(
            "bd",
            out(
                Some(1),
                "Error: failed to open database: Dolt server unreachable at 127.0.0.1:1: dial tcp 127.0.0.1:1: connect: connection refused\n\nTry: bd dolt start",
            ),
        )
        .unwrap_err();
        assert!(e.is_unavailable() && !e.is_timeout(), "{e:?}");
        assert!(e.to_string().contains("unreachable"), "{e}");
    }

    #[test]
    fn other_failures_keep_code_and_stderr() {
        assert!(matches!(
            check_exit("bd", out(Some(1), "no issue found")),
            Err(BeadsCacheError::Failed { code: 1, ref stderr }) if stderr == "no issue found"
        ));
        assert!(matches!(
            check_exit("bd", out(None, "killed")),
            Err(BeadsCacheError::Failed { code: -1, .. })
        ));
    }

    #[test]
    fn schema_migration_failure_is_typed() {
        assert!(matches!(
            check_exit(
                "bd",
                out(Some(1), "SQL logic error: no such column: spec_id")
            ),
            Err(BeadsCacheError::Btit(BeadsError::SchemaMigration { .. }))
        ));
    }

    #[test]
    fn version_is_the_token_after_version() {
        assert_eq!(
            BdCli::parse_version("bd version 1.3.0 (dev: 3c4dd8cecb5d)\n").unwrap(),
            "1.3.0"
        );
        assert!(matches!(
            BdCli::parse_version("weird"),
            Err(BeadsCacheError::Contract { .. })
        ));
        assert!(BdCli::parse_version("bd version").is_err());
    }

    #[test]
    fn schema_version_accepts_numbers_and_strings_only() {
        assert_eq!(
            BdCli::parse_schema_version(r#"{"schema_version": 1}"#).unwrap(),
            "1"
        );
        assert_eq!(
            BdCli::parse_schema_version(r#"{"schema_version": "2"}"#).unwrap(),
            "2"
        );
        for bad in ["{}", r#"{"schema_version": null}"#, "nope"] {
            assert!(
                matches!(
                    BdCli::parse_schema_version(bad),
                    Err(BeadsCacheError::Contract { .. })
                ),
                "{bad}"
            );
        }
    }
}
