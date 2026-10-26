# repo-clone

Clones one or several Git repos.

## Usage

```bash
repo-clone repo1 repo2
repo-clone --all
```

Specify one or more repository names, or use `--all` to select all remote
repositories allowed by configuration.

The include and exclude patterns also apply to explicitly named repositories.
Existing destination paths are skipped. After each successful clone,
`clone.post.cmd` runs if configured. Listing, cloning, or post-clone failures
exit with an error.

# Configuration file

`repo-clone` reads a configuration file `.repo-clone/config.toml` in the user's
home directory.

|Key                  |Mandatory |Description                                    |
|---------------------|----------|-----------------------------------------------|
|remote.url           |yes       |The git remote URL                             |
|remote.list.cmd      |yes       |A command to run to get a list of remote repos |
|repositories.include |no        |Repos to include; defaults to all              |
|repositories.exclude |no        |Repos to exclude; defaults to none             |
|clone.post.cmd       |no        |A command to run after each clone              |

## Variables

There are limited variable expansion support:

|Name  |Supported in key            |Description                            |
|------|----------------------------|---------------------------------------|
|user  |remote.url, remote.list.cmd |The OS user name                       |
|repo  |clone.post.cmd              |The repo name                          |
|dest  |clone.post.cmd              |The abs path to the local repo         |
|home  |clone.post.cmd              |The current user's home directory      |
|root  |clone.post.cmd              |The absolute path to the root directory|


## Example

Minimal config to use with a Gerrit server:

```toml
[remote]
url = "ssh://${user}@my-gerrit:29418"
list.cmd = [ "ssh", "ssh://${user}@my-gerrit:29418", "gerrit" ,"ls-projects" ]
```

## Example

Full configuration example:

```toml
[remote]
url = "ssh://${user}@my-gerrit:29418"
list.cmd = [ "ssh", "ssh://${user}@my-gerrit:29418", "gerrit" ,"ls-projects" ]

[repositories]
include = ["team-*", "tools-?"]
exclude = ["*-archive"]

[clone]
# Add commit hook
post.cmd = [ "cp", "${home}/.repo-clone/commit-msg", "${dest}/.git/hooks/commit-msg" ]
```
