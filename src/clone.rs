use gix::Repository;
use std::path::Path;

pub fn run(repo_url: &str, dst: &Path) -> anyhow::Result<Repository> {
    // SAFETY: The closure doesn't use mutexes or memory allocation, so it should be safe to call from a signal handler.
    unsafe {
        gix::interrupt::init_handler(1, || {})?;
    }

    std::fs::create_dir_all(&dst)?;

    let url = gix::url::parse(repo_url.into())?;

    let mut prepare_clone = gix::prepare_clone(url, &dst)?;

    let (mut prepare_checkout, _) = prepare_clone
        .fetch_then_checkout(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)?;

    let (repo, _) =
        prepare_checkout.main_worktree(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)?;

    Ok(repo)
}
