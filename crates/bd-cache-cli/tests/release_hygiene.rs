//! Acceptance 5 of sprint b-3a: a release build of the two binaries does not build
//! `fake-bd` and carries no `test-support` code (ADR-CLI-0001).

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "test code: a failed unwrap or index is a test failure"
)]

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::{Value, json};

/// A string that exists in a binary only when sc-bd-cli's `test-support` module is
/// compiled into it: the usage error `FakeBd` gives a write without `--actor`
/// (sc-bd-cli `src/testing/fake_bd.rs`). If that text changes, the control test
/// below fails, so the scan cannot silently stop finding anything.
const TEST_SUPPORT_MARKER: &[u8] = b"--actor is required by the cache's writes";

/// The `cargo` that runs this test.
fn cargo() -> Command {
    Command::new(std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into()))
}

/// The bin targets of `bd-cache-cli`, from `cargo metadata`.
fn bin_targets() -> Value {
    let out = cargo()
        .args(["metadata", "--format-version", "1", "--no-deps", "--locked"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata runs");
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    serde_json::from_slice(&out.stdout).expect("metadata is JSON")
}

/// Checks that `fake-bd` is a bin gated on the `fake-bd` feature, that no other bin
/// is gated, and that the package is not publishable.
///
/// # Errors
///
/// What is wrong with the metadata.
fn fake_bd_is_gated(metadata: &Value) -> Result<(), String> {
    let package = metadata["packages"]
        .as_array()
        .and_then(|p| p.iter().find(|p| p["name"] == "bd-cache-cli"))
        .ok_or("no bd-cache-cli package")?;
    if package["publish"] != json!([]) {
        return Err(format!("publish is {}, not false", package["publish"]));
    }
    let mut seen = Vec::new();
    for target in package["targets"].as_array().ok_or("no targets")? {
        if target["kind"] != json!(["bin"]) {
            continue;
        }
        let name = target["name"].as_str().ok_or("unnamed bin")?;
        let required = &target["required-features"];
        let gated = *required == json!(["fake-bd"]);
        match (name, gated) {
            ("fake-bd", true) | ("bd-cache" | "bdc", false) => seen.push(name.to_owned()),
            ("fake-bd", false) => {
                return Err(format!(
                    "fake-bd is a bin without required-features: {required}"
                ));
            }
            _ => return Err(format!("bin {name} has required-features {required}")),
        }
    }
    seen.sort();
    if seen == ["bd-cache", "bdc", "fake-bd"] {
        Ok(())
    } else {
        Err(format!("the bins are {seen:?}"))
    }
}

#[test]
fn fake_bd_is_a_gated_bin_and_the_crate_is_not_publishable() {
    fake_bd_is_gated(&bin_targets()).expect("fake-bd is gated");
}

/// The planted negative of acceptance 5: the metadata of an unconditional `fake-bd`
/// (and of a published crate, and of a gated release binary) fails the check.
#[test]
fn an_ungated_fake_bd_fails_the_metadata_check() {
    let good = bin_targets();
    let mutate = |f: &dyn Fn(&mut Value)| {
        let mut m = good.clone();
        let package = m["packages"]
            .as_array_mut()
            .and_then(|p| p.iter_mut().find(|p| p["name"] == "bd-cache-cli"))
            .expect("the package");
        f(package);
        m
    };
    let target = |package: &mut Value, name: &str| -> usize {
        package["targets"]
            .as_array()
            .and_then(|t| {
                t.iter()
                    .position(|t| t["name"] == name && t["kind"] == json!(["bin"]))
            })
            .expect("the target")
    };

    let ungated = mutate(&|p| {
        let i = target(p, "fake-bd");
        p["targets"][i]["required-features"] = json!([]);
    });
    let err = fake_bd_is_gated(&ungated).expect_err("an unconditional fake-bd");
    assert!(
        err.contains("fake-bd is a bin without required-features"),
        "{err}"
    );

    let gated_release_bin = mutate(&|p| {
        let i = target(p, "bdc");
        p["targets"][i]["required-features"] = json!(["fake-bd"]);
    });
    assert!(fake_bd_is_gated(&gated_release_bin).is_err());

    let published = mutate(&|p| p["publish"] = Value::Null);
    assert!(fake_bd_is_gated(&published).is_err());
}

fn contains(binary: &Path, needle: &[u8]) -> bool {
    let bytes = std::fs::read(binary).expect("the binary is readable");
    bytes.windows(needle.len()).any(|w| w == needle)
}

fn exe(dir: &Path, name: &str) -> PathBuf {
    dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
}

#[test]
fn the_scanner_finds_test_support_code_in_the_fake_bd_binary() {
    // The control of the scan below: `fake-bd` is built with test-support, so the
    // marker is in it. Without this the "absent" result could mean a stale marker.
    assert!(contains(Path::new(support::FAKE_BD), TEST_SUPPORT_MARKER));
}

#[test]
fn a_release_build_has_no_fake_bd_and_no_test_support_code() {
    let target = Path::new(env!("CARGO_TARGET_TMPDIR")).join("release-hygiene");
    let out = cargo()
        .args(["build", "--release", "--locked", "-p", "bd-cache-cli"])
        .args(["--bin", "bd-cache", "--bin", "bdc"])
        .arg("--target-dir")
        .arg(&target)
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo build runs");
    assert!(
        out.status.success(),
        "release build failed:\n{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let release = target.join("release");
    for name in ["bd-cache", "bdc"] {
        let binary = exe(&release, name);
        assert!(binary.is_file(), "{} was not built", binary.display());
        assert!(
            !contains(&binary, TEST_SUPPORT_MARKER),
            "{name} carries test-support code"
        );
    }
    assert!(
        !exe(&release, "fake-bd").exists(),
        "a release build of bd-cache and bdc built fake-bd"
    );
}
