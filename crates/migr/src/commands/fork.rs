use anyhow::{Result, bail, ensure};

use super::Context;
use crate::{
    prism::InstanceId,
    store::{MergeOutcome, PendingFork, TrackedPath},
};

pub fn run(context: &Context, from: &InstanceId, to: &InstanceId) -> Result<()> {
    let prism = context.prism()?;
    prism.ensure_closed()?;
    let store = context.store()?;
    store.ensure_no_pending()?;
    let from_branch = context.branch_for(from)?;
    let to_branch = context.branch_for(to)?;
    ensure!(store.exists(&from_branch.head())?, "{from} is not tracked");
    ensure!(
        !store.knows(&to_branch)?,
        "{to} is already tracked as {to_branch}"
    );
    let target = prism.instance(to)?;

    // Edits made in the old instance since its last session belong in the fork too.
    if store.is_attached(&from_branch)
        && let Ok(source) = prism.instance(from)
    {
        let checkout = store.checkout(&from_branch, &source.game_dir())?;
        checkout.commit_changes(&store.manifest(&from_branch)?, "changes before fork")?;
    }

    // The new pack's files, as a child of the old pack's: that makes the old pack the merge base,
    // so git keeps both your edits and whatever the pack changed itself.
    let manifest = store.manifest(&from_branch)?;
    let game_dir = target.game_dir();
    let paths = manifest.track.iter().map(TrackedPath::as_str);
    let pack_commit = store.snapshot(
        &game_dir,
        paths,
        None,
        Some(&from_branch.pack()),
        &format!("pack files of {to}"),
    )?;
    store.set_ref(&to_branch.pack(), &pack_commit)?;
    store.set_ref(&to_branch.head(), &pack_commit)?;

    let checkout = store.checkout(&to_branch, &game_dir)?;
    checkout.adopt_head()?;
    let previous_hooks = context.install_hooks(&target)?;

    match checkout.merge(&from_branch.head(), &format!("fork {from} into {to}")) {
        Ok(MergeOutcome::Clean) => {
            println!("forked {from} into {to}");
            super::report_playtime(context.carry_playtime(from, &target)?);
            Ok(())
        }
        Ok(MergeOutcome::Conflicts(paths)) => {
            store.save_pending(&PendingFork {
                from: from.clone(),
                to: to.clone(),
                previous_hooks,
            })?;
            println!(
                "both you and the pack changed these, in {}:",
                game_dir.display()
            );
            for path in &paths {
                println!("  {path}");
            }
            println!("fix them by hand, then `migr continue`; or `migr abort` to undo the fork");
            bail!("fork stopped on conflicts")
        }
        Err(error) => {
            store.delete_ref(&to_branch.head())?;
            store.delete_ref(&to_branch.pack())?;
            store.detach(&to_branch)?;
            context.restore_hooks(&target, &previous_hooks)?;
            Err(error)
        }
    }
}
