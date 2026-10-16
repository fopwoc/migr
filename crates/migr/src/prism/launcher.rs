use std::{
    fs, io,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use anyhow::{Context, Result, bail, ensure};

use super::{Instance, InstanceId};
use crate::utils::dirs;

/// The Prism Launcher installation whose instances migr manages.
pub struct Prism {
    instances_dir: PathBuf,
    detect_running: bool,
}

impl Prism {
    pub fn discover(root: Option<&Path>) -> Result<Self> {
        let root = match root {
            Some(root) => root.to_owned(),
            None => default_root()?,
        };
        let instances_dir = match instance_dir_setting(&root)? {
            Some(dir) => root.join(dir),
            None => root.join("instances"),
        };
        ensure!(
            instances_dir.is_dir(),
            "no Prism instances at {}; set prism_dir in the migr config",
            instances_dir.display()
        );
        log::info!("prism instances at {}", instances_dir.display());
        Ok(Self {
            instances_dir,
            detect_running: true,
        })
    }

    #[cfg(test)]
    pub fn at(instances_dir: PathBuf) -> Self {
        Self {
            instances_dir,
            detect_running: false,
        }
    }

    pub fn instance(&self, id: &InstanceId) -> Result<Instance> {
        let dir = self.instances_dir.join(id.as_str());
        ensure!(
            Instance::is_instance_dir(&dir),
            "no Prism instance `{id}` in {}",
            self.instances_dir.display()
        );
        Ok(Instance::new(id.clone(), dir))
    }

    pub fn instances(&self) -> Result<Vec<Instance>> {
        let entries = fs::read_dir(&self.instances_dir)
            .with_context(|| format!("cannot list {}", self.instances_dir.display()))?;
        let mut instances = Vec::new();
        for entry in entries {
            let entry = entry?;
            let dir = entry.path();
            let id = entry
                .file_name()
                .into_string()
                .ok()
                .and_then(|name| name.parse().ok());
            if let Some(id) = id
                && Instance::is_instance_dir(&dir)
            {
                instances.push(Instance::new(id, dir));
            }
        }
        instances.sort_by(|a, b| a.id().cmp(b.id()));
        Ok(instances)
    }

    /// Prism keeps `instance.cfg` in memory and writes it back, so edits must happen while it is closed.
    pub fn ensure_closed(&self) -> Result<()> {
        if self.detect_running && is_running() {
            bail!(
                "Prism Launcher is running; close it first, it would overwrite instance settings"
            );
        }
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.detect_running && is_running()
    }
}

fn default_root() -> Result<PathBuf> {
    if cfg!(target_os = "macos") {
        Ok(dirs::home()?.join("Library/Application Support/PrismLauncher"))
    } else {
        Ok(dirs::data_home()?.join("PrismLauncher"))
    }
}

/// `InstanceDir` in `prismlauncher.cfg`, relative to Prism's data folder unless absolute.
fn instance_dir_setting(root: &Path) -> Result<Option<String>> {
    let path = root.join("prismlauncher.cfg");
    let text = match fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("cannot read {}", path.display()));
        }
    };
    Ok(text
        .lines()
        .filter_map(|line| line.split_once('='))
        .find(|(key, _)| key.trim() == "InstanceDir")
        .map(|(_, value)| value.trim().trim_matches('"').to_owned())
        .filter(|value| !value.is_empty()))
}

fn is_running() -> bool {
    Command::new("pgrep")
        .args(["-x", "prismlauncher"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}
