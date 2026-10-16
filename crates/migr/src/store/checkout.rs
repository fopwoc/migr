use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use super::{Branch, Manifest, Store};
use crate::git::Git;

/// A branch bound to the game folder of one instance.
pub struct Checkout<'s> {
    store: &'s Store,
    branch: Branch,
    game_dir: PathBuf,
    git: Git,
}

pub enum MergeOutcome {
    Clean,
    Conflicts(Vec<String>),
}

impl<'s> Checkout<'s> {
    pub(super) fn open(store: &'s Store, branch: &Branch, game_dir: &Path) -> Result<Self> {
        let index = store.index_path(branch);
        if let Some(parent) = index.parent() {
            fs::create_dir_all(parent)?;
        }
        let git = store.git.with_work_tree(game_dir, &index);
        // HEAD is shared by every instance, so each checkout points it at its own branch first.
        git.run(["symbolic-ref", "HEAD", &branch.head()])?;
        Ok(Self {
            store,
            branch: branch.clone(),
            game_dir: game_dir.to_owned(),
            git,
        })
    }

    pub fn git(&self) -> &Git {
        &self.git
    }

    pub fn branch(&self) -> &Branch {
        &self.branch
    }

    pub fn store(&self) -> &Store {
        self.store
    }

    /// Commits whatever changed in the tracked paths; returns whether there was anything.
    pub fn commit_changes(&self, manifest: &Manifest, message: &str) -> Result<bool> {
        self.stage(manifest)?;
        self.commit(message)
    }

    pub fn stage(&self, manifest: &Manifest) -> Result<()> {
        let paths: Vec<&str> = manifest.committed_paths().collect();
        stage(&self.git, &self.game_dir, &paths)
    }

    pub fn stage_paths(&self, paths: &[&str]) -> Result<()> {
        stage(&self.git, &self.game_dir, paths)
    }

    pub fn commit(&self, message: &str) -> Result<bool> {
        if self.git.check(["diff", "--cached", "--quiet"])? {
            return Ok(false);
        }
        self.git
            .run(["commit", "--quiet", "--no-verify", "-m", message])?;
        log::debug!("committed {} on {}", message, self.branch);
        Ok(true)
    }

    /// Commits the index as a merge of HEAD and `other`, for merges whose result is already known.
    pub fn commit_merge(&self, other: &str, message: &str) -> Result<()> {
        let tree = self.git.run(["write-tree"])?;
        let commit = self.git.run([
            "commit-tree",
            &tree,
            "-p",
            "HEAD",
            "-p",
            other,
            "-m",
            message,
        ])?;
        self.git.run(["update-ref", "HEAD", &commit]).map(drop)
    }

    /// Makes the index match HEAD without touching the files, for a game folder that already holds them.
    pub fn adopt_head(&self) -> Result<()> {
        self.git.run(["read-tree", "HEAD"]).map(drop)
    }

    /// Writes HEAD's files into the game folder, replacing what is there.
    pub fn reset_hard(&self) -> Result<()> {
        self.git.run(["reset", "--quiet", "--hard"]).map(drop)
    }

    /// Moves the branch to `target` and writes its files; only for targets that already contain HEAD.
    pub fn fast_forward(&self, target: &str) -> Result<()> {
        self.git
            .run(["reset", "--quiet", "--hard", target])
            .map(drop)
    }

    pub fn merge(&self, other: &str, message: &str) -> Result<MergeOutcome> {
        let output = self.git.output([
            "merge",
            "--quiet",
            "--no-ff",
            "--no-edit",
            "-m",
            message,
            other,
        ])?;
        if output.status.success() {
            return Ok(MergeOutcome::Clean);
        }
        let conflicts = self.conflicts()?;
        if conflicts.is_empty() {
            bail!(
                "git could not merge: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            );
        }
        Ok(MergeOutcome::Conflicts(conflicts))
    }

    pub fn conflicts(&self) -> Result<Vec<String>> {
        let listed = self.git.run(["diff", "--name-only", "--diff-filter=U"])?;
        Ok(listed.lines().map(str::to_owned).collect())
    }

    /// Files that still carry conflict markers, which would break the mod that reads them.
    pub fn files_with_markers(&self, paths: &[String]) -> Vec<String> {
        paths
            .iter()
            .filter(|path| {
                fs::read_to_string(self.game_dir.join(path)).is_ok_and(|text| {
                    text.lines()
                        .any(|line| line.starts_with("<<<<<<< ") || line.starts_with(">>>>>>> "))
                })
            })
            .cloned()
            .collect()
    }

    pub fn conclude_merge(&self) -> Result<()> {
        self.git
            .run(["commit", "--quiet", "--no-verify", "--no-edit"])
            .map(drop)
    }

    pub fn abort_merge(&self) -> Result<()> {
        self.git.run(["merge", "--abort"]).map(drop)
    }
}

/// `git add -A` for exactly these paths: present ones get added, tracked ones gone from disk get removed.
pub(super) fn stage(git: &Git, work_tree: &Path, paths: &[&str]) -> Result<()> {
    let (present, missing): (Vec<&str>, Vec<&str>) =
        paths.iter().partition(|path| work_tree.join(path).exists());
    let mut specs = present;
    if !missing.is_empty() {
        let mut args = vec!["ls-files", "--"];
        args.extend(&missing);
        let tracked = git.run(args).context("cannot list tracked files")?;
        specs.extend(missing.into_iter().filter(|path| {
            tracked
                .lines()
                .any(|line| line == *path || line.starts_with(&format!("{path}/")))
        }));
    }
    if specs.is_empty() {
        return Ok(());
    }
    let mut args = vec!["add", "--all", "--"];
    args.extend(specs);
    git.run(args).map(drop)
}
