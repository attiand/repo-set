use anyhow::Context;
use git2::Repository;
use std::fs;
use std::path::Path;
use std::process::Command;

/// Run the repository's `commit-msg` hook (if present) on `message`, returning
/// the possibly edited message. libgit2 never runs hooks, so we replicate what
/// git does: write the message to a file, invoke the hook with its path, and
/// read the result back.
pub fn run_commit_msg_hook(repo: &Repository, message: &str) -> anyhow::Result<String> {
    let hook = repo.path().join("hooks").join("commit-msg");
    if !hook.is_file() {
        return Ok(message.to_string());
    }

    let msg_path = repo.path().join("COMMIT_EDITMSG");
    fs::write(&msg_path, message)?;

    let mut command = hook_command(&hook)?;
    command.arg(&msg_path);
    if let Some(workdir) = repo.workdir() {
        command.current_dir(workdir);
    }

    let status = command
        .status()
        .with_context(|| format!("Failed to run commit-msg hook {}", hook.display()))?;
    if !status.success() {
        return Err(anyhow::anyhow!("commit-msg hook failed"));
    }

    Ok(fs::read_to_string(&msg_path)?)
}

#[cfg(not(windows))]
fn hook_command(hook: &Path) -> anyhow::Result<Command> {
    Ok(Command::new(hook))
}

/// Windows cannot execute shebang scripts directly, so, like Git for Windows,
/// read the hook's shebang and run it through the matching interpreter
/// (falling back to `sh`), looked up on PATH or in the Git for Windows install.
#[cfg(windows)]
fn hook_command(hook: &Path) -> anyhow::Result<Command> {
    let content = fs::read(hook)?;
    let first_line = content.split(|b| *b == b'\n').next().unwrap_or_default();
    let first_line = String::from_utf8_lossy(first_line);

    let mut interpreter = "sh".to_string();
    let mut interpreter_args = Vec::new();
    if let Some(shebang) = first_line.strip_prefix("#!") {
        let mut parts = shebang.split_whitespace();
        if let Some(program) = parts.next() {
            let name = program.rsplit(['/', '\\']).next().unwrap_or(program);
            if name == "env" {
                if let Some(target) = parts.next() {
                    interpreter = target.to_string();
                }
            } else {
                interpreter = name.to_string();
            }
            interpreter_args.extend(parts.map(str::to_string));
        }
    }

    let program = find_windows_interpreter(&interpreter)
        .or_else(|| find_windows_interpreter("sh"))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Cannot run commit-msg hook {}: '{}' not found (is Git for Windows installed?)",
                hook.display(),
                interpreter
            )
        })?;

    let mut command = Command::new(program);
    command.args(interpreter_args);
    command.arg(hook.to_string_lossy().replace('\\', "/"));
    Ok(command)
}

#[cfg(windows)]
fn find_windows_interpreter(name: &str) -> Option<std::path::PathBuf> {
    let exe = if name.to_ascii_lowercase().ends_with(".exe") {
        name.to_string()
    } else {
        format!("{name}.exe")
    };
    let path = std::env::var_os("PATH")?;
    let dirs: Vec<_> = std::env::split_paths(&path).collect();

    if let Some(found) = dirs.iter().map(|d| d.join(&exe)).find(|p| p.is_file()) {
        return Some(found);
    }

    // Outside Git Bash usually only `<Git>\cmd` is on PATH; derive the Git
    // install root from git.exe and look in its bin directories.
    dirs.iter()
        .filter(|d| d.join("git.exe").is_file())
        .filter_map(|d| d.parent().map(Path::to_path_buf))
        .flat_map(|root| {
            [
                root.join("usr").join("bin").join(&exe),
                root.join("bin").join(&exe),
                root.join("..").join("usr").join("bin").join(&exe),
                root.join("..").join("bin").join(&exe),
            ]
        })
        .find(|p| p.is_file())
}
