use git2::Repository;
use std::fs;
use std::path::Path;

/// A single `git status --short` entry: the two-character status code and path.
pub struct FileStatus {
    pub status: String,
    pub path: String,
}

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

    /// List untracked files and directories that `clean` would remove. Ignored
    /// files are not included.
    pub fn clean_list(&self, dst: &Path) -> anyhow::Result<Vec<String>> {
        let repo = Repository::open(dst)?;

        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true).include_ignored(false);

        let mut paths = Vec::new();
        for entry in repo.statuses(Some(&mut opts))?.iter() {
            if entry.status().contains(git2::Status::WT_NEW)
                && let Ok(rel) = entry.path()
            {
                paths.push(rel.to_string());
            }
        }

        Ok(paths)
    }

    /// Remove untracked files and directories, like `git clean -fd`. Ignored
    /// files are left in place.
    pub fn clean(&self, dst: &Path) -> anyhow::Result<()> {
        if self.debug {
            eprintln!("[debug] clean {}", dst.display());
        }

        for rel in self.clean_list(dst)? {
            let full = dst.join(&rel);
            if full.is_dir() {
                fs::remove_dir_all(&full)?;
            } else if full.exists() {
                fs::remove_file(&full)?;
            }
        }

        Ok(())
    }

    /// Return the working tree status as `git status --short` entries. An empty
    /// vector means the working tree is clean.
    pub fn status_short(&self, dst: &Path) -> anyhow::Result<Vec<FileStatus>> {
        let repo = Repository::open(dst)?;

        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(true).recurse_untracked_dirs(true);

        let mut entries = Vec::new();
        for entry in repo.statuses(Some(&mut opts))?.iter() {
            let (x, y) = short_flags(entry.status());
            entries.push(FileStatus {
                status: format!("{}{}", x, y),
                path: entry.path().unwrap_or_default().to_string(),
            });
        }

        Ok(entries)
    }
}

/// Map a git2 status to the index (X) and worktree (Y) `git status --short` flags.
fn short_flags(s: git2::Status) -> (char, char) {
    use git2::Status;

    let index = Status::INDEX_NEW
        | Status::INDEX_MODIFIED
        | Status::INDEX_DELETED
        | Status::INDEX_RENAMED
        | Status::INDEX_TYPECHANGE;

    if s.contains(Status::WT_NEW) && !s.intersects(index) {
        return ('?', '?');
    }

    let x = if s.contains(Status::INDEX_NEW) {
        'A'
    } else if s.contains(Status::INDEX_MODIFIED) {
        'M'
    } else if s.contains(Status::INDEX_DELETED) {
        'D'
    } else if s.contains(Status::INDEX_RENAMED) {
        'R'
    } else if s.contains(Status::INDEX_TYPECHANGE) {
        'T'
    } else {
        ' '
    };

    let y = if s.contains(Status::WT_MODIFIED) {
        'M'
    } else if s.contains(Status::WT_DELETED) {
        'D'
    } else if s.contains(Status::WT_RENAMED) {
        'R'
    } else if s.contains(Status::WT_TYPECHANGE) {
        'T'
    } else {
        ' '
    };

    (x, y)
}
