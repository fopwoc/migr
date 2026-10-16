use std::{
    collections::BTreeMap,
    fs, io,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::{prism::InstanceId, utils::fs::write_atomic};

/// Settings of this machine; everything shared between machines lives in the repository.
#[derive(Default, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct MachineConfig {
    /// Prism's data folder, when it is not in the default place.
    pub prism_dir: Option<PathBuf>,
    /// Where the repository lives, when not in the default place.
    pub repo: Option<PathBuf>,
    /// Instances whose folder name differs from their branch.
    pub links: BTreeMap<InstanceId, String>,
}

impl MachineConfig {
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string(path) {
            Ok(text) => {
                toml::from_str(&text).with_context(|| format!("cannot parse {}", path.display()))
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Self::default()),
            Err(error) => Err(error).with_context(|| format!("cannot read {}", path.display())),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("cannot create {}", parent.display()))?;
        }
        write_atomic(path, &toml::to_string(self)?)
    }
}
