use std::{fs, io, path::Path};

use anyhow::{Context, Result};

/// Replaces `target` with a copy of `source`, so a copied directory loses files the source lacks.
pub fn replace_with_copy(source: &Path, target: &Path) -> Result<()> {
    match fs::symlink_metadata(target) {
        Ok(meta) if meta.is_dir() => fs::remove_dir_all(target),
        Ok(_) => fs::remove_file(target),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
    .with_context(|| format!("cannot remove {}", target.display()))?;

    copy(source, target)
        .with_context(|| format!("cannot copy {} to {}", source.display(), target.display()))
}

fn copy(source: &Path, target: &Path) -> io::Result<()> {
    if let Some(parent) = target.parent() {
        fs::create_dir_all(parent)?;
    }
    if !source.is_dir() {
        return fs::copy(source, target).map(drop);
    }
    fs::create_dir(target)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        copy(&entry.path(), &target.join(entry.file_name()))?;
    }
    Ok(())
}

/// Writes through a sibling temp file so a crash never leaves a half-written file behind.
pub fn write_atomic(path: &Path, contents: &str) -> Result<()> {
    let temp = path.with_extension("migr-tmp");
    fs::write(&temp, contents)
        .and_then(|()| fs::rename(&temp, path))
        .with_context(|| format!("cannot write {}", path.display()))
}
