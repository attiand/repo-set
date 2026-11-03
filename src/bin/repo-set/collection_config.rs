use anyhow::Context;
use serde::Deserialize;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CollectionConfig {
    collection: Vec<PathBuf>,
}

pub(super) fn roots(root: &Path) -> anyhow::Result<Option<Vec<PathBuf>>> {
    let path = root.join(".repo-set/collection.toml");
    let content = match fs::read_to_string(&path) {
        Ok(content) => content,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error).with_context(|| format!("Failed to read {}", path.display()));
        }
    };
    let config: CollectionConfig =
        toml::from_str(&content).with_context(|| format!("Failed to parse {}", path.display()))?;
    Ok(Some(
        config
            .collection
            .into_iter()
            .map(|member| root.join(member))
            .collect(),
    ))
}
