use gix::Repository;
use std::fs;
use std::path::{Path, PathBuf};

pub fn repo_exist(root: &Path, repo: &str) -> bool {
    let mut dest = PathBuf::from(root);
    dest.push(repo);
    dest.exists()
}

// Perhaps also check that is actually a git repo
pub fn repos(root: &Path) -> anyhow::Result<Vec<String>> {
    let mut res = Vec::new();

    for e in fs::read_dir(root)? {
        let p = e?.path();

        if p.is_dir()
            && let Some(f) = p.file_name()
        {
            res.push(String::from(f.to_str().expect("Can't convert file name")));
        }
    }

    Ok(res)
}

pub fn get_local_repos<T: AsRef<str>>(repos: &[T]) -> anyhow::Result<Vec<Repository>> {
    repos
        .iter()
        .map(|name| gix::discover(name.as_ref()).map_err(anyhow::Error::msg))
        .collect()
}
