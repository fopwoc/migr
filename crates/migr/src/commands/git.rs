use std::{ffi::OsString, process};

use anyhow::Result;

use super::Context;
use crate::prism::InstanceId;

/// Plain git on migr's repository, for the cases migr leaves to you.
pub fn run(context: &Context, instance: Option<&InstanceId>, args: &[OsString]) -> Result<()> {
    let store = context.store()?;
    let status = match instance {
        Some(id) => {
            let game_dir = context.prism()?.instance(id)?.game_dir();
            let checkout = store.checkout(&context.branch_for(id)?, &game_dir)?;
            checkout.git().interactive(args)?
        }
        None => store.git().interactive(args)?,
    };
    process::exit(status.code().unwrap_or(1))
}
