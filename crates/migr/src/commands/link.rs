use anyhow::{Result, ensure};

use super::Context;
use crate::{prism::InstanceId, store::Branch};

pub fn run(context: &mut Context, id: &InstanceId, branch: Option<&str>) -> Result<()> {
    let prism = context.prism()?;
    prism.ensure_closed()?;
    let instance = prism.instance(id)?;
    let store = context.store()?;
    store.ensure_no_pending()?;
    let branch = match branch {
        Some(name) => Branch::parse(name)?,
        None => context.branch_for(id)?,
    };
    if store.has_remote()? {
        store.fetch()?;
    }
    ensure!(
        store.knows(&branch)?,
        "no branch {branch} here or on the remote"
    );

    store.attach(&branch, &instance.game_dir())?;
    context.install_hooks(&instance)?;
    if branch != Branch::for_instance(id) {
        context.config.links.insert(id.clone(), branch.to_string());
        context.config.save(&context.config_path)?;
    }
    println!("wrote {branch} into {id}");
    Ok(())
}
