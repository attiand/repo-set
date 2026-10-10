use crate::local::Local;
use crate::remote::Remote;
use anyhow::Result;
use console::style;
use std::collections::HashSet;

/// Print all remote repositories.
pub fn remote(remote: &Remote) -> Result<()> {
    remote
        .repo_list()?
        .into_iter()
        .for_each(|r| println!("{}", r));
    Ok(())
}

/// Print all local repositories.
pub fn local(local: &Local) -> Result<()> {
    local.repos()?.into_iter().for_each(|r| println!("{}", r));
    Ok(())
}

/// Print, for every local repo with a dirty working tree, the repo name
/// followed by its `git status --short` output.
pub fn dirty(local: &Local) -> Result<()> {
    for repo in local.repos()? {
        let entries = local.status_short(&repo)?;
        if entries.is_empty() {
            continue;
        }
        println!("{}", style(&repo).bold());
        entries
            .iter()
            .for_each(|e| println!("  {} {}", style(&e.status).red(), e.path));
    }
    Ok(())
}

/// Print, for every local repo, the untracked files and directories that
/// `clean` would remove, grouped by repo name.
pub fn clean_preview(local: &Local) -> Result<()> {
    if !local.repos()?.is_empty() {
        println!("Specify --force to remove the following:");
    }

    for repo in local.repos()? {
        let paths = local.clean_list(&repo)?;
        if paths.is_empty() {
            continue;
        }
        println!("{}", style(&repo).bold());
        paths.iter().for_each(|p| println!("  {}", p));
    }
    Ok(())
}

/// Print, on a single space-separated line, local repos that have no remote
/// together with directories that are not git repos.
pub fn superfluous(remote: &Remote, local: &Local) -> Result<()> {
    let remote_set: HashSet<String> = remote.repo_list()?.into_iter().collect();

    let mut items: Vec<String> = local
        .repos()?
        .into_iter()
        .filter(|r| !remote_set.contains(r))
        .collect();
    items.extend(local.non_repos()?);

    println!("{}", items.join(" "));
    Ok(())
}

/// Print the +/- difference between remote and local repositories.
pub fn diff(remote: &Remote, local: &Local) -> Result<()> {
    let remote_repos = remote.repo_list()?;
    let local_repos = local.repos()?;

    let remote_set: HashSet<&String> = remote_repos.iter().collect();
    let local_set: HashSet<&String> = local_repos.iter().collect();

    remote_repos
        .iter()
        .filter(|r| !local_set.contains(*r))
        .for_each(|r| println!("-{}", r));
    local_repos
        .iter()
        .filter(|r| !remote_set.contains(*r))
        .for_each(|r| println!("+{}", r));
    Ok(())
}

/// Print a status overview: remote repos not cloned, local repos with no
/// remote, and directories that are not git repos. Headings are only shown
/// for non-empty sections.
pub fn status(remote: &Remote, local: &Local) -> Result<()> {
    let remote_repos = remote.repo_list()?;
    let local_repos = local.repos()?;
    let superfluous = local.non_repos()?;

    let remote_set: HashSet<&String> = remote_repos.iter().collect();
    let local_set: HashSet<&String> = local_repos.iter().collect();

    let not_cloned: Vec<&String> = remote_repos
        .iter()
        .filter(|r| !local_set.contains(*r))
        .collect();
    let no_remote: Vec<&String> = local_repos
        .iter()
        .filter(|r| !remote_set.contains(*r))
        .collect();

    let mut printed = false;
    print_section(
        "Remote repos not present locally:",
        &not_cloned,
        &mut printed,
    );
    print_section("Local repos with no remote:", &no_remote, &mut printed);
    print_section(
        "Superfluous directories (not git repos):",
        &superfluous,
        &mut printed,
    );
    Ok(())
}

fn print_section<T: std::fmt::Display>(heading: &str, items: &[T], printed: &mut bool) {
    if items.is_empty() {
        return;
    }
    if *printed {
        println!();
    }
    println!("{}", heading);
    items.iter().for_each(|r| println!("{}", style(r).red()));
    *printed = true;
}
