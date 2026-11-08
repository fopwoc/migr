use std::{
    fmt,
    path::{Path, PathBuf},
    str::FromStr,
};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use super::InstanceCfg;

/// The folder name of a Prism instance, which is also what Prism passes as `INST_ID`.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct InstanceId(String);

impl InstanceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for InstanceId {
    type Error = anyhow::Error;

    fn try_from(value: String) -> Result<Self> {
        if value.is_empty() || value == "." || value == ".." || value.contains('/') {
            bail!("`{value}` is not an instance folder name");
        }
        Ok(Self(value))
    }
}

impl FromStr for InstanceId {
    type Err = anyhow::Error;

    /// Forgives what a shell hands over for a folder: `./name/` from tab completion,
    /// and `name\ with\ spaces` escaped again inside quotes.
    fn from_str(value: &str) -> Result<Self> {
        let trimmed = value.trim_end_matches('/');
        let name = trimmed.strip_prefix("./").unwrap_or(trimmed);
        name.replace("\\ ", " ").try_into()
    }
}

impl From<InstanceId> for String {
    fn from(id: InstanceId) -> Self {
        id.0
    }
}

impl fmt::Display for InstanceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

pub struct Instance {
    id: InstanceId,
    dir: PathBuf,
}

impl Instance {
    pub(super) fn new(id: InstanceId, dir: PathBuf) -> Self {
        Self { id, dir }
    }

    pub fn id(&self) -> &InstanceId {
        &self.id
    }

    /// The folder Prism runs the game in, the same one it passes as `INST_MC_DIR`.
    pub fn game_dir(&self) -> PathBuf {
        [".minecraft", "minecraft"]
            .iter()
            .map(|name| self.dir.join(name))
            .find(|dir| dir.is_dir())
            .unwrap_or_else(|| self.dir.join(".minecraft"))
    }

    pub fn load_cfg(&self) -> Result<InstanceCfg> {
        InstanceCfg::load(&self.cfg_path())
    }

    pub(super) fn cfg_path(&self) -> PathBuf {
        self.dir.join("instance.cfg")
    }

    pub(super) fn is_instance_dir(dir: &Path) -> bool {
        dir.join("instance.cfg").is_file()
    }
}

#[cfg(test)]
mod tests {
    use super::InstanceId;

    #[test]
    fn shell_spellings_of_a_folder_parse_to_its_name() {
        for input in [
            "GT New Horizons",
            "GT New Horizons/",
            "./GT New Horizons/",
            r"GT\ New\ Horizons",
        ] {
            let id: InstanceId = input.parse().unwrap();
            assert_eq!(id.as_str(), "GT New Horizons", "{input}");
        }
        for input in ["", "/", "./", "a/b", "../a"] {
            assert!(input.parse::<InstanceId>().is_err(), "{input}");
        }
    }
}
