#[path = "../pool.rs"]
mod pool;
#[path = "../progress.rs"]
mod progress;
#[path = "remote-repo/config.rs"]
mod remote_repo_config;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::env::Shells;
use clap_complete::{ArgValueCompleter, CompleteEnv, CompletionCandidate, Shell};
use crossbeam_channel::unbounded;
use glob::Pattern;
use repo_set::config::RepositoryFilter;
use repo_set::remote;
use std::collections::HashSet;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Parser)]
#[command(
    name = "remote-repo",
    version,
    about = "Clone one or several Git repositories"
)]
// Manage remote Git repositories
struct Cli {
    /// Specify the root directory
    #[arg(short, long, global = true, default_value = ".", value_name = "DIR", value_hint = clap::ValueHint::DirPath)]
    root: PathBuf,

    /// Override the configured remote URL for cloning
    #[arg(long, global = true, value_name = "HOST", value_hint = clap::ValueHint::Hostname)]
    host: Option<String>,

    /// Print debug information to debug.log in the root directory; repeat to increase detail
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    debug: u8,

    /// Number of worker threads; defaults to an I/O-friendly count
    #[arg(short, long, global = true, default_value_t = 0)]
    threads: usize,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Clone one or several Git repositories
    #[clap(alias = "c")]
    Clone {
        /// Repository names or glob patterns to clone
        #[arg(value_name = "REPO-NAME", num_args = 1.., required_unless_present = "all",
            add = ArgValueCompleter::new(complete_repositories))]
        repos: Vec<String>,

        /// Clone all remote repositories allowed by configuration
        #[arg(long, conflicts_with = "repos")]
        all: bool,
    },

    /// List remote repositories allowed by configuration
    #[clap(alias = "l")]
    List,

    /// Generate shell completion script
    #[command(alias = "comp")]
    Completion {
        /// Shell to generate the completion script for
        #[arg(value_name = "SHELL")]
        shell: Shell,
    },
}

fn complete_repositories(current: &OsStr) -> Vec<CompletionCandidate> {
    let Some(prefix) = current.to_str() else {
        return Vec::new();
    };
    let repositories = || -> Result<Vec<String>> {
        let config = remote_repo_config::load()?;
        let list_cmd = config.remote_list_cmd()?;
        let repository_filter =
            RepositoryFilter::new(&config.repositories.include, &config.repositories.exclude)?;
        remote::Remote::new(&list_cmd, &[], Path::new("."), &repository_filter, 0).repo_list()
    };
    repositories()
        .unwrap_or_default()
        .into_iter()
        .filter(|repo| repo.starts_with(prefix))
        .map(CompletionCandidate::new)
        .collect()
}

fn is_glob_pattern(repository: &str) -> bool {
    repository
        .chars()
        .any(|character| matches!(character, '*' | '?' | '[' | '{'))
}

fn main() -> Result<()> {
    CompleteEnv::with_factory(Cli::command).complete();
    let cli = Cli::parse();
    if let Commands::Completion { shell } = &cli.command {
        let shells = Shells::builtins();
        let completer = shells
            .completer(&shell.to_string())
            .context("Unsupported completion shell")?;
        completer.write_registration(
            "COMPLETE",
            "remote-repo",
            "remote-repo",
            "remote-repo",
            &mut std::io::stdout(),
        )?;
        return Ok(());
    }

    let config = remote_repo_config::load()?;
    let list_cmd = config.remote_list_cmd()?;
    let repository_filter =
        RepositoryFilter::new(&config.repositories.include, &config.repositories.exclude)?;

    match cli.command {
        Commands::List => {
            let remote = remote::Remote::new(
                &list_cmd,
                &[],
                Path::new("."),
                &repository_filter,
                cli.debug,
            );
            for repo in remote.repo_list()? {
                println!("{repo}");
            }
        }
        Commands::Clone { repos, all } => {
            let configured_url = config.remote_url()?;
            let base_url = &cli.host.as_deref().unwrap_or(&configured_url);
            let remote = remote::Remote::new(
                &list_cmd,
                &config.clone.post.cmd,
                &cli.root,
                &repository_filter,
                cli.debug,
            );
            let repos = if all {
                remote.repo_list()?
            } else {
                let remote_repos = if repos.iter().any(|repo| is_glob_pattern(repo)) {
                    remote.repo_list()?
                } else {
                    Vec::new()
                };
                let mut selected = Vec::new();
                let mut selected_set = HashSet::new();
                for requested in repos {
                    if is_glob_pattern(&requested) {
                        let pattern = Pattern::new(&requested)
                            .with_context(|| format!("Invalid repository glob {requested:?}"))?;
                        for repository in &remote_repos {
                            if pattern.matches(repository)
                                && selected_set.insert(repository.clone())
                            {
                                selected.push(repository.clone());
                            }
                        }
                    } else if repository_filter.includes(&requested)
                        && selected_set.insert(requested.clone())
                    {
                        selected.push(requested);
                    }
                }
                selected
            };

            if cli.debug > 0 && !repos.is_empty() {
                std::fs::create_dir_all(&cli.root)?;
            }

            let workers = pool::worker_count(cli.threads, repos.len());
            let (sender, receiver) = unbounded::<progress::Update>();
            let consumer = progress::create_writer(receiver, repos.len(), workers, "Cloning");
            let failures = Mutex::new(Vec::new());

            pool::for_each_io(&repos, cli.threads, |slot, repo| {
                let dest = cli.root.join(repo);
                let exists = dest.try_exists();
                let skip = matches!(&exists, Ok(true));
                sender
                    .send(progress::Update::Start {
                        slot,
                        label: if skip { "Skipping" } else { "Cloning" },
                        repo: repo.clone(),
                    })
                    .unwrap();

                let error = match exists {
                    Ok(true) => None,
                    Ok(false) => remote
                        .clone(&format!("{base_url}/{repo}"), &dest)
                        .err()
                        .map(|error| {
                            format!("Failed to clone {repo} into {}: {error}", dest.display())
                        }),
                    Err(error) => Some(format!("Failed to inspect {}: {error}", dest.display())),
                };

                if let Some(error) = &error {
                    failures.lock().unwrap().push(error.clone());
                }
                sender
                    .send(progress::Update::Finish { slot, error })
                    .unwrap();
            });

            drop(sender);
            consumer.join().unwrap()?;
            let failures = failures.into_inner().unwrap();
            if !failures.is_empty() {
                anyhow::bail!("{}", failures.join("\n"));
            }
        }
        Commands::Completion { .. } => unreachable!(),
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{Cli, Commands};
    use clap::Parser;
    use clap_complete::Shell;

    #[test]
    fn accepts_completion_without_clone_arguments() {
        for command in ["completion", "comp"] {
            let cli = Cli::try_parse_from(["remote-repo", command, "bash"]).unwrap();
            assert!(matches!(
                cli.command,
                Commands::Completion { shell: Shell::Bash }
            ));
        }
    }

    #[test]
    fn completion_requires_a_supported_shell() {
        assert!(Cli::try_parse_from(["remote-repo", "completion"]).is_err());
        assert!(Cli::try_parse_from(["remote-repo", "completion", "invalid"]).is_err());
    }

    #[test]
    fn clone_options_require_the_clone_subcommand() {
        assert!(Cli::try_parse_from(["remote-repo", "--all"]).is_err());
        assert!(Cli::try_parse_from(["remote-repo", "repo-a"]).is_err());
    }

    #[test]
    fn completion_can_still_be_used_as_a_repository_name() {
        let cli = Cli::try_parse_from(["remote-repo", "clone", "--", "completion"]).unwrap();
        let repos = match cli.command {
            Commands::Clone { repos, .. } => repos,
            _ => panic!("expected clone command"),
        };
        assert_eq!(repos, ["completion"]);
    }

    #[test]
    fn accepts_repository_names() {
        let cli = Cli::try_parse_from(["remote-repo", "clone", "repo-a", "repo-b"]).unwrap();
        let (repos, all) = match cli.command {
            Commands::Clone { repos, all, .. } => (repos, all),
            _ => panic!("expected clone command"),
        };
        assert_eq!(repos, ["repo-a", "repo-b"]);
        assert!(!all);
    }

    #[test]
    fn accepts_all() {
        let cli = Cli::try_parse_from(["remote-repo", "clone", "--all"]).unwrap();
        let (repos, all) = match cli.command {
            Commands::Clone { repos, all, .. } => (repos, all),
            _ => panic!("expected clone command"),
        };
        assert!(all);
        assert!(repos.is_empty());
    }

    #[test]
    fn requires_repository_names_or_all() {
        assert!(Cli::try_parse_from(["remote-repo", "clone"]).is_err());
    }

    #[test]
    fn rejects_repository_names_with_all() {
        assert!(Cli::try_parse_from(["remote-repo", "clone", "--all", "repo-a"]).is_err());
        assert!(Cli::try_parse_from(["remote-repo", "clone", "repo-a", "--all"]).is_err());
    }

    #[test]
    fn accepts_root_with_either_selection() {
        for selection in ["repo-a", "--all"] {
            let cli = Cli::try_parse_from(["remote-repo", "--root", "repos", "clone", selection])
                .unwrap();
            assert_eq!(cli.root, std::path::PathBuf::from("repos"));
        }
    }

    #[test]
    fn accepts_root_after_clone_subcommand() {
        let cli =
            Cli::try_parse_from(["remote-repo", "clone", "--root", "repos", "repo-a"]).unwrap();
        assert_eq!(cli.root, std::path::PathBuf::from("repos"));
    }

    #[test]
    fn debug_option_counts_occurrences_before_or_after_subcommand() {
        let cli =
            Cli::try_parse_from(["remote-repo", "--debug", "--debug", "clone", "--all"]).unwrap();
        assert_eq!(cli.debug, 2);

        let cli = Cli::try_parse_from(["remote-repo", "clone", "--debug", "--all"]).unwrap();
        assert_eq!(cli.debug, 1);
    }

    #[test]
    fn threads_option_is_global() {
        let cli = Cli::try_parse_from(["remote-repo", "--threads", "3", "clone", "--all"]).unwrap();
        assert_eq!(cli.threads, 3);

        let cli = Cli::try_parse_from(["remote-repo", "clone", "--threads", "2", "--all"]).unwrap();
        assert_eq!(cli.threads, 2);
    }
}
