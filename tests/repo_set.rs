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

fn create_dirty_set(root: &std::path::Path) -> PathBuf {
    fs::create_dir_all(root.join(".repo-set")).unwrap();
    fs::write(
        root.join(".repo-set/config.toml"),
        "remote.url = 'unused'\nremote.list.cmd = ['unused']\n",
    )
    .unwrap();
    let path = root.join("repo");
    let repo = git2::Repository::init(&path).unwrap();
    fs::write(path.join("tracked"), "original").unwrap();
    let mut index = repo.index().unwrap();
    index.add_path(std::path::Path::new("tracked")).unwrap();
    index.write().unwrap();
    let tree_id = index.write_tree().unwrap();
    let tree = repo.find_tree(tree_id).unwrap();
    let signature = git2::Signature::now("Test", "test@example.com").unwrap();
    repo.commit(Some("HEAD"), &signature, &signature, "initial", &tree, &[])
        .unwrap();
    fs::write(path.join("tracked"), "modified").unwrap();
    path
}

#[test]
fn collection_reset_uses_each_member_root_and_config() {
    let fixture = Fixture::new();
    let root = fixture.0.join("collection");
    fs::create_dir_all(root.join(".repo-set")).unwrap();
    fs::write(
        root.join(".repo-set/collection.toml"),
        "collection = ['a', 'b']",
    )
    .unwrap();
    let first = create_dirty_set(&root.join("a"));
    let second = create_dirty_set(&root.join("b"));
    let output = Command::new(env!("CARGO_BIN_EXE_repo-set"))
        .current_dir(&fixture.0)
        .args(["--root", "collection", "reset", "--hard"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    for repo in [first, second] {
        assert_eq!(
            fs::read_to_string(repo.join("tracked")).unwrap(),
            "original"
        );
    }
}

#[test]
fn reset_without_collection_preserves_single_set_behavior() {
    let fixture = Fixture::new();
    let repo = create_dirty_set(&fixture.0);
    let output = Command::new(env!("CARGO_BIN_EXE_repo-set"))
        .current_dir(&fixture.0)
        .args(["reset", "--hard"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read_to_string(repo.join("tracked")).unwrap(),
        "original"
    );
}

#[test]
fn invalid_collection_reports_config_path() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.0.join(".repo-set")).unwrap();
    for content in ["collection = 'a'", "unknown = []", "collection = ["] {
        fs::write(fixture.0.join(".repo-set/collection.toml"), content).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_repo-set"))
            .current_dir(&fixture.0)
            .arg("status")
            .output()
            .unwrap();
        assert!(!output.status.success(), "{output:?}");
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("Failed to parse ./.repo-set/collection.toml")
        );
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
