use anyhow::{Result, ensure};

use super::Context;
use crate::{prism::InstanceId, store::TrackedPath, utils::fs::replace_with_copy};

pub fn run(
    context: &Context,
    id: &InstanceId,
    paths: Vec<TrackedPath>,
    from: Option<&InstanceId>,
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

    // The files as they are now become the pack's own, which is right for an instance you have not
    // changed them in yet. Without that, the next fork would have no base to merge them against.
    let pack_commit = store.snapshot(
        &game_dir,
        new_paths.iter().copied(),
        Some(&branch.pack()),
        Some(&branch.pack()),
        &format!("pack files for {}", new_paths.join(", ")),
    )?;
    store.set_ref(&branch.pack(), &pack_commit)?;
    // The branch already has these exact files on disk, so the merge result is known without a merge.
    checkout.stage_paths(&new_paths)?;
    checkout.commit_merge(
        &pack_commit,
        &format!("pack files for {}", new_paths.join(", ")),
    )?;

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
