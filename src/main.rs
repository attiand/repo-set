mod config;
mod local;
mod pool;
mod remote;
mod status;

mod progress;

use anyhow::Result;
use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use crossbeam_channel::unbounded;
use std::path::PathBuf;

/// Simplify managing a set of git repositories
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Specify the repo set root directory
    #[clap(short, long, num_args(1), value_name("DIR"), default_value = ".", value_hint = clap::ValueHint::DirPath)]
    root: PathBuf,

    /// Specify the git remote host, overrides the value specified in the configuration
    #[clap(long, num_args(1), value_name("HOST"), value_hint = clap::ValueHint::Hostname)]
    host: Option<String>,

    /// Print debug information
    #[arg(long, action = clap::ArgAction::Count)]
    debug: u8,

    /// Repository name to ignore, may be specified multiple times
    #[arg(long = "ignore-repo", num_args(1), value_name("REPO-NAME"))]
    ignore_repos: Vec<String>,

    /// Number of worker threads to use, defaults to an I/O-friendly count if not specified.
    #[arg(short, long, default_value_t = 0)]
    threads: usize,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum ListType {
    /// List remote repositories
    #[command(name = "--remote", visible_alias = "-r")]
    Remote,
    /// List local repositories
    #[command(name = "--local", visible_alias = "-l")]
    Local,
    /// List difference between local and remote repositories
    #[command(name = "--diff", visible_alias = "-d")]
    Diff,
    /// List superfluous local repositories
    #[command(name = "--superfluous", visible_alias = "-s")]
    Superfluous,
}

#[derive(Subcommand)]
enum Commands {
    /// Clone repositories, ignoring existing ones
    #[clap(alias = "c")]
    Clone {
        /// Repository names to clone
        #[clap(value_name = "REPO-NAME", num_args = 1.., required_unless_present = "all")]
        repos: Vec<String>,

        /// Clone all repositories
        #[arg(long, default_value_t = false, conflicts_with = "repos")]
        all: bool,
    },
    /// List repositories
    #[clap(alias = "ls")]
    List {
        #[command(subcommand)]
        mode: Option<ListType>,
    },
    /// Pull all local repositories
    #[clap(alias = "p")]
    Pull,
    /// Fetch all local repositories without updating the working tree
    #[clap(alias = "f")]
    Fetch,
    /// Push local repositories that are ahead of their tracking branch
    Push {
        /// Refspec to push, for example HEAD:refs/heads/master
        #[arg(value_name = "REFSPEC")]
        refspec: String,
    },
    /// List local repositories with an unclean working tree (git status --short)
    #[clap(alias = "s")]
    Status,
    /// Reset all local repositories, discarding local changes
    #[clap(alias = "r")]
    Reset {
        /// Reset the working tree to HEAD, discarding all local changes
        #[arg(long, required = true)]
        hard: bool,

        /// Commit or revspec to reset to (for example origin/master); defaults to HEAD
        #[arg(value_name = "COMMIT")]
        commit: Option<String>,
    },
    /// Remove untracked files and directories from all local repositories
    Clean {
        /// Force removal; without it, list what would be removed
        #[arg(long, default_value_t = false)]
        force: bool,
    },
    /// Stage changes in all local repositories
    #[clap(aliases = ["add", "a"])]
    Stage {
        /// Stage modifications and deletions of tracked files
        #[arg(short, long, required = true)]
        update: bool,
    },
    /// Commit staged changes in all local repositories
    Commit {
        /// Commit message
        #[arg(short, long, required = true, value_name = "MESSAGE")]
        message: String,
    },
    /// Generate shell completion script
    #[clap(alias = "comp")]
    Completion {
        /// Shell to generate the completion script for
        #[arg(value_name = "SHELL")]
        shell: Shell,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    if let Commands::Completion { shell } = &cli.command {
        let mut cmd = Cli::command();
        let name = cmd.get_name().to_string();
        generate(*shell, &mut cmd, name, &mut std::io::stdout());
        return Ok(());
    }

    let config = config::Config::new()?;
    let base_url = cli.host.as_ref().unwrap_or(&config.remote.url);

    // --ignore-repos overrides the ignore list from the configuration.
    let ignore = if cli.ignore_repos.is_empty() {
        &config.repo.ignore
    } else {
        &cli.ignore_repos
    };

    let remote = remote::Remote::new(
        &config.remote.list.cmd,
        &config.clone.post.cmd,
        ignore,
        cli.debug > 0,
    );
    let local = local::Local::new(ignore, cli.debug > 0);

    if cli.debug > 0 {
        eprintln!(
            "[debug] using {} worker threads",
            pool::worker_count(cli.threads, usize::MAX)
        );
    }

    match &cli.command {
        Commands::Clone { repos, all } => {
            let repos = if *all {
                remote.repo_list()?
            } else {
                repos
                    .iter()
                    .filter(|r| !ignore.iter().any(|i| i == *r))
                    .cloned()
                    .collect()
            };

            let (sender, receiver) = unbounded::<progress::Update>();

            let workers = pool::worker_count(cli.threads, repos.len());
            let consumer = progress::create_writer(receiver, repos.len(), workers, "Cloning");

            pool::for_each_io(&repos, cli.threads, |slot, r| {
                let mut dest = cli.root.clone();
                dest.push(r);

                let skip = local.repo_exist(&cli.root, r);
                let label = if skip { "Skipping" } else { "Cloning" };

                sender
                    .send(progress::Update::Start {
                        slot,
                        label,
                        repo: r.to_string(),
                    })
                    .unwrap();

                let error = if skip {
                    None
                } else {
                    let repo_url = format!("{}/{}", base_url, r);
                    remote
                        .clone(repo_url.as_ref(), dest.as_path())
                        .err()
                        .map(|e| e.to_string())
                };

                sender
                    .send(progress::Update::Finish { slot, error })
                    .unwrap();
            });

            drop(sender); // close the channel so the progress writer finishes
            consumer.join().unwrap()?;
        }
        Commands::List { mode } => match mode {
            Some(ListType::Remote) => status::remote(&remote)?,
            Some(ListType::Local) => status::local(&local, &cli.root)?,
            Some(ListType::Diff) => status::diff(&remote, &local, &cli.root)?,
            Some(ListType::Superfluous) => status::superfluous(&remote, &local, &cli.root)?,
            None => status::status(&remote, &local, &cli.root)?,
        },
        Commands::Pull => {
            let local_repos = local.repos(&cli.root)?;

            let (sender, receiver) = unbounded::<progress::Update>();

            let workers = pool::worker_count(cli.threads, local_repos.len());
            let consumer = progress::create_writer(receiver, local_repos.len(), workers, "Pulling");

            pool::for_each_io(&local_repos, cli.threads, |slot, r| {
                let mut dest = cli.root.clone();
                dest.push(r);

                sender
                    .send(progress::Update::Start {
                        slot,
                        label: "Pulling",
                        repo: r.to_string(),
                    })
                    .unwrap();

                let error = remote.pull(dest.as_path()).err().map(|e| e.to_string());

                sender
                    .send(progress::Update::Finish { slot, error })
                    .unwrap();
            });

            drop(sender); // close the channel so the progress writer finishes
            consumer.join().unwrap()?;
        }
        Commands::Fetch => {
            let local_repos = local.repos(&cli.root)?;

            let (sender, receiver) = unbounded::<progress::Update>();

            let workers = pool::worker_count(cli.threads, local_repos.len());
            let consumer =
                progress::create_writer(receiver, local_repos.len(), workers, "Fetching");

            pool::for_each_io(&local_repos, cli.threads, |slot, r| {
                let mut dest = cli.root.clone();
                dest.push(r);

                sender
                    .send(progress::Update::Start {
                        slot,
                        label: "Fetching",
                        repo: r.to_string(),
                    })
                    .unwrap();

                let error = remote.fetch(dest.as_path()).err().map(|e| e.to_string());

                sender
                    .send(progress::Update::Finish { slot, error })
                    .unwrap();
            });

            drop(sender); // close the channel so the progress writer finishes
            consumer.join().unwrap()?;
        }
        Commands::Push { refspec } => {
            let local_repos = local.repos(&cli.root)?;

            let (sender, receiver) = unbounded::<progress::Update>();

            let workers = pool::worker_count(cli.threads, local_repos.len());
            let consumer =
                progress::create_writer(receiver, local_repos.len(), workers, "Pushing");

            pool::for_each_io(&local_repos, cli.threads, |slot, r| {
                let mut dest = cli.root.clone();
                dest.push(r);

                let unpushed = remote.has_unpushed(dest.as_path());
                let skip = matches!(unpushed, Ok(false));
                let label = if skip { "Skipping" } else { "Pushing" };

                sender
                    .send(progress::Update::Start {
                        slot,
                        label,
                        repo: r.to_string(),
                    })
                    .unwrap();

                let error = match unpushed {
                    Ok(false) => None,
                    Ok(true) => remote
                        .push(dest.as_path(), refspec)
                        .err()
                        .map(|e| e.to_string()),
                    Err(e) => Some(e.to_string()),
                };

                sender
                    .send(progress::Update::Finish { slot, error })
                    .unwrap();
            });

            drop(sender); // close the channel so the progress writer finishes
            consumer.join().unwrap()?;
        }
        Commands::Status => status::dirty(&local, &cli.root)?,
        Commands::Reset { hard: _, commit } => {
            let local_repos = local.repos(&cli.root)?;

            let (sender, receiver) = unbounded::<progress::Update>();

            let workers = pool::worker_count(cli.threads, local_repos.len());
            let consumer =
                progress::create_writer(receiver, local_repos.len(), workers, "Resetting");

            pool::for_each_io(&local_repos, cli.threads, |slot, r| {
                let mut dest = cli.root.clone();
                dest.push(r);

                sender
                    .send(progress::Update::Start {
                        slot,
                        label: "Resetting",
                        repo: r.to_string(),
                    })
                    .unwrap();

                let error = local
                    .reset_hard(dest.as_path(), commit.as_deref())
                    .err()
                    .map(|e| e.to_string());

                sender
                    .send(progress::Update::Finish { slot, error })
                    .unwrap();
            });

            drop(sender); // close the channel so the progress writer finishes
            consumer.join().unwrap()?;
        }
        Commands::Clean { force } if !*force => status::clean_preview(&local, &cli.root)?,
        Commands::Clean { .. } => {
            let local_repos = local.repos(&cli.root)?;

            let (sender, receiver) = unbounded::<progress::Update>();

            let workers = pool::worker_count(cli.threads, local_repos.len());
            let consumer =
                progress::create_writer(receiver, local_repos.len(), workers, "Cleaning");

            pool::for_each_io(&local_repos, cli.threads, |slot, r| {
                let mut dest = cli.root.clone();
                dest.push(r);

                sender
                    .send(progress::Update::Start {
                        slot,
                        label: "Cleaning",
                        repo: r.to_string(),
                    })
                    .unwrap();

                let error = local.clean(dest.as_path()).err().map(|e| e.to_string());

                sender
                    .send(progress::Update::Finish { slot, error })
                    .unwrap();
            });

            drop(sender); // close the channel so the progress writer finishes
            consumer.join().unwrap()?;
        }
        Commands::Stage { update: _ } => {
            let local_repos = local.repos(&cli.root)?;

            let (sender, receiver) = unbounded::<progress::Update>();

            let workers = pool::worker_count(cli.threads, local_repos.len());
            let consumer =
                progress::create_writer(receiver, local_repos.len(), workers, "Staging");

            pool::for_each_io(&local_repos, cli.threads, |slot, r| {
                let mut dest = cli.root.clone();
                dest.push(r);

                let stageable = local.has_stageable(dest.as_path());
                let skip = matches!(stageable, Ok(false));
                let label = if skip { "Skipping" } else { "Staging" };

                sender
                    .send(progress::Update::Start {
                        slot,
                        label,
                        repo: r.to_string(),
                    })
                    .unwrap();

                let error = match stageable {
                    Ok(false) => None,
                    Ok(true) => local
                        .stage_update(dest.as_path())
                        .err()
                        .map(|e| e.to_string()),
                    Err(e) => Some(e.to_string()),
                };

                sender
                    .send(progress::Update::Finish { slot, error })
                    .unwrap();
            });

            drop(sender); // close the channel so the progress writer finishes
            consumer.join().unwrap()?;
        }
        Commands::Commit { message } => {
            let local_repos = local.repos(&cli.root)?;

            let (sender, receiver) = unbounded::<progress::Update>();

            let workers = pool::worker_count(cli.threads, local_repos.len());
            let consumer =
                progress::create_writer(receiver, local_repos.len(), workers, "Committing");

            pool::for_each_io(&local_repos, cli.threads, |slot, r| {
                let mut dest = cli.root.clone();
                dest.push(r);

                let staged = local.has_staged(dest.as_path());
                let skip = matches!(staged, Ok(false));
                let label = if skip { "Skipping" } else { "Committing" };

                sender
                    .send(progress::Update::Start {
                        slot,
                        label,
                        repo: r.to_string(),
                    })
                    .unwrap();

                let error = match staged {
                    Ok(false) => None,
                    Ok(true) => local
                        .commit(dest.as_path(), message)
                        .err()
                        .map(|e| e.to_string()),
                    Err(e) => Some(e.to_string()),
                };

                sender
                    .send(progress::Update::Finish { slot, error })
                    .unwrap();
            });

            drop(sender); // close the channel so the progress writer finishes
            consumer.join().unwrap()?;
        }
        Commands::Completion { .. } => unreachable!("handled before loading config"),
    }

    Ok(())
}
