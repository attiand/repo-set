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

/// Manage a set of git repositories
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Specify root directory
    #[clap(short, long, num_args(1), value_name("DIR"), default_value = ".", value_hint = clap::ValueHint::DirPath)]
    root: PathBuf,

    /// Git host
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
        /// Clone all available repositories
        #[clap(short, long)]
        all: bool,

        /// Repository names to clone
        #[clap(name = "REPO-NAME", required = false, num_args = 1..)]
        repos: Vec<String>,
    },
    /// List  repositories
    #[clap(alias = "ls")]
    List {
        #[command(subcommand)]
        mode: Option<ListType>,
    },
    /// Pull  repositories
    #[clap(alias = "p")]
    Pull {},
}

macro_rules! debug {
   ($($tt:tt)*) => {
        yellow_ln!($($tt)*);
    };
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = config::Config::new()?;

    rayon::ThreadPoolBuilder::new()
        .num_threads(cli.threads)
        .build_global()
        .unwrap();

    if cli.debug > 0 {
        debug!("using {} threads", rayon::current_num_threads());
    }

    match &cli.command {
        Commands::Clone { all, repos } => {
            let repo_list: Vec<String> = if *all {
                remote::repo_list(&config.repo.list.cmd)?
                    .into_iter()
                    .filter(|r| !local::repo_exist(&cli.root, r))
                    .collect()
            } else {
                repos.to_vec()
            };

            let (sender, receiver): (Sender<String>, Receiver<String>) = mpsc::channel();

            let consumer = progress::create_writer(receiver, repo_list.len());

            repo_list.par_iter().for_each_with(sender, |s, r| {
                if cli.debug > 0 {
                    debug!("processing {}", r);
                }
                s.send(r.to_string()).unwrap();

                let mut dest = cli.root.clone();
                dest.push(r);

                let url = &cli.host.as_ref().unwrap_or(&config.remote.url);

                debug!("cloning {}", r);

                if let Err(e) = remote::clone(url, dest.as_path()) {
                    eprintln!("failed to clone repo: {}: {}", r, e);
                }
            });
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
            Some(ListType::Diff) => {}
        },
        Commands::Pull {} => {
            /*
            let repo_names = local::repos(&cli.root)?;

            let repo_names: Vec<&str> = repo_names.iter().map(AsRef::as_ref).collect();

            let repos = local::get_local_repos(repo_names.as_slice());

            repos.iter().for_each(|r| r.pull())
             */
        }
    }

    Ok(())
}
