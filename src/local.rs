use git2::Repository;
use std::fs;
use std::path::Path;
use std::process::Command;

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

    /// Discard all local changes, resetting the working tree to `target`
    /// (a revspec such as `origin/master`) or to HEAD when `target` is None.
    pub fn reset_hard(&self, dst: &Path, target: Option<&str>) -> anyhow::Result<()> {
        if self.debug {
            eprintln!(
                "[debug] reset --hard {} {}",
                target.unwrap_or("HEAD"),
                dst.display()
            );
        }

        let repo = Repository::open(dst)?;
        let object = match target {
            Some(rev) => repo.revparse_single(rev)?.peel_to_commit()?,
            None => repo.head()?.peel_to_commit()?,
        };
        repo.reset(object.as_object(), git2::ResetType::Hard, None)?;

        Ok(())
    }

    /// Whether there are tracked modifications or deletions that
    /// `stage --update` would add to the index. Untracked files don't count.
    pub fn has_stageable(&self, dst: &Path) -> anyhow::Result<bool> {
        let repo = Repository::open(dst)?;

        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(false);

        let stageable = git2::Status::WT_MODIFIED
            | git2::Status::WT_DELETED
            | git2::Status::WT_TYPECHANGE
            | git2::Status::WT_RENAMED;

        for entry in repo.statuses(Some(&mut opts))?.iter() {
            if entry.status().intersects(stageable) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Stage modifications and deletions of already-tracked files, like
    /// `git add --update`. Untracked files are left unstaged.
    pub fn stage_update(&self, dst: &Path) -> anyhow::Result<()> {
        if self.debug {
            eprintln!("[debug] stage --update {}", dst.display());
        }

        let repo = Repository::open(dst)?;
        let mut index = repo.index()?;
        index.update_all(["*"].iter(), None)?;
        index.write()?;

        Ok(())
    }

    /// Whether the index holds staged changes relative to HEAD, i.e. there is
    /// something for `commit` to record.
    pub fn has_staged(&self, dst: &Path) -> anyhow::Result<bool> {
        let repo = Repository::open(dst)?;

        let mut opts = git2::StatusOptions::new();
        opts.include_untracked(false);

        let staged = git2::Status::INDEX_NEW
            | git2::Status::INDEX_MODIFIED
            | git2::Status::INDEX_DELETED
            | git2::Status::INDEX_RENAMED
            | git2::Status::INDEX_TYPECHANGE;

        for entry in repo.statuses(Some(&mut opts))?.iter() {
            if entry.status().intersects(staged) {
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// Commit the staged changes with `message`. Repos with nothing staged are
    /// left untouched. Returns true when a commit was created.
    pub fn commit(&self, dst: &Path, message: &str) -> anyhow::Result<bool> {
        if self.debug {
            eprintln!("[debug] commit {}", dst.display());
        }

        let repo = Repository::open(dst)?;
        let mut index = repo.index()?;
        let tree = repo.find_tree(index.write_tree()?)?;
        let parent = repo.head()?.peel_to_commit()?;

        // Nothing staged relative to HEAD, so there is nothing to commit.
        if tree.id() == parent.tree_id() {
            return Ok(false);
        }

        // libgit2 does not run git hooks, so run commit-msg ourselves (e.g. for
        // the Gerrit Change-Id hook) and use the possibly edited message.
        let message = run_commit_msg_hook(&repo, message)?;

        let signature = repo.signature()?;
        repo.commit(
            Some("HEAD"),
            &signature,
            &signature,
            &message,
            &tree,
            &[&parent],
        )?;

        Ok(true)
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

/// Run the repository's `commit-msg` hook (if present) on `message`, returning
/// the possibly edited message. libgit2 never runs hooks, so we replicate what
/// git does: write the message to a file, invoke the hook with its path, and
/// read the result back.
fn run_commit_msg_hook(repo: &Repository, message: &str) -> anyhow::Result<String> {
    let hook = repo.path().join("hooks").join("commit-msg");
    if !hook.is_file() {
        return Ok(message.to_string());
    }

    let msg_path = repo.path().join("COMMIT_EDITMSG");
    fs::write(&msg_path, message)?;

    let mut command = Command::new(&hook);
    command.arg(&msg_path);
    if let Some(workdir) = repo.workdir() {
        command.current_dir(workdir);
    }

    let status = command.status()?;
    if !status.success() {
        return Err(anyhow::anyhow!("commit-msg hook failed"));
    }

    Ok(fs::read_to_string(&msg_path)?)
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
