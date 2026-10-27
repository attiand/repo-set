use git2::Repository;
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_ID: AtomicUsize = AtomicUsize::new(0);

struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(list_cmd: &[&str], post_cmd: &[&str]) -> Self {
        let root = std::env::temp_dir().join(format!(
            "remote-repo-cli-{}-{}",
            std::process::id(),
            NEXT_ID.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).unwrap();
        fs::create_dir(root.join(".remote-repo")).unwrap();
        fs::create_dir(root.join("sources")).unwrap();
        for name in ["repo-a", "repo-b", "ignored"] {
            let repo = Repository::init(root.join("sources").join(name)).unwrap();
            let signature = git2::Signature::now("Test", "test@example.com").unwrap();
            let tree_id = repo.index().unwrap().write_tree().unwrap();
            let tree = repo.find_tree(tree_id).unwrap();
            repo.commit(Some("HEAD"), &signature, &signature, "Initial", &tree, &[])
                .unwrap();
        }
        let mut config = toml::Table::new();
        let mut remote = toml::Table::new();
        remote.insert(
            "url".into(),
            toml::Value::String(root.join("sources").to_str().unwrap().into()),
        );
        remote.insert(
            "list".into(),
            toml::Value::Table(toml::Table::from_iter([(
                "cmd".into(),
                toml::Value::try_from(list_cmd).unwrap(),
            )])),
        );
        config.insert("remote".into(), toml::Value::Table(remote));
        config.insert(
            "repositories".into(),
            toml::Value::Table(toml::Table::from_iter([(
                "exclude".into(),
                toml::Value::try_from(["ignored"]).unwrap(),
            )])),
        );
        config.insert(
            "clone".into(),
            toml::Value::Table(toml::Table::from_iter([(
                "post".into(),
                toml::Value::Table(toml::Table::from_iter([(
                    "cmd".into(),
                    toml::Value::try_from(post_cmd).unwrap(),
                )])),
            )])),
        );
        fs::write(
            root.join(".remote-repo/config.toml"),
            toml::to_string(&config).unwrap(),
        )
        .unwrap();
        Self { root }
    }

    fn set_repository_filters(&self, include: &[&str], exclude: &[&str]) {
        let config_path = self.root.join(".remote-repo/config.toml");
        let mut config: toml::Value =
            toml::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
        let repositories = config
            .get_mut("repositories")
            .unwrap()
            .as_table_mut()
            .unwrap();
        repositories.insert("include".into(), toml::Value::try_from(include).unwrap());
        repositories.insert("exclude".into(), toml::Value::try_from(exclude).unwrap());
        fs::write(config_path, toml::to_string(&config).unwrap()).unwrap();
    }

    fn run(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_remote-repo"))
            .env("HOME", &self.root)
            .arg("--root")
            .arg(self.root.join("dest"))
            .arg("clone")
            .args(args)
            .output()
            .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn completion_generates_scripts_without_loading_config() {
    let fixture = Fixture::new(&["false"], &[]);
    fs::remove_file(fixture.root.join(".remote-repo/config.toml")).unwrap();

    for shell in ["bash", "zsh", "fish", "powershell", "elvish"] {
        let output = Command::new(env!("CARGO_BIN_EXE_remote-repo"))
            .env("HOME", &fixture.root)
            .args(["completion", shell])
            .output()
            .unwrap();
        assert!(output.status.success(), "{shell}: {output:?}");
        let script = std::str::from_utf8(&output.stdout).unwrap();
        assert!(script.contains("remote-repo"), "{shell}: {script}");
        assert!(script.contains("COMPLETE"), "{shell}: {script}");
        assert!(output.stderr.is_empty(), "{shell}: {output:?}");
    }
}

#[test]
fn completes_repository_names_dynamically() {
    let fixture = Fixture::new(&["printf", "repo-a\nrepo-b\nignored\nother\n"], &[]);
    for (words, expected) in [
        (vec!["repo-"], vec!["repo-a", "repo-b"]),
        (vec!["repo-a", "repo-b"], vec!["repo-b"]),
        (vec!["ignored"], vec![]),
        (vec!["missing"], vec![]),
    ] {
        let output = Command::new(env!("CARGO_BIN_EXE_remote-repo"))
            .current_dir(&fixture.root)
            .env("HOME", &fixture.root)
            .env("COMPLETE", "bash")
            .env("_CLAP_COMPLETE_INDEX", (words.len() + 1).to_string())
            .args(["--", "remote-repo", "clone"])
            .args(&words)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let stdout = std::str::from_utf8(&output.stdout).unwrap();
        assert_eq!(stdout.lines().collect::<Vec<_>>(), expected);
        assert!(output.stderr.is_empty(), "{output:?}");
    }
    assert!(!fixture.root.join("repo-a").exists());
}

#[test]
fn completion_listing_failures_are_quiet() {
    let fixture = Fixture::new(&["false"], &[]);
    for missing_config in [false, true] {
        if missing_config {
            fs::remove_file(fixture.root.join(".remote-repo/config.toml")).unwrap();
        }
        let output = Command::new(env!("CARGO_BIN_EXE_remote-repo"))
            .env("HOME", &fixture.root)
            .env("COMPLETE", "bash")
            .env("_CLAP_COMPLETE_INDEX", "2")
            .args(["--", "remote-repo", "clone", "repo-"])
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(output.stdout.is_empty(), "{output:?}");
        assert!(output.stderr.is_empty(), "{output:?}");
    }
}

#[test]
fn list_prints_remote_repositories_allowed_by_config() {
    let fixture = Fixture::new(&["printf", "repo-a\nrepo-b\nignored\n"], &[]);
    let output = Command::new(env!("CARGO_BIN_EXE_remote-repo"))
        .env("HOME", &fixture.root)
        .arg("list")
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        std::str::from_utf8(&output.stdout)
            .unwrap()
            .lines()
            .collect::<Vec<_>>(),
        ["repo-a", "repo-b"]
    );
    assert!(output.stderr.is_empty(), "{output:?}");
}

#[test]
fn clones_named_repositories_and_skips_ignored_and_existing_destinations() {
    let fixture = Fixture::new(&["false"], &[]);
    let existing = fixture.root.join("dest/repo-b");
    fs::create_dir_all(&existing).unwrap();
    fs::write(existing.join("keep"), "unchanged").unwrap();

    let output = fixture.run(&["repo-a", "repo-b", "ignored"]);
    assert!(output.status.success(), "{output:?}");
    let repo = Repository::open(fixture.root.join("dest/repo-a")).unwrap();
    assert_eq!(
        repo.head()
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .message()
            .unwrap(),
        "Initial"
    );
    assert_eq!(
        fs::read_to_string(existing.join("keep")).unwrap(),
        "unchanged"
    );
    assert!(!fixture.root.join("dest/ignored").exists());
}

#[test]
fn clone_host_overrides_configured_remote_url() {
    let fixture = Fixture::new(&["false"], &[]);
    let config_path = fixture.root.join(".remote-repo/config.toml");
    let mut config: toml::Value =
        toml::from_str(&fs::read_to_string(&config_path).unwrap()).unwrap();
    config["remote"]["url"] = toml::Value::String("invalid-base-url".into());
    fs::write(config_path, toml::to_string(&config).unwrap()).unwrap();
    let host = fixture.root.join("sources").to_string_lossy().into_owned();

    let output = fixture.run(&["--host", &host, "repo-a"]);

    assert!(output.status.success(), "{output:?}");
    Repository::open(fixture.root.join("dest/repo-a")).unwrap();
}

#[test]
fn debug_option_writes_remote_operation_logs() {
    let fixture = Fixture::new(&["false"], &[]);
    let output = fixture.run(&["--debug", "repo-a"]);

    assert!(output.status.success(), "{output:?}");
    let log = fs::read_to_string(fixture.root.join("dest/debug.log")).unwrap();
    assert!(log.contains("[debug] clone "));
    assert!(log.contains("[debug] clone done "));
}

#[test]
fn all_clones_non_ignored_repositories_and_runs_post_clone_command() {
    let fixture = Fixture::new(
        &["printf", "repo-a\nrepo-b\nignored\n"],
        &["touch", "${dest}/${repo}.hook", "${root}/${repo}.root-hook"],
    );

    let output = fixture.run(&["--all"]);
    assert!(output.status.success(), "{output:?}");
    for name in ["repo-a", "repo-b"] {
        Repository::open(fixture.root.join("dest").join(name)).unwrap();
        assert!(
            fixture
                .root
                .join("dest")
                .join(name)
                .join(format!("{name}.hook"))
                .exists()
        );
        assert!(
            fixture
                .root
                .join("dest")
                .join(format!("{name}.root-hook"))
                .exists()
        );
    }
    assert!(!fixture.root.join("dest/ignored").exists());
}

#[test]
fn repository_globs_filter_all_and_explicit_clone_selection() {
    let fixture = Fixture::new(&["printf", "repo-a\nrepo-b\nignored\n"], &[]);
    fixture.set_repository_filters(&["repo-*"], &["repo-b"]);

    let output = fixture.run(&["--all"]);
    assert!(output.status.success(), "{output:?}");
    assert!(fixture.root.join("dest/repo-a").exists());
    assert!(!fixture.root.join("dest/repo-b").exists());
    assert!(!fixture.root.join("dest/ignored").exists());

    let fixture = Fixture::new(&["printf", "repo-a\nrepo-b\nignored\n"], &[]);
    fixture.set_repository_filters(&["repo-*"], &["repo-b"]);

    let output = fixture.run(&["repo-*"]);
    assert!(output.status.success(), "{output:?}");
    assert!(fixture.root.join("dest/repo-a").exists());
    assert!(!fixture.root.join("dest/repo-b").exists());
    assert!(!fixture.root.join("dest/ignored").exists());
}

#[test]
fn clones_matching_repositories_for_globs_without_duplicates() {
    let fixture = Fixture::new(&["printf", "repo-a\nrepo-b\nignored\n"], &[]);
    let output = fixture.run(&["repo-a", "repo-*"]);

    assert!(output.status.success(), "{output:?}");
    for name in ["repo-a", "repo-b"] {
        Repository::open(fixture.root.join("dest").join(name)).unwrap();
    }
    assert!(!fixture.root.join("dest/ignored").exists());
}

#[test]
fn root_variable_works_when_repo_set_root_is_relative() {
    let fixture = Fixture::new(
        &["printf", "repo-a\n"],
        &[
            "cp",
            "${root}/.repo-set/commit-msg",
            "${dest}/.git/hooks/commit-msg",
        ],
    );
    let root = fixture.root.join("dest");
    let config_dir = root.join(".repo-set");
    fs::create_dir_all(&config_dir).unwrap();
    fs::copy(
        fixture.root.join(".remote-repo/config.toml"),
        config_dir.join("config.toml"),
    )
    .unwrap();
    fs::write(config_dir.join("commit-msg"), "hook content").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_repo-set"))
        .current_dir(&fixture.root)
        .args(["--root", "dest", "clone"])
        .output()
        .unwrap();

    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read_to_string(root.join("repo-a/.git/hooks/commit-msg")).unwrap(),
        "hook content"
    );
}

#[test]
fn post_clone_command_is_printed_only_with_double_debug() {
    for (debug_args, expect_command) in
        [(vec!["--debug"], false), (vec!["--debug", "--debug"], true)]
    {
        let fixture = Fixture::new(&["printf", "repo-a\n"], &["touch", "${dest}/${repo}.hook"]);
        let repo_set_config_dir = fixture.root.join("dest/.repo-set");
        fs::create_dir_all(&repo_set_config_dir).unwrap();
        fs::copy(
            fixture.root.join(".remote-repo/config.toml"),
            repo_set_config_dir.join("config.toml"),
        )
        .unwrap();

        let output = Command::new(env!("CARGO_BIN_EXE_repo-set"))
            .arg("--root")
            .arg(fixture.root.join("dest"))
            .args(debug_args)
            .arg("clone")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        assert!(
            !String::from_utf8(output.stdout)
                .unwrap()
                .contains("[debug] post-clone")
        );
        assert!(
            !String::from_utf8(output.stderr)
                .unwrap()
                .contains("[debug]")
        );
        let log = fs::read_to_string(fixture.root.join("dest/debug.log")).unwrap();
        assert!(log.contains("[debug] clone "));
        assert_eq!(
            log.contains("[debug] post-clone [\"touch\""),
            expect_command
        );
    }
}

#[test]
fn listing_clone_and_post_clone_failures_are_reported() {
    let fixture = Fixture::new(&["false"], &[]);
    assert!(!fixture.run(&["--all"]).status.success());
    let output = fixture.run(&["missing"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("Failed to clone missing")
    );

    let fixture = Fixture::new(&["false"], &["false"]);
    let output = fixture.run(&["repo-a"]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("post-clone command")
    );
}
