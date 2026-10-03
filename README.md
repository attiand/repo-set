# repo-set

Handles a set of git repos. A typical git command such as `reset` resets all repos in the set. The location of the local set is current working directory or specified with the `--root` option.

Implemented sub commands:

## clone

Clone specified or all remote repos to the root directory.

## list

List remote, local or difference between remote and local.

## pull

Pull all local repos

## reset

Reset all local repos, only `--hard` mode implemented.

## status

Print git status for each repo with changes.

## clean

Remove untracked files from the working tree.

## help

Print help information.

# Install

```bash
cargo install --git https://github.com/attiand/repo-set.git
```

# Configuration file

repo-set reads a configuration file in the users home directory named `.repo-set.toml`

To use with a gerrit server:

```toml
remote.url = "my-gerrit:29418"

[repo]
list.cmd = [ "ssh", "ssh://my-gerrit:29418", "gerrit" ,"ls-projects" ]
ignore =["non-important-repo"]
```
