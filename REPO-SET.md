# repo-set

Handles a set of git repos. A typical git command such as `reset` resets all repos in the set. The location of the local set is current working directory or specified with the `--root` option.

Implemented sub commands:

## init

Create an initial `.repo-set/config.toml` in the root directory (the current
directory by default). Missing directories are created; existing config files
are never overwritten. This command does not require an existing configuration.

```bash
repo-set init
repo-set --root path/to/repos init
```

Edit the generated example remote URL and repository-listing command before use.

## clone

Clone all remote repositories, excluding ignored repositories, to the root directory.

## Debug logging

Use `--debug` to append debug output to `debug.log` in the root directory. Repeat
the flag to include more detail, such as the expanded post-clone command.

## list

List remote, local or difference between remote and local.

## fetch

Fetch all local repos

## pull

Pull all local repos

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

# Install

```bash
cargo install --git https://github.com/attiand/repo-set.git
```

# Configuration file

`repo-set` reads the configuration file `.repo-set/config.toml` in the root
directory specified by `--root` (the current directory by default). No file in
the user's home directory is read.

|Key                  |Mandatory |Description                                    |
|---------------------|----------|-----------------------------------------------|
|remote.url           |yes       |The git remote URL                             |
|remote.list.cmd      |yes       |A command to run to get a list of remote repos |
|repositories.ignore  |no        |An array of repos to ignore                    |
|clone.post.cmd       |no        |A command to run after each clone              |
|push.options         |no        |A table of git push options                    |

Both binaries can access shared settings through the library's `config::Config`.
`RepoSetConfig` lives in `src/bin/repo-set/config.rs`, private to the `repo-set`
binary, and adds `push.options`. The binaries use their respective config file
locations. The shared configuration ignores the `push` section; only `repo-set`
exposes and validates its contents. Unknown shared fields are still rejected.


## Variables

There are limited variable expansion support:

|Name  |Supported in key            |Description                      |
|------|----------------------------|---------------------------------|
|user  |remote.url, remote.list.cmd |The OS user name                 |
|repo  |clone.post.cmd              |The repo name                    |
|dest  |clone.post.cmd              |The abs path to the local repo   |
|home  |clone.post.cmd              |The current user's home directory|
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
ignore = ["repo1", "repo2"]

[clone]
# Add commit hook
post.cmd = [ "cp", "${home}/.hooks/commit-msg", "${dest}/.git/hooks/commit-msg" ]

[push.options]
# Always vote +2
l = "Code-Review+2"
```
