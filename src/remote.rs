use anyhow::anyhow;
use git2::{RemoteCallbacks, Repository};
use std::path::Path;
use std::process::{Command, Stdio};

pub fn clone(repo_url: &str, dst: &Path) -> anyhow::Result<Repository> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username, _allowed_types| {
        let username = username.unwrap_or("git");
        git2::Cred::ssh_key_from_agent(username)
    });

    // Prepare fetch options.
    let mut fo = git2::FetchOptions::new();
    fo.remote_callbacks(callbacks);

    // Prepare builder.
    let mut builder = git2::build::RepoBuilder::new();
    builder.fetch_options(fo);

    // Clone the project.
    builder.clone(repo_url, dst).map_err(anyhow::Error::msg)
}

pub fn repo_list(cmd: &[String]) -> anyhow::Result<Vec<String>> {
    let program: Option<&String> = cmd.first();

    if program.is_some() {
        let output = Command::new(program.unwrap())
            .args(&cmd[1..])
            .stdin(Stdio::null())
            .output()?;

        if output.status.success() {
            let out = String::from_utf8(output.stdout);

            Ok(out?.split_whitespace().map(String::from).collect())
        } else {
            Err(anyhow!(format!("Can't run command {}", program.unwrap())))
        }
    } else {
        Ok(Vec::new())
    }
}
