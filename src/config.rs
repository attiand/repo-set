use anyhow::anyhow;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;
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
    #[serde(default)]
    pub threads: Option<usize>,
    pub remote: Remote,
    #[serde(default)]
    pub repositories: Repo,
    #[serde(default)]
    pub clone: Clone,
    #[serde(default)]
    pub push: Push,
}

impl Config {
    pub fn new() -> anyhow::Result<Self> {
        let path = resolve_config_path(
            std::env::var_os("REPO_SET_CONFIG").map(PathBuf::from),
            home_dir(),
        )?;

        if !fs::exists(&path)? {
            return Err(anyhow!("No config file found at {}", path.display()));
        }

        let content: String = fs::read_to_string(&path)?;
        toml::from_str(content.as_str()).map_err(|e| anyhow!("{}: {}", path.display(), e))
    }
}

fn resolve_config_path(
    override_path: Option<PathBuf>,
    home: Option<PathBuf>,
) -> anyhow::Result<PathBuf> {
    if let Some(path) = override_path {
        return Ok(path);
    }

    let mut home = home.ok_or_else(|| anyhow!("Can't get user home directory"))?;
    home.push(".repo-set.toml");
    Ok(home)
}

#[cfg(test)]
mod tests {
    use super::{Config, resolve_config_path};
    use std::path::PathBuf;

    #[test]
    fn config_path_uses_environment_override() {
        let path = resolve_config_path(Some(PathBuf::from("/tmp/custom.toml")), None).unwrap();
        assert_eq!(path, PathBuf::from("/tmp/custom.toml"));
    }

    #[test]
    fn config_path_falls_back_to_home() {
        let path = resolve_config_path(None, Some(PathBuf::from("/home/tester"))).unwrap();
        assert_eq!(path, PathBuf::from("/home/tester/.repo-set.toml"));
    }

    fn parse(toml: &str) -> Config {
        toml::from_str(toml).expect("config should parse")
    }

    #[test]
    fn parses_full_config() {
        let config = parse(
            r#"
            threads = 6
            remote.url = "my-gerrit:29418"
            remote.list.cmd = ["ssh", "gerrit", "ls-projects"]
            repositories.ignore = ["a", "b"]
            clone.post.cmd = ["git", "submodule", "update"]
            push.options = { l = "Code-Review+2" }
            "#,
        );

        assert_eq!(config.threads, Some(6));
        assert_eq!(config.remote.url, "my-gerrit:29418");
        assert_eq!(config.remote.list.cmd, ["ssh", "gerrit", "ls-projects"]);
        assert_eq!(config.repositories.ignore, ["a", "b"]);
        assert_eq!(config.clone.post.cmd, ["git", "submodule", "update"]);
        assert_eq!(config.push.options.get("l").unwrap(), "Code-Review+2");
    }

    #[test]
    fn remote_fields_are_mandatory() {
        let config = parse(
            r#"
            remote.url = "host:29418"
            remote.list.cmd = ["gerrit", "ls-projects"]
            "#,
        );

        assert_eq!(config.threads, None);
        assert_eq!(config.remote.url, "host:29418");
        assert_eq!(config.remote.list.cmd, ["gerrit", "ls-projects"]);
        assert!(config.repositories.ignore.is_empty());
        assert!(config.clone.post.cmd.is_empty());
    }

    #[test]
    fn missing_list_cmd_fails() {
        let result = toml::from_str::<Config>(r#"remote.url = "host:29418""#);
        assert!(result.is_err());
    }

    #[test]
    fn missing_url_fails() {
        let result = toml::from_str::<Config>("repositories.ignore = [\"a\"]");
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
