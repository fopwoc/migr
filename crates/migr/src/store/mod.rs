mod branch;
mod checkout;
mod manifest;
mod pending;

use std::{
    fs, io,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result, bail, ensure};

pub use branch::Branch;
pub use checkout::{Checkout, MergeOutcome};
pub use manifest::{MANIFEST_FILE, Manifest, TrackedPath};
pub use pending::PendingFork;

use crate::{git::Git, utils::fs::write_atomic};

const IGNORED: [&str; 1] = [".DS_Store"];

/// migr's own git repository: one branch per instance, plus `pack/<branch>` with the pack's untouched files.
pub struct Store {
    dir: PathBuf,
    git: Git,
}

/// How a local branch stands against its copy on the remote.
#[derive(Debug, PartialEq, Eq)]
pub enum Relation {
    MissingRemote,
    Same,
    Behind,
    Ahead,
    Diverged,
}

impl Store {
    pub fn open(dir: &Path) -> Result<Self> {
        ensure!(
            dir.join("HEAD").is_file(),
            "no migr repository at {}; run `migr init` first",
            dir.display()
        );
        log::info!("repository at {}", dir.display());
        let store = Self::at(dir);
        store.ensure_excludes()?;
        Ok(store)
    }

    pub fn create(dir: &Path, remote: Option<&str>) -> Result<Self> {
        ensure!(!dir.exists(), "{} already exists", dir.display());
        if let Some(parent) = dir.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {}", parent.display()))?;
        }
        let mut command = Command::new("git");
        match remote {
            None => command.args(["init", "--quiet", "--bare", "--initial-branch=main"]),
            Some(url) => command.args(["clone", "--quiet", "--bare", url]),
        };
        let status = command
            .arg(dir)
            .status()
            .context("cannot run git; is it installed?")?;
        ensure!(status.success(), "git could not create the repository");

        let store = Self::at(dir);
        store.ensure_excludes()?;
        for (key, value) in [
            // Hooks commit with nobody at the keyboard to type a passphrase.
            ("commit.gpgsign", "false"),
            ("merge.conflictStyle", "zdiff3"),
        ] {
            store.git.run(["config", key, value])?;
        }
        if store.git.run(["config", "user.email"]).is_err() {
            store.git.run(["config", "user.name", "migr"])?;
            store.git.run(["config", "user.email", "migr@localhost"])?;
        }
        if remote.is_some() {
            // A bare clone copies branches but does not track the remote; pulls need remote-tracking refs.
            store.git.run([
                "config",
                "remote.origin.fetch",
                "+refs/heads/*:refs/remotes/origin/*",
            ])?;
            store.fetch()?;
        }
        log::info!("created repository at {}", dir.display());
        Ok(store)
    }

    /// Files the OS drops into tracked folders, kept out without relying on the user's global gitignore.
    fn ensure_excludes(&self) -> Result<()> {
        let path = self.dir.join("info/exclude");
        let current = match fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => String::new(),
            Err(error) => {
                return Err(error).with_context(|| format!("cannot read {}", path.display()));
            }
        };
        let missing: Vec<&str> = IGNORED
            .iter()
            .copied()
            .filter(|pattern| !current.lines().any(|line| line.trim() == *pattern))
            .collect();
        if missing.is_empty() {
            return Ok(());
        }
        fs::create_dir_all(self.dir.join("info"))?;
        let mut text = current;
        if !text.is_empty() && !text.ends_with('\n') {
            text.push('\n');
        }
        for pattern in missing {
            text.push_str(pattern);
            text.push('\n');
        }
        write_atomic(&path, &text)
    }

    fn at(dir: &Path) -> Self {
        Self {
            dir: dir.to_owned(),
            git: Git::new(dir.to_owned()),
        }
    }

    pub fn git(&self) -> &Git {
        &self.git
    }

    pub fn exists(&self, reference: &str) -> Result<bool> {
        self.git.check([
            "rev-parse",
            "--verify",
            "--quiet",
            &format!("{reference}^{{commit}}"),
        ])
    }

    pub fn is_ancestor(&self, ancestor: &str, descendant: &str) -> Result<bool> {
        self.git
            .check(["merge-base", "--is-ancestor", ancestor, descendant])
    }

    pub fn set_ref(&self, reference: &str, target: &str) -> Result<()> {
        self.git.run(["update-ref", reference, target]).map(drop)
    }

    pub fn delete_ref(&self, reference: &str) -> Result<()> {
        if self.exists(reference)? {
            self.git.run(["update-ref", "-d", reference])?;
        }
        Ok(())
    }

    pub fn branches(&self) -> Result<Vec<Branch>> {
        let listed = self
            .git
            .run(["for-each-ref", "--format=%(refname:strip=2)", "refs/heads"])?;
        Ok(listed.lines().filter_map(Branch::from_listed).collect())
    }

    /// Whether the branch exists here or, after a fetch, on the remote.
    pub fn knows(&self, branch: &Branch) -> Result<bool> {
        Ok(self.exists(&branch.head())? || self.exists(&branch.remote_head())?)
    }

    pub fn manifest(&self, branch: &Branch) -> Result<Manifest> {
        let text = self
            .git
            .run(["show", &format!("{}:{MANIFEST_FILE}", branch.head())])
            .with_context(|| format!("cannot read the manifest of {branch}"))?;
        Manifest::parse(&text)
    }

    /// Commits the given paths of `source` as a tree of their own, independent of any instance's index.
    ///
    /// With `base`, the commit starts from that commit's files and replaces only `paths`.
    pub fn snapshot<'p>(
        &self,
        source: &Path,
        paths: impl IntoIterator<Item = &'p str>,
        base: Option<&str>,
        parent: Option<&str>,
        message: &str,
    ) -> Result<String> {
        let index = self.dir.join("migr/snapshot.index");
        fs::create_dir_all(self.dir.join("migr"))?;
        remove_if_exists(&index)?;

        let git = self.git.with_work_tree(source, &index);
        match base {
            Some(base) => git.run(["read-tree", base])?,
            None => git.run(["read-tree", "--empty"])?,
        };
        let paths: Vec<&str> = paths.into_iter().collect();
        checkout::stage(&git, source, &paths)?;
        let tree = git.run(["write-tree"])?;
        remove_if_exists(&index)?;

        let mut args = vec!["commit-tree", tree.as_str(), "-m", message];
        if let Some(parent) = parent {
            args.extend(["-p", parent]);
        }
        self.git.run(args)
    }

    pub fn checkout(&self, branch: &Branch, game_dir: &Path) -> Result<Checkout<'_>> {
        Checkout::open(self, branch, game_dir)
    }

    /// Whether this machine ever wrote the branch into an instance, which is when its index exists.
    pub fn is_attached(&self, branch: &Branch) -> bool {
        self.index_path(branch).is_file()
    }

    /// Writes a branch into an instance for the first time, taking it from the remote when needed.
    pub fn attach(&self, branch: &Branch, game_dir: &Path) -> Result<Checkout<'_>> {
        if !self.exists(&branch.head())? {
            ensure!(
                self.exists(&branch.remote_head())?,
                "no branch {branch} here or on the remote"
            );
            self.set_ref(&branch.head(), &branch.remote_head())?;
            if self.exists(&branch.remote_pack())? {
                self.set_ref(&branch.pack(), &branch.remote_pack())?;
            }
        }
        let checkout = self.checkout(branch, game_dir)?;
        checkout.reset_hard()?;
        Ok(checkout)
    }

    pub fn detach(&self, branch: &Branch) -> Result<()> {
        remove_if_exists(&self.index_path(branch))
    }

    pub(crate) fn index_path(&self, branch: &Branch) -> PathBuf {
        self.dir.join("migr/index").join(branch.as_str())
    }

    pub fn has_remote(&self) -> Result<bool> {
        Ok(self
            .git
            .run(["remote"])?
            .lines()
            .any(|name| name == "origin"))
    }

    pub fn remote_url(&self) -> Option<String> {
        self.git.run(["remote", "get-url", "origin"]).ok()
    }

    pub fn fetch(&self) -> Result<()> {
        self.git
            .run(["fetch", "--quiet", "--prune", "origin"])
            .map(drop)
    }

    pub fn push(&self, branch: &Branch) -> Result<()> {
        let refs = [branch.head(), branch.pack()];
        let mut args = vec!["push".to_owned(), "--quiet".to_owned(), "origin".to_owned()];
        for reference in refs {
            if self.exists(&reference)? {
                args.push(format!("{reference}:{reference}"));
            }
        }
        self.git.run(args).map(drop)
    }

    pub fn relation(&self, local: &str, remote: &str) -> Result<Relation> {
        if !self.exists(remote)? {
            return Ok(Relation::MissingRemote);
        }
        let local_id = self.git.run(["rev-parse", local])?;
        let remote_id = self.git.run(["rev-parse", remote])?;
        Ok(if local_id == remote_id {
            Relation::Same
        } else if self.is_ancestor(local, remote)? {
            Relation::Behind
        } else if self.is_ancestor(remote, local)? {
            Relation::Ahead
        } else {
            Relation::Diverged
        })
    }

    pub fn pending(&self) -> Result<Option<PendingFork>> {
        match fs::read_to_string(self.pending_path()) {
            Ok(text) => Ok(Some(
                toml::from_str(&text).context("cannot parse the pending fork")?,
            )),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error).context("cannot read the pending fork"),
        }
    }

    pub fn save_pending(&self, pending: &PendingFork) -> Result<()> {
        write_atomic(&self.pending_path(), &toml::to_string(pending)?)
    }

    pub fn clear_pending(&self) -> Result<()> {
        remove_if_exists(&self.pending_path())
    }

    /// One merge at a time: git keeps merge state per repository, not per instance.
    pub fn ensure_no_pending(&self) -> Result<()> {
        if let Some(pending) = self.pending()? {
            bail!(
                "the fork of {} into {} is waiting on conflicts; run `migr continue` or `migr abort` first",
                pending.from,
                pending.to
            );
        }
        Ok(())
    }

    fn pending_path(&self) -> PathBuf {
        self.dir.join("migr/pending.toml")
    }
}

fn remove_if_exists(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => {
            Err(error).with_context(|| format!("cannot remove {}", path.display()))
        }
        _ => Ok(()),
    }
}
