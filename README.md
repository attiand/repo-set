# repo-set

This project builds two binaries.

`repo-set` manages a set of git repositories under a root directory.
A single command (clone, pull, fetch, push, status, reset, clean, stage, commit) operates on every repository in the set.
`repo-set` reads the configuration file `.repo-set/config.toml` in the root directory.

See [repo-set](REPO-SET.md) for more information.


`repo-clone` is a simple utility that clone or list remote repositories. `repo-clone` reads the configuration file `.repo-clone/config.toml` in the user's home directory.

See [remote-repo](REMOTE-REPO.md) for more information.