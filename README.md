# Git Worktree Manager

```
Usage: git-wt [BRANCH] [COMMAND]

Commands:
  init    Print shell integration script (e.g. git-wt init fish | source)
  clone   Clone a repository with bare worktree structure
  fetch   Fetch from origin with prune
  add     Add a new worktree
  rm      Remove a worktree
  switch  Switch to a worktree by branch name
  pull    Pull changes in a worktree
  help    Print this message or the help of the given subcommand(s)

Arguments:
  [BRANCH]  Branch name to switch to (when no subcommand is provided)

Options:
  -h, --help  Print help
```

## Installation

### Using Cargo (Linux/macOS/Windows)

```sh
cargo install git-wt
```

Then [set up your shell](#shell-setup) (optional).

### Build from source

```sh
git clone https://github.com/grievouz/git-wt.git
cd git-wt
cargo build --release
```

Place the binary from `target/release/git-wt` on your `PATH`, then [set up your shell](#shell-setup) (optional).

### Using a release binary

- Download the [latest release](https://github.com/grievouz/git-wt/releases) for your system
- Put the binary on your `PATH`
- [Set up your shell](#shell-setup) (optional)

## Shell setup

To use `git wt` instead of `git-wt`, run the appropriate command for your shell and add the output to your config. Without it, switching branches won't work.

### Bash

Add to your `.bashrc`:

```sh
eval "$(git-wt init bash)"
```

### Zsh

Add to your `.zshrc`:

```sh
eval "$(git-wt init zsh)"
```

### Fish

Create `~/.config/fish/conf.d/git-wt.fish` (or add to an existing config) and run:

```sh
git-wt init fish | source
```

Or add this line to the file:

```sh
git-wt init fish | source
```

Without shell setup you can still use `git-wt` directly; you just won’t have the `git wt` alias or auto-cd after clone.

## Contributing

Contributions to `git-wt` are welcome! Please open an issue or submit a pull request on the GitHub repository.

## License

`git-wt` is released under the [Unlicense](LICENSE). This means the code is in the public domain, and you can use, modify, and distribute it without any restrictions. For more information, please see the [Unlicense website](https://unlicense.org/).
