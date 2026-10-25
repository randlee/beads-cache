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
use std::process::{Command, Stdio};

use serde_json::Value;
use support::{
    BD_CACHE, BDC, FAKE_BD, Ran, TestRepo, envelope_json, metadata, parse_envelope, path_str,
    two_beads,
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
        &["issues", "list", "--scope", "phase-x"],
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
            &["issues", "list", "--scope", "phase-zz"][..],
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
    std::fs::write(&input, r#"{"scope": "phase-x"}"#).unwrap();
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
    let mut child = Command::new(BDC)
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
        Command::new(bin)
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
    let mut cmd = Command::new("script");
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
    // never opened before syncs phase-y only.
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
    assert_eq!(v["data"]["results"][0]["scope"], "phase-y", "{v}");
    assert_eq!(v["data"]["results"].as_array().map(Vec::len), Some(1));
    // An env value that does not parse is CONFIG_INVALID, exit 3.
    let (exit, v) = envelope_json(&repo.bdc_raw(
        &root,
        &["--repo", path_str(&root), "context"],
        &[("BD_CACHE__CALL_TIMEOUT_S", "soon")],
    ));
    assert_eq!((exit, code(&v)), (3, "BD_CACHE.CONFIG_INVALID"), "{v}");
}

/// `bd = "…"` in TOML is ignored: a planted bd named there never runs.
#[test]
fn bd_is_never_read_from_toml() {
    let repo = TestRepo::new(&two_beads());
    let tools = tempfile::TempDir::new().unwrap();
    let planted = tools.path().join(if cfg!(windows) {
        "planted-bd.exe"
    } else {
        "planted-bd"
    });
    std::fs::copy(FAKE_BD, &planted).unwrap();
    let empty_path = tempfile::TempDir::new().unwrap();
    let toml = format!(
        "scopes = [\"x\"]\nbd = {}\n",
        Value::from(path_str(&planted))
    );
    write_toml(&repo, &toml);
    let root = repo.root();
    let args = ["--repo", path_str(&root), "cache", "refresh", "scopes", "x"];
    // No --bd, no BD_CACHE__BD, and no bd on PATH: the refresh cannot reach a bd.
    let (exit, v) =
        envelope_json(&repo.bdc_raw(&root, &args, &[("PATH", path_str(empty_path.path()))]));
    assert_eq!(exit, 0, "the bd key does not make the file invalid: {v}");
    assert_eq!(v["data"]["failed"], 1, "the planted bd from TOML ran: {v}");
    assert!(!v["data"]["results"][0]["error"].is_null(), "{v}");
    // The control: the same binary named by BD_CACHE__BD runs and syncs.
    let (exit, v) = envelope_json(&repo.bdc_raw(
        &root,
        &args,
        &[
            ("PATH", path_str(empty_path.path())),
            ("BD_CACHE__BD", path_str(&planted)),
        ],
    ));
    assert_eq!((exit, &v["data"]["failed"]), (0, &Value::from(0)), "{v}");
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

/// The legacy `.atm-bd/` file, with no `cache_path` and no default file, is
/// `CONFIG_INVALID` naming `cache_path` and the legacy path.
#[test]
fn the_legacy_cache_file_is_config_invalid() {
    let repo = TestRepo::new(&two_beads());
    let legacy = repo.root().join(".atm-bd").join("beads.bd-cache.sqlite3");
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    std::fs::write(&legacy, b"").unwrap();
    let (exit, v) = repo.json(&["context"]);
    assert_eq!((exit, code(&v)), (3, "BD_CACHE.CONFIG_INVALID"), "{v}");
    let details = &v["error"]["details"];
    assert_eq!(details["key"], "cache_path");
    assert_eq!(Path::new(details["legacy_path"].as_str().unwrap()), legacy);
    assert!(
        v["error"]["suggested_action"]
            .as_str()
            .unwrap()
            .contains("cache_path = \".atm-bd/beads.bd-cache.sqlite3\"")
    );
    assert!(
        !repo.root().join(".bd-cache").exists(),
        "nothing was created"
    );
    // A configured cache_path lifts the rule.
    write_toml(
        &repo,
        "scopes = [\"x\"]\ncache_path = \".bd-cache/beads.bd-cache.sqlite3\"\n",
    );
    assert_eq!(repo.json(&["context"]).0, 0);
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
