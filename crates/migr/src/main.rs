mod cli;
mod commands;
mod config;
mod git;
mod logging;
mod prism;
mod store;
mod utils;

use std::process::ExitCode;

use clap::Parser;

fn main() -> ExitCode {
    logging::init();
    let cli = cli::Cli::parse();
    log::info!("migr {} starting", env!("MIGR_VERSION"));

    match commands::run(cli.command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("migr: {error:#}");
            ExitCode::FAILURE
        }
    }
}
