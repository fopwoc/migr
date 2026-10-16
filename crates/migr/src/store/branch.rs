use std::fmt;

use anyhow::{Result, ensure};

use crate::prism::InstanceId;

const PACK_NAMESPACE: &str = "pack";

/// The branch holding one instance's files; its pristine pack files live under `pack/<name>`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Branch(String);

impl Branch {
    /// Prism folder names may contain spaces and symbols that git refuses in branch names.
    pub fn for_instance(id: &InstanceId) -> Self {
        let mut name: String = id
            .as_str()
            .chars()
            .map(|c| if allowed(c) { c } else { '_' })
            .collect();
        while name.contains("..") {
            name = name.replace("..", "._");
        }
        if name.starts_with(['.', '-']) {
            name.insert(0, '_');
        }
        if name.ends_with(".lock") || name == PACK_NAMESPACE {
            name.push('_');
        }
        Self(name)
    }

    pub fn parse(name: &str) -> Result<Self> {
        let valid = !name.is_empty()
            && name.chars().all(allowed)
            && !name.contains("..")
            && !name.starts_with(['.', '-'])
            && !name.ends_with(".lock")
            && name != PACK_NAMESPACE;
        ensure!(valid, "`{name}` cannot be a migr branch name");
        Ok(Self(name.to_owned()))
    }

    pub fn head(&self) -> String {
        format!("refs/heads/{}", self.0)
    }

    pub fn pack(&self) -> String {
        format!("refs/heads/{PACK_NAMESPACE}/{}", self.0)
    }

    pub fn remote_head(&self) -> String {
        format!("refs/remotes/origin/{}", self.0)
    }

    pub fn remote_pack(&self) -> String {
        format!("refs/remotes/origin/{PACK_NAMESPACE}/{}", self.0)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Branch names as `for-each-ref` lists them, skipping the pack namespace.
    pub(super) fn from_listed(name: &str) -> Option<Self> {
        if name.starts_with(&format!("{PACK_NAMESPACE}/")) {
            return None;
        }
        Self::parse(name).ok()
    }
}

impl fmt::Display for Branch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn allowed(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '+')
}

#[cfg(test)]
mod tests {
    use super::*;

    fn branch(id: &str) -> String {
        Branch::for_instance(&id.parse().unwrap()).0
    }

    #[test]
    fn keeps_gtnh_folder_names() {
        assert_eq!(
            branch("GT_New_Horizons_2.9.0-RC-1_Java_17-26"),
            "GT_New_Horizons_2.9.0-RC-1_Java_17-26"
        );
    }

    #[test]
    fn sanitizes_what_git_refuses() {
        assert_eq!(branch("Fabric 1.21.1"), "Fabric_1.21.1");
        assert_eq!(
            branch("Prominence™ II- Hasturian Era"),
            "Prominence__II-_Hasturian_Era"
        );
        assert_eq!(branch(".hidden..x.lock"), "_.hidden._x.lock_");
        assert_eq!(branch("pack"), "pack_");
    }

    #[test]
    fn sanitized_names_parse() {
        for id in ["Fabric 1.21.1", ".hidden..x.lock", "pack", "-x"] {
            assert!(Branch::parse(&branch(id)).is_ok(), "{id}");
        }
        assert!(Branch::parse("pack").is_err());
        assert!(Branch::parse("a/b").is_err());
    }
}
