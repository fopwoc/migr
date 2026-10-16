use anyhow::{Context as _, Result, bail};

use super::Context;
use crate::store::{PendingFork, Store};

/// `migr continue`: commits the fork once every conflict is fixed.
pub fn resume(context: &Context) -> Result<()> {
    let (store, pending) = pending(context)?;
    let prism = context.prism()?;
    prism.ensure_closed()?;
    let target = prism.instance(&pending.to)?;
    let branch = context.branch_for(&pending.to)?;
    let checkout = store.checkout(&branch, &target.game_dir())?;

    let conflicted = checkout.conflicts()?;
    let marked = checkout.files_with_markers(&conflicted);
    if !marked.is_empty() {
        println!("still has conflict markers:");
        for path in &marked {
            println!("  {path}");
        }
        bail!("fix these first, then `migr continue` again");
    }
    checkout.stage(&store.manifest(&context.branch_for(&pending.from)?)?)?;
    let remaining = checkout.conflicts()?;
    if !remaining.is_empty() {
        bail!("git still sees conflicts in: {}", remaining.join(", "));
    }
    checkout.conclude_merge()?;
    store.clear_pending()?;
    println!("forked {} into {}", pending.from, pending.to);
    super::report_playtime(context.carry_playtime(&pending.from, &target)?);
    Ok(())
}

/// `migr abort`: puts the target instance back the way it was before the fork.
pub fn abort(context: &Context) -> Result<()> {
    let (store, pending) = pending(context)?;
    let prism = context.prism()?;
    prism.ensure_closed()?;
    let target = prism.instance(&pending.to)?;
    let branch = context.branch_for(&pending.to)?;
    let checkout = store.checkout(&branch, &target.game_dir())?;

    checkout.abort_merge()?;
    store.detach(&branch)?;
    store.delete_ref(&branch.head())?;
    store.delete_ref(&branch.pack())?;
    context.restore_hooks(&target, &pending.previous_hooks)?;
    store.clear_pending()?;
    println!("undid the fork of {} into {}", pending.from, pending.to);
    Ok(())
}

fn pending(context: &Context) -> Result<(Store, PendingFork)> {
    let store = context.store()?;
    let pending = store
        .pending()?
        .context("no fork is waiting on conflicts")?;
    Ok((store, pending))
}
