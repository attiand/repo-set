# repo-clone

Clones one or several Git repos.

## Usage

```bash
repo-clone repo1 repo2
repo-clone --all
```

Specify one or more repository names, or use `--all` to select all remote
repositories allowed by `repositories.include` and `repositories.exclude`.
Repository names and `--all` cannot be combined. Use `--root DIR` to select the
destination root (the current directory by default).

The include and exclude patterns also apply to explicitly named repositories.
Existing destination paths are skipped. After each successful clone,
`clone.post.cmd` runs if configured. Listing, cloning, or post-clone failures
exit with an error.

## Shell completion

Generate a shell completion script with `repo-clone completion SHELL` (alias:
`comp`). Supported shells are `bash`, `zsh`, `fish`, `powershell`, and `elvish`.
Completion generation does not read the configuration file or clone repositories.
Repository names are completed dynamically by running `remote.list.cmd` from
`~/.repo-clone/config.toml` on each completion request. Suggestions match the
typed prefix and apply the configured include/exclude patterns. Missing or
invalid configuration and listing failures produce no repository suggestions.
To clone a repository named `completion` or `comp`, place `--` before its name.

```bash
source <(repo-clone completion bash)
```

For Zsh, use `source <(repo-clone completion zsh)` after `compinit`.
For Fish, use `repo-clone completion fish | source`.
Keep `repo-clone` on `PATH` so the shell can request suggestions. Regenerate or
re-source completion scripts after upgrading; older static scripts do not
complete repository names dynamically.

# Configuration file

`repo-clone` reads a configuration file `.repo-clone/config.toml` in the user's
home directory.

|Key                  |Mandatory |Description                                    |
|---------------------|----------|-----------------------------------------------|
|remote.url           |yes       |The git remote URL                             |
|remote.list.cmd      |yes       |A command to run to get a list of remote repos |
|repositories.include |no |Glob patterns selecting repositories; defaults to all      |
|repositories.exclude |no |Glob patterns excluding repositories; defaults to none    |
|clone.post.cmd       |no        |A command to run after each clone              |

`push.options` is supported only by `repo-set`; it is not a valid setting in
`repo-clone` configuration files. Unknown fields are rejected.
Patterns support `*` (any number of characters), `?` (one character), and
character classes such as `[abc]`. If `include` is non-empty, a repository must
match at least one include pattern; any match in `exclude` takes precedence.


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
