use crate::config::RepositoryFilter;
use anyhow::{Context, anyhow};
use git2::{RemoteCallbacks, Repository};
use std::path::Path;
use std::process::{Command, Stdio};

/// Performs git remote operations, optionally logging debug output to `debug.log`.
pub struct Remote<'a> {
    list_cmd: &'a [String],
    post_clone_cmd: &'a [String],
    root: &'a Path,
    repository_filter: &'a RepositoryFilter,
    debug: u8,
}

impl<'a> Remote<'a> {
    pub fn new(
        list_cmd: &'a [String],
        post_clone_cmd: &'a [String],
        root: &'a Path,
        repository_filter: &'a RepositoryFilter,
        debug: u8,
    ) -> Self {
        Self {
            list_cmd,
            post_clone_cmd,
            root,
            repository_filter,
            debug,
        }
    }

    /// List remote repositories allowed by the configured include/exclude patterns.
    pub fn repo_list(&self) -> anyhow::Result<Vec<String>> {
        let cmd = self.list_cmd;
        let Some(program) = cmd.first() else {
            return Ok(Vec::new());
        };

        let output = Command::new(program)
            .args(&cmd[1..])
            .stdin(Stdio::null())
            .output()?;

        if !output.status.success() {
            return Err(anyhow!("Can't run command {}", program));
        }

        let out = String::from_utf8(output.stdout)?;
        Ok(out
            .split_whitespace()
            .filter(|repo| self.repository_filter.includes(repo))
            .map(String::from)
            .collect())
    }

    /// Build callbacks with SSH-agent credentials and, when debugging, progress logging.
    fn callbacks(&self, label: String) -> RemoteCallbacks<'static> {
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username, _allowed_types| {
            git2::Cred::ssh_key_from_agent(username.unwrap_or("git"))
        });
        if self.debug > 2 {
            set_debug_callbacks(&mut callbacks, label, self.root.to_path_buf());
        }
        callbacks
    }

    pub fn clone(&self, repo_url: &str, dst: &Path) -> anyhow::Result<Repository> {
        if self.debug > 0 {
            crate::debug::log(
                self.root,
                &format!("[debug] clone {} -> {}", repo_url, dst.display()),
            )?;
        }

        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(self.callbacks(repo_url.to_string()));

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fo);

        let repo = builder.clone(repo_url, dst).map_err(anyhow::Error::msg)?;
        if self.debug > 0 {
            crate::debug::log(self.root, &format!("[debug] clone done {}", dst.display()))?;
        }

        self.run_post_clone(dst)?;

        Ok(repo)
    }

    /// Run the configured post-clone command inside the freshly cloned repo,
    /// expanding `${repo}`, `${dest}`, `${root}`, and `${home}`.
    fn run_post_clone(&self, dst: &Path) -> anyhow::Result<()> {
        if self.post_clone_cmd.is_empty() {
            return Ok(());
        }

        let repo = dst
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let dest = std::fs::canonicalize(dst)?;
        let args =
            crate::config::expand_clone_post_cmd(self.post_clone_cmd, &repo, &dest, self.root)?;

        if self.debug > 1 {
            crate::debug::log(
                self.root,
                &format!("[debug] post-clone {:?} in {}", args, dst.display()),
            )?;
        }

        let program = &args[0];
        let output = Command::new(program)
            .args(&args[1..])
            .current_dir(dst)
            .stdin(Stdio::null())
            .output()?;

        if !output.status.success() {
            return Err(anyhow!("post-clone command {} failed", program));
        }

        Ok(())
    }

    pub fn fetch(&self, dst: &Path) -> anyhow::Result<()> {
        if self.debug > 0 {
            crate::debug::log(self.root, &format!("[debug] fetch {}", dst.display()))?;
        }

        let repo = open_repository(dst)?;
        self.fetch_origin(&repo, dst)
            .with_context(|| format!("Failed to fetch repository {}", dst.display()))?;
        Ok(())
    }

    pub fn pull(&self, dst: &Path) -> anyhow::Result<()> {
        if self.debug > 0 {
            crate::debug::log(self.root, &format!("[debug] pull {}", dst.display()))?;
        }

        let repo = open_repository(dst)?;
        let head = repo
            .head()
            .with_context(|| format!("Failed to read HEAD in {}", dst.display()))?;
        if !head.is_branch() {
            return Err(anyhow!(
                "HEAD is detached in {}, nothing to pull",
                dst.display()
            ));
        }
        let branch_ref = head
            .name()
            .with_context(|| format!("Failed to read branch in {}", dst.display()))?
            .to_string();
        drop(head);

        self.fast_forward(&repo, dst, &branch_ref)
            .with_context(|| format!("Failed to pull repository {}", dst.display()))
    }

    /// Fetch the current branch from `origin` and fast-forward `branch_ref` to it,
    /// like `git pull --ff-only origin <branch>`.
    fn fast_forward(&self, repo: &Repository, dst: &Path, branch_ref: &str) -> anyhow::Result<()> {
        let (branch, fetch_commit) = self.fetch_origin(repo, dst)?;
        let (analysis, _) = repo.merge_analysis(&[&fetch_commit])?;

        if analysis.is_up_to_date() {
            if self.debug > 0 {
                crate::debug::log(
                    self.root,
                    &format!("[debug] pull {}: already up to date", dst.display()),
                )?;
            }
            return Ok(());
        }

        if !analysis.is_fast_forward() {
            return Err(anyhow!("cannot fast-forward {}, merge required", branch));
        }

        let target = repo.find_commit(fetch_commit.id())?;
        let current_tree = repo.head()?.peel_to_tree()?;

        let conflicts = local_changes_in_diff(repo, &current_tree, &target.tree()?)?;
        if !conflicts.is_empty() {
            return Err(anyhow!(
                "local changes to {} would be overwritten by pull, commit or stash them first",
                conflicts.join(", ")
            ));
        }

        repo.checkout_tree(
            target.as_object(),
            Some(git2::build::CheckoutBuilder::new().safe()),
        )?;

        repo.find_reference(branch_ref)?.set_target(
            target.id(),
            &format!("pull: Fast-forward to {}", target.id()),
        )?;

        if self.debug > 0 {
            crate::debug::log(
                self.root,
                &format!(
                    "[debug] pull {}: fast-forwarded {} to {}",
                    dst.display(),
                    branch,
                    target.id()
                ),
            )?;
        }

        Ok(())
    }

    /// Fetch the current branch from `origin`, returning the branch name and the
    /// fetched commit. The branch is fetched explicitly so we don't depend on a
    /// configured upstream tracking branch.
    fn fetch_origin<'r>(
        &self,
        repo: &'r Repository,
        dst: &Path,
    ) -> anyhow::Result<(String, git2::AnnotatedCommit<'r>)> {
        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(self.callbacks(dst.display().to_string()));

        let branch = {
            let head = repo.head()?;
            if !head.is_branch() {
                return Err(anyhow!("HEAD is detached, nothing to fetch"));
            }
            head.shorthand()?.to_string()
        };

        let mut remote = repo.find_remote("origin")?;
        remote.fetch(&[branch.as_str()], Some(&mut fo), None)?;

        let fetch_head = repo.find_reference("FETCH_HEAD")?;
        let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;

        Ok((branch, fetch_commit))
    }

    /// Whether the current branch has local commits ahead of its upstream
    /// tracking branch. Branches with no upstream are considered not ahead.
    pub fn has_unpushed(&self, dst: &Path) -> anyhow::Result<bool> {
        let repo = open_repository(dst)?;

        let head = repo.head()?;
        if !head.is_branch() {
            return Ok(false);
        }

        let Some(local_oid) = head.target() else {
            return Ok(false);
        };

        let branch = repo.find_branch(head.shorthand()?, git2::BranchType::Local)?;
        let Ok(upstream) = branch.upstream() else {
            return Ok(false);
        };
        let Some(upstream_oid) = upstream.get().target() else {
            return Ok(false);
        };

        let (ahead, _behind) = repo.graph_ahead_behind(local_oid, upstream_oid)?;
        Ok(ahead > 0)
    }

    /// Push `refspec` to `repository`, sending `push_options` as git push options.
    pub fn push(
        &self,
        dst: &Path,
        repository: &str,
        refspec: &str,
        push_options: &[String],
    ) -> anyhow::Result<()> {
        if self.debug > 0 {
            crate::debug::log(
                self.root,
                &format!("[debug] push {} {} {}", repository, refspec, dst.display()),
            )?;
        }

        let repo = open_repository(dst)?;

        let mut remote = repo.find_remote(repository)?;

        // libgit2 does not fail the push when the server rejects a ref, so we
        // collect per-ref status here and turn it into an error afterwards.
        let rejected = std::cell::RefCell::new(Vec::new());
        {
            let mut callbacks = RemoteCallbacks::new();
            callbacks.credentials(|_url, username, _allowed_types| {
                git2::Cred::ssh_key_from_agent(username.unwrap_or("git"))
            });
            callbacks.push_update_reference(|refname, status| {
                if let Some(msg) = status {
                    rejected.borrow_mut().push(format!("{}: {}", refname, msg));
                }
                Ok(())
            });
            if self.debug > 0 {
                set_debug_callbacks(
                    &mut callbacks,
                    dst.display().to_string(),
                    self.root.to_path_buf(),
                );
            }

            let mut po = git2::PushOptions::new();
            po.remote_callbacks(callbacks);

            let options: Vec<&str> = push_options.iter().map(String::as_str).collect();
            if !options.is_empty() {
                po.remote_push_options(&options);
            }

            remote.push(&[refspec], Some(&mut po))?;
        }

        let rejected = rejected.into_inner();
        if !rejected.is_empty() {
            return Err(anyhow!("push rejected: {}", rejected.join(", ")));
        }

        Ok(())
    }
}

fn open_repository(dst: &Path) -> anyhow::Result<Repository> {
    Repository::open(dst).with_context(|| format!("Failed to open repository {}", dst.display()))
}

/// Paths changed between `from` and `to` that have staged, unstaged or untracked
/// local changes. Like `git pull`, such files block the update even when their
/// content already matches `to`; ignored files may be overwritten.
fn local_changes_in_diff(
    repo: &Repository,
    from: &git2::Tree,
    to: &git2::Tree,
) -> anyhow::Result<Vec<String>> {
    let diff = repo.diff_tree_to_tree(Some(from), Some(to), None)?;
    let paths: std::collections::BTreeSet<&Path> = diff
        .deltas()
        .flat_map(|delta| [delta.old_file().path(), delta.new_file().path()])
        .flatten()
        .collect();

    let mut changed = Vec::new();
    for path in paths {
        let status = match repo.status_file(path) {
            Ok(status) => status,
            Err(error) if error.code() == git2::ErrorCode::NotFound => continue,
            Err(error) => return Err(error.into()),
        };
        if !status.is_empty() && !status.is_ignored() {
            changed.push(path.to_string_lossy().into_owned());
        }
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::Remote;
    use crate::config::RepositoryFilter;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn pull_reports_the_path_when_the_repository_cannot_be_opened() {
        let path: PathBuf = std::env::temp_dir().join(format!(
            "repo-set-pull-error-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::write(&path, "not a repository").unwrap();

        let filter = RepositoryFilter::new(&[], &[]).unwrap();
        let remote = Remote::new(&[], &[], path.parent().unwrap(), &filter, 0);
        let error = remote.pull(&path).unwrap_err();
        let message = format!("{error:#}");

        fs::remove_file(&path).unwrap();
        assert!(message.contains("Failed to open repository"), "{message}");
        assert!(message.contains(&path.display().to_string()), "{message}");
    }

    struct PullFixture {
        root: PathBuf,
        origin: git2::Repository,
        local: git2::Repository,
    }

    impl Drop for PullFixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn commit_file(repo: &git2::Repository, name: &str, content: &str) -> git2::Oid {
        let workdir = repo.workdir().unwrap();
        fs::write(workdir.join(name), content).unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new(name)).unwrap();
        index.write().unwrap();
        let tree = repo.find_tree(index.write_tree().unwrap()).unwrap();
        let signature = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        repo.commit(Some("HEAD"), &signature, &signature, name, &tree, &parents)
            .unwrap()
    }

    fn pull_fixture() -> PullFixture {
        let root = std::env::temp_dir().join(format!(
            "repo-set-pull-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        let origin = git2::Repository::init(root.join("origin")).unwrap();
        commit_file(&origin, "file.txt", "one\n");
        let local =
            git2::Repository::clone(root.join("origin").to_str().unwrap(), root.join("local"))
                .unwrap();
        PullFixture {
            root,
            origin,
            local,
        }
    }

    fn pull(fixture: &PullFixture) -> anyhow::Result<()> {
        let filter = RepositoryFilter::new(&[], &[]).unwrap();
        Remote::new(&[], &[], &fixture.root, &filter, 0).pull(fixture.local.workdir().unwrap())
    }

    fn local_head(fixture: &PullFixture) -> git2::Oid {
        fixture.local.head().unwrap().target().unwrap()
    }

    #[test]
    fn pull_fast_forwards_branch_and_working_tree() {
        let fixture = pull_fixture();
        let new_commit = commit_file(&fixture.origin, "file.txt", "two\n");

        pull(&fixture).unwrap();

        assert_eq!(local_head(&fixture), new_commit);
        let branch = fixture
            .local
            .head()
            .unwrap()
            .shorthand()
            .unwrap()
            .to_string();
        let tracking = fixture
            .local
            .find_reference(&format!("refs/remotes/origin/{branch}"))
            .unwrap();
        assert_eq!(tracking.target(), Some(new_commit));
        let content = fs::read_to_string(fixture.root.join("local/file.txt")).unwrap();
        assert_eq!(content, "two\n");
        assert!(fixture.local.statuses(None).unwrap().is_empty());
    }

    #[test]
    fn pull_refuses_to_overwrite_local_changes() {
        let fixture = pull_fixture();
        let before = local_head(&fixture);
        commit_file(&fixture.origin, "file.txt", "two\n");
        fs::write(fixture.root.join("local/file.txt"), "local\n").unwrap();

        let message = format!("{:#}", pull(&fixture).unwrap_err());

        assert!(message.contains("would be overwritten"), "{message}");
        assert_eq!(local_head(&fixture), before);
        let content = fs::read_to_string(fixture.root.join("local/file.txt")).unwrap();
        assert_eq!(content, "local\n");
    }

    #[test]
    fn pull_refuses_local_changes_identical_to_upstream() {
        let fixture = pull_fixture();
        let before = local_head(&fixture);
        commit_file(&fixture.origin, "file.txt", "two\n");
        fs::write(fixture.root.join("local/file.txt"), "two\n").unwrap();

        let message = format!("{:#}", pull(&fixture).unwrap_err());

        assert!(message.contains("would be overwritten"), "{message}");
        assert_eq!(local_head(&fixture), before);
    }

    #[test]
    fn pull_refuses_diverged_history() {
        let fixture = pull_fixture();
        commit_file(&fixture.origin, "file.txt", "two\n");
        let local_commit = commit_file(&fixture.local, "other.txt", "local\n");

        let message = format!("{:#}", pull(&fixture).unwrap_err());

        assert!(message.contains("cannot fast-forward"), "{message}");
        assert_eq!(local_head(&fixture), local_commit);
    }
}

/// Attach sideband and transfer-progress callbacks that append to `debug.log`.
fn set_debug_callbacks(callbacks: &mut RemoteCallbacks, label: String, root: std::path::PathBuf) {
    let sideband_root = root.clone();
    callbacks.sideband_progress(move |data| {
        if let Err(error) = crate::debug::log_bytes(&sideband_root, "remote: ", data) {
            eprintln!("{error:#}");
            return false;
        }
        true
    });
    callbacks.transfer_progress(move |stats| {
        if let Err(error) = crate::debug::log(
            &root,
            &format!(
                "{}: received {}/{} objects, indexed {}, {} bytes",
                label,
                stats.received_objects(),
                stats.total_objects(),
                stats.indexed_objects(),
                stats.received_bytes()
            ),
        ) {
            eprintln!("{error:#}");
            return false;
        }
        true
    });
}
