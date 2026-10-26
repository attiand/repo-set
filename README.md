# repo-set

This project builds two binaries.

`repo-set` manages a set of git repositories under a root directory.
A single command (clone, pull, fetch, push, status, reset, clean, stage, commit) operates on every repository in the set.
`repo-set` reads a configuration file `.repo-set/config.toml` in the root directory.

See [repo-set](REPO-SET.md) for more information.


`repo-clone` simply clones one or several Git repositories. `repo-clone` reads a configuration file `.repo-clone/config.toml`
in the user's home directory.

See [repo-clone](REPO-CLONE.md) for more information.