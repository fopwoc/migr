use anyhow::Result;

use super::Context;

pub fn run(context: &Context) -> Result<()> {
    let store = context.store()?;
    match store.remote_url() {
        Some(url) => println!("repository {} (remote {url})", context.repo_dir.display()),
        None => println!("repository {} (no remote)", context.repo_dir.display()),
    }
    if let Some(pending) = store.pending()? {
        println!(
            "fork of {} into {} is waiting on conflicts: fix them, then `migr continue` or `migr abort`",
            pending.from, pending.to
        );
    }

    let prism = context.prism()?;
    let mut any = false;
    for instance in prism.instances()? {
        let branch = context.branch_for(instance.id())?;
        if !store.knows(&branch)? {
            continue;
        }
        any = true;
        let cfg = instance.load_cfg()?;
        let hooked = cfg.hook_commands().is_migr();
        let state = match (store.is_attached(&branch), hooked) {
            (true, true) => "synced",
            (false, true) => "files arrive on first launch",
            (_, false) => "no hooks; `migr link` sets them",
        };
        println!("  {}  {}  {state}", instance.id(), cfg.playtime());
    }
    if !any {
        println!("no tracked instances yet; `migr new <instance>` starts one");
    }
    Ok(())
}
