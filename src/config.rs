use anyhow::anyhow;
use serde::Deserialize;
use serde::de::{DeserializeOwned, IgnoredAny};
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

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileConfig {
    pub remote: Remote,
    #[serde(default)]
    pub repositories: Repo,
    #[serde(default)]
    pub clone: Clone,
    #[serde(default)]
    #[serde(rename = "push")]
    _push: Option<IgnoredAny>,
}

#[derive(Deserialize)]
#[serde(from = "FileConfig")]
pub struct Config {
    pub remote: Remote,
    pub repositories: Repo,
    pub clone: Clone,
}

impl From<FileConfig> for Config {
    fn from(config: FileConfig) -> Self {
        Self {
            remote: config.remote,
            repositories: config.repositories,
            clone: config.clone,
        }
    }
}

impl Config {
    pub fn new(root: &Path) -> anyhow::Result<Self> {
        load_from_root(root)
    }

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
}

pub fn load_from_root<T: DeserializeOwned>(root: &Path) -> anyhow::Result<T> {
    load_config(&repo_set_config_path(root))
}

pub fn load_from_path<T: DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    load_config(path)
}

fn load_config<T: DeserializeOwned>(path: &Path) -> anyhow::Result<T> {
    if !fs::exists(path)? {
        return Err(anyhow!("No config file found at {}", path.display()));
    }

    let content: String = fs::read_to_string(path)?;
    toml::from_str(content.as_str()).map_err(|e| anyhow!("{}: {}", path.display(), e))
}

fn repo_set_config_path(root: &Path) -> PathBuf {
    root.join(".repo-set").join("config.toml")
}

pub fn expand_clone_post_cmd(
    command: &[String],
    repo: &str,
    dest: &Path,
    root: &Path,
) -> anyhow::Result<Vec<String>> {
    let dest = dest.to_string_lossy();
    let root = if root.is_absolute() {
        root.to_path_buf()
    } else {
        std::env::current_dir()?.join(root)
    };
    let root = root.to_string_lossy();
    let home = if command.iter().any(|arg| arg.contains("${home}")) {
        Some(home_dir().ok_or_else(|| anyhow!("Can't get user home directory"))?)
    } else {
        None
    };

    Ok(command
        .iter()
        .map(|arg| {
            let expanded = arg
                .replace("${repo}", repo)
                .replace("${dest}", &dest)
                .replace("${root}", &root);
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
        .map_err(|_| {
            anyhow!("configuration uses ${{user}}, but the current user could not be determined")
        })?;

    Ok(value.replace("${user}", &user))
}

#[cfg(test)]
mod tests {
    use super::{Config, expand_clone_post_cmd, load_from_root, repo_set_config_path};
    use std::env::home_dir;
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    #[test]
    fn repo_set_config_path_uses_the_root_directory() {
        assert_eq!(
            repo_set_config_path(Path::new("/work/repos")),
            PathBuf::from("/work/repos/.repo-set/config.toml")
        );
    }

    #[test]
    fn repo_set_loads_config_from_the_root() {
        let root =
            std::env::temp_dir().join(format!("repo-set-config-test-{}", std::process::id()));
        let config_dir = root.join(".repo-set");
        fs::create_dir_all(&config_dir).unwrap();
        fs::write(
            repo_set_config_path(&root),
            "[remote]\nurl = \"host\"\nlist.cmd = [\"list\"]\n",
        )
        .unwrap();

        let config: Config = load_from_root(&root).unwrap();
        assert_eq!(config.remote.url, "host");

        fs::remove_file(repo_set_config_path(&root)).unwrap();
        fs::remove_dir(&config_dir).unwrap();
        fs::remove_dir(&root).unwrap();
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
    fn shared_config_accepts_repo_set_settings() {
        let config = parse(
            r#"
            remote.url = "host:29418"
            remote.list.cmd = ["gerrit", "ls-projects"]
            push.options = { l = "Code-Review+2" }
            "#,
        );

        assert_eq!(config.remote.url, "host:29418");
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
        let command = vec!["${home}/${repo}:${dest}:${root}".to_string()];
        let expanded = expand_clone_post_cmd(
            &command,
            "repo-a",
            Path::new("/tmp/repos/repo-a"),
            Path::new("/tmp/repos"),
        )
        .unwrap();
        let home = home_dir().unwrap().to_string_lossy().into_owned();

        assert_eq!(
            expanded,
            [format!("{}/repo-a:/tmp/repos/repo-a:/tmp/repos", home)]
        );
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
