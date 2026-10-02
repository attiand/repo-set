mod config;
mod local;
mod pool;
mod remote;

mod progress;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossbeam_channel::unbounded;
use std::collections::HashSet;
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
    #[arg(long, global = true, action = clap::ArgAction::Count)]
    debug: u8,

    /// Number of worker threads to use, defaults to an I/O-friendly count if not specified.
    #[arg(short, long, default_value_t = 0)]
    threads: usize,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum ListType {
    /// List remote repositories
    #[command(name = "--remote")]
    Remote,
    /// List local repositories
    #[command(name = "--local")]
    Local,
    /// List difference between local and remote repositories
    #[command(name = "--diff")]
    Diff,
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
    /// Reset all local repositories, discarding local changes
    Reset {
        /// Reset the working tree to HEAD, discarding all local changes
        #[arg(long, required = true)]
        hard: bool,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = config::Config::new()?;
    let base_url = cli.host.as_ref().unwrap_or(&config.remote.url);
    let remote = remote::Remote::new(cli.debug > 0);

    if cli.debug > 0 {
        eprintln!(
            "[debug] using {} worker threads",
            pool::worker_count(cli.threads, usize::MAX)
        );
    }

    match &cli.command {
        Commands::Clone { repos, all } => {
            let repos = if *all {
                remote::repo_list(&config.repo.list.cmd)?
            } else {
                repos.clone()
            };

            let (sender, receiver) = unbounded::<progress::Update>();

            let consumer = progress::create_writer(receiver, repos.len(), "Cloning");

            pool::for_each_io(&repos, cli.threads, |r| {
                let mut dest = cli.root.clone();
                dest.push(r);

                let (task, error) = if local::repo_exist(&cli.root, r) {
                    let error = None;
                    ("Skipping", error)
                } else {
                    let repo_url = format!("{}/{}", base_url, r);
                    let error = remote
                        .clone(repo_url.as_ref(), dest.as_path())
                        .err()
                        .map(|e| e.to_string());
                    ("Cloning", error)
                };

                sender
                    .send(progress::Update::with_task(
                        r.to_string(),
                        task.to_string(),
                        error,
                    ))
                    .unwrap();
            });

            drop(sender); // close the channel so the progress writer finishes
            consumer.join().unwrap()?;
        }
        Commands::List { mode } => match mode {
            Some(ListType::Remote) => {
                remote::repo_list(&config.repo.list.cmd)?
                    .into_iter()
                    .for_each(|r| println!("{}", r));
            }
            Some(ListType::Local) => {
                local::repos(&cli.root)?
                    .into_iter()
                    .for_each(|r| println!("{}", r));
            }
            Some(ListType::Diff) => {
                let remote_repos = remote::repo_list(&config.repo.list.cmd)?;
                let local_repos = local::repos(&cli.root)?;

                let remote_set: HashSet<&String> = remote_repos.iter().collect();
                let local_set: HashSet<&String> = local_repos.iter().collect();

                remote_repos
                    .iter()
                    .filter(|r| !local_set.contains(*r))
                    .for_each(|r| println!("-{}", r));
                local_repos
                    .iter()
                    .filter(|r| !remote_set.contains(*r))
                    .for_each(|r| println!("+{}", r));
            }
            None => {
                let remote_repos = remote::repo_list(&config.repo.list.cmd)?;
                let local_repos = local::repos(&cli.root)?;

                let remote_set: HashSet<&String> = remote_repos.iter().collect();
                let local_set: HashSet<&String> = local_repos.iter().collect();

                println!("Remote repos not present locally:");
                remote_repos
                    .iter()
                    .filter(|r| !local_set.contains(*r))
                    .for_each(|r| println!("{}", r));

                println!();
                println!("Local directories with no remote:");
                local_repos
                    .iter()
                    .filter(|r| !remote_set.contains(*r))
                    .for_each(|r| println!("{}", r));
            }
        },
        Commands::Pull => {
            let local_repos = local::repos(&cli.root)?;

            let (sender, receiver) = unbounded::<progress::Update>();

            let consumer = progress::create_writer(receiver, local_repos.len(), "Pulling");

            pool::for_each_io(&local_repos, cli.threads, |r| {
                let mut dest = cli.root.clone();
                dest.push(r);

                let error = remote
                    .pull(dest.as_path())
                    .err()
                    .map(|e| e.to_string());

                sender
                    .send(progress::Update::new(r.to_string(), error))
                    .unwrap();
            });

            drop(sender); // close the channel so the progress writer finishes
            consumer.join().unwrap()?;
        }
        Commands::Reset { hard: _ } => {
            let local_repos = local::repos(&cli.root)?;

            let (sender, receiver) = unbounded::<progress::Update>();

            let consumer = progress::create_writer(receiver, local_repos.len(), "Resetting");

            pool::for_each_io(&local_repos, cli.threads, |r| {
                let mut dest = cli.root.clone();
                dest.push(r);

                let error = local::reset_hard(dest.as_path(), cli.debug > 0)
                    .err()
                    .map(|e| e.to_string());

                sender
                    .send(progress::Update::new(r.to_string(), error))
                    .unwrap();
            });

            drop(sender); // close the channel so the progress writer finishes
            consumer.join().unwrap()?;
        }
    }

    Ok(())
}
