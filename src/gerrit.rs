use anyhow::anyhow;
use std::process::{Command, Stdio};

pub fn repos(host: &str) -> anyhow::Result<Vec<String>> {
    let output = Command::new("ssh")
        .arg(format!("ssh://{}", host))
        .arg("gerrit")
        .arg("ls-projects")
        .stdin(Stdio::null())
        .output()?;

    if output.status.success() {
        let out = String::from_utf8(output.stdout);

        Ok(out?.split_whitespace().map(String::from).collect())
    } else {
        Err(anyhow!("Can't run gerrit ls-projects"))
    }
}
