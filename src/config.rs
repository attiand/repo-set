use anyhow::anyhow;
use serde::Deserialize;
use std::{env::home_dir, fs};

#[derive(Deserialize)]
pub struct Remote {
    pub url: String,
    #[serde(default)]
    pub list: List,
}

#[derive(Deserialize, Default)]
pub struct List {
    #[serde(default)]
    pub cmd: Vec<String>,
}

#[derive(Deserialize, Default)]
pub struct Repo {
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Deserialize, Default)]
pub struct Clone {
    #[serde(default)]
    pub post: Post,
}

#[derive(Deserialize, Default)]
pub struct Post {
    #[serde(default)]
    pub cmd: Vec<String>,
}

#[derive(Deserialize)]
pub struct Config {
    pub remote: Remote,
    #[serde(default)]
    pub repo: Repo,
    #[serde(default)]
    pub clone: Clone,
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
        toml::from_str(content.as_str()).map_err(anyhow::Error::msg)
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
            repo.ignore = ["a", "b"]
            clone.post.cmd = ["git", "submodule", "update"]
            "#,
        );

        assert_eq!(config.remote.url, "my-gerrit:29418");
        assert_eq!(config.remote.list.cmd, ["ssh", "gerrit", "ls-projects"]);
        assert_eq!(config.repo.ignore, ["a", "b"]);
        assert_eq!(config.clone.post.cmd, ["git", "submodule", "update"]);
    }

    #[test]
    fn only_url_is_mandatory() {
        let config = parse(r#"remote.url = "host:29418""#);

        assert_eq!(config.remote.url, "host:29418");
        assert!(config.remote.list.cmd.is_empty());
        assert!(config.repo.ignore.is_empty());
        assert!(config.clone.post.cmd.is_empty());
    }

    #[test]
    fn missing_url_fails() {
        let result = toml::from_str::<Config>("repo.ignore = [\"a\"]");
        assert!(result.is_err());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let config = parse(
            r#"
            remote.url = "host:29418"
            unknown = "value"
            "#,
        );

        assert_eq!(config.remote.url, "host:29418");
    }
}

