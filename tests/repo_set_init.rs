use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "repo-set-init-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn init_creates_config_in_current_directory_without_existing_config() {
    let fixture = Fixture::new();
    let output = Command::new(env!("CARGO_BIN_EXE_repo-set"))
        .current_dir(&fixture.0)
        .arg("init")
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let content = fs::read_to_string(fixture.0.join(".repo-set/config.toml")).unwrap();
    toml::from_str::<repo_set::config::Config>(&content).unwrap();
    assert!(
        String::from_utf8(output.stdout)
            .unwrap()
            .contains("Created")
    );
}

#[test]
fn init_uses_explicit_root_and_preserves_existing_config() {
    let fixture = Fixture::new();
    let root = fixture.0.join("nested/repos");
    let run = || {
        Command::new(env!("CARGO_BIN_EXE_repo-set"))
            .current_dir(&fixture.0)
            .args(["--root", "nested/repos", "init"])
            .output()
            .unwrap()
    };
    let output = run();
    assert!(output.status.success(), "{output:?}");
    let config = root.join(".repo-set/config.toml");
    fs::write(&config, "existing configuration").unwrap();
    let output = run();
    assert!(!output.status.success(), "{output:?}");
    assert_eq!(
        fs::read_to_string(config).unwrap(),
        "existing configuration"
    );
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("not overwritten")
    );
    assert!(!fixture.0.join(".repo-set").exists());
}

#[test]
fn init_reports_directory_creation_errors() {
    let fixture = Fixture::new();
    fs::write(fixture.0.join(".repo-set"), "not a directory").unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_repo-set"))
        .current_dir(&fixture.0)
        .arg("init")
        .output()
        .unwrap();
    assert!(!output.status.success(), "{output:?}");
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Failed to create")
    );
}
