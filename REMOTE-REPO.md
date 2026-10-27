# remote-repo

List or clones one or several Git repos.

Sub commands:

## list

List all remote repos.

## clone

Clone one or several remote repositories to root. The repo list allows globs.

## help

Print help information.

# Configuration file

`remote-repo` reads a configuration file `.remote-repo/config.toml` in the user's
home directory.

|Key                  |Mandatory |Description                                           |
|---------------------|----------|------------------------------------------------------|
|remote.url           |yes       |The git remote URL                                    |
|remote.list.cmd      |yes       |A command to run to get a list of remote repos        |
|repositories.include |no        |Glob patterns selecting repositories; defaults to all |
|repositories.exclude |no        |Glob patterns excluding repositories; defaults to none|
|clone.post.cmd       |no        |A command to run after each clone                     |

## Variables

There are limited variable expansion support:

|Name  |Supported in key            |Description                            |
|------|----------------------------|---------------------------------------|
|user  |remote.url, remote.list.cmd |The OS user name                       |
|repo  |clone.post.cmd              |The repo name                          |
|dest  |clone.post.cmd              |The abs path to the local repo         |
|home  |clone.post.cmd              |The current user's home directory      |
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
post.cmd = [ "cp", "${home}/.remote-repo/commit-msg", "${dest}/.git/hooks/commit-msg" ]
```
