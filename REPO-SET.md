# repo-set

Handles a set of git repos. A typical git command such as `reset` resets all repos in the set. The location of the local set is current working directory or specified with the `--root` option.

Sub commands:

## init

Create an initial `.repo-set/config.toml` in the root directory (the current
directory by default).

Edit the generated example remote URL and repository-listing command before use.

## clone

Clone all remote repositories. Ignoring any existing local repositories.

## list

List remote, local or difference between remote and local.

## fetch

Fetch all local repos.

## pull

Pull all local repos.

## reset

Reset all local repos, only `--hard` mode implemented for now.

## status

Print git status for repos with dirty workspaces.

## clean

List or remove untracked files from the working tree.

## stage/add

Stage local repos that do not have a clean workspace.

## commit

Commit local repositories with staged changes.

## push

Push local repos that is ahead of its tracking branch.

### Example

Push options:

```bash
repo-set push --push-option='m=#AS' HEAD:refs/for/master
```

## help

Print help information.

# Configuration file

`repo-set` reads the configuration file `.repo-set/config.toml` in the root
directory specified by `--root` (the current directory by default).

|Key                  |Mandatory |Description                                           |
|---------------------|----------|------------------------------------------------------|
|remote.url           |yes       |The git remote URL                                    |
|remote.list.cmd      |yes       |A command to run to get a list of remote repos        |
|repositories.include |no        |Glob patterns selecting repositories; defaults to all |
|repositories.exclude |no        |Glob patterns excluding repositories; defaults to none|
|clone.post.cmd       |no        |A command to run after each clone                     |
|push.options         |no        |A table of git push options                           |


## Variables

There are limited variable expansion support:

|Name  |Supported in key            |Description                      |
|------|----------------------------|---------------------------------|
|user  |remote.url, remote.list.cmd |The OS user name                 |
|repo  |clone.post.cmd              |The repo name                    |
|dest  |clone.post.cmd              |The abs path to the local repo   |
|home  |clone.post.cmd              |The current user's home directory|
|root  |clone.post.cmd              |The absolute path to the root directory|


## Minimal example

Minimal config to use with a Gerrit server:

```toml
[remote]
url = "ssh://${user}@my-gerrit:29418"
list.cmd = [ "ssh", "ssh://${user}@my-gerrit:29418", "gerrit" ,"ls-projects" ]
```

## Full example

```toml
[remote]
url = "ssh://${user}@my-gerrit:29418"
list.cmd = [ "ssh", "ssh://${user}@my-gerrit:29418", "gerrit" ,"ls-projects" ]

[repositories]
include = ["team-*", "tools-?"]
exclude = ["*-archive"]

[clone]
# Add commit hook
post.cmd = [ "cp", "${root}/.repo-set/commit-msg", "${dest}/.git/hooks" ]

[push.options]
# Always vote +2
l = "Code-Review+2"

# Auto submit
#m=#AS
```
