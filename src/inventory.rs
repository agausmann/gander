use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::Deserialize;
use walkdir::WalkDir;

pub fn load_inventory(base: impl AsRef<Path>) -> anyhow::Result<Inventory> {
    let base = base.as_ref();

    let mut group_manifests = HashMap::new();
    let mut host_manifests = Vec::new();

    for entry_result in WalkDir::new(base) {
        let entry = entry_result?;
        let path = entry.path();
        if path.is_dir() {
            continue;
        }
        let relative_path = path
            .strip_prefix(base)
            .expect("walkdir path not in base path");

        let raw_manifest = std::fs::read_to_string(entry.path())
            .with_context(|| format!("cannot read manifest {:?}", entry.path()))?;
        let manifest: Manifest = toml::from_str(&raw_manifest)
            .with_context(|| format!("cannot parse manifest {:?}", entry.path()))?;

        if path.file_stem().unwrap() == "defaults" {
            group_manifests.insert(relative_path.parent().unwrap().to_owned(), manifest);
        } else {
            host_manifests.push((relative_path.to_owned(), manifest));
        }
    }

    let hosts: Vec<HostSpec> = host_manifests
        .into_iter()
        .map(|(host_path, host_manifest)| {
            host_path
                .ancestors()
                .flat_map(|group_path| group_manifests.get(group_path))
                .fold(host_manifest, Manifest::or)
                .validate(host_path)
        })
        .collect::<anyhow::Result<Vec<HostSpec>>>()?;

    Ok(Inventory { hosts })
}

#[derive(Debug)]
pub struct Inventory {
    pub hosts: Vec<HostSpec>,
}

#[derive(Debug)]
pub struct HostSpec {
    pub path: PathBuf,
    pub address: String,
    pub ssh_user: String,
    pub extra_keys: HashMap<String, String>,
}

#[derive(Clone, Deserialize)]
pub struct Manifest {
    #[serde(default)]
    pub address: Option<String>,

    #[serde(default)]
    pub ssh_user: Option<String>,

    #[serde(flatten)]
    pub extra_keys: HashMap<String, String>,
}

impl Manifest {
    /// Attempts to fill empty keys in `self` with keys in `other`.
    pub fn or(self, other: &Self) -> Self {
        // Union extra_keys. If the key is in both `self` and `other`, then
        // take the value from `self`.
        let mut extra_keys = other.extra_keys.clone();
        extra_keys.extend(self.extra_keys);

        Self {
            address: self.address.or_else(|| other.address.clone()),
            ssh_user: self.ssh_user.or_else(|| other.ssh_user.clone()),
            extra_keys,
        }
    }

    pub fn validate(self, path: PathBuf) -> anyhow::Result<HostSpec> {
        Ok(HostSpec {
            path,
            address: self.address.context("missing key: `address`")?,
            ssh_user: self.ssh_user.context("missing key: `ssh_user`")?,
            extra_keys: self.extra_keys,
        })
    }
}
