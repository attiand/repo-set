mod clone;
mod gerrit;
mod local;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use rayon::prelude::*;
use std::path::PathBuf;

use colour::*;
use users::{get_current_uid, get_user_by_uid};

/// Manage a set of git repositories
#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Cli {
    /// Specify root directory
    #[clap(short, long, num_args(1), value_name("DIR"), default_value = ".", value_hint = clap::ValueHint::DirPath)]
    root: PathBuf,

    /// Git host
    #[clap(long, num_args(1), value_name("HOST"), default_value = "nya-gerrit.its.umu.se:29418", value_hint = clap::ValueHint::Hostname)]
    host: String,

    /// Print debug information
    #[arg(long, action = clap::ArgAction::Count)]
    debug: u8,

    /// Number of threads to use, use rayon default if not specified.
    #[arg(short, long, default_value_t = 0)]
    threads: usize,

    #[command(subcommand)]
    command: Commands,
}

#[derive(ValueEnum, Copy, Clone, Debug, PartialEq, Eq)]
enum ListType {
    Remote,
    Local,
    Diff,
}

impl std::fmt::Display for ListType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.to_possible_value()
            .expect("no values are skipped")
            .get_name()
            .fmt(f)
    }
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
        #[clap(name = "REPO-NAMEString", required = false, num_args = 1..)]
        repos: Vec<String>,
    },
    /// List  repositories
    #[clap(alias = "ls")]
    List {
        #[arg(
            name = "type",
            short,
            long,
            require_equals = false,
            value_name = "TYPE",
            num_args = 0..=1,
            default_value_t = ListType::Remote,
            value_enum
        )]
        mode: ListType,
    },
}

macro_rules! debug {
   ($($tt:tt)*) => {
        yellow_ln!($($tt)*);
    };
}

fn get_user_name() -> String {
    let user = get_user_by_uid(get_current_uid()).expect("Can't find user");

    String::from(user.name().to_str().expect("Can't convert user name"))
}

fn main() -> Result<()> {
    let cli = Cli::parse();

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
                gerrit::repos(&cli.host)?
                    .into_iter()
                    .filter(|r| local::repo_exist(&cli.root, r))
                    .collect()
            } else {
                repos.to_vec()
            };

            repo_list.par_iter().for_each(|r| {
                debug!("processing {}", r);
                let mut dest = cli.root.clone();
                dest.push(r);
                let url = format!("ssh://{}@{}/{}", get_user_name(), &cli.host, r);

                if dest.exists() {
                    eprintln!("skipping {} already exists", r);
                    return;
                }

                debug!("cloning {}", r);

                if let Err(e) = clone::run(&url, dest.as_path()) {
                    eprintln!("failed to clone repo: {}: {}", r, e);
                } else {
                    println!("{} cloned", r)
                }
            });
        }
        Commands::List { mode } => match mode {
            ListType::Remote => {
                gerrit::repos(&cli.host)?
                    .into_iter()
                    .for_each(|r| println!("{}", r));
            }
            ListType::Local => {
                local::repos(&cli.root)?
                    .into_iter()
                    .for_each(|r| println!("{}", r));
            }
            ListType::Diff => {}
        },
    }

    Ok(())
}
