use anyhow::anyhow;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::{env::home_dir, fs};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Remote {
    pub url: String,
    pub list: List,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct List {
    pub cmd: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Repo {
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Clone {
    #[serde(default)]
    pub post: Post,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Post {
    #[serde(default)]
    pub cmd: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct Push {
    /// Arbitrary key/value pairs passed to `git push` as push options.
    #[serde(default)]
    pub options: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub remote: Remote,
    #[serde(default)]
    pub repository: Repo,
    #[serde(default)]
    pub clone: Clone,
    #[serde(default)]
    pub push: Push,
}

impl Config {
    pub fn new() -> anyhow::Result<Self> {
        let mut home = home_dir()
            .ok_or("Can't get user home directory")
            .map_err(anyhow::Error::msg)?;
        home.push(".repo-set.toml");

        if !fs::exists(&home)? {
            return Err(anyhow!("No user config found (~/.repo-set.toml)"));
        }

        let content: String = fs::read_to_string(&home)?;
        toml::from_str(content.as_str()).map_err(|e| anyhow!("{}: {}", home.display(), e))
    }
}

#[cfg(test)]
mod tests {
    use super::Config;

    fn parse(toml: &str) -> Config {
        toml::from_str(toml).expect("config should parse")
    }

    #[test]
    fn parses_full_config() {
        let config = parse(
            r#"
            remote.url = "my-gerrit:29418"
            remote.list.cmd = ["ssh", "gerrit", "ls-projects"]
            repository.ignore = ["a", "b"]
            clone.post.cmd = ["git", "submodule", "update"]
            "#,
        );

        assert_eq!(config.remote.url, "my-gerrit:29418");
        assert_eq!(config.remote.list.cmd, ["ssh", "gerrit", "ls-projects"]);
        assert_eq!(config.repository.ignore, ["a", "b"]);
        assert_eq!(config.clone.post.cmd, ["git", "submodule", "update"]);
    }

    #[test]
    fn remote_fields_are_mandatory() {
        let config = parse(
            r#"
            remote.url = "host:29418"
            remote.list.cmd = ["gerrit", "ls-projects"]
            "#,
        );

        assert_eq!(config.remote.url, "host:29418");
        assert_eq!(config.remote.list.cmd, ["gerrit", "ls-projects"]);
        assert!(config.repository.ignore.is_empty());
        assert!(config.clone.post.cmd.is_empty());
    }

    #[test]
    fn missing_list_cmd_fails() {
        let result = toml::from_str::<Config>(r#"remote.url = "host:29418""#);
        assert!(result.is_err());
    }

    #[test]
    fn missing_url_fails() {
        let result = toml::from_str::<Config>("repository.ignore = [\"a\"]");
        assert!(result.is_err());
    }

    #[test]
    fn unknown_fields_fail() {
        let result = toml::from_str::<Config>(
            r#"
            remote.url = "host:29418"
            remote.list.cmd = ["gerrit"]
            unknown = "value"
            "#,
        );

        assert!(result.is_err());
    }
}

