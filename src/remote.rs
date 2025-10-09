use git2::{RemoteCallbacks, Repository};
use std::path::Path;

pub fn clone(repo_url: &str, dst: &Path) -> anyhow::Result<Repository> {
    let mut callbacks = RemoteCallbacks::new();
    callbacks.credentials(|_url, username, _allowed_types| {
        let username = username.unwrap_or("git");
        println!("Auth callback with user {}", username);
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
