//! The argv grammar: one table of commands, the clap tree built from it, and
//! the pre-scan of the words that decide `--help`, `--version` and the `/v0`
//! operations the cache does not serve (contract §2, §3, §8).
//!
//! Every command of the contract is one [`CommandSpec`]: its path, its
//! target (a `bd-cache-api` [`Command`] or `cache tick`) and its arguments.
//! Each [`ArgSpec`] names the request member it fills, so the clap tree, the
//! request built from argv ([`crate::request`]) and the help ([`crate::help`])
//! all come from the same row.
//!
//! Long options must be spelled out in full: clap's `infer_long_args` stays
//! off, so `--scop` is an unknown argument (`USAGE`, with `did_you_mean`).

use clap::{Arg, ArgAction, ArgGroup, Command as Clap};

use bd_cache_api::Command;

use crate::NAME;
use crate::commands::{beads, cron, manage, schedule, scopes, sync, tick};

/// What a command runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Target {
    /// A `bd-cache-api` command: decode, then execute.
    Api(Command),
    /// A phase-B verb `bd-cache-api` decodes and this crate runs over
    /// `bd-cache-sync` (ADR-BDC-0015): `cache schedule *`, `cron run`,
    /// `tick`, `sync *`.
    Sync(Command),
}

/// How many values an argument takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Shape {
    /// One value.
    One,
    /// One value per occurrence; the flag may repeat.
    Many,
    /// One or more values per occurrence (`--issue-id A B`); the flag may repeat.
    List,
    /// A boolean switch.
    Switch,
}

/// How a value becomes JSON in the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Ty {
    /// A JSON string.
    Str,
    /// A JSON integer when the text is one; otherwise the text, so the
    /// decoder reports the member (`BAD_ARGS`), as for every front end.
    Int,
    /// Parsed as JSON; invalid JSON is `BAD_ARGS` (contract §3).
    Json,
}

/// One argument of a command.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ArgSpec {
    /// The clap id, and the `name` in the help.
    pub(crate) id: &'static str,
    /// The long flag without `--`; `None` for a positional argument.
    pub(crate) long: Option<&'static str>,
    /// The value's placeholder in the synopsis.
    pub(crate) value_name: &'static str,
    /// How many values it takes.
    pub(crate) shape: Shape,
    /// How a value becomes JSON.
    pub(crate) ty: Ty,
    /// Required on the command line (unless `--input` gives the request).
    pub(crate) required: bool,
    /// The request member it fills: a dotted path (`patch.status`).
    pub(crate) member: &'static str,
    /// One line for the help.
    pub(crate) help: &'static str,
}

impl ArgSpec {
    /// A positional argument.
    pub(crate) const fn positional(
        id: &'static str,
        shape: Shape,
        ty: Ty,
        required: bool,
        member: &'static str,
        help: &'static str,
    ) -> Self {
        Self {
            id,
            long: None,
            value_name: id,
            shape,
            ty,
            required,
            member,
            help,
        }
    }

    /// A long option; its clap id is the flag's name.
    pub(crate) const fn flag(
        long: &'static str,
        value_name: &'static str,
        shape: Shape,
        ty: Ty,
        required: bool,
        member: &'static str,
        help: &'static str,
    ) -> Self {
        Self {
            id: long,
            long: Some(long),
            value_name,
            shape,
            ty,
            required,
            member,
            help,
        }
    }

    /// A boolean switch.
    pub(crate) const fn switch(
        long: &'static str,
        member: &'static str,
        help: &'static str,
    ) -> Self {
        Self {
            id: long,
            long: Some(long),
            value_name: "",
            shape: Shape::Switch,
            ty: Ty::Str,
            required: false,
            member,
            help,
        }
    }

    /// `--flag` as the help and the synopsis spell it, or `<id>`.
    pub(crate) fn spelled(&self) -> String {
        match self.long {
            Some(long) => format!("--{long}"),
            None => format!("<{}>", self.id),
        }
    }

    /// The argument in a synopsis: `<id>`, `[--flag N]`, `[<ids>...]`.
    pub(crate) fn synopsis(&self) -> String {
        let body = match (self.long, self.shape) {
            (Some(long), Shape::Switch) => format!("--{long}"),
            (Some(long), Shape::One) => format!("--{long} {}", self.value_name),
            (Some(long), Shape::Many | Shape::List) => {
                format!("--{long} {}...", self.value_name)
            }
            (None, Shape::Many | Shape::List) => format!("<{}>...", self.id),
            (None, _) => format!("<{}>", self.id),
        };
        if self.required {
            body
        } else {
            format!("[{body}]")
        }
    }
}

/// One command of the contract.
#[derive(Debug, Clone, Copy)]
pub(crate) struct CommandSpec {
    /// The command path, words joined by one space.
    pub(crate) path: &'static str,
    /// One line for the help.
    pub(crate) about: &'static str,
    /// What it runs.
    pub(crate) target: Target,
    /// Its arguments.
    pub(crate) args: &'static [ArgSpec],
    /// Arguments of which exactly one must be given.
    pub(crate) one_of: Option<&'static [&'static str]>,
}

/// Every command, surface (a) first, then surface (b), then `cache tick`.
pub(crate) fn commands() -> impl Iterator<Item = &'static CommandSpec> {
    beads::COMMANDS
        .iter()
        .chain(manage::COMMANDS)
        .chain(scopes::COMMANDS)
        .chain(schedule::COMMANDS)
        .chain(cron::COMMANDS)
        .chain(std::iter::once(&tick::COMMAND))
        .chain(sync::COMMANDS)
}

/// The command with this path.
pub(crate) fn command(path: &str) -> Option<&'static CommandSpec> {
    commands().find(|c| c.path == path)
}

/// The global options (contract §2), accepted before or after the subcommand.
pub(crate) const GLOBALS: &[GlobalSpec] = &[
    GlobalSpec {
        id: "repo",
        flag: "--repo",
        value_name: Some("DIR"),
        help: "The repository root (env BD_CACHE__REPO; default: the working directory).",
    },
    GlobalSpec {
        id: "cache-path",
        flag: "--cache-path",
        value_name: Some("PATH"),
        help: "The cache file, relative to --repo (env BD_CACHE__CACHE_PATH; TOML cache_path).",
    },
    GlobalSpec {
        id: "bd",
        flag: "--bd",
        value_name: Some("PATH"),
        help: "The bd binary (env BD_CACHE__BD; never read from TOML; default: bd on PATH).",
    },
    GlobalSpec {
        id: "format",
        flag: "--format",
        value_name: Some("json|text"),
        help: "The output format, from this flag only (default json).",
    },
    GlobalSpec {
        id: "input",
        flag: "--input",
        value_name: Some("FILE|-"),
        help: "The whole request as JSON; combining it with request flags is USAGE.",
    },
    GlobalSpec {
        id: "verbose",
        flag: "-v",
        value_name: None,
        help: "More stderr logging (repeat for more); wins over BD_CACHE__LOG.",
    },
    GlobalSpec {
        id: "quiet",
        flag: "-q",
        value_name: None,
        help: "Only errors in the stderr log; wins over BD_CACHE__LOG.",
    },
    GlobalSpec {
        id: "version",
        flag: "--version",
        value_name: None,
        help: "Print {name, version, contract_version}.",
    },
    GlobalSpec {
        id: "help",
        flag: "--help",
        value_name: None,
        help: "Print the help: text on a terminal, else the envelope.",
    },
];

/// One global option, for the clap tree and the help.
#[derive(Debug, Clone, Copy)]
pub(crate) struct GlobalSpec {
    /// The clap id.
    pub(crate) id: &'static str,
    /// The flag as typed.
    pub(crate) flag: &'static str,
    /// The value's placeholder; `None` for a switch.
    pub(crate) value_name: Option<&'static str>,
    /// One line for the help.
    pub(crate) help: &'static str,
}

/// The global options that take a value (`--repo DIR`, `--format json`).
fn takes_value(token: &str) -> bool {
    GLOBALS
        .iter()
        .any(|g| g.value_name.is_some() && g.flag == token)
}

/// The leading command words of `argv`: the tokens before the first option
/// that is not a global one, skipping global options and their values.
///
/// `bdc --repo r cache journal get 3` gives `[cache, journal, get, 3]`; the
/// caller matches as many words as name a command.
pub(crate) fn words(argv: &[String]) -> Vec<&str> {
    let mut out = Vec::new();
    let mut it = argv.iter().map(String::as_str);
    while let Some(token) = it.next() {
        if token == "--" {
            break;
        }
        if takes_value(token) {
            it.next();
            continue;
        }
        if token.starts_with('-') {
            if GLOBALS
                .iter()
                .any(|g| g.flag == token || token.starts_with(&format!("{}=", g.flag)))
                || is_short_cluster(token)
            {
                continue;
            }
            break;
        }
        out.push(token);
    }
    out
}

/// `-v`, `-vv`, `-q`: the short global switches, alone or clustered.
fn is_short_cluster(token: &str) -> bool {
    token
        .strip_prefix('-')
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c == 'v' || c == 'q'))
}

/// True when `flag` (`--help`, `--version`, `--input`) appears before any `--`.
pub(crate) fn has_flag(argv: &[String], flag: &str) -> bool {
    argv.iter()
        .take_while(|t| t.as_str() != "--")
        .any(|t| t == flag || t.starts_with(&format!("{flag}=")))
}

/// The value of `--format`, when given: `--format text` or `--format=text`.
pub(crate) fn format_flag(argv: &[String]) -> Option<&str> {
    let mut it = argv.iter().take_while(|t| t.as_str() != "--");
    while let Some(t) = it.next() {
        if t == "--format" {
            return it.next().map(String::as_str);
        }
        if let Some(v) = t.strip_prefix("--format=") {
            return Some(v);
        }
    }
    None
}

/// The command groups and commands under `prefix` (`""` is the root), in table
/// order, each once: `cache` under the root, `journal` under `cache`.
pub(crate) fn children(prefix: &str) -> Vec<&'static str> {
    let mut out: Vec<&'static str> = Vec::new();
    for c in commands() {
        let rest = if prefix.is_empty() {
            Some(c.path)
        } else {
            c.path
                .strip_prefix(prefix)
                .and_then(|r| r.strip_prefix(' '))
        };
        if let Some(word) = rest.and_then(|r| r.split(' ').next())
            && !out.contains(&word)
        {
            out.push(word);
        }
    }
    out
}

/// The clap tree. `relaxed` (set when `--input` gives the request) drops every
/// `required`, so the request can come whole from the file.
pub(crate) fn clap_tree(relaxed: bool) -> Clap {
    let mut root = Clap::new(NAME)
        .disable_help_flag(true)
        .disable_version_flag(true)
        .disable_help_subcommand(true);
    for g in GLOBALS {
        root = root.arg(global_arg(g));
    }
    group(root, "", relaxed)
}

fn global_arg(g: &GlobalSpec) -> Arg {
    let arg = Arg::new(g.id).global(true);
    let arg = match g.flag.strip_prefix("--") {
        Some(long) => arg.long(long),
        None => arg.short(g.flag.chars().nth(1).unwrap_or('v')),
    };
    match (g.id, g.value_name) {
        ("verbose", _) => arg.action(ArgAction::Count).conflicts_with("quiet"),
        ("format", Some(v)) => arg.value_name(v).num_args(1).value_parser(["json", "text"]),
        (_, Some(v)) => arg.value_name(v).num_args(1),
        (_, None) => arg.action(ArgAction::SetTrue),
    }
}

/// The subcommands under `prefix`, recursively.
fn group(mut parent: Clap, prefix: &str, relaxed: bool) -> Clap {
    for word in children(prefix) {
        let path = if prefix.is_empty() {
            word.to_owned()
        } else {
            format!("{prefix} {word}")
        };
        let sub = match command(&path) {
            // A command that is also a group (`cache scopes` and `cache
            // scopes list` …): its own arguments, and its subcommands.
            Some(spec) if !children(&path).is_empty() => group(
                leaf(spec, relaxed)
                    .subcommand_required(false)
                    .args_conflicts_with_subcommands(true)
                    .disable_help_subcommand(true),
                &path,
                relaxed,
            ),
            Some(spec) => leaf(spec, relaxed),
            None => group(
                Clap::new(word)
                    .subcommand_required(true)
                    .disable_help_subcommand(true),
                &path,
                relaxed,
            ),
        };
        parent = parent.subcommand(sub);
    }
    parent
}

fn leaf(spec: &CommandSpec, relaxed: bool) -> Clap {
    let word = spec.path.rsplit(' ').next().unwrap_or(spec.path);
    let mut cmd = Clap::new(word);
    for a in spec.args {
        let mut arg = Arg::new(a.id).required(a.required && !relaxed);
        if let Some(long) = a.long {
            arg = arg.long(long);
        }
        arg = match a.shape {
            Shape::Switch => arg.action(ArgAction::SetTrue),
            Shape::One => arg.value_name(a.value_name).num_args(1),
            Shape::Many if a.long.is_some() => arg
                .value_name(a.value_name)
                .num_args(1)
                .action(ArgAction::Append),
            Shape::Many | Shape::List => arg
                .value_name(a.value_name)
                .num_args(1..)
                .action(ArgAction::Append),
        };
        cmd = cmd.arg(arg);
    }
    // clap ids are static here (no `string` feature): one group per command.
    if let Some(ids) = spec.one_of {
        cmd = cmd.group(
            ArgGroup::new("one-of")
                .args(ids.iter().copied())
                .multiple(false)
                .required(!relaxed),
        );
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    fn argv(words: &[&str]) -> Vec<String> {
        words.iter().map(|w| (*w).to_owned()).collect()
    }

    #[test]
    fn words_skip_global_options_and_their_values() {
        let a = argv(&[
            "--repo",
            "r",
            "-vv",
            "cache",
            "--format=text",
            "journal",
            "get",
            "3",
            "--bd",
            "b",
        ]);
        assert_eq!(words(&a), ["cache", "journal", "get", "3"]);
        assert_eq!(
            words(&argv(&["issues", "list", "--scope", "x"])),
            ["issues", "list"]
        );
    }

    #[test]
    fn every_command_path_is_reachable_in_the_tree() {
        let tree = clap_tree(false);
        for c in commands() {
            let mut node = &tree;
            for w in c.path.split(' ') {
                node = node
                    .find_subcommand(w)
                    .unwrap_or_else(|| unreachable_path(c.path));
            }
            let own = node.get_arguments().filter(|a| !a.is_global_set()).count();
            assert_eq!(own, c.args.len(), "{}", c.path);
        }
        tree.debug_assert();
    }

    #[expect(clippy::panic, reason = "test helper")]
    fn unreachable_path(path: &str) -> &'static Clap {
        panic!("{path} is not in the tree")
    }

    #[test]
    fn children_are_listed_once_in_table_order() {
        assert_eq!(
            children(""),
            ["issues", "ready", "dependencies", "context", "cache"]
        );
        assert_eq!(children("cache refresh"), ["scopes", "issues"]);
    }
}
