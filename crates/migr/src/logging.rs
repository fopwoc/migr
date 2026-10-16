use env_logger::{Builder, Env, Target};

/// Logs go to stderr so they never mix with output meant for the user or for Prism's log.
pub fn init() {
    Builder::from_env(Env::new().filter_or("LOG_LEVEL", "error"))
        .target(Target::Stderr)
        .init();
}
