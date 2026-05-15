# git-wt

```
usage: git-wt [BRANCH] [COMMAND]

commands:
  init    Print shell integration script (e.g. git-wt init fish | source)
  clone   Clone a repository with bare worktree structure
  fetch   Fetch from origin with prune
  add     Add a new worktree
  rm      Remove a worktree
  switch  Switch to a worktree by branch name
  pull    Pull changes in a worktree
  prune   Delete local branches whose remote tracking branch is gone
  list    List all worktrees

Options:
  -v, --version  Print version
  -h, --help     Print help

arguments:
  [BRANCH]  Branch name to switch to (when no subcommand is provided)
```

## Installation

### Homebrew (macOS and Linux)

```sh
brew install grievouz/tap/git-wt
```

### Cargo

```sh
cargo install --git https://github.com/grievouz/git-wt
```

Then follow the [Shell setup](#shell-setup) section below.

### Prebuilt binaries

Download a release tarball from the [Releases page](https://github.com/grievouz/git-wt/releases), place `git-wt` on your `PATH`, and follow the [Shell setup](#shell-setup) section below.

## Shell setup

> If you installed via Homebrew, the formula handles it. These instructions are for `cargo install` / manual installs.

Set up shell integration to enable the `git wt` subcommand seamless directory switching. Pass `--alias` to also define a standalone `wt` function; drop it if you only want `git wt`.

### Bash

Add to your `.bashrc`:

```sh
eval "$(git-wt init bash --alias)"
```

### Zsh

Add to your `.zshrc`:

```sh
eval "$(git-wt init zsh --alias)"
```

### Fish

Add to your `~/.config/fish/config.fish`:

```sh
git-wt init fish --alias | source
```

## Contributing

Contributions to `git-wt` are welcome! Please open an issue or submit a pull request on the GitHub repository.

## License

`git-wt` is released under the [Unlicense](LICENSE). This means the code is in the public domain, and you can use, modify, and distribute it without any restrictions. For more information, please see the [Unlicense website](https://unlicense.org/).
