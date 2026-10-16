use serde::{Deserialize, Serialize};

use crate::prism::{HookCommands, InstanceId};

/// A fork that stopped on conflicts, kept until `migr continue` or `migr abort`.
#[derive(Serialize, Deserialize)]
pub struct PendingFork {
    pub from: InstanceId,
    pub to: InstanceId,
    /// What the target instance had before migr put its hooks in, restored on abort.
    pub previous_hooks: HookCommands,
}
