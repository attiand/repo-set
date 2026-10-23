use anyhow::anyhow;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
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
    pub repositories: Repo,
    #[serde(default)]
    pub clone: Clone,
    #[serde(default)]
    pub push: Push,
}

impl Config {
    pub fn remote_url(&self) -> anyhow::Result<String> {
        expand_user(&self.remote.url)
    }

    pub fn remote_list_cmd(&self) -> anyhow::Result<Vec<String>> {
        self.remote
            .list
            .cmd
            .iter()
            .map(|arg| expand_user(arg))
            .collect()
    }

    pub fn push_options(&self) -> Vec<String> {
        self.push
            .options
            .iter()
            .map(|(key, value)| format!("{}={}", key, value))
            .collect()
    }

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

pub fn expand_clone_post_cmd(
    command: &[String],
    repo: &str,
    dest: &Path,
) -> anyhow::Result<Vec<String>> {
    let dest = dest.to_string_lossy();
    let home = if command.iter().any(|arg| arg.contains("${home}")) {
        Some(home_dir().ok_or_else(|| anyhow!("Can't get user home directory"))?)
    } else {
        None
    };

    Ok(command
        .iter()
        .map(|arg| {
            let expanded = arg.replace("${repo}", repo).replace("${dest}", &dest);
            match &home {
                Some(home) => expanded.replace("${home}", &home.to_string_lossy()),
                None => expanded,
            }
        })
        .collect())
}

fn expand_user(value: &str) -> anyhow::Result<String> {
    if !value.contains("${user}") {
        return Ok(value.to_string());
    }

    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .or_else(|_| std::env::var("LOGNAME"))
        .map_err(|_| anyhow!("configuration uses ${{user}}, but the current user could not be determined"))?;

    Ok(value.replace("${user}", &user))
}

#[cfg(test)]
mod tests {
    use super::{Config, expand_clone_post_cmd, resolve_config_path};
    use std::env::home_dir;
    use std::path::Path;
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
            remote.url = "my-gerrit:29418"
            remote.list.cmd = ["ssh", "gerrit", "ls-projects"]
            repositories.ignore = ["a", "b"]
            clone.post.cmd = ["git", "submodule", "update"]
            push.options = { l = "Code-Review+2" }
            "#,
        );

        assert_eq!(config.remote.url, "my-gerrit:29418");
        assert_eq!(config.remote.list.cmd, ["ssh", "gerrit", "ls-projects"]);
        assert_eq!(config.repositories.ignore, ["a", "b"]);
        assert_eq!(config.clone.post.cmd, ["git", "submodule", "update"]);
        assert_eq!(config.push_options(), ["l=Code-Review+2"]);
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
        assert!(config.repositories.ignore.is_empty());
        assert!(config.clone.post.cmd.is_empty());
    }

    #[test]
    fn remote_url_expands_user() {
        let config = parse(
            r#"
            remote.url = "ssh://${user}@gerrit.example:29418"
            remote.list.cmd = ["gerrit", "ls-projects"]
            "#,
        );

        assert_eq!(
            config.remote_url().unwrap(),
            format!(
                "ssh://{}@gerrit.example:29418",
                std::env::var("USER")
                    .or_else(|_| std::env::var("USERNAME"))
                    .or_else(|_| std::env::var("LOGNAME"))
                    .unwrap()
            )
        );
    }

    #[test]
    fn remote_list_cmd_expands_user() {
        let config = parse(
            r#"
            remote.url = "host:29418"
            remote.list.cmd = ["ssh", "${user}@host", "gerrit", "ls-projects"]
            "#,
        );
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap();

        assert_eq!(
            config.remote_list_cmd().unwrap(),
            ["ssh", &format!("{}@host", user), "gerrit", "ls-projects"]
        );
    }

    #[test]
    fn expands_clone_post_command_variables() {
        let command = vec!["${home}/${repo}:${dest}".to_string()];
        let expanded = expand_clone_post_cmd(
            &command,
            "repo-a",
            Path::new("/tmp/repo-a"),
        )
        .unwrap();
        let home = home_dir().unwrap().to_string_lossy().into_owned();

        assert_eq!(expanded, [format!("{}/repo-a:/tmp/repo-a", home)]);
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

