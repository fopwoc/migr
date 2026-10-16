use std::process::Command;

// Build identity comes from the tag the binary is built from, not from Cargo.toml.
fn main() {
    println!("cargo:rerun-if-env-changed=MIGR_VERSION");
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");

    let version = std::env::var("MIGR_VERSION")
        .ok()
        .filter(|version| !version.is_empty())
        .or_else(describe)
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=MIGR_VERSION={version}");
}

fn describe() -> Option<String> {
    let output = Command::new("git")
        .args(["describe", "--tags", "--always", "--dirty"])
        .output()
        .ok()?;
    let version = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (output.status.success() && !version.is_empty()).then_some(version)
}
