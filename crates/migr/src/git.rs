use std::{
    env,
    ffi::OsStr,
    path::{Path, PathBuf},
    process::{Command, ExitStatus, Output},
};

use anyhow::{Context, Result, bail};

/// The `git` binary pointed at migr's repository, optionally with an instance as its work tree.
#[derive(Clone)]
pub struct Git {
    git_dir: PathBuf,
    work_tree: Option<(PathBuf, PathBuf)>,
}

impl Git {
    pub fn new(git_dir: PathBuf) -> Self {
        Self {
            git_dir,
            work_tree: None,
        }
    }

    /// Each instance has its own index, so switching instances never leaves git with a stale view.
    pub fn with_work_tree(&self, work_tree: &Path, index: &Path) -> Self {
        Self {
            git_dir: self.git_dir.clone(),
            work_tree: Some((work_tree.to_owned(), index.to_owned())),
        }
    }

    /// Runs git and returns its trimmed stdout, failing with git's stderr.
    pub fn run<I, S>(&self, args: I) -> Result<String>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.output(args)?;
        if !output.status.success() {
            bail!("{}", failure(&output));
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_owned())
    }

    /// Runs a git command that answers yes or no through exit codes 0 and 1.
    pub fn check<I, S>(&self, args: I) -> Result<bool>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let output = self.output(args)?;
        match output.status.code() {
            Some(0) => Ok(true),
            Some(1) => Ok(false),
            _ => bail!("{}", failure(&output)),
        }
    }

    pub fn output<I, S>(&self, args: I) -> Result<Output>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = self.command();
        // Instance paths are names, never patterns.
        command.env("GIT_LITERAL_PATHSPECS", "1").args(args);
        log::debug!("{command:?}");
        let output = command
            .output()
            .context("cannot run git; is it installed?")?;
        log::debug!("git exited with {}", output.status);
        Ok(output)
    }

    /// Runs git attached to the terminal, for people resolving things by hand.
    pub fn interactive<I, S>(&self, args: I) -> Result<ExitStatus>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<OsStr>,
    {
        let mut command = self.command();
        command.args(args);
        log::debug!("{command:?}");
        command.status().context("cannot run git; is it installed?")
    }

    fn command(&self) -> Command {
        let mut command = Command::new("git");
        command
            .env("GIT_DIR", &self.git_dir)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env_remove("GIT_WORK_TREE")
            .env_remove("GIT_INDEX_FILE")
            .args(["-c", "core.quotePath=false"]);
        // Hooks run with nobody watching, so a dead network must fail fast instead of hanging the launch.
        if env::var_os("GIT_SSH_COMMAND").is_none() {
            command.env("GIT_SSH_COMMAND", "ssh -o ConnectTimeout=10");
        }
        match &self.work_tree {
            Some((work_tree, index)) => {
                command
                    .env("GIT_WORK_TREE", work_tree)
                    .env("GIT_INDEX_FILE", index)
                    .current_dir(work_tree);
            }
            None => {
                command.current_dir(&self.git_dir);
            }
        }
        command
    }
}

fn failure(output: &Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let message = [stderr.trim(), stdout.trim()]
        .into_iter()
        .find(|text| !text.is_empty())
        .unwrap_or("no output");
    format!("git failed ({}): {message}", output.status)
}
