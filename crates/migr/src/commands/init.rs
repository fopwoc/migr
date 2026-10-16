use anyhow::Result;

use super::Context;
use crate::store::Store;

pub fn run(context: &Context, remote: Option<&str>) -> Result<()> {
    let store = Store::create(&context.repo_dir, remote)?;
    println!("created {}", context.repo_dir.display());
    if remote.is_none() {
        println!("next: `migr new <instance>` for the instance you play now");
        return Ok(());
    }

    let branches = store.branches()?;
    let prism = context.prism()?;
    let mut matches = Vec::new();
    for instance in prism.instances()? {
        if branches.contains(&context.branch_for(instance.id())?) {
            matches.push(instance);
        }
    }
    if matches.is_empty() {
        println!(
            "no Prism instance here matches a branch yet; `migr link <instance> <branch>` binds one by hand"
        );
        return Ok(());
    }
    if prism.is_open() {
        println!("close Prism and run `migr link <instance>` for each of these:");
        for instance in &matches {
            println!("  {}", instance.id());
        }
        return Ok(());
    }
    // Only the hooks go in now; the first launch writes the files, so nothing changes before you play.
    for instance in &matches {
        context.install_hooks(instance)?;
        println!(
            "hooked {}; its first launch brings your files",
            instance.id()
        );
    }
    Ok(())
}
