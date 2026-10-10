use crate::config::Config;
use crate::{Cli, Commands};
use anyhow::{Result, anyhow};
use std::env::home_dir;
use std::path::{Path, PathBuf};

pub struct Parameters {
    pub root: PathBuf,
    pub remote_list_cmd: Vec<String>,
    pub ignore_repos: Vec<String>,
    pub push_options: Vec<String>,
    pub debug: bool,
    pub threads: usize,
    clone_post_cmd: Vec<String>,
    remote_url: String,
}

impl Parameters {
    pub fn new(config: &Config, cli: &Cli) -> Result<Self> {
        let remote_url = match cli.host.as_deref() {
            Some(host) => host.to_string(),
            None => expand_user(&config.remote.url)?,
        };
        let remote_list_cmd = config
            .remote
            .list
            .cmd
            .iter()
            .map(|arg| expand_user(arg))
            .collect::<Result<Vec<_>>>()?;
        let ignore_repos = if cli.ignore_repos.is_empty() {
            config.repositories.ignore.clone()
        } else {
            cli.ignore_repos.clone()
        };
        let mut merged_push_options: Vec<String> = config
            .push
            .options
            .iter()
            .map(|(key, value)| format!("{}={}", key, value))
            .collect();
        if let Commands::Push {
            push_option_cmd, ..
        } = &cli.command
        {
            merged_push_options.extend_from_slice(push_option_cmd);
        }

        Ok(Self {
            root: cli.root.clone(),
            remote_url,
            remote_list_cmd,
            clone_post_cmd: config.clone.post.cmd.clone(),
            ignore_repos,
            push_options: merged_push_options,
            debug: cli.debug > 0,
            threads: cli.threads.or(config.threads).unwrap_or(0),
        })
    }

    pub fn remote_repo_url(&self, repo: &str) -> String {
        format!("{}/{}", self.remote_url, repo)
    }

    pub fn local_repo_path(&self, repo: &str) -> PathBuf {
        self.root.join(repo)
    }

    pub fn clone_post_cmd(&self, dst: &Path) -> Result<Vec<String>> {
        if self.clone_post_cmd.is_empty() {
            return Ok(Vec::new());
        }

        let repo = dst
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default();
        let dest = std::fs::canonicalize(dst)?;
        let dest = dest.to_string_lossy();
        let home = if self
            .clone_post_cmd
            .iter()
            .any(|arg| arg.contains("${home}"))
        {
            Some(home_dir().ok_or_else(|| anyhow!("Can't get user home directory"))?)
        } else {
            None
        };

        Ok(self
            .clone_post_cmd
            .iter()
            .map(|arg| {
                let expanded = arg.replace("${repo}", &repo).replace("${dest}", &dest);
                match &home {
                    Some(home) => expanded.replace("${home}", &home.to_string_lossy()),
                    None => expanded,
                }
            })
            .collect())
    }
}

fn expand_user(value: &str) -> Result<String> {
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
    use super::Parameters;
    use crate::config::Config;
    use crate::{Cli, Commands};
    use clap::Parser;
    use std::env::home_dir;
    use std::path::Path;

    fn config() -> Config {
        toml::from_str(
            r#"
            remote.url = "host:29418"
            remote.list.cmd = ["gerrit", "ls-projects"]
            repositories.ignore = ["configured-repo"]
            clone.post.cmd = ["cp", "hook", "${dest}/.git/hooks/hook"]
            push.options = { l = "Code-Review+2" }
            "#,
        )
        .unwrap()
    }

    fn resolve(
        config: &Config,
        host: Option<&str>,
        ignore_repos: &[String],
        push_options: &[String],
        debug: bool,
        root: &Path,
    ) -> anyhow::Result<Parameters> {
        let cli = Cli {
            root: root.to_path_buf(),
            host: host.map(String::from),
            debug: u8::from(debug),
            ignore_repos: ignore_repos.to_vec(),
            threads: None,
            command: Commands::Push {
                refspec: "HEAD:refs/heads/main".to_string(),
                push_option_cmd: push_options.to_vec(),
            },
        };
        Parameters::new(config, &cli)
    }

    #[test]
    fn thread_count_precedence() {
        for (configured, args, expected) in [
            (None, vec!["repo-set", "pull"], 0),
            (Some(6), vec!["repo-set", "pull"], 6),
            (Some(0), vec!["repo-set", "pull"], 0),
            (None, vec!["repo-set", "--threads", "3", "pull"], 3),
            (Some(6), vec!["repo-set", "--threads", "3", "pull"], 3),
            (Some(6), vec!["repo-set", "--threads", "0", "pull"], 0),
            (Some(6), vec!["repo-set", "-t", "2", "pull"], 2),
        ] {
            let mut config = config();
            config.threads = configured;
            let cli = Cli::try_parse_from(args).unwrap();
            let parameters = Parameters::new(&config, &cli).unwrap();

            assert_eq!(parameters.threads, expected);
        }
    }

    #[test]
    fn resolves_push_command_parameters() {
        let cli = Cli::try_parse_from([
            "repo-set",
            "--debug",
            "--threads",
            "3",
            "push",
            "--push-option",
            "m=message",
            "HEAD:refs/for/main",
        ])
        .unwrap();
        let parameters = Parameters::new(&config(), &cli).unwrap();

        assert!(parameters.debug);
        assert_eq!(parameters.threads, 3);
        assert_eq!(parameters.push_options, ["l=Code-Review+2", "m=message"]);
    }

    #[test]
    fn uses_configured_parameters_without_overrides() {
        let config = config();
        let parameters = resolve(&config, None, &[], &[], false, Path::new(".")).unwrap();

        assert_eq!(parameters.root, Path::new("."));
        assert_eq!(parameters.remote_url, config.remote.url);
        assert_eq!(parameters.remote_list_cmd, config.remote.list.cmd);
        assert_eq!(parameters.clone_post_cmd, config.clone.post.cmd);
        assert_eq!(parameters.ignore_repos, config.repositories.ignore);
        assert_eq!(parameters.push_options, ["l=Code-Review+2"]);
    }

    #[test]
    fn preserves_debug_setting() {
        for debug in [false, true] {
            let parameters = resolve(&config(), None, &[], &[], debug, Path::new(".")).unwrap();
            assert_eq!(parameters.debug, debug);
        }
    }

    #[test]
    fn cli_overrides_host_and_ignore_list() {
        let config = config();
        let parameters = resolve(
            &config,
            Some("other-host:29418"),
            &["cli-repo".to_string()],
            &[],
            false,
            Path::new("."),
        )
        .unwrap();

        assert_eq!(parameters.remote_url, "other-host:29418");
        assert_eq!(parameters.ignore_repos, ["cli-repo"]);
        assert_eq!(parameters.remote_list_cmd, config.remote.list.cmd);
        assert_eq!(config.repositories.ignore, ["configured-repo"]);
    }

    #[test]
    fn appends_cli_push_options_in_order() {
        let parameters = resolve(
            &config(),
            None,
            &[],
            &["l=Code-Review+1".to_string(), "m=message".to_string()],
            false,
            Path::new("."),
        )
        .unwrap();

        assert_eq!(
            parameters.push_options,
            ["l=Code-Review+2", "l=Code-Review+1", "m=message"]
        );
    }

    #[test]
    fn expands_user_in_remote_parameters() {
        let mut config = config();
        config.remote.url = "ssh://${user}@host:29418".to_string();
        config.remote.list.cmd = vec!["ssh".to_string(), "${user}@host".to_string()];
        let parameters = resolve(&config, None, &[], &[], false, Path::new(".")).unwrap();
        let user = std::env::var("USER")
            .or_else(|_| std::env::var("USERNAME"))
            .or_else(|_| std::env::var("LOGNAME"))
            .unwrap();

        assert_eq!(parameters.remote_url, format!("ssh://{}@host:29418", user));
        assert_eq!(
            parameters.remote_list_cmd,
            ["ssh", &format!("{}@host", user)]
        );
    }

    #[test]
    fn resolves_clone_url_and_destination() {
        let parameters = resolve(
            &config(),
            Some("ssh://other-host:29418"),
            &[],
            &[],
            false,
            Path::new("/tmp/repo-set"),
        )
        .unwrap();

        assert_eq!(
            parameters.remote_repo_url("team/repo-a"),
            "ssh://other-host:29418/team/repo-a"
        );
        assert_eq!(
            parameters.local_repo_path("team/repo-a"),
            Path::new("/tmp/repo-set/team/repo-a")
        );
    }

    #[test]
    fn expands_clone_post_command_variables() {
        let mut config = config();
        config.clone.post.cmd = vec!["${home}/${repo}:${dest}".to_string()];
        let parameters = resolve(&config, None, &[], &[], false, Path::new(".")).unwrap();
        let dest = std::env::current_dir().unwrap();
        let expanded = parameters.clone_post_cmd(&dest).unwrap();
        let repo = dest.file_name().unwrap().to_string_lossy();
        let absolute_dest = std::fs::canonicalize(&dest).unwrap();
        let home = home_dir().unwrap().to_string_lossy().into_owned();

        assert_eq!(
            expanded,
            [format!("{}/{}:{}", home, repo, absolute_dest.display())]
        );
        assert_eq!(parameters.clone_post_cmd, config.clone.post.cmd);
    }

    #[test]
    fn empty_clone_post_command_does_not_resolve_destination() {
        let mut config = config();
        config.clone.post.cmd.clear();
        let parameters = resolve(&config, None, &[], &[], false, Path::new(".")).unwrap();

        assert!(parameters.clone_post_cmd(Path::new("")).unwrap().is_empty());
    }

    #[test]
    fn clone_post_command_requires_existing_destination() {
        let parameters = resolve(&config(), None, &[], &[], false, Path::new(".")).unwrap();

        assert!(parameters.clone_post_cmd(Path::new("")).is_err());
    }

    #[test]
    fn preserves_unknown_variables_and_literal_arguments() {
        let mut config = config();
        config.clone.post.cmd = vec!["cp".to_string(), "${unknown}".to_string()];
        let parameters = resolve(&config, None, &[], &[], false, Path::new(".")).unwrap();
        let expanded = parameters
            .clone_post_cmd(&std::env::current_dir().unwrap())
            .unwrap();

        assert_eq!(expanded, config.clone.post.cmd);
    }
}
