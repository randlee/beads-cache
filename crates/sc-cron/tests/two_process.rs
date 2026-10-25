//! Single flight across processes: generation fencing (REQ-CRON-0006,
//! REQ-CRON-0007) with two real processes on one database file.
//!
//! The test binary runs itself as the peer (`--two-process-peer <db>`), driven
//! over stdin and stdout one line at a time, so every step waits for the
//! previous one and nothing depends on timing. Each process holds its own
//! connection to the file, as two `sc-cron` users would.
//!
//! The guarantee under test is the store's, not a lock's: only one process can
//! hold a name active, and a process that kept an older generation changes
//! nothing when it writes, whichever process restarted the schedule.
//!
//! This file has its own `main` (`harness = false` in `Cargo.toml`) because
//! the peer is this very binary.

#![expect(
    clippy::expect_used,
    clippy::panic,
    clippy::clone_on_ref_ptr,
    clippy::print_stdout,
    reason = "test code: a failed expectation is the test failure"
)]

use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use sc_cron::{
    Generation, ManualClock, RunOutcome, ScheduleId, ScheduleSpec, ScheduleStore, Scheduler,
    SeqJitter, SqliteScheduleStore, Timestamp,
};

const PEER_FLAG: &str = "--two-process-peer";

type Shared = Scheduler<SqliteScheduleStore, ManualClock, SeqJitter>;

fn scheduler(path: &Path) -> Shared {
    let clock = Arc::new(ManualClock::new(
        "2026-10-25T00:00:00Z".parse().expect("timestamp"),
    ));
    Scheduler::new(
        Arc::new(SqliteScheduleStore::open(path).expect("open")),
        clock,
        Arc::new(SeqJitter::default()),
    )
}

fn spec() -> ScheduleSpec {
    ScheduleSpec::new(
        "shared",
        "0 * * * * *",
        Duration::from_secs(20),
        serde_json::json!({}),
    )
    .expect("spec")
}

fn stamp() -> Timestamp {
    "2026-10-25T00:00:30Z".parse().expect("timestamp")
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let [_, flag, path] = args.as_slice()
        && flag == PEER_FLAG
    {
        peer(Path::new(path));
    } else {
        parent();
    }
}

/// The peer: runs the commands the parent sends, one reply line each.
fn peer(path: &Path) {
    let scheduler = scheduler(path);
    let store = scheduler.store().clone();
    let target = |words: &[&str]| -> (ScheduleId, Generation) {
        let id = words.get(1).and_then(|w| w.parse().ok()).expect("id");
        let generation = words
            .get(2)
            .and_then(|w| w.parse().ok())
            .expect("generation");
        (
            ScheduleId::new(id),
            Generation::new(generation).expect("generation"),
        )
    };
    for line in std::io::stdin().lock().lines() {
        let line = line.expect("command");
        let words: Vec<&str> = line.split_whitespace().collect();
        let reply = match words.first().copied() {
            Some("start") => match scheduler.start(spec()) {
                Ok(row) => format!("started {} {}", row.id, row.generation),
                Err(error) => format!("error {}", error.code()),
            },
            Some("record") => {
                let (id, generation) = target(&words);
                let changed = store.record_run(id, generation, stamp(), &RunOutcome::Ok);
                format!("changed {}", changed.expect("record"))
            }
            Some("set_next") => {
                let (id, generation) = target(&words);
                let changed = store.set_next(id, generation, stamp());
                format!("changed {}", changed.expect("set_next"))
            }
            Some("stop") => {
                let (id, generation) = target(&words);
                let changed = store.stop(id, generation, stamp(), "peer");
                format!("changed {}", changed.expect("stop"))
            }
            Some("quit") => break,
            other => panic!("unknown command {other:?}"),
        };
        println!("{reply}");
    }
}

/// Kills the peer if the test fails before it quits.
struct Peer {
    child: Child,
    to: ChildStdin,
    from: BufReader<ChildStdout>,
}

impl Peer {
    fn spawn(path: &Path) -> Self {
        let mut child = Command::new(std::env::current_exe().expect("current exe"))
            .arg(PEER_FLAG)
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn peer");
        let to = child.stdin.take().expect("peer stdin");
        let from = BufReader::new(child.stdout.take().expect("peer stdout"));
        Self { child, to, from }
    }

    fn ask(&mut self, command: &str) -> String {
        writeln!(self.to, "{command}").expect("send");
        self.to.flush().expect("flush");
        let mut reply = String::new();
        self.from.read_line(&mut reply).expect("reply");
        reply.trim().to_owned()
    }

    fn finish(mut self) {
        writeln!(self.to, "quit").expect("quit");
        let status = self.child.wait().expect("peer exit");
        assert!(status.success(), "peer exited with {status}");
    }
}

impl Drop for Peer {
    fn drop(&mut self) {
        // After a clean `finish` this finds the peer gone.
        let _ = self.child.kill();
    }
}

fn parent() {
    let root = tempfile::tempdir().expect("tempdir");
    let path = root.path().join("cron.db");
    let mine = scheduler(&path);
    let store = mine.store().clone();
    let mut peer = Peer::spawn(&path);

    // Process A starts the schedule: generation 1.
    let first = mine.start(spec()).expect("start");
    assert_eq!(first.generation.get(), 1);

    // Process B cannot start the name while A holds it active.
    assert_eq!(peer.ask("start"), "error ALREADY_ACTIVE");

    // A ends it and B restarts it: same id, generation 2.
    mine.stop(first.id, first.generation, "handover")
        .expect("stop");
    assert_eq!(
        peer.ask("start"),
        format!("started {} 2", first.id),
        "B restarts the ended name in place"
    );

    // A, still holding generation 1, changes nothing, and A cannot start
    // the name either: B has it active.
    let stale = first.generation;
    assert!(
        !store
            .record_run(first.id, stale, stamp(), &RunOutcome::Ok)
            .expect("record")
    );
    assert!(!store.set_next(first.id, stale, stamp()).expect("set_next"));
    assert!(!store.stop(first.id, stale, stamp(), "stale").expect("stop"));
    assert!(matches!(
        mine.start(spec()),
        Err(sc_cron::CronError::AlreadyActive(_))
    ));
    let row = store.get(first.id).expect("get").expect("row");
    assert!(row.is_active());
    assert_eq!(row.generation.get(), 2);
    assert_eq!(row.last_outcome, None, "A's stale run recorded nothing");
    assert_eq!(row.next_run_at, Some(first.next_run_at.expect("first run")));

    // The other way round: B ends generation 2, A restarts it as generation 3,
    // and B's generation-2 writes change nothing.
    assert_eq!(peer.ask(&format!("stop {} 2", first.id)), "changed true");
    let third = mine.start(spec()).expect("restart");
    assert_eq!((third.id, third.generation.get()), (first.id, 3));
    for command in ["record", "set_next", "stop"] {
        assert_eq!(
            peer.ask(&format!("{command} {} 2", first.id)),
            "changed false",
            "B's stale {command}"
        );
    }
    let row = store.get(first.id).expect("get").expect("row");
    assert_eq!(row, third, "generation 3 is exactly as A started it");

    // The current generation does write.
    assert_eq!(peer.ask(&format!("record {} 3", first.id)), "changed true");
    let row = store.get(first.id).expect("get").expect("row");
    assert_eq!(row.last_outcome, Some(RunOutcome::Ok));

    peer.finish();
    println!("test two_process::generation_fencing_holds_across_two_processes ... ok");
}
