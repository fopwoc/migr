use std::{ffi::OsString, path::PathBuf};

use clap::{Parser, Subcommand, ValueEnum};

use crate::{prism::InstanceId, store::TrackedPath};

/// Carries your configs from one Prism instance to the next with git.
#[derive(Parser)]
#[command(name = "migr", version = env!("MIGR_VERSION"))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Subcommand)]
pub enum Command {
    /// Create the repository, or clone it from a remote and hook the instances it knows.
    Init {
        /// A git remote to clone; pushes and pulls go there from then on.
        remote: Option<String>,
    },
    /// Start tracking an instance and hook it into Prism.
    New {
        instance: InstanceId,
        /// A pristine .minecraft of the same pack version, for an instance you already changed.
        #[arg(long, value_name = "DIR")]
        pack: Option<PathBuf>,
        /// Track this path on top of the defaults; repeatable.
        #[arg(long = "track", value_name = "PATH")]
        track: Vec<TrackedPath>,
    },
    /// Carry your files from an old instance into a fresh one of a newer pack version.
    Fork { from: InstanceId, to: InstanceId },
    /// Start tracking more paths of an instance.
    Track {
        instance: InstanceId,
        #[arg(required = true)]
        paths: Vec<TrackedPath>,
        /// Take your version of the paths from this instance instead of the current files.
        #[arg(long, value_name = "INSTANCE")]
        from: Option<InstanceId>,
    },
    /// Write a tracked branch into an instance whose folder name differs from it.
    Link {
        instance: InstanceId,
        branch: Option<String>,
    },
    /// Show tracked instances and any fork waiting on conflicts.
    Status,
    /// Finish a fork after resolving its conflicts.
    Continue,
    /// Undo a fork that stopped on conflicts.
    Abort,
    /// Run git on the repository, or on an instance's checkout with --instance.
    Git {
        #[arg(short, long)]
        instance: Option<InstanceId>,
        #[arg(required = true, trailing_var_arg = true, allow_hyphen_values = true)]
        args: Vec<OsString>,
    },
    /// What Prism runs before launch and after exit.
    Hook {
        phase: HookPhase,
        #[arg(long, env = "INST_ID")]
        instance: InstanceId,
        #[arg(long, env = "INST_MC_DIR")]
        game_dir: Option<PathBuf>,
    },
}

#[derive(Clone, Copy, ValueEnum)]
pub enum HookPhase {
    Pre,
    Post,
}
