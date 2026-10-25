//! The extended `PATH` used by every CLI spawn.
//!
//! GUI launches (Finder/Dock) start with a minimal `PATH`, so the per-OS install
//! directories of Homebrew, Go and Cargo are prepended to the inherited `PATH`
//! (`;`-separated on Windows, `:` elsewhere). Reads `PATH`, `HOME`, `GOPATH`,
//! `USERPROFILE` and `LOCALAPPDATA` from the environment on every call.

use std::env;

/// The inherited `PATH` with the platform install directories prepended.
#[must_use]
#[cfg_attr(
    not(target_os = "windows"),
    expect(
        clippy::uninlined_format_args,
        reason = "moved verbatim from btit-app cli.rs in b-4 (the Windows raw-string branch does not trigger it)"
    )
)]
pub fn get_extended_path() -> String {
    let current_path = env::var("PATH").unwrap_or_default();

    #[cfg(target_os = "windows")]
    {
        let userprofile = env::var("USERPROFILE").unwrap_or_default();
        let localappdata = env::var("LOCALAPPDATA").unwrap_or_default();
        let extra_paths = vec![
            format!(r"{}\AppData\Local\bin", userprofile),
            format!(r"{}\.local\bin", userprofile),
            format!(r"{}\Programs", localappdata),
            // `go install` and `cargo install` targets (bd / br)
            format!(r"{}\go\bin", userprofile),
            format!(r"{}\.cargo\bin", userprofile),
        ];
        join_extended(extra_paths, &current_path, ';')
    }
    #[cfg(not(target_os = "windows"))]
    {
        let home = env::var("HOME").unwrap_or_default();
        let gopath = env::var("GOPATH").unwrap_or_else(|_| format!("{}/go", home));
        let extra_paths = vec![
            "/opt/homebrew/bin".to_string(),
            "/usr/local/bin".to_string(),
            "/home/linuxbrew/.linuxbrew/bin".to_string(),
            "/usr/bin".to_string(),
            "/bin".to_string(),
            format!("{}/.local/bin", home),
            format!("{}/bin", home),
            // `go install` and `cargo install` targets (bd / br)
            format!("{}/bin", gopath),
            format!("{}/.cargo/bin", home),
        ];
        join_extended(extra_paths, &current_path, ':')
    }
}

/// `extra` followed by the non-empty segments of the inherited `PATH`, joined
/// with `sep`.
///
/// An empty segment (an unset or empty `PATH`, or a leading, trailing or
/// doubled separator) means the working directory to `execvp` and to Windows'
/// search, and every `bd` child runs in the repository root: kept, it would
/// run a committed `<repo>/bd` for a bare `bd` (lead ruling S76, QA RCR-002).
/// So empty segments are dropped, and an unset `PATH` appends nothing.
fn join_extended(mut extra: Vec<String>, inherited: &str, sep: char) -> String {
    extra.extend(
        inherited
            .split(sep)
            .filter(|segment| !segment.is_empty())
            .map(String::from),
    );
    extra.join(&sep.to_string())
}

/// Directories the CLI probe searches, in order, without duplicates.
/// Uses the platform PATH separator (`;` on Windows, `:` elsewhere).
#[must_use]
pub fn extended_path_entries() -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    std::env::split_paths(&get_extended_path())
        .map(|p| p.to_string_lossy().to_string())
        .filter(|p| !p.is_empty())
        .filter(|p| seen.insert(p.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lead ruling S76 (QA RCR-002): no inherited `PATH` adds an empty
    /// segment, which would resolve a bare `bd` in the repository root.
    /// Both separators, whatever the host platform.
    #[test]
    fn the_inherited_path_never_adds_an_empty_segment() {
        let extra = || vec!["/opt/homebrew/bin".to_owned(), "/usr/bin".to_owned()];
        for sep in [':', ';'] {
            let s = sep.to_string();
            let cases = [
                (String::new(), vec![]),
                (s.clone(), vec![]),
                (format!("{s}{s}"), vec![]),
                (format!("/usr/bin{s}{s}/bin"), vec!["/usr/bin", "/bin"]),
                (format!("{s}/usr/bin"), vec!["/usr/bin"]),
                (format!("/usr/bin{s}"), vec!["/usr/bin"]),
                (format!("{s}/a{s}{s}/b{s}"), vec!["/a", "/b"]),
            ];
            for (inherited, tail) in cases {
                let joined = join_extended(extra(), &inherited, sep);
                let segments: Vec<&str> = joined.split(sep).collect();
                assert!(
                    segments.iter().all(|seg| !seg.is_empty()),
                    "{inherited:?} with {sep:?}: {joined:?}"
                );
                let mut want = vec!["/opt/homebrew/bin", "/usr/bin"];
                want.extend(tail);
                assert_eq!(segments, want, "{inherited:?} with {sep:?}");
            }
        }
    }

    #[test]
    fn extended_path_entries_are_nonempty_and_deduplicated() {
        let entries = extended_path_entries();
        assert!(!entries.is_empty());
        assert!(entries.iter().all(|e| !e.is_empty()));
        let mut sorted = entries.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            entries.len(),
            "duplicates present: {entries:?}"
        );
        // Split on the platform separator, not a hardcoded one
        let sep = if cfg!(windows) { ';' } else { ':' };
        assert!(
            entries.iter().all(|e| !e.contains(sep)),
            "entry contains separator: {entries:?}"
        );
    }

    #[test]
    fn extended_path_includes_gui_launch_locations() {
        let path = get_extended_path();
        #[cfg(not(target_os = "windows"))]
        {
            for needle in [
                "/opt/homebrew/bin",
                "/usr/local/bin",
                "/.cargo/bin",
                "/.local/bin",
            ] {
                assert!(
                    path.contains(needle),
                    "extended PATH missing {needle}: {path}"
                );
            }
            assert!(
                path.contains("go/bin") || env::var("GOPATH").is_ok(),
                "extended PATH missing go bin dir: {path}"
            );
            let ambient = env::var("PATH").unwrap_or_default();
            let ambient = join_extended(Vec::new(), &ambient, ':');
            assert!(ambient.is_empty() || path.ends_with(&ambient));
        }
        #[cfg(target_os = "windows")]
        {
            for needle in [r"\go\bin", r"\.cargo\bin", r"\.local\bin"] {
                assert!(
                    path.contains(needle),
                    "extended PATH missing {needle}: {path}"
                );
            }
        }
    }

    #[test]
    fn extended_path_entries_contain_platform_install_dirs_in_order() {
        let entries = extended_path_entries();
        assert!(
            entries.iter().all(|e| !e.is_empty()),
            "empty entry: {entries:?}"
        );

        #[cfg(not(target_os = "windows"))]
        {
            let idx = |needle: &str| {
                entries
                    .iter()
                    .position(|e| e == needle)
                    .unwrap_or_else(|| panic!("missing {needle}: {entries:?}"))
            };
            let homebrew = idx("/opt/homebrew/bin");
            let usr_local = idx("/usr/local/bin");
            let usr_bin = idx("/usr/bin");
            assert!(homebrew < usr_local, "order not preserved: {entries:?}");
            assert!(usr_local < usr_bin, "order not preserved: {entries:?}");
            assert!(
                entries.iter().any(|e| e.ends_with("/.cargo/bin")),
                "{entries:?}"
            );
            assert!(
                entries
                    .iter()
                    .any(|e| e.ends_with("/bin")
                        && (e.contains("/go/") || env::var("GOPATH").is_ok())),
                "missing go bin dir: {entries:?}"
            );
        }
        #[cfg(target_os = "windows")]
        {
            assert!(
                entries.iter().any(|e| e.ends_with(r"\go\bin")),
                "{entries:?}"
            );
            assert!(
                entries.iter().any(|e| e.ends_with(r"\.cargo\bin")),
                "{entries:?}"
            );
            assert!(
                entries.iter().any(|e| e.ends_with(r"\.local\bin")),
                "{entries:?}"
            );
            let go = entries
                .iter()
                .position(|e| e.ends_with(r"\go\bin"))
                .unwrap();
            let cargo = entries
                .iter()
                .position(|e| e.ends_with(r"\.cargo\bin"))
                .unwrap();
            assert!(go < cargo, "order not preserved: {entries:?}");
        }
    }

    #[test]
    #[cfg_attr(
        target_os = "windows",
        expect(
            clippy::map_unwrap_or,
            reason = "test moved verbatim from btit-app cli.rs in b-4"
        )
    )]
    fn extended_path_entries_precede_ambient_path() {
        // The extra install dirs must come before whatever PATH the process
        // inherited, so a GUI launch with a minimal PATH still finds bd.
        let entries = extended_path_entries();
        #[cfg(not(target_os = "windows"))]
        assert_eq!(
            entries.first().map(String::as_str),
            Some("/opt/homebrew/bin")
        );
        #[cfg(target_os = "windows")]
        assert!(
            entries
                .first()
                .map(|e| e.ends_with(r"\AppData\Local\bin"))
                .unwrap_or(false),
            "{entries:?}"
        );
    }
}
