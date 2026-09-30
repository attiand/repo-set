mod config;
mod local;
mod remote;

mod progress;

use anyhow::Result;
use clap::{Parser, Subcommand};
use rayon::prelude::*;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, Sender};

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

    /// Number of threads to use, use rayon default if not specified.
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
        #[clap(name = "REPO-NAME", required = true, num_args = 1..)]
        repos: Vec<String>,
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

    rayon::ThreadPoolBuilder::new()
        .num_threads(cli.threads)
        .build_global()
        .unwrap();

    if cli.debug > 0 {
        debug!("using {} threads", rayon::current_num_threads());
    }

    match &cli.command {
        Commands::Clone { repos } => {
            repos.par_iter().for_each(|r| {
                let mut dest = cli.root.clone();
                dest.push(r);

                println!("cloning {}", r);

                let repo_url = format!("{}/{}", base_url, r);

                if cli.debug > 0 {
                    debug!("cloning {}", repo_url);
                }

                if let Err(e) = remote::clone(repo_url.as_ref(), dest.as_path()) {
                    eprintln!("failed to clone repo: {}: {}", r, e);
                }
            });
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
        Commands::Sync {} => {
            let all_repos: Vec<_> = remote::repo_list(&config.repo.list.cmd)?
                .into_iter()
                .collect();

            //.filter(|r| !local::repo_exist(&cli.root, r))

            let (sender, receiver): (Sender<String>, Receiver<String>) = mpsc::channel();

            let consumer = progress::create_writer(receiver, all_repos.len());

            all_repos.par_iter().for_each_with(sender, |s, r| {
                s.send(r.to_string()).unwrap();

                let mut dest = cli.root.clone();
                dest.push(r);

                let repo_url = format!("{}/{}", base_url, r);

                if cli.debug > 0 {
                    debug!("processing {}", repo_url);
                }

                if let Err(e) = remote::clone(repo_url.as_ref(), dest.as_path()) {
                    eprintln!("failed to clone repo: {}: {}", r, e);
                }
            });
            consumer?.join().unwrap()?;
        }
    }

    Ok(())
}
