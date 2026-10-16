use std::{env, path::PathBuf};

use anyhow::{Context as _, Result, anyhow};

use crate::{
    config::MachineConfig,
    prism::{HookCommands, Instance, InstanceId, Playtime, Prism},
    store::{Branch, Store},
    utils::dirs,
};

/// Everything a command needs to know about this machine.
pub struct Context {
    pub config: MachineConfig,
    pub config_path: PathBuf,
    pub repo_dir: PathBuf,
    prism: Result<Prism, String>,
    executable: PathBuf,
}

impl Context {
    pub fn load() -> Result<Self> {
        let config_path = dirs::config_home()?.join("migr/config.toml");
        let config = MachineConfig::load(&config_path)?;
        log::info!("config loaded from {}", config_path.display());
        let repo_dir = match &config.repo {
            Some(repo) => repo.clone(),
            None => dirs::data_home()?.join("migr/repo.git"),
        };
        // Hooks get their paths from Prism, so a missing Prism only matters to commands that ask for it.
        let prism =
            Prism::discover(config.prism_dir.as_deref()).map_err(|error| format!("{error:#}"));
        let executable = env::current_exe().context("cannot locate the migr binary")?;
        Ok(Self {
            config,
            config_path,
            repo_dir,
            prism,
            executable,
        })
    }

    #[cfg(test)]
    pub fn for_test(root: &std::path::Path, prism: Prism) -> Self {
        Self {
            config: MachineConfig::default(),
            config_path: root.join("config.toml"),
            repo_dir: root.join("repo.git"),
            prism: Ok(prism),
            executable: PathBuf::from("/usr/local/bin/migr"),
        }
    }

    pub fn prism(&self) -> Result<&Prism> {
        self.prism.as_ref().map_err(|error| anyhow!("{error}"))
    }

    pub fn store(&self) -> Result<Store> {
        Store::open(&self.repo_dir)
    }

    pub fn branch_for(&self, id: &InstanceId) -> Result<Branch> {
        match self.config.links.get(id) {
            Some(name) => Branch::parse(name),
            None => Ok(Branch::for_instance(id)),
        }
    }

    /// Points the instance's Prism hooks at migr, returning what was there before.
    pub fn install_hooks(&self, instance: &Instance) -> Result<HookCommands> {
        let mut cfg = instance.load_cfg()?;
        let previous = cfg.hook_commands();
        cfg.set_hook_commands(&HookCommands::migr(&self.executable));
        cfg.save()?;
        log::info!("hooks installed into {}", instance.id());
        Ok(previous)
    }

    pub fn restore_hooks(&self, instance: &Instance, previous: &HookCommands) -> Result<()> {
        let mut cfg = instance.load_cfg()?;
        cfg.set_hook_commands(previous);
        cfg.save()
    }

    /// Adds the old instance's playtime to the new one, so the total survives every pack update.
    ///
    /// Returns nothing when the old instance is not on this machine.
    pub fn carry_playtime(&self, from: &InstanceId, to: &Instance) -> Result<Option<Playtime>> {
        let Ok(source) = self.prism()?.instance(from) else {
            log::warn!("{from} is not on this machine; playtime stays as it is");
            return Ok(None);
        };
        let mut cfg = to.load_cfg()?;
        let total = cfg.playtime() + source.load_cfg()?.playtime();
        cfg.set_playtime(total);
        cfg.save()?;
        Ok(Some(total))
    }
}
