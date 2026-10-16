use std::{env, path::PathBuf};

use anyhow::{Context, Result};

pub fn home() -> Result<PathBuf> {
    env::home_dir().context("cannot determine the home directory")
}

pub fn config_home() -> Result<PathBuf> {
    xdg("XDG_CONFIG_HOME", ".config")
}

pub fn data_home() -> Result<PathBuf> {
    xdg("XDG_DATA_HOME", ".local/share")
}

fn xdg(variable: &str, fallback: &str) -> Result<PathBuf> {
    match env::var_os(variable).map(PathBuf::from) {
        Some(path) if path.is_absolute() => Ok(path),
        _ => Ok(home()?.join(fallback)),
    }
}
