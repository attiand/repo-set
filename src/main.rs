mod clone;
mod gerrit;

use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use rayon::prelude::*;
use std::path::PathBuf;

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
        #[clap(name = "REPO-NAME", required = false, num_args = 1..)]
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

fn get_user_name() -> String {
    let user = get_user_by_uid(get_current_uid()).expect("Can't find user");

    return String::from(user.name().to_str().expect("Can't convert user name"));
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Clone { all, repos } => {
            let repo_list = if *all {
                gerrit::remote_repos(&cli.host)?
            } else {
                repos.to_vec()
            };

            repo_list.par_iter().for_each(|r| {
                let mut dest = cli.root.clone();
                dest.push(r);
                let url = format!("ssh://{}@{}/{}", get_user_name(), &cli.host, r);

                if let Err(e) = clone::run(&url, dest.as_path()) {
                    eprintln!("{}: {}: {}", "failed to clone repo", r, e);
                } else {
                    println!("{} cloned", r)
                }
            });
        }
        Commands::List { mode } => match mode {
            ListType::Remote => {
                gerrit::remote_repos(&cli.host)?
                    .into_iter()
                    .for_each(|r| println!("{}", r));
            }
            ListType::Local => {}
            ListType::Diff => {}
        },
    }

    Ok(())
}
