use git2::Repository;
use std::fs;
use std::path::Path;

pub fn repo_exist(root: &Path, repo: &str) -> bool {
    root.join(repo).exists()
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

/// Discard all local changes, resetting the working tree to HEAD.
pub fn reset_hard(dst: &Path, debug: bool) -> anyhow::Result<()> {
    if debug {
        eprintln!("[debug] reset --hard {}", dst.display());
    }

    let repo = Repository::open(dst)?;
    let head = repo.head()?.peel_to_commit()?;
    repo.reset(head.as_object(), git2::ResetType::Hard, None)?;

    Ok(())
}
