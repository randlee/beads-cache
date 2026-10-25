//! The extended `PATH` used by every CLI spawn.
//!
//! The inherited `PATH` comes first, so the user's own `PATH` decides which
//! `bd` runs (contract §2). GUI launches (Finder/Dock) start with a minimal
//! `PATH`, so the per-OS install directories of Homebrew, Go and Cargo follow
//! it as a fallback (`;`-separated on Windows, `:` elsewhere). Only absolute
//! directories are kept, each once (lead rulings S76 and S77). Reads `PATH`,
//! `HOME`, `GOPATH`, `USERPROFILE` and `LOCALAPPDATA` from the environment on
//! every call.
//!
//! The inherited `PATH` is read as an `OsString` and split with
//! [`std::env::split_paths`], so a `PATH` with one segment that is not UTF-8
//! keeps its other segments (it was dropped whole before). The spawns of this
//! crate pass the child the `OsString` form, which keeps such a segment too;
//! [`get_extended_path`], whose type is `String`, leaves it out.
//! [`absolute_path_entries`] is the S76/S77 rule on its own, for another
//! crate's own `PATH` search (`bd-cache-sync`'s `atm` lookup, RSH-003).

use std::collections::HashSet;
use std::env;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

/// The platform `PATH` separator.
const SEP: char = if cfg!(windows) { ';' } else { ':' };

/// The absolute segments of the inherited `PATH`, then the platform install
/// directories that are not already in it.
///
/// A segment that is not UTF-8 is left out of this `String`; the crate's own
/// spawns use the `OsString` form, which keeps it.
#[must_use]
pub fn get_extended_path() -> String {
    let joined = extended_path_os();
    match joined.into_string() {
        Ok(text) => text,
        Err(lossy) => {
            let kept: Vec<String> = env::split_paths(&lossy)
                .filter_map(|p| p.into_os_string().into_string().ok())
                .collect();
            kept.join(&SEP.to_string())
        }
    }
}

/// [`get_extended_path`] as an `OsString`, keeping a segment that is not
/// UTF-8: the `PATH` every spawn of this crate gives its child.
pub(crate) fn extended_path_os() -> OsString {
    let inherited = env::var_os("PATH").unwrap_or_default();
    join_extended_os(&inherited, fallback_dirs_from_env(), SEP)
}

/// The absolute directories of a `PATH` value, in order: an empty or relative
/// segment is dropped (lead rulings S76 and S77).
///
/// An empty segment, `.` or `bin` would be resolved against the working
/// directory, where a program committed to a repository could be picked up
/// instead of the installed one. Use this for any search of `PATH` for a
/// program to run.
///
/// # Examples
///
/// ```
/// use std::ffi::OsStr;
/// use sc_bd_cli::path::absolute_path_entries;
/// # #[cfg(unix)] {
/// let dirs = absolute_path_entries(OsStr::new(":.:/usr/bin:bin:/bin"));
/// assert_eq!(dirs, ["/usr/bin", "/bin"].map(std::path::PathBuf::from));
/// # }
/// ```
#[must_use]
pub fn absolute_path_entries(path: &OsStr) -> Vec<PathBuf> {
    env::split_paths(path)
        .filter(|dir| !dir.as_os_str().is_empty() && dir.is_absolute())
        .collect()
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
#[cfg(test)]
fn join_extended(inherited: &str, fallback: Vec<String>, sep: char) -> String {
    let segments = inherited.split(sep).map(OsString::from);
    join_segments(segments, fallback, sep)
        .to_string_lossy()
        .into_owned()
}

/// [`join_extended`] over an `OsStr` `PATH`, split with the platform's own
/// rule ([`env::split_paths`]), so a segment that is not UTF-8 is kept.
fn join_extended_os(inherited: &OsStr, fallback: Vec<String>, sep: char) -> OsString {
    let segments = env::split_paths(inherited).map(PathBuf::into_os_string);
    join_segments(segments, fallback, sep)
}

/// The absolute `segments`, then the absolute `fallback` directories, each
/// once, joined with `sep`. A segment that holds `sep` itself (a fallback
/// built from a `HOME` with a `:` in it) cannot be one entry of the result
/// and is dropped too.
fn join_segments(
    segments: impl Iterator<Item = OsString>,
    fallback: Vec<String>,
    sep: char,
) -> OsString {
    let mut seen = HashSet::new();
    let mut joined = OsString::new();
    let sep_text = sep.to_string();
    let candidates = segments.chain(fallback.into_iter().map(OsString::from));
    for segment in candidates {
        let usable = !segment.is_empty()
            && Path::new(&segment).is_absolute()
            && !segment.to_string_lossy().contains(sep);
        if usable && seen.insert(segment.clone()) {
            if !joined.is_empty() {
                joined.push(&sep_text);
            }
            joined.push(&segment);
        }
    }
    joined
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

    /// A `PATH` segment that is not UTF-8 keeps the others, and is kept itself
    /// in the child's `PATH` (it used to empty the whole inherited `PATH`).
    #[cfg(unix)]
    #[test]
    fn a_non_utf8_segment_keeps_the_rest_of_the_path() {
        use std::os::unix::ffi::{OsStrExt as _, OsStringExt as _};
        let inherited = OsStr::from_bytes(b"/a:/caf\xe9:.:/b:");
        let joined = join_extended_os(inherited, vec!["/c".to_owned()], ':');
        assert_eq!(joined.into_vec(), b"/a:/caf\xe9:/b:/c".to_vec());
    }

    /// The S76/S77 rule on its own: only absolute segments, in order.
    #[test]
    fn absolute_path_entries_drop_empty_and_relative_segments() {
        let (a, b) = (abs("a"), abs("b"));
        let path = env::join_paths(["", ".", a.as_str(), "bin", "./x", b.as_str(), ""])
            .unwrap_or_default();
        assert_eq!(
            absolute_path_entries(&path),
            [PathBuf::from(&a), PathBuf::from(&b)]
        );
        assert!(absolute_path_entries(OsStr::new("")).is_empty());
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
