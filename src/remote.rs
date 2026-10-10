use crate::parameters::Parameters;
use anyhow::anyhow;
use git2::{RemoteCallbacks, Repository};
use std::path::Path;
use std::process::{Command, Stdio};

/// Performs git remote operations, optionally logging debug output to stderr.
pub struct Remote<'a> {
    parameters: &'a Parameters,
}

impl<'a> Remote<'a> {
    pub fn new(parameters: &'a Parameters) -> Self {
        Self { parameters }
    }

    /// List remote repositories, excluding any configured to be ignored.
    pub fn repo_list(&self) -> anyhow::Result<Vec<String>> {
        let cmd = &self.parameters.remote_list_cmd;
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
            .filter(|r| !self.parameters.ignore_repos.iter().any(|i| i == r))
            .map(String::from)
            .collect())
    }

    /// Build callbacks with SSH-agent credentials and, when debugging, progress logging.
    fn callbacks(&self, label: String) -> RemoteCallbacks<'static> {
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username, _allowed_types| {
            git2::Cred::ssh_key_from_agent(username.unwrap_or("git"))
        });
        if self.parameters.debug {
            set_debug_callbacks(&mut callbacks, label);
        }
        callbacks
    }

    pub fn clone(&self, repo_name: &str) -> anyhow::Result<Repository> {
        let remote_repo_url = self.parameters.remote_repo_url(repo_name);
        let dst = self.parameters.local_repo_path(repo_name);
        if self.parameters.debug {
            eprintln!("[debug] clone {} -> {}", remote_repo_url, dst.display());
        }

        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(self.callbacks(remote_repo_url.clone()));

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fo);

        let repo = builder
            .clone(&remote_repo_url, &dst)
            .map_err(anyhow::Error::msg)?;
        if self.parameters.debug {
            eprintln!("[debug] clone done {}", dst.display());
        }

        self.run_post_clone(&dst)?;

        Ok(repo)
    }

    /// Run the resolved post-clone command inside the freshly cloned repo.
    fn run_post_clone(&self, dst: &Path) -> anyhow::Result<()> {
        let args = self.parameters.clone_post_cmd(dst)?;
        let Some(program) = args.first() else {
            return Ok(());
        };

        if self.parameters.debug {
            eprintln!("[debug] post-clone {:?} in {}", args, dst.display());
        }

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

    pub fn fetch(&self, repo_name: &str) -> anyhow::Result<()> {
        let dst = self.parameters.local_repo_path(repo_name);
        if self.parameters.debug {
            eprintln!("[debug] fetch {}", dst.display());
        }

        let repo = Repository::open(&dst)?;
        self.fetch_origin(&repo, &dst)?;
        Ok(())
    }

    pub fn pull(&self, repo_name: &str) -> anyhow::Result<()> {
        let dst = self.parameters.local_repo_path(repo_name);
        if self.parameters.debug {
            eprintln!("[debug] pull {}", dst.display());
        }

        let repo = Repository::open(&dst)?;
        let head = repo.head()?;
        if !head.is_branch() {
            return Err(anyhow!("HEAD is detached, nothing to pull"));
        }
        let branch = head.shorthand()?.to_string();

        let output = Command::new("git")
            .args(["pull", "--ff-only", "origin", branch.as_str()])
            .current_dir(&dst)
            .output()?;

        if self.parameters.debug {
            eprint!("{}", String::from_utf8_lossy(&output.stdout));
            eprint!("{}", String::from_utf8_lossy(&output.stderr));
        }

        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("git pull failed: {}", detail.trim()));
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
    pub fn has_unpushed(&self, repo_name: &str) -> anyhow::Result<bool> {
        let repo = Repository::open(self.parameters.local_repo_path(repo_name))?;

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

    /// Push the supplied refspec to origin with the configured push options.
    pub fn push(&self, repo_name: &str, refspec: &str) -> anyhow::Result<()> {
        let dst = self.parameters.local_repo_path(repo_name);
        if self.parameters.debug {
            eprintln!("[debug] push {} {}", refspec, dst.display());
        }

        let repo = Repository::open(&dst)?;

        let mut remote = repo.find_remote("origin")?;

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
            if self.parameters.debug {
                set_debug_callbacks(&mut callbacks, dst.display().to_string());
            }

            let mut po = git2::PushOptions::new();
            po.remote_callbacks(callbacks);

            let options: Vec<&str> = self
                .parameters
                .push_options
                .iter()
                .map(String::as_str)
                .collect();
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

/// Attach sideband and transfer-progress callbacks that log to stderr.
fn set_debug_callbacks(callbacks: &mut RemoteCallbacks, label: String) {
    callbacks.sideband_progress(|data| {
        eprint!("remote: {}", String::from_utf8_lossy(data));
        true
    });
    callbacks.transfer_progress(move |stats| {
        eprintln!(
            "{}: received {}/{} objects, indexed {}, {} bytes",
            label,
            stats.received_objects(),
            stats.total_objects(),
            stats.indexed_objects(),
            stats.received_bytes()
        );
        true
    });
}
