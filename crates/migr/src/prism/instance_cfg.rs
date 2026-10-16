use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use super::playtime::Playtime;
use crate::utils::fs::write_atomic;

const GENERAL: &str = "[General]";
const TOTAL_TIME_PLAYED: &str = "totalTimePlayed";
const OVERRIDE_COMMANDS: &str = "OverrideCommands";
const PRE_LAUNCH_COMMAND: &str = "PreLaunchCommand";
const POST_EXIT_COMMAND: &str = "PostExitCommand";

/// An instance's `instance.cfg`, edited line by line so Prism's other settings stay byte for byte.
pub struct InstanceCfg {
    path: PathBuf,
    lines: Vec<String>,
}

impl InstanceCfg {
    pub fn load(path: &Path) -> Result<Self> {
        let text =
            fs::read_to_string(path).with_context(|| format!("cannot read {}", path.display()))?;
        Ok(Self {
            path: path.to_owned(),
            lines: text.lines().map(str::to_owned).collect(),
        })
    }

    pub fn save(&self) -> Result<()> {
        let mut text = self.lines.join("\n");
        text.push('\n');
        write_atomic(&self.path, &text)
    }

    pub fn playtime(&self) -> Playtime {
        let seconds = self
            .get(TOTAL_TIME_PLAYED)
            .and_then(|value| value.parse().ok());
        Playtime::from_seconds(seconds.unwrap_or(0))
    }

    pub fn set_playtime(&mut self, playtime: Playtime) {
        self.set(TOTAL_TIME_PLAYED, &playtime.seconds().to_string());
    }

    pub fn hook_commands(&self) -> HookCommands {
        HookCommands {
            overridden: self.get(OVERRIDE_COMMANDS).as_deref() == Some("true"),
            pre_launch: self.get(PRE_LAUNCH_COMMAND).unwrap_or_default(),
            post_exit: self.get(POST_EXIT_COMMAND).unwrap_or_default(),
        }
    }

    pub fn set_hook_commands(&mut self, commands: &HookCommands) {
        self.set(
            OVERRIDE_COMMANDS,
            if commands.overridden { "true" } else { "false" },
        );
        self.set(PRE_LAUNCH_COMMAND, &commands.pre_launch);
        self.set(POST_EXIT_COMMAND, &commands.post_exit);
    }

    fn get(&self, key: &str) -> Option<String> {
        self.find(key)
            .and_then(|index| self.lines[index].split_once('='))
            .map(|(_, value)| unescape(value.trim()))
    }

    fn set(&mut self, key: &str, value: &str) {
        let line = format!("{key}={}", escape(value));
        match self.find(key) {
            Some(index) => self.lines[index] = line,
            None => {
                let index = self.general_end();
                self.lines.insert(index, line);
            }
        }
    }

    fn find(&self, key: &str) -> Option<usize> {
        self.general_lines().find(|&index| {
            self.lines[index]
                .split_once('=')
                .is_some_and(|(name, _)| name.trim() == key)
        })
    }

    /// Where a new key goes: after the last `[General]` line, or a fresh section when there is none.
    fn general_end(&mut self) -> usize {
        match self.general_lines().last() {
            Some(index) => index + 1,
            None => match self.lines.iter().position(|line| line.trim() == GENERAL) {
                Some(header) => header + 1,
                None => {
                    self.lines.insert(0, GENERAL.to_owned());
                    1
                }
            },
        }
    }

    fn general_lines(&self) -> impl Iterator<Item = usize> + '_ {
        let mut in_general = false;
        self.lines
            .iter()
            .enumerate()
            .filter_map(move |(index, line)| {
                let line = line.trim();
                if line.starts_with('[') {
                    in_general = line == GENERAL;
                    return None;
                }
                (in_general && !line.is_empty()).then_some(index)
            })
    }
}

/// Prism's custom commands of one instance.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookCommands {
    pub overridden: bool,
    pub pre_launch: String,
    pub post_exit: String,
}

impl HookCommands {
    /// Prism runs commands without a shell and with the launcher's PATH, so the binary goes in by absolute path.
    pub fn migr(executable: &Path) -> Self {
        let executable = executable.display().to_string();
        let executable = if executable.contains(' ') {
            format!("\"{executable}\"")
        } else {
            executable
        };
        Self {
            overridden: true,
            pre_launch: format!("{executable} hook pre"),
            post_exit: format!("{executable} hook post"),
        }
    }

    pub fn is_migr(&self) -> bool {
        self.overridden
            && self.pre_launch.ends_with(" hook pre")
            && self.post_exit.ends_with(" hook post")
    }
}

// QSettings quotes values with characters that would otherwise change their meaning.
fn escape(value: &str) -> String {
    let needs_quotes = value != value.trim() || value.contains(['"', '\\', ';', ',', '=']);
    if !needs_quotes {
        return value.to_owned();
    }
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn unescape(value: &str) -> String {
    match value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
    {
        Some(inner) => inner.replace("\\\"", "\"").replace("\\\\", "\\"),
        None => value.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(text: &str) -> InstanceCfg {
        InstanceCfg {
            path: PathBuf::new(),
            lines: text.lines().map(str::to_owned).collect(),
        }
    }

    #[test]
    fn edits_only_general_keys() {
        let mut cfg = cfg(
            "[General]\nOverrideCommands=false\nPreLaunchCommand=\ntotalTimePlayed=56826\n\n[UI]\nPostExitCommand=keep\n",
        );
        assert_eq!(cfg.playtime().seconds(), 56826);

        cfg.set_hook_commands(&HookCommands::migr(Path::new("/opt/homebrew/bin/migr")));
        cfg.set_playtime(Playtime::from_seconds(60));

        assert_eq!(
            cfg.lines,
            [
                "[General]",
                "OverrideCommands=true",
                "PreLaunchCommand=/opt/homebrew/bin/migr hook pre",
                "totalTimePlayed=60",
                "PostExitCommand=/opt/homebrew/bin/migr hook post",
                "",
                "[UI]",
                "PostExitCommand=keep",
            ]
        );
    }

    #[test]
    fn quotes_round_trip() {
        let commands = HookCommands::migr(Path::new("/Apps/my tools/migr"));
        let mut cfg = cfg("");
        cfg.set_hook_commands(&commands);
        assert_eq!(cfg.lines[0], "[General]");
        assert_eq!(
            cfg.lines[2],
            r#"PreLaunchCommand="\"/Apps/my tools/migr\" hook pre""#
        );
        assert_eq!(cfg.hook_commands(), commands);
    }
}
