//! [`new_command`]: the platform-adjusted `std::process::Command` every spawn starts from,
//! the crate-private `ChildEnv`, the environment changes a [`CliRunner`](crate::CliRunner)
//! applies to every child it spawns (ADR-BDCLI-0004), and the crate-private
//! `lead_process_group`, which makes a bounded child the leader of its own process
//! group so a timeout can kill everything it started (PE-RSH-001).

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

/// Windows `CREATE_NO_WINDOW`: no console window for the child (as [`new_command`]).
#[cfg(target_os = "windows")]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Windows `CREATE_NEW_PROCESS_GROUP`: the child is the root of a new process group.
#[cfg(target_os = "windows")]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

/// Makes the child `cmd` starts the leader of a new process group (PE-RSH-001).
///
/// On Unix the group id is the child's pid (`process_group(0)`), so a kill of the
/// negative pid reaches every process the child started that stayed in its group
/// (a `dolt` that `bd` spawned). On Windows the child gets
/// `CREATE_NEW_PROCESS_GROUP` (with [`new_command`]'s `CREATE_NO_WINDOW` kept); the
/// tree is killed there with `taskkill /T`, which follows parent ids.
///
/// Only the deadline-bounded spawns use it: they never read the terminal (standard
/// input is closed, the outputs are pipes), so leaving the terminal's foreground
/// group changes nothing for them, while btit's unbounded calls keep btit's
/// behaviour (Ctrl-C reaches them with the app).
pub(crate) fn lead_process_group(cmd: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt as _;
        cmd.process_group(0);
    }
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt as _;
        cmd.creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP);
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        let _ = cmd;
    }
}

/// Environment variables a runner sets on, or removes from, every child it spawns
/// (ADR-BDCLI-0004): a composition that resolved its paths from an explicit value, not
/// the process environment, gives the child that same value.
///
/// Applied right after [`new_command`], so the spawn's own `PATH` and `BEADS_PATH`,
/// set later, still win for those two names. One entry per name: a later
/// [`set`](Self::set) of a name replaces the earlier one.
///
/// What "one name" is (lead ruling S50, RSH-040, QA10-002): `ChildEnv` compares
/// names exactly on Unix and ASCII case-insensitively on Windows (`Path` and
/// `PATH` are one variable there; QA9-002). Names are ASCII (`set` asserts it in
/// debug builds), so that is the whole of Windows' rule for them. Whatever this
/// dedup leaves, `std::process::Command` applies the platform's full rule when it
/// builds the child's environment, and its last change of a variable wins, as
/// the last `set` does here.
#[derive(Debug, Clone, Default)]
pub(crate) struct ChildEnv {
    vars: Vec<(OsString, Option<OsString>)>,
}

impl ChildEnv {
    /// Sets `key` to `value` in every child (`Some`), or removes it (`None`).
    ///
    /// `key` is an ASCII name (asserted in debug builds; lead ruling S50): the
    /// dedup's Windows comparison folds ASCII case only.
    pub(crate) fn set(&mut self, key: &OsStr, value: Option<&OsStr>) {
        debug_assert!(
            key.is_ascii(),
            "child variable names are ASCII: {}",
            key.display()
        );
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

/// Whether `a` and `b` name one environment variable in [`ChildEnv`]'s dedup:
/// without regard to ASCII case on Windows, exactly on Unix. For the ASCII names
/// [`ChildEnv::set`] takes, that is the platform's rule (lead ruling S50).
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

    /// A non-ASCII name is a programming error, caught in debug builds (lead
    /// ruling S50, RSH-040).
    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "child variable names are ASCII")]
    fn a_non_ascii_name_is_refused_in_debug_builds() {
        ChildEnv::default().set(OsStr::new("SC_BD_\u{e9}"), None);
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
