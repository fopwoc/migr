use std::path::Path;

use anyhow::{Result, ensure};

use super::Context;
use crate::{prism::InstanceId, store::TrackedPath, utils::fs::replace_with_copy};

pub fn run(
    context: &Context,
    id: &InstanceId,
    paths: Vec<TrackedPath>,
    from: Option<&InstanceId>,
    pack: Option<&Path>,
) -> Result<()> {
    let store = context.store()?;
    store.ensure_no_pending()?;
    let branch = context.branch_for(id)?;
    ensure!(store.exists(&branch.head())?, "{id} is not tracked");
    let prism = context.prism()?;
    let game_dir = prism.instance(id)?.game_dir();
    let checkout = store.checkout(&branch, &game_dir)?;
    let mut manifest = store.manifest(&branch)?;
    checkout.commit_changes(&manifest, "changes before track")?;

    let new: Vec<TrackedPath> = paths
        .into_iter()
        .filter(|path| !manifest.track.contains(path))
        .collect();
    if new.is_empty() {
        println!("already tracked");
        return Ok(());
    }
    let new_paths: Vec<&str> = new.iter().map(TrackedPath::as_str).collect();

    // The pack's own versions are the base the next fork merges against. Without a pristine copy,
    // the current files stand in, which is right for an instance you have not changed them in yet.
    let pack_dir = pack.unwrap_or(&game_dir);
    ensure!(pack_dir.is_dir(), "{} is not a folder", pack_dir.display());
    let message = format!("pack files for {}", new_paths.join(", "));
    let pack_commit = store.snapshot(
        pack_dir,
        new_paths.iter().copied(),
        Some(&branch.pack()),
        Some(&branch.pack()),
        &message,
    )?;
    store.set_ref(&branch.pack(), &pack_commit)?;
    // The branch takes the pack's versions in a merge commit; your own versions follow as a normal commit.
    checkout.take_from(&pack_commit, &new_paths)?;
    checkout.commit_merge(&pack_commit, &message)?;

    let source = from.map(|from| prism.instance(from)).transpose()?;
    if let Some(source) = &source {
        let source_dir = source.game_dir();
        for path in &new {
            let source_path = source_dir.join(path.as_str());
            if source_path.exists() {
                replace_with_copy(&source_path, &game_dir.join(path.as_str()))?;
            } else {
                println!("{path} is not in {}, keeping the current one", source.id());
            }
        }
    }

    manifest.track.extend(new.iter().cloned());
    manifest.write(&game_dir)?;
    let message = match &source {
        Some(source) => format!("track {} from {}", new_paths.join(", "), source.id()),
        None => format!("track {}", new_paths.join(", ")),
    };
    checkout.commit_changes(&manifest, &message)?;
    println!("{message}");
    Ok(())
}
