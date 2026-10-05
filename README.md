# repo-set

Handles a set of git repos. A typical git command such as `reset` resets all repos in the set. The location of the local set is current working directory or specified with the `--root` option.

Implemented sub commands:

## clone

Clone specified or all remote repos to the root directory.

## list

List remote, local or difference between remote and local.

## fetch

Fetch all local repos

## pull

Pull all local repos

## reset

Reset all local repos, only `--hard` mode implemented.

## status

Print git status for each repo with dirty workspaces.

## clean

Remove untracked files from the working tree.

## Stage/Add

Stage all local repos that do not have a clean workspace.

## Commit

Commit all local repos that has something in the index.

## Push

Push all local repos that is ahead of its tracking branch.

## help

Print help information.

# Install

```bash
cargo install --git https://github.com/attiand/repo-set.git
```

# Configuration file

`repo-set` reads a configuration file in the users home directory named `.repo-set.toml`

To use with a gerrit server:

```toml
[remote]
url = "my-gerrit:29418"
list.cmd = [ "ssh", "ssh://my-gerrit:29418", "gerrit" ,"ls-projects" ]

[repo]
# Optional list of repos to ignore
ignore =["non-important-repo"]

[clone]
# Optional command run in each repo after it is cloned.
# ${repo} expands to the repository name, ${dest} to the repo's absolute path.
post.cmd = [ "cp", "myhook", "${dest}/.git/hooks/myhook" ]

[push.options]
# Optional git push options
l="Code-Review+2"
```
