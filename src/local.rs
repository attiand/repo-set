use git2::Repository;
use std::fs;
use std::path::Path;

/// Operates on the local repo set, optionally logging debug output to stderr.
pub struct Local<'a> {
    ignore: &'a [String],
    debug: bool,
}

impl<'a> Local<'a> {
    pub fn new(ignore: &'a [String], debug: bool) -> Self {
        Self { ignore, debug }
    }

    pub fn repo_exist(&self, root: &Path, repo: &str) -> bool {
        root.join(repo).exists()
    }

    /// List local repository directories, excluding any configured to be ignored.
    pub fn repos(&self, root: &Path) -> anyhow::Result<Vec<String>> {
        let mut res = Vec::new();

        for e in fs::read_dir(root)? {
            let p = e?.path();

            if p.join(".git").is_dir()
                && let Some(f) = p.file_name()
            {
                let name = f.to_str().expect("Can't convert file name");
                if !self.ignore.iter().any(|i| i == name) {
                    res.push(name.to_string());
                }
            }
        }

        Ok(res)
    }

    /// List directories under `root` that are not git repositories.
    pub fn non_repos(&self, root: &Path) -> anyhow::Result<Vec<String>> {
        let mut res = Vec::new();

        for e in fs::read_dir(root)? {
            let p = e?.path();

            if p.is_dir()
                && !p.join(".git").is_dir()
                && let Some(f) = p.file_name()
            {
                res.push(f.to_str().expect("Can't convert file name").to_string());
            }
        }

        Ok(res)
    }

    /// Discard all local changes, resetting the working tree to HEAD.
    pub fn reset_hard(&self, dst: &Path) -> anyhow::Result<()> {
        if self.debug {
            eprintln!("[debug] reset --hard {}", dst.display());
        }

        let repo = Repository::open(dst)?;
        let head = repo.head()?.peel_to_commit()?;
        repo.reset(head.as_object(), git2::ResetType::Hard, None)?;

        Ok(())
    }
}
