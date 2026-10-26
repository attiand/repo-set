#[path = "repo-clone/config.rs"]
mod repo_clone_config;

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::env::Shells;
use clap_complete::{ArgValueCompleter, CompleteEnv, CompletionCandidate, Shell};
use repo_set::config::RepositoryFilter;
use repo_set::remote;
use std::ffi::OsStr;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(
    name = "repo-clone",
    version,
    about = "Clone one or several Git repositories",
    subcommand_negates_reqs = true,
    args_conflicts_with_subcommands = true
)]
struct Cli {
    /// Specify the root directory
    #[arg(short, long, default_value = ".", value_name = "DIR", value_hint = clap::ValueHint::DirPath)]
    root: PathBuf,

    /// Repository names to clone
    #[arg(value_name = "REPO-NAME", num_args = 1.., required_unless_present = "all",
        add = ArgValueCompleter::new(complete_repositories))]
    repos: Vec<String>,

    /// Clone all remote repositories allowed by include/exclude patterns
    #[arg(long, conflicts_with = "repos")]
    all: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
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
        let config = repo_clone_config::load()?;
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

fn main() -> Result<()> {
    CompleteEnv::with_factory(Cli::command).complete();
    let cli = Cli::parse();
    if let Some(Commands::Completion { shell }) = &cli.command {
        let shells = Shells::builtins();
        let completer = shells
            .completer(&shell.to_string())
            .context("Unsupported completion shell")?;
        completer.write_registration(
            "COMPLETE",
            "repo-clone",
            "repo-clone",
            "repo-clone",
            &mut std::io::stdout(),
        )?;
        return Ok(());
    }

    let config = repo_clone_config::load()?;
    let base_url = config.remote_url()?;
    let list_cmd = config.remote_list_cmd()?;
    let repository_filter =
        RepositoryFilter::new(&config.repositories.include, &config.repositories.exclude)?;
    let remote = remote::Remote::new(
        &list_cmd,
        &config.clone.post.cmd,
        &cli.root,
        &repository_filter,
        0,
    );
    let repos = if cli.all {
        remote.repo_list()?
    } else {
        cli.repos
            .into_iter()
            .filter(|repo| repository_filter.includes(repo))
            .collect()
    };

    for repo in repos {
        let dest = cli.root.join(&repo);
        if dest.try_exists()? {
            println!("Skipping {repo}");
            continue;
        }

        println!("Cloning {repo}");
        remote
            .clone(&format!("{base_url}/{repo}"), &dest)
            .with_context(|| format!("Failed to clone {repo} into {}", dest.display()))?;
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
            let cli = Cli::try_parse_from(["repo-clone", command, "bash"]).unwrap();
            assert!(matches!(
                cli.command,
                Some(Commands::Completion { shell: Shell::Bash })
            ));
        }
    }

    #[test]
    fn completion_requires_a_supported_shell() {
        assert!(Cli::try_parse_from(["repo-clone", "completion"]).is_err());
        assert!(Cli::try_parse_from(["repo-clone", "completion", "invalid"]).is_err());
    }

    #[test]
    fn completion_conflicts_with_clone_selection() {
        assert!(Cli::try_parse_from(["repo-clone", "--all", "completion", "bash"]).is_err());
    }

    #[test]
    fn completion_can_still_be_used_as_a_repository_name() {
        let cli = Cli::try_parse_from(["repo-clone", "--", "completion"]).unwrap();
        assert_eq!(cli.repos, ["completion"]);
        assert!(cli.command.is_none());
    }

    #[test]
    fn accepts_repository_names() {
        let cli = Cli::try_parse_from(["repo-clone", "repo-a", "repo-b"]).unwrap();
        assert_eq!(cli.repos, ["repo-a", "repo-b"]);
        assert!(!cli.all);
    }

    #[test]
    fn accepts_all() {
        let cli = Cli::try_parse_from(["repo-clone", "--all"]).unwrap();
        assert!(cli.all);
        assert!(cli.repos.is_empty());
    }

    #[test]
    fn requires_repository_names_or_all() {
        assert!(Cli::try_parse_from(["repo-clone"]).is_err());
    }

    #[test]
    fn rejects_repository_names_with_all() {
        assert!(Cli::try_parse_from(["repo-clone", "--all", "repo-a"]).is_err());
        assert!(Cli::try_parse_from(["repo-clone", "repo-a", "--all"]).is_err());
    }

    #[test]
    fn accepts_root_with_either_selection() {
        for selection in ["repo-a", "--all"] {
            let cli = Cli::try_parse_from(["repo-clone", "--root", "repos", selection]).unwrap();
            assert_eq!(cli.root, std::path::PathBuf::from("repos"));
        }
    }
}
