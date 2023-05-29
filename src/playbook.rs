use std::{collections::HashMap, path::Path};

use anyhow::Context;
use indexmap::IndexMap;
use serde::Deserialize;

pub fn load_playbook(path: impl AsRef<Path>) -> anyhow::Result<Playbook> {
    let path = path.as_ref();

    let raw_playbook =
        std::fs::read_to_string(path).with_context(|| format!("cannot read {:?}", path))?;

    toml::from_str(&raw_playbook).with_context(|| format!("cannot parse {:?}", path))
}

#[derive(Debug, Deserialize)]
pub struct Playbook {
    #[serde(flatten)]
    pub tasks: IndexMap<String, Task>,
}

#[derive(Debug, Deserialize)]
pub struct Task {
    #[serde(default)]
    pub hosts: Vec<String>,

    #[serde(default)]
    pub filter: HashMap<String, String>,

    #[serde(default)]
    pub doas: Option<String>,

    pub commands: Vec<String>,
}
