//! The extended `PATH` used by every CLI spawn.
//!
//! The inherited `PATH` comes first, so the user's own `PATH` decides which
//! `bd` runs (contract §2). GUI launches (Finder/Dock) start with a minimal
//! `PATH`, so the per-OS install directories of Homebrew, Go and Cargo follow
//! it as a fallback (`;`-separated on Windows, `:` elsewhere). Only absolute
//! directories are kept, each once (lead rulings S76 and S77). Reads `PATH`,
//! `HOME`, `GOPATH`, `USERPROFILE` and `LOCALAPPDATA` from the environment on
//! every call.

use std::collections::HashSet;
use std::env;
use std::path::Path;

/// The platform `PATH` separator.
const SEP: char = if cfg!(windows) { ';' } else { ':' };

/// The absolute segments of the inherited `PATH`, then the platform install
/// directories that are not already in it.
#[must_use]
pub fn get_extended_path() -> String {
    let inherited = env::var("PATH").unwrap_or_default();
    join_extended(&inherited, fallback_dirs_from_env(), SEP)
}

/// The fallback directories for this process's environment.
fn fallback_dirs_from_env() -> Vec<String> {
    let var = |key: &str| env::var(key).ok();
    #[cfg(target_os = "windows")]
    {
        fallback_dirs(
            var("USERPROFILE").as_deref(),
            var("LOCALAPPDATA").as_deref(),
        )
    }
    #[cfg(not(target_os = "windows"))]
    {
        fallback_dirs(var("HOME").as_deref(), var("GOPATH").as_deref())
    }
}

/// `base` joined with `rel`, when `base` is an absolute directory; `None` for
/// an unset, empty or relative one (lead ruling S77: a directory built from a
/// relative `HOME` or `GOPATH` would resolve against the working directory,
/// which is the repository root for every `bd` child).
fn under(base: Option<&str>, rel: &str) -> Option<String> {
    let base = base.filter(|b| !b.is_empty() && Path::new(b).is_absolute())?;
    let base = base.trim_end_matches(['/', '\\']);
    let sep = if cfg!(windows) { '\\' } else { '/' };
    Some(format!("{base}{sep}{rel}"))
}

/// The Windows install directories, from `USERPROFILE` and `LOCALAPPDATA`.
#[cfg(target_os = "windows")]
fn fallback_dirs(userprofile: Option<&str>, localappdata: Option<&str>) -> Vec<String> {
    [
        under(userprofile, r"AppData\Local\bin"),
        under(userprofile, r".local\bin"),
        under(localappdata, "Programs"),
        // `go install` and `cargo install` targets (bd / br)
        under(userprofile, r"go\bin"),
        under(userprofile, r".cargo\bin"),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// The Unix install directories: the fixed system ones, then those under
/// `HOME` and `GOPATH` (`$HOME/go` when `GOPATH` is unset).
#[cfg(not(target_os = "windows"))]
fn fallback_dirs(home: Option<&str>, gopath: Option<&str>) -> Vec<String> {
    let gobin = match gopath {
        Some(g) => under(Some(g), "bin"),
        None => under(home, "go/bin"),
    };
    let fixed = [
        "/opt/homebrew/bin",
        "/usr/local/bin",
        "/home/linuxbrew/.linuxbrew/bin",
        "/usr/bin",
        "/bin",
    ]
    .map(|d| Some(d.to_owned()));
    fixed
        .into_iter()
        .chain([
            under(home, ".local/bin"),
            under(home, "bin"),
            // `go install` and `cargo install` targets (bd / br)
            gobin,
            under(home, ".cargo/bin"),
        ])
        .flatten()
        .collect()
}

/// The absolute segments of `inherited` (split on `sep`), then the absolute
/// `fallback` directories, each directory once (its first occurrence), joined
/// with `sep`.
///
/// An empty or relative segment (an unset or empty `PATH`, a leading,
/// trailing or doubled separator, `.`, `bin`, `./x`) resolves against the
/// working directory, and every `bd` child runs in the repository root: kept,
/// it would run a committed `<repo>/bd` for a bare `bd` (lead rulings S76 and
/// S77, QA RCR-002). So only absolute segments are kept (`Path::is_absolute`:
/// a drive or UNC root on Windows). The inherited `PATH` comes first, so the
/// user's `PATH` wins over the fallback (S77, contract §2).
fn join_extended(inherited: &str, fallback: Vec<String>, sep: char) -> String {
    let mut seen = HashSet::new();
    let kept: Vec<String> = inherited
        .split(sep)
        .map(str::to_owned)
        .chain(fallback)
        .filter(|segment| !segment.is_empty() && Path::new(segment).is_absolute())
        .filter(|segment| seen.insert(segment.clone()))
        .collect();
    kept.join(&sep.to_string())
}

/// Directories the CLI probe searches, in order, without duplicates.
/// Uses the platform PATH separator (`;` on Windows, `:` elsewhere).
#[must_use]
pub fn extended_path_entries() -> Vec<String> {
    let mut seen = HashSet::new();
    std::env::split_paths(&get_extended_path())
        .map(|p| p.to_string_lossy().to_string())
        .filter(|p| !p.is_empty())
        .filter(|p| seen.insert(p.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An absolute directory on this platform.
    fn abs(name: &str) -> String {
        if cfg!(windows) {
            format!(r"C:\{name}")
        } else {
            format!("/{name}")
        }
    }

    /// Lead rulings S76 and S77 (QA RCR-002): no inherited `PATH` adds an
    /// empty or relative segment, which would resolve a bare `bd` in the
    /// repository root. Both separators, whatever the host platform.
    #[test]
    fn the_inherited_path_keeps_only_absolute_segments() {
        let (a, b) = (abs("a"), abs("b"));
        for sep in [':', ';'] {
            if cfg!(windows) && sep == ':' {
                continue; // `C:\a` itself holds a ':'
            }
            let s = sep.to_string();
            let cases = [
                (String::new(), vec![]),
                (s.clone(), vec![]),
                (format!("{s}{s}"), vec![]),
                (format!("{a}{s}{s}{b}"), vec![a.as_str(), b.as_str()]),
                (format!("{s}{a}"), vec![a.as_str()]),
                (format!("{a}{s}"), vec![a.as_str()]),
                (format!(".{s}{a}"), vec![a.as_str()]),
                (
                    format!("bin{s}{a}{s}./x{s}..{s}{b}"),
                    vec![a.as_str(), b.as_str()],
                ),
            ];
            for (inherited, want) in cases {
                let joined = join_extended(&inherited, Vec::new(), sep);
                let segments: Vec<&str> = if joined.is_empty() {
                    Vec::new()
                } else {
                    joined.split(sep).collect()
                };
                assert_eq!(segments, want, "{inherited:?} with {sep:?}");
            }
        }
    }

    /// S77: the inherited `PATH` comes first and the fallback after it, so the
    /// user's `PATH` wins; a directory in both is kept once, where the
    /// inherited `PATH` has it; a relative fallback entry is dropped too.
    #[test]
    fn the_inherited_path_comes_first_and_duplicates_are_removed() {
        let (user, shared, fallback) = (abs("user/bin"), abs("usr/bin"), abs("opt/bin"));
        let inherited = format!("{user}{SEP}{shared}{SEP}{user}");
        let joined = join_extended(
            &inherited,
            vec![fallback.clone(), shared.clone(), "rel/bin".to_owned()],
            SEP,
        );
        let segments: Vec<&str> = joined.split(SEP).collect();
        assert_eq!(
            segments,
            [user.as_str(), shared.as_str(), fallback.as_str()]
        );
    }

    /// S77: an unset, empty or relative `HOME` or `GOPATH` builds no fallback
    /// directory; only the fixed system ones remain.
    #[cfg(not(target_os = "windows"))]
    #[test]
    fn a_relative_or_empty_home_or_gopath_adds_no_fallback_directory() {
        let fixed = [
            "/opt/homebrew/bin",
            "/usr/local/bin",
            "/home/linuxbrew/.linuxbrew/bin",
            "/usr/bin",
            "/bin",
        ];
        for (home, gopath) in [
            (None, None),
            (Some(""), None),
            (Some("."), None),
            (Some("home"), Some("go")),
            (None, Some("./go")),
            (Some(""), Some("")),
        ] {
            assert_eq!(fallback_dirs(home, gopath), fixed, "{home:?} {gopath:?}");
        }
        let tail = |home, gopath| fallback_dirs(home, gopath).split_off(fixed.len());
        assert_eq!(
            tail(Some("/h/"), None),
            ["/h/.local/bin", "/h/bin", "/h/go/bin", "/h/.cargo/bin"]
        );
        assert_eq!(
            tail(Some("/h"), Some("/g")),
            ["/h/.local/bin", "/h/bin", "/g/bin", "/h/.cargo/bin"]
        );
        assert_eq!(
            tail(Some("."), Some("/g")),
            ["/g/bin"],
            "GOPATH is used on its own when HOME is relative"
        );
    }

    /// S77 on Windows: an unset, empty or relative `USERPROFILE` or
    /// `LOCALAPPDATA` builds no fallback directory.
    #[cfg(target_os = "windows")]
    #[test]
    fn a_relative_or_empty_profile_adds_no_fallback_directory() {
        for (profile, local) in [
            (None, None),
            (Some(""), Some("")),
            (Some("."), Some(r"x\y")),
            (Some(r"\Users\u"), None),
        ] {
            assert!(
                fallback_dirs(profile, local).is_empty(),
                "{profile:?} {local:?}"
            );
        }
        assert_eq!(
            fallback_dirs(Some(r"C:\Users\u"), Some(r"C:\L")),
            [
                r"C:\Users\u\AppData\Local\bin",
                r"C:\Users\u\.local\bin",
                r"C:\L\Programs",
                r"C:\Users\u\go\bin",
                r"C:\Users\u\.cargo\bin",
            ]
        );
    }

    #[test]
    fn extended_path_entries_are_absolute_and_deduplicated() {
        let entries = extended_path_entries();
        assert!(!entries.is_empty());
        assert!(
            entries.iter().all(|e| Path::new(e).is_absolute()),
            "{entries:?}"
        );
        let mut sorted = entries.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(
            sorted.len(),
            entries.len(),
            "duplicates present: {entries:?}"
        );
        assert!(
            entries.iter().all(|e| !e.contains(SEP)),
            "entry contains separator: {entries:?}"
        );
    }

    /// The process's own absolute `PATH` entries come first, in order (S77).
    #[test]
    fn extended_path_entries_start_with_the_ambient_path() {
        let ambient = env::var("PATH").unwrap_or_default();
        let want = join_extended(&ambient, Vec::new(), SEP);
        let path = get_extended_path();
        assert!(path.starts_with(&want), "{path} does not start with {want}");
    }

    #[test]
    fn extended_path_includes_gui_launch_locations() {
        let path = get_extended_path();
        #[cfg(not(target_os = "windows"))]
        for needle in ["/opt/homebrew/bin", "/usr/local/bin"] {
            assert!(
                path.split(SEP).any(|e| e == needle),
                "extended PATH missing {needle}: {path}"
            );
        }
        #[cfg(target_os = "windows")]
        if env::var("USERPROFILE").is_ok_and(|p| Path::new(&p).is_absolute()) {
            for needle in [r"\go\bin", r"\.cargo\bin", r"\.local\bin"] {
                assert!(
                    path.contains(needle),
                    "extended PATH missing {needle}: {path}"
                );
            }
        }
    }
}
