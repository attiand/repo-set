# AGENT.md

Guidance for AI coding agents working in this repository.

## Project

The project produces two Rust CLI applications. Git operations use `git2` (libgit2). CLI arguments are handled by Clap.

`repo-set` manages a set of git repositories under a root directory. A single command (clone, pull, fetch, push, status, reset, clean, stage, commit) operates on every
repository in the set. 

`remote-repo` simply list or clone one or several remote Git repositories.

## Build & run

```sh
cargo build            # debug build
cargo run -- <args>    # run with CLI arguments
cargo clippy           # lint
cargo fmt              # format
cargo test             # test
```

## Configuration

At runtime the CLI reads a configuration file (`~/.repo-set.toml` by default). see [README.md](README.md)
for a description and examples.

## Source layout (`src/`)

- `bin/repo-set.rs` — CLI definition (clap derive) and command dispatch for `repo-set`
- `bin/repo-set/config.rs - Extends config.rs and add config settings for `repo-set`
- `bin/remote-repo.rs` — CLI definition (clap derive) and command dispatch for `remote-repo`
- `bin/remote-repo/config.rs - Extends config.rs and add config settings for `remote-repo`
- `lib.rs` — exposes shared modules to both binaries.
- `config.rs` — shared configuration and configuration file loading.
- `bin/repo-set/config.rs` — private `RepoSetConfig` and push options.
- `remote.rs` — `Remote`: methods that operates on the remote repo.
- `local.rs` — `Local`: methods that operates on a local repo.
- `status.rs` — `ls`/status output helpers.
- `pool.rs` — I/O-friendly worker-thread pool for parallel operations.
- `progress.rs` — progress reporting over a crossbeam channel.

## Conventions

- `Remote` and `Local` take the shared repository filter, not the `Config`
  struct, so they stay decoupled from configuration loading.
- Errors propagate with `anyhow::Result`.
- Keep comments to a single line that states what the code cannot show on its
  own; do not restate what the next line does.
- Verify changes with `cargo build` (and `cargo clippy`) before finishing.
- Keep documentation short and concise
