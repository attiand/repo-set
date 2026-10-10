use crate::parameters::Parameters;
use git2::Repository;
use std::fs;
use std::process::Command;

/// A single `git status --short` entry: the two-character status code and path.
pub struct FileStatus {
    pub status: String,
    pub path: String,
}

/// Operates on the local repo set, optionally logging debug output to stderr.
pub struct Local<'a> {
    parameters: &'a Parameters,
}

impl<'a> Local<'a> {
    pub fn new(parameters: &'a Parameters) -> Self {
        Self { parameters }
    }

    pub fn repo_exist(&self, repo: &str) -> bool {
        self.parameters.local_repo_path(repo).exists()
    }

    /// List local repository directories, excluding any configured to be ignored.
    pub fn repos(&self) -> anyhow::Result<Vec<String>> {
        let mut res = Vec::new();

        for e in fs::read_dir(&self.parameters.root)? {
            let p = e?.path();

            if p.join(".git").is_dir()
                && let Some(f) = p.file_name()
            {
                let name = f.to_str().expect("Can't convert file name");
                if !self.parameters.ignore_repos.iter().any(|i| i == name) {
                    res.push(name.to_string());
                }
            }
        }

        Ok(res)
    }

    /// List directories under the configured root that are not git repositories.
    pub fn non_repos(&self) -> anyhow::Result<Vec<String>> {
        let mut res = Vec::new();

        for e in fs::read_dir(&self.parameters.root)? {
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

    /// Discard local changes, resetting to the supplied target or HEAD.
    pub fn reset_hard(&self, repo_name: &str, target: Option<&str>) -> anyhow::Result<()> {
        let dst = self.parameters.local_repo_path(repo_name);
        if self.parameters.debug {
            eprintln!(
                "[debug] reset --hard {} {}",
                target.unwrap_or("HEAD"),
                dst.display()
            );
        }

        let repo = Repository::open(&dst)?;
        let object = match target {
            Some(rev) => repo.revparse_single(rev)?.peel_to_commit()?,
            None => repo.head()?.peel_to_commit()?,
        };
        repo.reset(object.as_object(), git2::ResetType::Hard, None)?;

        Ok(())
    }

    /// Whether there are tracked modifications or deletions that
    /// `stage --update` would add to the index. Untracked files don't count.
    pub fn has_stageable(&self, repo_name: &str) -> anyhow::Result<bool> {
        let repo = Repository::open(self.parameters.local_repo_path(repo_name))?;

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
    pub fn stage_update(&self, repo_name: &str) -> anyhow::Result<()> {
        let dst = self.parameters.local_repo_path(repo_name);
        if self.parameters.debug {
            eprintln!("[debug] stage --update {}", dst.display());
        }

        let repo = Repository::open(&dst)?;
        let mut index = repo.index()?;
        index.update_all(["*"].iter(), None)?;
        index.write()?;

        Ok(())
    }

    /// Whether the index holds staged changes relative to HEAD, i.e. there is
    /// something for `commit` to record.
    pub fn has_staged(&self, repo_name: &str) -> anyhow::Result<bool> {
        let repo = Repository::open(self.parameters.local_repo_path(repo_name))?;

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

    /// Commit staged changes with the supplied message. Repos with nothing staged are
    /// left untouched. Returns true when a commit was created.
    pub fn commit(&self, repo_name: &str, message: &str) -> anyhow::Result<bool> {
        let dst = self.parameters.local_repo_path(repo_name);
        if self.parameters.debug {
            eprintln!("[debug] commit {}", dst.display());
        }

        let repo = Repository::open(&dst)?;
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
    pub fn clean_list(&self, repo_name: &str) -> anyhow::Result<Vec<String>> {
        let repo = Repository::open(self.parameters.local_repo_path(repo_name))?;

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
    pub fn clean(&self, repo_name: &str) -> anyhow::Result<()> {
        let dst = self.parameters.local_repo_path(repo_name);
        if self.parameters.debug {
            eprintln!("[debug] clean {}", dst.display());
        }

        for rel in self.clean_list(repo_name)? {
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
    pub fn status_short(&self, repo_name: &str) -> anyhow::Result<Vec<FileStatus>> {
        let repo = Repository::open(self.parameters.local_repo_path(repo_name))?;

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

#[cfg(test)]
mod tests {
    use super::Local;
    use crate::config::Config;
    use crate::parameters::Parameters;
    use crate::remote::Remote;
    use crate::{Cli, Commands};
    use git2::Repository;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    struct TestRoot(PathBuf);

    impl TestRoot {
        fn new() -> Self {
            let unique = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path =
                std::env::temp_dir().join(format!("repo-set-{}-{}", std::process::id(), unique));
            fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for TestRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn operations_resolve_repository_names_and_command_settings() {
        let root = TestRoot::new();
        let config: Config = toml::from_str(
            r#"
            remote.url = "host:29418"
            remote.list.cmd = []
            repositories.ignore = ["ignored"]
            "#,
        )
        .unwrap();
        let cli = Cli {
            root: root.0.clone(),
            host: None,
            debug: 0,
            ignore_repos: Vec::new(),
            threads: None,
            command: Commands::Pull,
        };
        let parameters = Parameters::new(&config, &cli).unwrap();
        let local = Local::new(&parameters);
        let local_repo_path = root.0.join("managed");
        let repo = Repository::init(&local_repo_path).unwrap();
        Repository::init(root.0.join("ignored")).unwrap();
        fs::create_dir(root.0.join("extra")).unwrap();
        let mut git_config = repo.config().unwrap();
        git_config.set_str("user.name", "Test User").unwrap();
        git_config
            .set_str("user.email", "test@example.com")
            .unwrap();
        fs::write(local_repo_path.join("tracked"), "initial\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(Path::new("tracked")).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let signature = repo.signature().unwrap();
        repo.commit(Some("HEAD"), &signature, &signature, "Initial", &tree, &[])
            .unwrap();

        assert!(local.repo_exist("managed"));
        assert!(!local.repo_exist("missing"));
        assert_eq!(local.repos().unwrap(), ["managed"]);
        assert_eq!(local.non_repos().unwrap(), ["extra"]);
        assert!(local.status_short("managed").unwrap().is_empty());
        assert!(!Remote::new(&parameters).has_unpushed("managed").unwrap());

        fs::write(local_repo_path.join("tracked"), "changed\n").unwrap();
        assert!(local.has_stageable("managed").unwrap());
        local.stage_update("managed").unwrap();
        assert!(local.has_staged("managed").unwrap());
        assert!(local.commit("managed", "Explicit message").unwrap());
        assert_eq!(
            repo.head()
                .unwrap()
                .peel_to_commit()
                .unwrap()
                .message()
                .unwrap(),
            "Explicit message"
        );
        assert!(!local.commit("managed", "Explicit message").unwrap());

        fs::write(local_repo_path.join("tracked"), "discard\n").unwrap();
        local.reset_hard("managed", None).unwrap();
        assert_eq!(
            fs::read_to_string(local_repo_path.join("tracked")).unwrap(),
            "changed\n"
        );
        fs::write(local_repo_path.join("untracked"), "remove\n").unwrap();
        assert_eq!(local.clean_list("managed").unwrap(), ["untracked"]);
        local.clean("managed").unwrap();
        assert!(local.status_short("managed").unwrap().is_empty());

        local.reset_hard("managed", Some("HEAD~1")).unwrap();
        assert_eq!(
            fs::read_to_string(local_repo_path.join("tracked")).unwrap(),
            "initial\n"
        );
    }
}
