use std::{collections::BTreeSet, fmt, path::Path, str::FromStr};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::utils::fs::write_atomic;

/// The manifest sits in the game folder and in each branch, so forks inherit what is tracked.
pub const MANIFEST_FILE: &str = ".migr.toml";

const HEADER: &str = "# Paths migr carries between instances, relative to the game folder.\n";
const DEFAULTS: [&str; 4] = [
    "localconfig.cfg",
    "options.txt",
    "optionsnf.txt",
    "servers.dat",
];

#[derive(Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    #[serde(default)]
    pub track: BTreeSet<TrackedPath>,
}

impl Manifest {
    pub fn defaults() -> Self {
        Self {
            track: DEFAULTS
                .iter()
                .map(|path| TrackedPath(path.to_string()))
                .collect(),
        }
    }

    pub fn parse(text: &str) -> Result<Self> {
        toml::from_str(text).with_context(|| format!("cannot parse {MANIFEST_FILE}"))
    }

    pub fn write(&self, game_dir: &Path) -> Result<()> {
        write_atomic(
            &game_dir.join(MANIFEST_FILE),
            &format!("{HEADER}{}", toml::to_string(self)?),
        )
    }

    /// Everything that belongs in a commit of the instance branch, the manifest itself included.
    pub fn committed_paths(&self) -> impl Iterator<Item = &str> {
        self.track
            .iter()
            .map(TrackedPath::as_str)
            .chain([MANIFEST_FILE])
    }
}

/// A file or folder inside the game folder, normalized to `a/b/c`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct TrackedPath(String);

impl TrackedPath {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for TrackedPath {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        if value.starts_with('/') {
            bail!("`{value}` must be relative to the game folder");
        }
        let parts: Vec<&str> = value.split('/').filter(|part| !part.is_empty()).collect();
        if parts.is_empty() || parts.iter().any(|part| matches!(*part, "." | "..")) {
            bail!("`{value}` is not a path inside the game folder");
        }
        let path = parts.join("/");
        if parts[0] == ".git" || path == MANIFEST_FILE {
            bail!("`{value}` belongs to migr itself");
        }
        Ok(Self(path))
    }
}

impl FromStr for TrackedPath {
    type Err = anyhow::Error;

    fn from_str(value: &str) -> Result<Self> {
        value.to_owned().try_into()
    }
}

impl From<TrackedPath> for String {
    fn from(path: TrackedPath) -> Self {
        path.0
    }
}

impl fmt::Display for TrackedPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_paths() {
        let path: TrackedPath = "config//journeymap/".parse().unwrap();
        assert_eq!(path.as_str(), "config/journeymap");
    }

    #[test]
    fn rejects_paths_outside_the_game_folder() {
        for path in [
            "/etc/passwd",
            "../x",
            "config/../../x",
            "",
            ".git/config",
            ".migr.toml",
        ] {
            assert!(path.parse::<TrackedPath>().is_err(), "{path}");
        }
    }

    #[test]
    fn round_trips_with_header() {
        let manifest = Manifest::defaults();
        let text = format!("{HEADER}{}", toml::to_string(&manifest).unwrap());
        assert!(text.starts_with("# Paths"));
        assert_eq!(Manifest::parse(&text).unwrap().track, manifest.track);
    }
}
