# AGENT.md

Guidance for AI coding agents working in this repository.

## Project

`repo-set` is a Rust CLI application that manages a set of git repositories under a root
directory. A single command (clone, pull, fetch, push, status, reset, clean, stage, commit)
operates on every repository in the set. Git operations use `git2` (libgit2). CLI arguments
are handled by Clap.

## Build & run

```sh
cargo build            # debug build
cargo run -- <args>    # run with CLI arguments
cargo clippy           # lint
cargo fmt              # format
cargo test             # test
```

## Configuration

At runtime the CLI reads a configuration file (`~/.repo-set.toml` by default). see [README.md](README.md) for a description and examples.

## Source layout (`src/`)

- `main.rs` — CLI definition (clap derive) and command dispatch.
- `config.rs` — loads and deserializes the configuration file.
- `parameters.rs` - Holds application parameters, either from config or CLI.
- `remote.rs` — `Remote`: methods that operates on the remote repo.
- `local.rs` — `Local`: methods that operates on a local repo.
- `status.rs` — `ls`/status output helpers.
- `pool.rs` — I/O-friendly worker-thread pool for parallel operations.
- `progress.rs` — progress reporting over a crossbeam channel.

## Conventions

- Errors propagate with `anyhow::Result`.
- Keep comments to a single line that states what the code cannot show on its
  own; do not restate what the next line does.
- Verify changes with `cargo build` (and `cargo clippy`) before finishing.
