use std::path::Path;

use anyhow::{Result, ensure};

use super::Context;
use crate::{
    prism::InstanceId,
    store::{Manifest, TrackedPath},
};

pub fn run(
    context: &Context,
    id: &InstanceId,
    pack: Option<&Path>,
    extra: Vec<TrackedPath>,
) -> Result<()> {
    let prism = context.prism()?;
    prism.ensure_closed()?;
    let instance = prism.instance(id)?;
    let store = context.store()?;
    store.ensure_no_pending()?;
    let branch = context.branch_for(id)?;
    ensure!(
        !store.knows(&branch)?,
        "{id} is already tracked as {branch}"
    );

    let mut manifest = Manifest::defaults();
    manifest.track.extend(extra);
    let game_dir = instance.game_dir();
    let pack_dir = pack.unwrap_or(&game_dir);
    ensure!(pack_dir.is_dir(), "{} is not a folder", pack_dir.display());

    let paths = manifest.track.iter().map(TrackedPath::as_str);
    let pack_commit =
        store.snapshot(pack_dir, paths, None, None, &format!("pack files of {id}"))?;
    store.set_ref(&branch.pack(), &pack_commit)?;
    store.set_ref(&branch.head(), &pack_commit)?;

    let checkout = store.checkout(&branch, &game_dir)?;
    checkout.adopt_head()?;
    manifest.write(&game_dir)?;
    checkout.commit_changes(&manifest, &format!("track {id}"))?;
    context.install_hooks(&instance)?;

    println!("tracking {id} as {branch}:");
    for path in &manifest.track {
        println!("  {path}");
    }
    if pack.is_none() {
        println!(
            "the current files count as the pack's own; if you changed them already, start over with --pack <a fresh .minecraft>"
        );
    }
    Ok(())
}
