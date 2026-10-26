use anyhow::{Context, Result};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

static LOG_LOCK: Mutex<()> = Mutex::new(());

pub fn log(root: &Path, message: &str) -> Result<()> {
    let _guard = LOG_LOCK.lock().expect("debug log lock poisoned");
    let path = root.join("debug.log");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("Can't open debug log {}", path.display()))?;
    writeln!(file, "{message}").with_context(|| format!("Can't write debug log {}", path.display()))
}

pub fn log_bytes(root: &Path, label: &str, output: &[u8]) -> Result<()> {
    let _guard = LOG_LOCK.lock().expect("debug log lock poisoned");
    let path = root.join("debug.log");
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .with_context(|| format!("Can't open debug log {}", path.display()))?;
    write!(file, "{label}").with_context(|| format!("Can't write debug log {}", path.display()))?;
    file.write_all(output)
        .with_context(|| format!("Can't write debug log {}", path.display()))?;
    if !output.ends_with(b"\n") {
        writeln!(file).with_context(|| format!("Can't write debug log {}", path.display()))?;
    }
    Ok(())
}
