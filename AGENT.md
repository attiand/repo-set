# AGENT.md

Guidance for AI coding agents working in this repository.

## Project

`repo-set` is a Rust CLI that manages a set of git repositories under a root
directory. A single command (clone, pull, reset, list, status) operates on every
repository in the set. Git operations use `git2` (libgit2); repository listing
can shell out to an external command (e.g. Gerrit `ls-projects`).

## Build & run

```sh
cargo build            # debug build
cargo run -- <args>    # run with CLI arguments
cargo clippy           # lint
cargo fmt              # format
cargo test             # test
```

There are currently no automated tests.

## Configuration

At runtime the CLI reads `~/.repo-set.toml`. Example:

```toml
[remote]
url = "my-gerrit:29418"
list.cmd = [ "ssh", "ssh://my-gerrit:29418", "gerrit" ,"ls-projects" ]

[repo]
ignore =["non-important-repo"]

[clone]
post.cmd = [ "cp", "myhook", "${dest}/.git/hooks/myhook" ]
```

## Source layout (`src/`)

- `main.rs` — CLI definition (clap derive) and command dispatch.
- `config.rs` — loads and deserializes `~/.repo-set.toml`.
- `remote.rs` — `Remote`: methods that operates on the remote repo.
- `local.rs` — `Local`: methods that operates on a local repo.
- `status.rs` — `ls`/status output helpers.
- `pool.rs` — I/O-friendly worker-thread pool for parallel operations.
- `progress.rs` — progress reporting over a crossbeam channel.

## Conventions

- `Remote` and `Local` take `&[String]` slices (e.g. the ignore list), not the
  `Config` struct, so they stay decoupled from configuration loading.
- Errors propagate with `anyhow::Result`.
- Keep comments to a single line that states what the code cannot show on its
  own; do not restate what the next line does.
- Verify changes with `cargo build` (and `cargo clippy`) before finishing.
