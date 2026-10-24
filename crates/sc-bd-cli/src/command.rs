//! [`new_command`]: the platform-adjusted `std::process::Command` every spawn starts from,
//! and the crate-private `ChildEnv`, the environment changes a [`CliRunner`](crate::CliRunner)
//! applies to every child it spawns (ADR-BDCLI-0004).

use std::ffi::{OsStr, OsString};
use std::process::Command;

/// Creates a Command with platform-specific flags.
/// On Windows, sets `CREATE_NO_WINDOW` to prevent console popups.
#[must_use]
#[cfg_attr(
    target_os = "windows",
    expect(
        clippy::unreadable_literal,
        reason = "CREATE_NO_WINDOW literal moved verbatim from btit-app cli.rs in b-4"
    )
)]
pub fn new_command(program: &str) -> Command {
    #[cfg_attr(
        not(target_os = "windows"),
        expect(
            unused_mut,
            reason = "`mut` is needed only by the Windows creation_flags call below"
        )
    )]
    let mut cmd = Command::new(program);
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }
    cmd
}

/// Environment variables a runner sets on, or removes from, every child it spawns
/// (ADR-BDCLI-0004): a composition that resolved its paths from an explicit value, not
/// the process environment, gives the child that same value.
///
/// Applied right after [`new_command`], so the spawn's own `PATH` and `BEADS_PATH`,
/// set later, still win for those two names. One entry per name: a later
/// [`set`](Self::set) of a name replaces the earlier one. Names compare as the
/// platform's environment does: exactly on Unix, ASCII case-insensitively on
/// Windows (`Path` and `PATH` are one variable there; QA9-002).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct ChildEnv {
    vars: Vec<(OsString, Option<OsString>)>,
}

impl ChildEnv {
    /// Sets `key` to `value` in every child (`Some`), or removes it (`None`).
    pub(crate) fn set(&mut self, key: &OsStr, value: Option<&OsStr>) {
        self.vars.retain(|(k, _)| !same_name(k, key));
        self.vars
            .push((key.to_os_string(), value.map(OsStr::to_os_string)));
    }

    /// [`new_command`] for `program` with these changes applied.
    pub(crate) fn command(&self, program: &str) -> Command {
        let mut cmd = new_command(program);
        for (key, value) in &self.vars {
            match value {
                Some(value) => cmd.env(key, value),
                None => cmd.env_remove(key),
            };
        }
        cmd
    }
}

/// Whether `a` and `b` name one environment variable: Windows compares names
/// without regard to ASCII case, Unix compares them exactly.
fn same_name(a: &OsStr, b: &OsStr) -> bool {
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A name set again in another case replaces the first entry on Windows,
    /// where it is the same variable, and is a second entry on Unix (QA9-002).
    #[test]
    fn names_compare_as_the_platform_environment_does() {
        let mut env = ChildEnv::default();
        env.set(OsStr::new("SC_BD_CASE"), Some(OsStr::new("1")));
        env.set(OsStr::new("sc_bd_case"), None);
        if cfg!(windows) {
            assert_eq!(env.vars, [(OsString::from("sc_bd_case"), None)]);
        } else {
            assert_eq!(
                env.vars,
                [
                    (OsString::from("SC_BD_CASE"), Some(OsString::from("1"))),
                    (OsString::from("sc_bd_case"), None),
                ]
            );
        }
        assert!(same_name(OsStr::new("A"), OsStr::new("A")));
        assert_eq!(
            same_name(OsStr::new("Path"), OsStr::new("PATH")),
            cfg!(windows)
        );
    }

    #[test]
    fn a_later_set_of_a_name_replaces_the_earlier_one() {
        let mut env = ChildEnv::default();
        env.set(OsStr::new("A"), Some(OsStr::new("1")));
        env.set(OsStr::new("B"), Some(OsStr::new("2")));
        env.set(OsStr::new("A"), None);
        assert_eq!(
            env.vars,
            [
                (OsString::from("B"), Some(OsString::from("2"))),
                (OsString::from("A"), None),
            ]
        );
        let cmd = env.command("prog");
        let envs: Vec<_> = cmd.get_envs().collect();
        assert_eq!(
            envs,
            [
                (OsStr::new("A"), None),
                (OsStr::new("B"), Some(OsStr::new("2"))),
            ]
        );
    }
}
