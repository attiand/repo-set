use anyhow::{Result, anyhow};
use repo_set::config::{self, Config};
use std::env::home_dir;
use std::path::{Path, PathBuf};

pub(super) fn load() -> Result<Config> {
    let home = home_dir().ok_or_else(|| anyhow!("Can't get user home directory"))?;
    config::load_from_path(&config_path(&home))
}

fn config_path(home: &Path) -> PathBuf {
    home.join(".repo-clone").join("config.toml")
}

#[cfg(test)]
mod tests {
    use super::config_path;
    use std::path::{Path, PathBuf};

    #[test]
    fn config_path_uses_home_directory() {
        assert_eq!(
            config_path(Path::new("/home/tester")),
            PathBuf::from("/home/tester/.repo-clone/config.toml")
        );
    }
}
