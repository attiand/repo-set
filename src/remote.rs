use anyhow::anyhow;
use git2::{RemoteCallbacks, Repository};
use std::path::Path;
use std::process::{Command, Stdio};

/// Performs git remote operations, optionally logging debug output to stderr.
pub struct Remote {
    debug: bool,
}

impl Remote {
    pub fn new(debug: bool) -> Self {
        Self { debug }
    }

    /// Build callbacks with SSH-agent credentials and, when debugging, progress logging.
    fn callbacks(&self, label: String) -> RemoteCallbacks<'static> {
        let mut callbacks = RemoteCallbacks::new();
        callbacks.credentials(|_url, username, _allowed_types| {
            git2::Cred::ssh_key_from_agent(username.unwrap_or("git"))
        });
        if self.debug {
            set_debug_callbacks(&mut callbacks, label);
        }
        callbacks
    }

    pub fn clone(&self, repo_url: &str, dst: &Path) -> anyhow::Result<Repository> {
        if self.debug {
            eprintln!("[debug] clone {} -> {}", repo_url, dst.display());
        }

        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(self.callbacks(repo_url.to_string()));

        let mut builder = git2::build::RepoBuilder::new();
        builder.fetch_options(fo);

        let repo = builder.clone(repo_url, dst).map_err(anyhow::Error::msg)?;
        if self.debug {
            eprintln!("[debug] clone done {}", dst.display());
        }
        Ok(repo)
    }

    pub fn pull(&self, dst: &Path) -> anyhow::Result<()> {
        if self.debug {
            eprintln!("[debug] pull {}", dst.display());
        }

        let repo = Repository::open(dst)?;

        let mut fo = git2::FetchOptions::new();
        fo.remote_callbacks(self.callbacks(dst.display().to_string()));

        // Fetch the current branch explicitly so we don't depend on a configured
        // upstream tracking branch.
        let branch = {
            let head = repo.head()?;
            if !head.is_branch() {
                return Err(anyhow!("HEAD is detached, nothing to pull"));
            }
            head.shorthand()?.to_string()
        };

        let mut remote = repo.find_remote("origin")?;
        remote.fetch(&[branch.as_str()], Some(&mut fo), None)?;

        let fetch_head = repo.find_reference("FETCH_HEAD")?;
        let fetch_commit = repo.reference_to_annotated_commit(&fetch_head)?;
        let (analysis, _) = repo.merge_analysis(&[&fetch_commit])?;

        if analysis.is_up_to_date() {
            if self.debug {
                eprintln!("[debug] pull {}: already up to date", dst.display());
            }
            return Ok(());
        }

        if !analysis.is_fast_forward() {
            return Err(anyhow!("cannot fast-forward, merge required"));
        }

        let mut head = repo.head()?;
        head.set_target(fetch_commit.id(), "pull: fast-forward")?;
        repo.checkout_head(Some(git2::build::CheckoutBuilder::default().force()))?;

        if self.debug {
            eprintln!(
                "[debug] pull {}: fast-forwarded {} to {}",
                dst.display(),
                branch,
                fetch_commit.id()
            );
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

pub fn repo_list(cmd: &[String]) -> anyhow::Result<Vec<String>> {
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
    Ok(out.split_whitespace().map(String::from).collect())
}
