# repo-set

Handles a set of git repos. A typical git command such as status prints the status of all repos in the set. The location of the local set is current working directory or specified with the `--root` option.

Implemented sub commands:

## clone

Clone specified or all remote repos to the root directory.

## list

List remote, local or difference between remote and local.

## sync

Synchronize remote repos to the root directory. That is clone missing, pull existing, and remove superflous local repos.

## help

Print help information.

# Configuration file

repo-set reads a configuration file in the users home directory named `.repo-set.toml`

To use with a gerrit server 

```toml
remote.url = "my-gerrit:29418"

repo.list.cmd = [ "ssh", "ssh://my-gerrit:29418", "gerrit" ,"ls-projects" ]
```