mod config;
mod local;
mod pool;
mod remote;

mod progress;

use anyhow::Result;
use clap::{Parser, Subcommand};
use crossbeam_channel::unbounded;
use std::path::PathBuf;

use colour::*;

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
    /// Clone specified repositories
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
    /// Synchronize all repositories, that is clone missing repos, reset and pull existing repos, list superfluous local repos
    #[clap(alias = "s")]
    Sync {
        /// Without force no local repos will be deleted, only printed.
        #[arg(short, long, default_value_t = false)]
        force: bool,
    },
}

macro_rules! debug {
   ($($tt:tt)*) => {
        yellow_ln!($($tt)*);
    };
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = config::Config::new()?;
    let base_url = &cli.host.as_ref().unwrap_or(&config.remote.url);

    if cli.debug > 0 {
        debug!(
            "using {} worker threads",
            pool::worker_count(cli.threads, usize::MAX)
        );
    }

    match &cli.command {
        Commands::Clone { repos, all } => {

            let r = if *all {
                remote::repo_list(&config.repo.list.cmd)?
            } else {
                repos.clone()
            };

            let (sender, receiver) = unbounded::<progress::Update>();

            let consumer = progress::create_writer(receiver, r.len(), "Cloning");

            pool::for_each_io(&r, cli.threads, |r| {
                let mut dest = cli.root.clone();
                dest.push(r);

                let repo_url = format!("{}/{}", base_url, r);

                let error = remote::clone(repo_url.as_ref(), dest.as_path())
                    .err()
                    .map(|e| e.to_string());

                sender
                    .send(progress::Update {
                        repo: r.to_string(),
                        error,
                    })
                    .unwrap();
            });

            drop(sender); // close the channel so the progress writer finishes
            consumer?.join().unwrap()?;
        }
        Commands::List { mode } => match mode {
            Some(ListType::Remote) | None => {
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
                eprintln!("sorry, diff not yet implemented")
            }
        },
        Commands::Sync {force: _bool} => {
            let all_repos: Vec<_> = remote::repo_list(&config.repo.list.cmd)?
                .into_iter()
                .collect();

            //.filter(|r| !local::repo_exist(&cli.root, r))

            let (sender, receiver) = unbounded::<progress::Update>();

            let consumer = progress::create_writer(receiver, all_repos.len(), "Synchronising");

            pool::for_each_io(&all_repos, cli.threads, |r| {
                let mut dest = cli.root.clone();
                dest.push(r);

                let repo_url = format!("{}/{}", base_url, r);

                let error = remote::clone(repo_url.as_ref(), dest.as_path())
                    .err()
                    .map(|e| e.to_string());

                sender
                    .send(progress::Update {
                        repo: r.to_string(),
                        error,
                    })
                    .unwrap();
            });

            drop(sender); // close the channel so the progress writer finishes
            consumer?.join().unwrap()?;
        }
    }

    Ok(())
}
