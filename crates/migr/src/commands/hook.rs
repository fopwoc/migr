use std::path::{Path, PathBuf};

use anyhow::{Result, bail};

use super::Context;
use crate::{
    cli::HookPhase,
    prism::InstanceId,
    store::{Branch, Checkout, Relation, Store},
};

/// Prism shows this output in the instance's log, and a failing pre-launch hook stops the launch.
pub fn run(
    context: &Context,
    phase: HookPhase,
    id: &InstanceId,
    game_dir: Option<PathBuf>,
) -> Result<()> {
    let store = context.store()?;
    let branch = context.branch_for(id)?;
    let game_dir = match game_dir {
        Some(dir) => dir,
        None => context.prism()?.instance(id)?.game_dir(),
    };
    match phase {
        HookPhase::Pre => pre_launch(&store, id, &branch, &game_dir),
        HookPhase::Post => post_exit(&store, id, &branch, &game_dir),
    }
}

fn pre_launch(store: &Store, id: &InstanceId, branch: &Branch, game_dir: &Path) -> Result<()> {
    if let Some(pending) = store.pending()? {
        if &pending.to == id {
            bail!("the fork into {id} still has conflicts; run `migr continue` or `migr abort`");
        }
        println!(
            "migr: a fork into {} is in progress, not syncing {id}",
            pending.to
        );
        return Ok(());
    }
    let online = store.has_remote()? && fetch(store);

    let checkout = if store.is_attached(branch) {
        let checkout = store.checkout(branch, game_dir)?;
        if checkout.commit_changes(&store.manifest(branch)?, "changes outside the game")? {
            println!("migr: saved changes made since the last session");
        }
        checkout
    } else {
        if !store.knows(branch)? {
            bail!("{id} is not tracked; run `migr new {id}` or `migr fork <old instance> {id}`");
        }
        let checkout = store.attach(branch, game_dir)?;
        println!("migr: wrote your files from {branch} into {id}");
        checkout
    };
    if online {
        pull(&checkout);
    }
    Ok(())
}

fn post_exit(store: &Store, id: &InstanceId, branch: &Branch, game_dir: &Path) -> Result<()> {
    if let Some(pending) = store.pending()? {
        println!(
            "migr: a fork into {} is in progress, not saving {id}",
            pending.to
        );
        return Ok(());
    }
    if !store.is_attached(branch) {
        println!("migr: {id} was never set up on this machine, not saving");
        return Ok(());
    }
    let checkout = store.checkout(branch, game_dir)?;
    if checkout.commit_changes(&store.manifest(branch)?, "session")? {
        println!("migr: saved");
    }
    if store.has_remote()?
        && let Err(error) = store.push(branch)
    {
        println!("migr: push failed, will retry next time: {error:#}");
    }
    Ok(())
}

/// Being offline never stops the game: sync is a bonus, playing is the point.
fn fetch(store: &Store) -> bool {
    match store.fetch() {
        Ok(()) => true,
        Err(error) => {
            println!("migr: cannot reach the remote, launching with local files: {error:#}");
            false
        }
    }
}

fn pull(checkout: &Checkout<'_>) {
    let store = checkout.store();
    let branch = checkout.branch();
    let result = store
        .relation(&branch.head(), &branch.remote_head())
        .and_then(|relation| match relation {
            Relation::Behind => {
                checkout.fast_forward(&branch.remote_head())?;
                println!("migr: pulled newer files from the remote");
                Ok(())
            }
            Relation::Diverged => {
                println!(
                    "migr: this machine and the remote both changed {branch}; launching with local files. Sort it out with `migr git -i <instance> -- ...`"
                );
                Ok(())
            }
            Relation::MissingRemote | Relation::Same | Relation::Ahead => Ok(()),
        })
        .and_then(|()| match store.relation(&branch.pack(), &branch.remote_pack())? {
            Relation::Behind => store.set_ref(&branch.pack(), &branch.remote_pack()),
            _ => Ok(()),
        });
    if let Err(error) = result {
        println!("migr: sync failed, launching with local files: {error:#}");
    }
}
