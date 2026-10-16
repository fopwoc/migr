use std::process::Command;

// A tagged commit is that release; anything else is `<YYYYMMDD>-<short hash>` of the commit.
fn main() {
    println!("cargo:rerun-if-changed=../../.git/HEAD");
    println!("cargo:rerun-if-changed=../../.git/refs");

    let version = git(&["describe", "--tags", "--exact-match", "HEAD"])
        .or_else(|| {
            git(&[
                "log",
                "-1",
                "--date=format:%Y%m%d",
                "--format=%cd-%h",
                "--abbrev=8",
            ])
        })
        .unwrap_or_else(|| "unknown".to_owned());
    println!("cargo:rustc-env=MIGR_VERSION={version}");
}

fn git(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    let text = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (output.status.success() && !text.is_empty()).then_some(text)
}
