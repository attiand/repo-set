use anyhow::anyhow;
use serde::Deserialize;
use std::{env::home_dir, fs};

#[derive(Deserialize)]
pub struct Remote {
    pub url: String,
    #[serde(default)]
    pub list: List,
}

#[derive(Deserialize, Default)]
pub struct List {
    #[serde(default)]
    pub cmd: Vec<String>,
}

#[derive(Deserialize, Default)]
pub struct Repo {
    #[serde(default)]
    pub ignore: Vec<String>,
}

#[derive(Deserialize, Default)]
pub struct Clone {
    #[serde(default)]
    pub post: Post,
}

#[derive(Deserialize, Default)]
pub struct Post {
    #[serde(default)]
    pub cmd: Vec<String>,
}

#[derive(Deserialize)]
pub struct Config {
    pub remote: Remote,
    #[serde(default)]
    pub repo: Repo,
    #[serde(default)]
    pub clone: Clone,
}

impl Config {
    pub fn new() -> anyhow::Result<Self> {
        let mut home = home_dir()
            .ok_or("Can't get user home directory")
            .map_err(anyhow::Error::msg)?;
        home.push(".repo-set.toml");

        if !fs::exists(&home)? {
            return Err(anyhow!("No user config found (~/.repo-set.toml)"));
        }

        let content: String = fs::read_to_string(&home)?;
        toml::from_str(content.as_str()).map_err(anyhow::Error::msg)
    }
}
