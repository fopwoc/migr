mod context;
mod fork;
mod git;
mod hook;
mod init;
mod link;
mod new;
mod resolve;
mod status;
#[cfg(test)]
mod tests;
mod track;

use anyhow::Result;

use crate::{cli::Command, prism::Playtime};
pub use context::Context;

fn report_playtime(playtime: Option<Playtime>) {
    match playtime {
        Some(total) => println!("playtime carried over: {total}"),
        None => println!(
            "the old instance is not on this machine, so its playtime was not carried over"
        ),
    }
}

pub fn run(command: Command) -> Result<()> {
    let mut context = Context::load()?;
    match command {
        Command::Init { remote } => init::run(&context, remote.as_deref()),
        Command::New {
            instance,
            pack,
            track,
        } => new::run(&context, &instance, pack.as_deref(), track),
        Command::Fork { from, to } => fork::run(&context, &from, &to),
        Command::Track {
            instance,
            paths,
            from,
        } => track::run(&context, &instance, paths, from.as_ref()),
        Command::Link { instance, branch } => link::run(&mut context, &instance, branch.as_deref()),
        Command::Status => status::run(&context),
        Command::Continue => resolve::resume(&context),
        Command::Abort => resolve::abort(&context),
        Command::Git { instance, args } => git::run(&context, instance.as_ref(), &args),
        Command::Hook {
            phase,
            instance,
            game_dir,
        } => hook::run(&context, phase, &instance, game_dir),
    }
}
