use anyhow::anyhow;
use std::process::{Command, Stdio};

pub fn remote_repos(host: &str) -> anyhow::Result<Vec<String>> {
    let output = Command::new("ssh")
        .arg("ssh://")
        .arg(host)
        .arg("gerrit")
        .arg("ls-projects")
        .stdin(Stdio::null())
        .output()?;

    if output.status.success() {
        let out = String::from_utf8(output.stdout);

        Ok(out?.split_whitespace().map(|s| String::from(s)).collect())
    } else {
        return Err(anyhow!("Can't run gerrit ls-projects"));
    }
}
