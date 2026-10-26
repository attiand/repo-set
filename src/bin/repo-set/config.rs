use anyhow::Context;
use repo_set::config::{self, Clone, Config, Remote, Repo};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

const INITIAL_CONFIG: &str = r#"# Replace the example host and listing command with your server settings.
[remote]
url = "ssh://${user}@my-gerrit:29418"
list.cmd = ["ssh", "ssh://${user}@my-gerrit:29418", "gerrit", "ls-projects"]
"#;

pub(super) fn init(root: &Path) -> anyhow::Result<PathBuf> {
    let directory = root.join(".repo-set");
    fs::create_dir_all(&directory)
        .with_context(|| format!("Failed to create {}", directory.display()))?;
    let path = directory.join("config.toml");
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .with_context(|| {
            format!(
                "Failed to create {} (existing files are not overwritten)",
                path.display()
            )
        })?;
    file.write_all(INITIAL_CONFIG.as_bytes())
        .with_context(|| format!("Failed to write {}", path.display()))?;
    Ok(path)
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct Push {
    #[serde(default)]
    options: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    remote: Remote,
    #[serde(default)]
    repositories: Repo,
    #[serde(default)]
    clone: Clone,
    #[serde(default)]
    push: Push,
}

#[derive(Deserialize)]
#[serde(from = "FileConfig")]
pub(super) struct RepoSetConfig {
    pub common: Config,
    push: Push,
}

impl From<FileConfig> for RepoSetConfig {
    fn from(config: FileConfig) -> Self {
        Self {
            common: Config {
                remote: config.remote,
                repositories: config.repositories,
                clone: config.clone,
            },
            push: config.push,
        }
    }
}

impl RepoSetConfig {
    pub fn new(root: &Path) -> anyhow::Result<Self> {
        config::load_from_root(root)
    }

    pub fn push_options(&self) -> Vec<String> {
        self.push
            .options
            .iter()
            .map(|(key, value)| format!("{key}={value}"))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{INITIAL_CONFIG, RepoSetConfig};

    #[test]
    fn initial_config_is_valid() {
        let config: RepoSetConfig = toml::from_str(INITIAL_CONFIG).unwrap();
        assert!(!config.common.remote.url.is_empty());
        assert!(!config.common.remote.list.cmd.is_empty());
        assert!(config.common.repositories.ignore.is_empty());
        assert!(config.common.clone.post.cmd.is_empty());
        assert!(config.push_options().is_empty());
    }

    const COMMON: &str = r#"
        remote.url = "host:29418"
        remote.list.cmd = ["gerrit", "ls-projects"]
        repositories.ignore = ["ignored"]
        clone.post.cmd = ["git", "submodule", "update"]
    "#;

    #[test]
    fn parses_common_and_push_settings() {
        let config: RepoSetConfig = toml::from_str(&format!(
            "{COMMON}\npush.options = {{ l = \"Code-Review+2\" }}"
        ))
        .unwrap();

        assert_eq!(config.common.remote.url, "host:29418");
        assert_eq!(config.common.remote.list.cmd, ["gerrit", "ls-projects"]);
        assert_eq!(config.common.repositories.ignore, ["ignored"]);
        assert_eq!(config.common.clone.post.cmd, ["git", "submodule", "update"]);
        assert_eq!(config.push_options(), ["l=Code-Review+2"]);
    }

    #[test]
    fn push_options_default_to_empty() {
        let config: RepoSetConfig = toml::from_str(COMMON).unwrap();
        assert!(config.push_options().is_empty());
    }

    #[test]
    fn invalid_fields_fail() {
        for extra in [
            "push.unknown = \"value\"",
            "push.options = { l = 2 }",
            "unknown = \"value\"",
            "remote.unknown = \"value\"",
        ] {
            assert!(
                toml::from_str::<RepoSetConfig>(&format!("{COMMON}\n{extra}")).is_err(),
                "accepted invalid field: {extra}"
            );
        }
    }

    #[test]
    fn required_remote_fields_fail_when_missing() {
        for content in [
            "",
            "remote.url = \"host:29418\"",
            "remote.list.cmd = [\"gerrit\"]",
        ] {
            assert!(toml::from_str::<RepoSetConfig>(content).is_err());
        }
    }
}
