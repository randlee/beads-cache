//! `--help` (contract §8): text on a terminal, otherwise the envelope.
//!
//! The envelope's `data` is `{command, synopsis, arguments: [{name, flag,
//! required, description}]}`. For a command group (`cache`, `cache journal`)
//! the one argument is `<command>`, which names the commands under it. The
//! global options close every list. Both forms come from the argv table
//! ([`crate::args`]), so the help cannot drift from the grammar.

use std::fmt::Write as _;

use serde_json::{Value, json};

use crate::args::{self, GLOBALS};

/// One row of `arguments`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct HelpArg {
    name: String,
    flag: Option<String>,
    required: bool,
    description: String,
}

/// The help of one command or group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Help {
    command: String,
    about: String,
    synopsis: String,
    arguments: Vec<HelpArg>,
}

/// The help of `path` (`""` is the whole program); `None` when `path` names
/// neither a command nor a group.
pub(crate) fn of(path: &str) -> Option<Help> {
    let display = if path.is_empty() {
        "bdc".to_owned()
    } else {
        format!("bdc {path}")
    };
    let (about, synopsis, mut arguments) = if let Some(spec) = args::command(path) {
        let mut synopsis = display.clone();
        for a in spec.args {
            synopsis.push(' ');
            synopsis.push_str(&a.synopsis());
        }
        let arguments = spec
            .args
            .iter()
            .map(|a| HelpArg {
                name: a.id.to_owned(),
                flag: a.long.map(|l| format!("--{l}")),
                required: a.required,
                description: a.help.to_owned(),
            })
            .collect();
        (spec.about.to_owned(), synopsis, arguments)
    } else {
        let children = args::children(path);
        if children.is_empty() {
            return None;
        }
        let about = if path.is_empty() {
            "The bd-cache CLI: the beads commands (/v0 vocabulary) and the cache commands."
                .to_owned()
        } else {
            format!("The {path} commands.")
        };
        let arguments = vec![HelpArg {
            name: "command".to_owned(),
            flag: None,
            required: true,
            description: format!("One of: {}.", children.join(", ")),
        }];
        (about, format!("{display} <command>"), arguments)
    };
    arguments.extend(GLOBALS.iter().map(|g| HelpArg {
        name: g.id.to_owned(),
        flag: Some(g.flag.to_owned()),
        required: false,
        description: g.help.to_owned(),
    }));
    Some(Help {
        command: path.to_owned(),
        about,
        synopsis,
        arguments,
    })
}

impl Help {
    /// The envelope's `data`: `{command, synopsis, arguments}`.
    #[must_use]
    pub(crate) fn data(&self) -> Value {
        let arguments: Vec<Value> = self
            .arguments
            .iter()
            .map(|a| {
                json!({
                    "name": a.name,
                    "flag": a.flag,
                    "required": a.required,
                    "description": a.description,
                })
            })
            .collect();
        json!({
            "command": self.command,
            "synopsis": self.synopsis,
            "arguments": arguments,
        })
    }

    /// The text form, for a terminal.
    #[must_use]
    pub(crate) fn text(&self) -> String {
        let mut out = format!("{}\n\nUsage: {}\n\nArguments:\n", self.about, self.synopsis);
        let width = self
            .arguments
            .iter()
            .map(|a| spelled(a).len())
            .max()
            .unwrap_or(0);
        for a in &self.arguments {
            let required = if a.required { " (required)" } else { "" };
            let _ = writeln!(out, "  {:width$}  {}{required}", spelled(a), a.description);
        }
        out
    }
}

fn spelled(a: &HelpArg) -> String {
    a.flag.clone().unwrap_or_else(|| format!("<{}>", a.name))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::indexing_slicing, reason = "test code: JSON indexing")]

    use super::*;

    #[test]
    fn a_command_lists_its_arguments_then_the_globals() {
        let data = of("issues get").map(|h| h.data()).unwrap_or_default();
        assert_eq!(data["command"], "issues get");
        assert_eq!(
            data["synopsis"],
            "bdc issues get <id> [--miss-timeout-ms N]"
        );
        assert_eq!(
            data["arguments"][0],
            json!({"name": "id", "flag": null, "required": true, "description": "The bead."})
        );
        assert_eq!(data["arguments"][1]["flag"], "--miss-timeout-ms");
        assert_eq!(data["arguments"][2]["flag"], "--repo");
    }

    #[test]
    fn a_group_names_its_commands_and_an_unknown_path_has_none() {
        let data = of("cache refresh").map(|h| h.data()).unwrap_or_default();
        assert_eq!(
            data["arguments"][0]["description"],
            "One of: scopes, issues."
        );
        assert!(of("cache frobnicate").is_none());
        let text = of("").map(|h| h.text()).unwrap_or_default();
        assert!(text.contains("Usage: bdc <command>"), "{text}");
    }
}
