//! Sprint b-3, the real-process cases: exit codes and one envelope on stdout
//! (acceptance 2, REQ-BDC-0802), configuration (acceptance 3, REQ-BDC-0810),
//! `--help` (acceptance 4, contract §8), `--format text`, `--input`, the stderr
//! log, a caught panic, and the two binaries (REQ-BDC-0805).
//!
//! Every test runs `bdc` as a process against `fake-bd` (contract §8, "Real
//! process").

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod support;

use std::path::Path;
use std::process::Stdio;

use serde_json::Value;
#[cfg(unix)]
use support::write_script;
use support::{
    BD_CACHE, BDC, FAKE_BD, Ran, TestRepo, copy_executable, envelope_json, hermetic, metadata,
    parse_envelope, path_str, two_beads,
};

/// A repository whose scope `phase-x` has been synced once.
fn synced() -> TestRepo {
    let repo = TestRepo::new(&two_beads());
    let (exit, v) = repo.json(&["cache", "refresh", "scopes", "x"]);
    assert_eq!((exit, &v["data"]["failed"]), (0, &Value::from(0)), "{v}");
    repo
}

fn code(v: &Value) -> &str {
    v["error"]["code"].as_str().unwrap_or("")
}

// ---- acceptance 2: exit codes, one JSON document --------------------------------

/// Exit 0 is `ok: true`, and stdout is exactly one envelope.
#[test]
fn exit_0_is_ok_true() {
    let repo = synced();
    for args in [
        &["issues", "get", "b-1"][..],
        &["issues", "list", "--scope", "x"],
        &["cache", "status"],
        &["--version"],
    ] {
        let (exit, v) = repo.json(args);
        assert_eq!(exit, 0, "{args:?}: {v}");
        assert_eq!(v["ok"], true, "{args:?}");
        assert!(v["error"].is_null());
    }
}

/// Exit 3 is any typed error other than `USAGE`, with the code the decoder or
/// the cache decides.
#[test]
fn exit_3_is_any_other_typed_error() {
    let repo = synced();
    for (args, want) in [
        (
            &["issues", "list", "--scope", "zz"][..],
            "BD_CACHE.SCOPE_UNKNOWN",
        ),
        (
            &["issues", "get", "zz", "--miss-timeout-ms", "0"],
            "BD_CACHE.NOT_CACHED",
        ),
        (
            &["issues", "create", "--title", "t"],
            "BD_CACHE.UNSUPPORTED",
        ),
        (&["blocked", "-l", "phase-x"], "BD_CACHE.UNSUPPORTED"),
        (
            &["issues", "list", "--scope", "x", "--limit", "many"],
            "BD_CACHE.BAD_ARGS",
        ),
        (
            &[
                "issues",
                "update",
                "b-1",
                "--actor",
                "a",
                "--idempotency-key",
                "k",
                "--patch",
                "{",
            ],
            "BD_CACHE.BAD_ARGS",
        ),
        (
            &[
                "issues",
                "update",
                "b-1",
                "--actor",
                "a",
                "--idempotency-key",
                "k",
                "--status",
                "closed",
            ],
            "BD_CACHE.BAD_ARGS",
        ),
        (
            &[
                "cache", "journal", "resolve", "9", "--note", "n", "--actor", "a",
            ],
            "BD_CACHE.NOT_FOUND",
        ),
    ] {
        let (exit, v) = repo.json(args);
        assert_eq!(exit, 3, "{args:?}: {v}");
        assert_eq!(code(&v), want, "{args:?}: {v}");
        assert_eq!(v["ok"], false);
        assert!(v["data"].is_null());
    }
    let (_, v) = repo.json(&["issues", "create"]);
    assert_eq!(v["error"]["details"]["unsupported"], "operation");
    assert_eq!(v["error"]["details"]["name"], "createIssue");
}

/// Exit 2 is `USAGE`, for the shape of argv only: an unknown flag, an
/// abbreviated option, `--input` mixed with flags, a bare `cache refresh`, an
/// unknown command, a missing argument.
#[test]
fn exit_2_is_usage_for_the_shape_of_argv_only() {
    let repo = synced();
    let input = repo.root().join("request.json");
    std::fs::write(&input, r#"{"scope": "x"}"#).unwrap();
    let input = path_str(&input).to_owned();
    let cases: Vec<(Vec<&str>, Option<&str>)> = vec![
        (vec!["issues", "list", "--bogus"], None),
        (vec!["issues", "list", "--scop", "x"], Some("--scope")),
        (
            vec!["issues", "get", "b-1", "--miss-timeout", "0"],
            Some("--miss-timeout-ms"),
        ),
        (
            vec!["issues", "list", "--input", &input, "--scope", "x"],
            None,
        ),
        (vec!["cache", "refresh"], None),
        (vec!["frobnicate"], None),
        (vec!["isues", "list"], Some("issues")),
        (vec!["issues", "get"], None),
        (
            vec!["cache", "journal", "get", "1", "--idempotency-key", "k"],
            None,
        ),
        (vec!["--forma", "text", "context"], Some("--format")),
    ];
    for (args, did_you_mean) in cases {
        let (exit, v) = repo.json(&args);
        assert_eq!(exit, 2, "{args:?}: {v}");
        assert_eq!(code(&v), "BD_CACHE.USAGE", "{args:?}");
        assert_eq!(v["error"]["kind"], "invalid_input");
        let details = &v["error"]["details"];
        assert!(details["valid"].is_array(), "{args:?}: {v}");
        assert!(details.get("did_you_mean").is_some(), "{args:?}: {v}");
        if let Some(want) = did_you_mean {
            assert_eq!(details["did_you_mean"], want, "{args:?}: {v}");
        }
        assert_eq!(
            v["error"]["suggested_action"], "run `bdc <cmd> --help`",
            "{args:?}"
        );
    }
    let (_, v) = repo.json(&["cache", "refresh"]);
    assert_eq!(
        v["error"]["details"]["valid"],
        serde_json::json!(["scopes", "issues"])
    );
}

/// `--limit +5` is refused exactly as `--input {"limit": "+5"}` is: the CLI
/// hands the decoder the same value (QA minor on 254317c).
#[test]
fn a_signed_integer_flag_is_refused_as_through_input() {
    let repo = synced();
    let dir = tempfile::TempDir::new().unwrap();
    let file = dir.path().join("req.json");
    std::fs::write(&file, r#"{"scope": "x", "limit": "+5"}"#).unwrap();
    let (flag_exit, flag) = repo.json(&["issues", "list", "--scope", "x", "--limit", "+5"]);
    let (input_exit, input) = repo.json(&["issues", "list", "--input", path_str(&file)]);
    assert_eq!((flag_exit, code(&flag)), (3, "BD_CACHE.BAD_ARGS"), "{flag}");
    assert_eq!(
        (input_exit, code(&input)),
        (flag_exit, code(&flag)),
        "{input}"
    );
    assert_eq!(flag["error"]["details"], input["error"]["details"]);
}

/// `USAGE` is JSON even with `--format text` (contract §8).
#[test]
fn usage_is_json_even_with_format_text() {
    let repo = synced();
    let ran = repo.bdc(&["--format", "text", "issues", "list", "--bogus"]);
    assert_eq!(ran.exit, 2);
    let (_, v) = envelope_json(&ran);
    assert_eq!(code(&v), "BD_CACHE.USAGE");
}

/// The planted negatives of acceptance 2, on the checks themselves: an error
/// printed on stderr only leaves stdout unparseable, and the exit check
/// tells exit 1 from exit 2.
#[test]
fn an_error_on_stderr_only_or_a_wrong_exit_fails_the_checks() {
    let stderr_only = Ran {
        exit: 2,
        stdout: String::new(),
        stderr: "error: unexpected argument '--bogus'".to_owned(),
    };
    assert!(parse_envelope(&stderr_only.stdout).is_err());
    let real = synced().bdc(&["issues", "list", "--bogus"]);
    assert_ne!(real.exit, 1, "USAGE is exit 2, never clap's default");
    assert!(parse_envelope(&real.stdout).is_ok());
}

/// `--input FILE|-` gives the request as is.
#[test]
fn input_gives_the_whole_request_from_a_file_or_stdin() {
    let repo = synced();
    let file = repo.root().join("get.json");
    std::fs::write(&file, r#"{"id": "b-2"}"#).unwrap();
    let (exit, v) = repo.json(&["issues", "get", "--input", path_str(&file)]);
    assert_eq!(exit, 0, "{v}");
    assert_eq!(v["data"]["id"], "b-2");

    let root = repo.root();
    let mut child = hermetic(BDC)
        .args([
            "--repo",
            path_str(&root),
            "--bd",
            FAKE_BD,
            "issues",
            "list",
            "--input",
            "-",
        ])
        .env(bd_cache_cli::scenario::SCENARIO_ENV, repo.scenario_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    {
        use std::io::Write as _;
        let mut stdin = child.stdin.take().unwrap();
        stdin.write_all(br#"{"scope": "x", "limit": 1}"#).unwrap();
    }
    let ran: Ran = child.wait_with_output().unwrap().into();
    let (exit, v) = envelope_json(&ran);
    assert_eq!(exit, 0, "{v}");
    assert_eq!(v["data"]["items"].as_array().map(Vec::len), Some(1));
    assert_eq!(v["data"]["has_more"], true);

    // Invalid JSON in --input is BAD_ARGS, exit 3.
    std::fs::write(&file, "{").unwrap();
    let (exit, v) = repo.json(&["issues", "get", "--input", path_str(&file)]);
    assert_eq!((exit, code(&v)), (3, "BD_CACHE.BAD_ARGS"), "{v}");
}

/// A caught panic is `BD_CACHE.INTERNAL`, exit 3, still one envelope.
#[test]
fn a_caught_panic_is_internal_with_exit_3() {
    let repo = synced();
    let root = repo.root();
    let ran = repo.bdc_raw(
        &root,
        &["--repo", path_str(&root), "--bd", FAKE_BD, "context"],
        &[(bd_cache_cli::PANIC_ENV, "1")],
    );
    let (exit, v) = envelope_json(&ran);
    assert_eq!(exit, 3, "{v}");
    assert_eq!(code(&v), "BD_CACHE.INTERNAL");
    assert_eq!(v["error"]["kind"], "internal");
    assert!(
        ran.stderr.contains("panicked"),
        "the panic is reported on stderr"
    );
}

// ---- --format text, logs, both binaries -----------------------------------------

/// `--format text` renders the same data for a person, with the same exits.
#[test]
fn format_text_renders_the_data_for_a_person() {
    let repo = synced();
    let ran = repo.bdc(&["context", "--format", "text"]);
    assert_eq!(ran.exit, 0, "{ran:?}");
    assert!(
        parse_envelope(&ran.stdout).is_err(),
        "not JSON: {}",
        ran.stdout
    );
    assert!(ran.stdout.contains("database: beads"), "{}", ran.stdout);
    assert!(
        ran.stdout.contains("  contract_version: 1.0.0"),
        "{}",
        ran.stdout
    );
    let ran = repo.bdc(&[
        "--format",
        "text",
        "issues",
        "get",
        "zz",
        "--miss-timeout-ms",
        "0",
    ]);
    assert_eq!(ran.exit, 3);
    assert!(
        ran.stdout.starts_with("error: BD_CACHE.NOT_CACHED: "),
        "{}",
        ran.stdout
    );
    let ran = repo.bdc(&["--version", "--format", "text"]);
    assert_eq!(
        ran.stdout,
        format!("bd-cache {} (contract 1.0.0)\n", env!("CARGO_PKG_VERSION"))
    );
}

/// The log goes to stderr at `-v` / `-q` / `BD_CACHE__LOG`; the flag wins over
/// the env; stdout never changes.
#[test]
fn logs_go_to_stderr_and_never_change_stdout() {
    let repo = synced();
    let root = repo.root();
    let base = ["--repo", path_str(&root), "--bd", FAKE_BD, "context"];
    let with = |extra: &[&str], envs: &[(&str, &str)]| {
        let mut args: Vec<&str> = base.to_vec();
        args.extend_from_slice(extra);
        repo.bdc_raw(&root, &args, envs)
    };
    let quiet = with(&[], &[]);
    let verbose = with(&["-v"], &[]);
    let by_env = with(&[], &[("BD_CACHE__LOG", "info")]);
    let flag_wins = with(&["-q"], &[("BD_CACHE__LOG", "info")]);
    assert_eq!(quiet.stderr, "", "warn by default, and nothing warns");
    assert!(verbose.stderr.contains("bdc: INFO"), "{}", verbose.stderr);
    assert!(by_env.stderr.contains("bdc: INFO"), "{}", by_env.stderr);
    assert_eq!(flag_wins.stderr, "", "-q wins over BD_CACHE__LOG");
    for r in [&verbose, &by_env, &flag_wins] {
        assert_eq!(r.stdout, quiet.stdout);
        assert_eq!(r.exit, 0);
    }
    // An env key bdc does not read is reported, not silently taken.
    let unknown = with(&[], &[("BD_CACHE__FORMAT", "text")]);
    assert!(
        unknown.stderr.contains("BD_CACHE__FORMAT"),
        "{}",
        unknown.stderr
    );
    assert_eq!(
        unknown.stdout, quiet.stdout,
        "--format is read from the flag only"
    );
}

/// `bd-cache` and `bdc` give identical output for a real command (REQ-BDC-0805).
#[test]
fn both_binaries_give_identical_output_for_a_command() {
    let repo = synced();
    let root = repo.root();
    let args = [
        "--repo",
        path_str(&root),
        "--bd",
        FAKE_BD,
        "issues",
        "get",
        "b-1",
    ];
    let run = |bin: &str| -> Ran {
        hermetic(bin)
            .args(args)
            .env(bd_cache_cli::scenario::SCENARIO_ENV, repo.scenario_path())
            .output()
            .unwrap()
            .into()
    };
    let (a, b) = (run(BDC), run(BD_CACHE));
    assert_eq!(a, b);
    assert_eq!(a.exit, 0, "{a:?}");
}

// ---- acceptance 4: --help -------------------------------------------------------

/// Piped, `--help` is the envelope with `{command, synopsis, arguments}`; exit 0.
#[test]
fn help_when_piped_is_the_envelope() {
    let repo = synced();
    for (args, command) in [
        (&["issues", "get", "--help"][..], "issues get"),
        (
            &["--help", "cache", "journal", "resolve"],
            "cache journal resolve",
        ),
        (&["cache", "--help"], "cache"),
        (&["--help"], ""),
    ] {
        let (exit, v) = repo.json(args);
        assert_eq!(exit, 0, "{args:?}: {v}");
        let data = &v["data"];
        assert_eq!(data["command"], command);
        assert!(data["synopsis"].as_str().unwrap().starts_with("bdc"));
        let first = &data["arguments"][0];
        for key in ["name", "flag", "required", "description"] {
            assert!(first.get(key).is_some(), "{args:?}: {key} in {first}");
        }
    }
    let (_, v) = repo.json(&["issues", "get", "--help"]);
    assert_eq!(v["data"]["arguments"][0]["name"], "id");
    assert_eq!(v["data"]["arguments"][0]["required"], true);
    // An unknown command under --help is USAGE, never help.
    let (exit, v) = repo.json(&["frobnicate", "--help"]);
    assert_eq!((exit, code(&v)), (2, "BD_CACHE.USAGE"), "{v}");
}

/// Under a pseudo-terminal, `--help` is text; with `--format json` it is the
/// envelope; both exit 0.
#[cfg(unix)]
#[test]
fn help_on_a_terminal_is_text() {
    let text = under_pty(&[BDC, "issues", "get", "--help"]);
    assert_eq!(text.exit, 0, "{text:?}");
    let out = text.stdout.replace("\r\n", "\n");
    assert!(out.contains("Usage: bdc issues get <id>"), "{out:?}");
    assert!(!out.trim_start().starts_with('{'), "{out:?}");
    let json = under_pty(&[BDC, "issues", "get", "--help", "--format", "json"]);
    assert_eq!(json.exit, 0, "{json:?}");
    let out = json.stdout.replace("\r\n", "\n");
    // script(1) may echo control characters before the child's output.
    let line = out
        .find('{')
        .map_or("", |i| out[i..].lines().next().unwrap_or(""));
    let v: Value = serde_json::from_str(line).unwrap_or_else(|e| panic!("{e}: {out:?}"));
    assert_eq!(v["data"]["command"], "issues get");
}

/// The planted negative of acceptance 4 on the check: text where the
/// envelope is due fails the envelope parse.
#[test]
fn text_help_where_the_envelope_is_due_fails_the_parse() {
    let text = bd_cache_cli::run_with(
        &["issues".to_owned(), "get".to_owned(), "--help".to_owned()],
        &std::collections::BTreeMap::new(),
        None,
        std::sync::Arc::new(sc_beads::SystemClock),
        bd_cache_cli::Stdout::Terminal,
    );
    assert_eq!(text.0, 0);
    assert!(parse_envelope(&text.1).is_err(), "{}", text.1);
}

/// Runs `argv` with its stdout on a pseudo-terminal, through `script(1)`.
#[cfg(unix)]
fn under_pty(argv: &[&str]) -> Ran {
    let quoted: Vec<String> = argv.iter().map(|a| format!("'{a}'")).collect();
    let mut cmd = hermetic("script");
    if cfg!(target_os = "macos") {
        cmd.arg("-q").arg("/dev/null").args(argv);
    } else {
        cmd.args(["-q", "-e", "-c", &quoted.join(" "), "/dev/null"]);
    }
    cmd.stdin(Stdio::null())
        .output()
        .expect("script(1) runs")
        .into()
}

// ---- acceptance 3: configuration ------------------------------------------------

fn write_toml(repo: &TestRepo, text: &str) {
    std::fs::write(repo.root().join(".beads").join("bd-cache.toml"), text).unwrap();
}

/// The cache file `context` reports.
fn cache_path_of(repo: &TestRepo, args: &[&str], envs: &[(&str, &str)]) -> String {
    let root = repo.root();
    let mut full = vec!["--repo", path_str(&root), "--bd", FAKE_BD];
    full.extend_from_slice(args);
    full.push("context");
    let (exit, v) = envelope_json(&repo.bdc_raw(&root, &full, envs));
    assert_eq!(exit, 0, "{v}");
    v["data"]["x_cache"]["cache_path"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// One key set in TOML, env and flag: the flag wins, then the env, then the
/// TOML, then the default. A second key (`scopes`) follows the same rule.
#[test]
fn flag_beats_env_beats_toml_beats_default() {
    let repo = TestRepo::new(&two_beads());
    let env = [("BD_CACHE__CACHE_PATH", "env.sqlite3")];
    let flag = ["--cache-path", "flag.sqlite3"];
    let file = |p: &str| {
        Path::new(p)
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned()
    };

    assert_eq!(
        file(&cache_path_of(&repo, &[], &[])),
        "beads.bd-cache.sqlite3"
    );
    write_toml(&repo, "scopes = [\"x\"]\ncache_path = \"toml.sqlite3\"\n");
    assert_eq!(file(&cache_path_of(&repo, &[], &[])), "toml.sqlite3");
    assert_eq!(file(&cache_path_of(&repo, &[], &env)), "env.sqlite3");
    assert_eq!(file(&cache_path_of(&repo, &flag, &env)), "flag.sqlite3");
    assert_eq!(file(&cache_path_of(&repo, &flag, &[])), "flag.sqlite3");
    // A relative cache_path is resolved against --repo.
    let path = cache_path_of(&repo, &[], &env);
    assert_eq!(Path::new(&path), repo.root().join("env.sqlite3"));

    // scopes: TOML [x], env "y" -> refreshing every known scope of a cache file
    // never opened before syncs y only (the env list names phase-a scopes).
    let repo = TestRepo::new(&two_beads());
    let root = repo.root();
    let args = [
        "--repo",
        path_str(&root),
        "--bd",
        FAKE_BD,
        "cache",
        "refresh",
        "scopes",
    ];
    let (_, v) = envelope_json(&repo.bdc_raw(&root, &args, &[("BD_CACHE__SCOPES", "y")]));
    assert_eq!(v["data"]["results"][0]["scope"], "y", "{v}");
    assert_eq!(v["data"]["results"].as_array().map(Vec::len), Some(1));
    // An env value that does not parse is CONFIG_INVALID, exit 3.
    let (exit, v) = envelope_json(&repo.bdc_raw(
        &root,
        &["--repo", path_str(&root), "context"],
        &[("BD_CACHE__CALL_TIMEOUT_S", "soon")],
    ));
    assert_eq!((exit, code(&v)), (3, "BD_CACHE.CONFIG_INVALID"), "{v}");
}

/// Writes an executable shell script at `path` that records its run in
/// `marker` and exits 1 (through [`write_script`], so no ETXTBSY).
#[cfg(unix)]
fn marker_script(path: &Path, marker: &Path) {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).unwrap();
    }
    write_script(
        path,
        &format!("#!/bin/sh\n: > '{}'\nexit 1\n", marker.display()),
    );
}

/// `bd = "…"` in TOML is ignored: a planted bd named there never runs.
///
/// The planted bd is a copy of fake-bd over this scenario, so had it run the
/// refresh would store the scenario's two rows, as the control below shows.
/// There is no `--bd` and no `BD_CACHE__BD`, so the default bare `bd` is
/// resolved from `PATH` first (lead ruling S77). On Unix the test puts a stub
/// `bd` first on `PATH` that records its run and exits 1: it runs, whatever
/// bd the host has installed in a fallback directory, and the scope fails
/// with the same error on every host. On Windows `PATH` is an empty directory
/// and the [`hermetic`] child has no `USERPROFILE` or `LOCALAPPDATA`, so there
/// is no fallback directory either and no bd is found. The scope is a failed
/// result (lead ruling S75).
#[test]
fn bd_is_never_read_from_toml() {
    let repo = TestRepo::new(&two_beads());
    let tools = tempfile::TempDir::new().unwrap();
    let planted = tools.path().join(if cfg!(windows) {
        "planted-bd.exe"
    } else {
        "planted-bd"
    });
    copy_executable(Path::new(FAKE_BD), &planted);
    let on_path = tempfile::TempDir::new().unwrap();
    let marker = tools.path().join("path-bd-ran");
    #[cfg(unix)]
    marker_script(&on_path.path().join("bd"), &marker);
    let toml = format!(
        "scopes = [\"x\"]\nbd = {}\n",
        Value::from(path_str(&planted))
    );
    write_toml(&repo, &toml);
    let root = repo.root();
    let args = ["--repo", path_str(&root), "cache", "refresh", "scopes", "x"];
    let path = if cfg!(windows) {
        path_str(on_path.path()).to_owned()
    } else {
        format!("{}:/usr/bin:/bin", path_str(on_path.path()))
    };
    let (exit, v) = envelope_json(&repo.bdc_raw(&root, &args, &[("PATH", &path)]));
    assert_eq!(exit, 0, "the bd key does not make the file invalid: {v}");
    let result = &v["data"]["results"][0];
    // The security property: the planted bd never ran, so nothing was stored.
    assert_eq!(
        result["report"]["stored"].as_u64().unwrap_or(0),
        0,
        "the planted bd from TOML ran: {v}"
    );
    assert_eq!(
        v["data"]["failed"], 1,
        "the refresh reached no usable bd: {v}"
    );
    let want = if cfg!(windows) {
        "BD_CACHE.BD_UNAVAILABLE"
    } else {
        "BD_CACHE.SYNC_FAILED"
    };
    assert_eq!(result["error"]["code"], want, "{v}");
    assert_eq!(result["error"]["retryable"], true, "{v}");
    assert_eq!(
        marker.exists(),
        cfg!(unix),
        "the default bd is the one on PATH: {v}"
    );
    // The control: the same binary named by BD_CACHE__BD runs and syncs both rows.
    let (exit, v) = envelope_json(&repo.bdc_raw(
        &root,
        &args,
        &[("PATH", &path), ("BD_CACHE__BD", path_str(&planted))],
    ));
    assert_eq!((exit, &v["data"]["failed"]), (0, &Value::from(0)), "{v}");
    assert_eq!(v["data"]["results"][0]["report"]["stored"], 2, "{v}");
}

/// Lead rulings S76 and S77 (QA RCR-002): an empty or relative `PATH`
/// segment, or a fallback directory under a relative `HOME`, never resolves
/// a bare bd name against the repository root, where every bd child runs.
/// An executable committed in the repository (at its root, and under the
/// directories a `HOME` of `.` would name) writes a marker if it runs. The
/// name is one no host installs, so no real directory can answer first and
/// hide the bug.
#[cfg(unix)]
#[test]
fn an_empty_or_relative_path_segment_never_runs_a_bd_committed_in_the_repo() {
    let repo = TestRepo::new(&two_beads());
    let root = repo.root();
    let marks = tempfile::TempDir::new().unwrap();
    let marker = marks.path().join("ran");
    let name = "bdc-s76-committed-bd";
    for dir in ["", ".local/bin", "bin", "go/bin", ".cargo/bin"] {
        marker_script(&root.join(dir).join(name), &marker);
    }
    let args = [
        "--repo",
        path_str(&root),
        "--bd",
        name,
        "cache",
        "refresh",
        "scopes",
        "x",
    ];
    let cases: [(Option<&str>, Option<&str>); 7] = [
        (None, None),
        (Some("/usr/bin::/bin"), None),
        (Some(":/usr/bin"), None),
        (Some("/usr/bin:"), None),
        (Some(".:/usr/bin"), None),
        (Some("bin:./go/bin:/usr/bin"), None),
        (Some("/usr/bin:/bin"), Some(".")),
    ];
    for (path, home) in cases {
        let mut cmd = hermetic(BDC);
        cmd.args(args)
            .current_dir(&root)
            .env(bd_cache_cli::scenario::SCENARIO_ENV, repo.scenario_path());
        match path {
            None => cmd.env_remove("PATH"),
            Some(p) => cmd.env("PATH", p),
        };
        if let Some(h) = home {
            cmd.env("HOME", h);
        }
        let (exit, v) = envelope_json(&cmd.output().unwrap().into());
        assert!(
            !marker.exists(),
            "PATH {path:?}, HOME {home:?}: the {name} committed in the repository ran: {v}"
        );
        assert_eq!(exit, 0, "PATH {path:?}, HOME {home:?}: {v}");
        assert_eq!(v["data"]["failed"], 1, "PATH {path:?}, HOME {home:?}: {v}");
        assert_eq!(
            v["data"]["results"][0]["error"]["code"], "BD_CACHE.BD_UNAVAILABLE",
            "PATH {path:?}, HOME {home:?}: {v}"
        );
    }
}

/// Lead ruling S77: the user's `PATH` wins over sc-bd-cli's fallback
/// directories. A bd under `$HOME/.local/bin` (a fallback directory) writes a
/// marker if it runs; fake-bd, copied as `bd` into a directory named first on
/// `PATH`, is the one that runs and syncs the scope.
#[cfg(unix)]
#[test]
fn the_users_path_beats_a_fallback_directory() {
    let repo = TestRepo::new(&two_beads());
    let root = repo.root();
    let home = tempfile::TempDir::new().unwrap();
    let marker = home.path().join("fallback-ran");
    marker_script(&home.path().join(".local/bin/bd"), &marker);
    let user = tempfile::TempDir::new().unwrap();
    copy_executable(Path::new(FAKE_BD), &user.path().join("bd"));
    let path = format!("{}:/usr/bin:/bin", path_str(user.path()));
    let args = ["--repo", path_str(&root), "cache", "refresh", "scopes", "x"];
    let (exit, v) = envelope_json(&repo.bdc_raw(
        &root,
        &args,
        &[("PATH", &path), ("HOME", path_str(home.path()))],
    ));
    assert!(!marker.exists(), "the fallback bd ran: {v}");
    assert_eq!((exit, &v["data"]["failed"]), (0, &Value::from(0)), "{v}");
    assert_eq!(v["data"]["results"][0]["report"]["stored"], 2, "{v}");
}

/// No upward search: from a subdirectory, the parent's `.beads` is not found.
#[test]
fn a_subdirectory_does_not_find_the_parent_beads() {
    let repo = TestRepo::new(&two_beads());
    let sub = repo.root().join("sub");
    std::fs::create_dir_all(&sub).unwrap();
    let (exit, v) = envelope_json(&repo.bdc_raw(&sub, &["--bd", FAKE_BD, "context"], &[]));
    assert_eq!((exit, code(&v)), (3, "BD_CACHE.NO_DATABASE"), "{v}");
    assert!(!repo.root().join(".bd-cache").exists());
    assert!(!sub.join(".bd-cache").exists());
    // The control: from the root itself, the working directory is the repository.
    let (exit, v) = envelope_json(&repo.bdc_raw(&repo.root(), &["--bd", FAKE_BD, "context"], &[]));
    assert_eq!(exit, 0, "{v}");
    // And BD_CACHE__REPO names it from anywhere.
    let (exit, _) = envelope_json(&repo.bdc_raw(
        &sub,
        &["--bd", FAKE_BD, "context"],
        &[("BD_CACHE__REPO", path_str(&repo.root()))],
    ));
    assert_eq!(exit, 0);
}

/// No consumer path is known to beads-cache (ADR-BDC-0015 amends
/// ADR-CLI-0004): a cache file of the same name in another directory of the
/// repository changes nothing, and the cache opens at the store's default.
/// Planted negative: `cache_path` naming that file opens it.
#[test]
fn a_cache_file_in_another_repository_directory_is_ignored_and_the_repository_opens_as_any_other() {
    let repo = TestRepo::new(&two_beads());
    let other = repo
        .root()
        .join(".old-cache")
        .join("beads.bd-cache.sqlite3");
    std::fs::create_dir_all(other.parent().unwrap()).unwrap();
    std::fs::write(&other, b"").unwrap();
    let (exit, v) = repo.json(&["context"]);
    assert_eq!(exit, 0, "{v}");
    let path = v["data"]["x_cache"]["cache_path"]
        .as_str()
        .unwrap()
        .replace('\\', "/");
    assert!(path.ends_with(".bd-cache/beads.bd-cache.sqlite3"), "{path}");
    write_toml(
        &repo,
        "scopes = [\"x\"]\ncache_path = \".old-cache/beads.bd-cache.sqlite3\"\n",
    );
    let (exit, v) = repo.json(&["context"]);
    assert_eq!(exit, 0, "{v}");
    let path = v["data"]["x_cache"]["cache_path"]
        .as_str()
        .unwrap()
        .replace('\\', "/");
    assert!(
        path.ends_with(".old-cache/beads.bd-cache.sqlite3"),
        "{path}"
    );
}

/// `metadata.json` is read from `$BEADS_DIR` when it is set.
#[test]
fn metadata_comes_from_beads_dir_when_set() {
    let repo = TestRepo::new(&two_beads());
    let other = tempfile::TempDir::new().unwrap();
    std::fs::write(
        other.path().join("metadata.json"),
        metadata("elsewhere", "proj-2"),
    )
    .unwrap();
    let path = cache_path_of(&repo, &[], &[("BEADS_DIR", path_str(other.path()))]);
    assert!(path.ends_with("elsewhere.bd-cache.sqlite3"), "{path}");
}
