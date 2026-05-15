#![warn(clippy::all, clippy::pedantic)]

use anyhow::{Context, Result};
use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use inquire::ui::{
    Attributes, Color as InquireColor, ErrorMessageRenderConfig, RenderConfig, StyleSheet, Styled,
};
use inquire::{Confirm, Select};
use nucleo_matcher::pattern::{Atom, AtomKind, CaseMatching, Normalization};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command};

#[derive(Clone, ValueEnum)]
enum Shell {
    Fish,
    Bash,
    Zsh,
}

const BASH_INTEGRATION: &str = include_str!("../shell/bash.sh");
const BASH_WT_ALIAS: &str = include_str!("../shell/bash_wt.sh");
const FISH_INTEGRATION: &str = include_str!("../shell/fish.fish");
const FISH_WT_ALIAS: &str = include_str!("../shell/fish_wt.fish");

#[derive(Parser)]
#[command(name = "git-wt")]
#[command(about = None, long_about = None)]
#[command(
    help_template = "usage: {usage}\n\n{all-args}{after-help}",
    subcommand_help_heading = "commands",
    disable_help_subcommand = true
)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Branch name to switch to (when no subcommand is provided)
    #[arg(help_heading = "arguments")]
    branch: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Print shell integration script (e.g. git-wt init fish | source)
    Init {
        /// Shell: fish, bash, or zsh
        #[arg(value_enum)]
        shell: Option<Shell>,
        /// Also register a standalone 'wt' command for fast access
        #[arg(long)]
        alias: bool,
    },
    /// Clone a repository with bare worktree structure
    Clone {
        /// Repository URL
        url: String,
        /// Optional directory name (defaults to repo name)
        name: Option<String>,
    },
    /// Fetch from origin with prune
    Fetch,
    /// Add a new worktree
    Add {
        /// Branch name for the new worktree
        branch: String,
        /// Create branch from this ref (defaults to origin/branch)
        #[arg(short, long)]
        from: Option<String>,
    },
    /// Remove a worktree
    #[command(alias = "remove")]
    Rm {
        /// Branch name of the worktree to remove (defaults to current worktree)
        branch: Option<String>,
        /// Force removal even if worktree has uncommitted changes
        #[arg(short, long)]
        force: bool,
    },
    /// Switch to a worktree by branch name
    #[command(alias = "s")]
    Switch {
        /// Branch name to switch to
        branch: String,
    },
    /// Pull changes in a worktree
    Pull {
        /// Branch name of the worktree to pull (defaults to current worktree)
        branch: Option<String>,
    },
    /// Delete local branches whose remote tracking branch is gone
    Prune {
        /// Skip confirmation prompt
        #[arg(short = 'y', long)]
        yes: bool,
    },
    /// List all worktrees
    #[command(alias = "ls")]
    List,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init { shell, alias }) => init_shell_integration(shell, alias)?,
        Some(Commands::Clone { url, name }) => clone_bare_for_worktrees(&url, name.as_deref())?,
        Some(Commands::Fetch) => fetch_with_prune()?,
        Some(Commands::Add { branch, from }) => add_worktree(&branch, from.as_deref())?,
        Some(Commands::Rm { branch, force }) => remove_worktree(branch.as_deref(), force)?,
        Some(Commands::Switch { branch }) => switch_to_worktree(&branch)?,
        Some(Commands::Pull { branch }) => pull_worktree(branch.as_deref())?,
        Some(Commands::Prune { yes }) => prune_branches(yes)?,
        None => {
            // No subcommand provided, check for branch argument
            if let Some(branch) = cli.branch {
                switch_to_worktree(&branch)?;
            } else {
                // No subcommand and no branch, print help
                Cli::command().print_help()?;
                process::exit(1);
            }
        }
        Some(Commands::List) => list_worktrees()?,
    }

    Ok(())
}

fn init_shell_integration(shell: Option<Shell>, alias: bool) -> Result<()> {
    let Some(shell) = shell else {
        eprintln!("usage: git-wt init <shell>");
        eprintln!("  e.g. git-wt init fish | source");
        process::exit(1);
    };

    let (script, alias_script) = match shell {
        Shell::Fish => (FISH_INTEGRATION, FISH_WT_ALIAS),
        Shell::Bash | Shell::Zsh => (BASH_INTEGRATION, BASH_WT_ALIAS),
    };

    io::stdout().write_all(script.as_bytes())?;
    if alias {
        io::stdout().write_all(b"\n")?;
        io::stdout().write_all(alias_script.as_bytes())?;
    }
    Ok(())
}

fn fatal(message: &str) -> ! {
    eprintln!("fatal: {message}");
    process::exit(128);
}

fn create_select_render_config() -> RenderConfig<'static> {
    RenderConfig {
        prompt_prefix: Styled::new("select:"),
        highlighted_option_prefix: Styled::new(">"),
        answered_prompt_prefix: Styled::new("select:"),
        prompt: StyleSheet::new(),
        help_message: StyleSheet::new(),
        answer: StyleSheet::new().with_attr(Attributes::BOLD),
        option: StyleSheet::new(), //.with_fg(InquireColor::DarkGrey),
        selected_option: Some(
            StyleSheet::new()
                .with_fg(InquireColor::Black)
                .with_bg(InquireColor::White),
        ),
        ..Default::default()
    }
}

fn create_confirm_render_config(prompt: &str) -> RenderConfig<'_> {
    RenderConfig {
        prompt_prefix: Styled::new(prompt),
        answered_prompt_prefix: Styled::new(prompt),
        prompt: StyleSheet::new(),
        help_message: StyleSheet::new(),
        answer: StyleSheet::new().with_attr(Attributes::BOLD),
        canceled_prompt_indicator: Styled::new("canceled"),
        error_message: ErrorMessageRenderConfig::empty()
            .with_prefix(Styled::new("error: "))
            .with_message(StyleSheet::new()),
        ..Default::default()
    }
}

fn run_command(cmd: &str, args: &[&str], cwd: Option<&Path>) -> Result<()> {
    let mut command = Command::new(cmd);
    command.args(args);

    if let Some(dir) = cwd {
        command.current_dir(dir);
    }

    let status = command
        .status()
        .with_context(|| format!("failed to run '{cmd}'"))?;

    if !status.success() {
        process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}

fn clone_bare_for_worktrees(url: &str, name: Option<&str>) -> Result<()> {
    let basename = url.rsplit('/').next().context("invalid repository URL")?;
    let default_name = basename.trim_end_matches(".git");
    let dir_name = name.unwrap_or(default_name);

    if let Err(e) = fs::create_dir(dir_name) {
        fatal(&format!("could not create directory '{dir_name}': {e}"));
    }

    let dir_path = PathBuf::from(dir_name);

    run_command("git", &["clone", "--bare", url, ".bare"], Some(&dir_path))?;

    fs::write(dir_path.join(".git"), "gitdir: ./.bare\n").context("could not create .git file")?;

    run_command(
        "git",
        &[
            "config",
            "remote.origin.fetch",
            "+refs/heads/*:refs/remotes/origin/*",
        ],
        Some(&dir_path),
    )?;

    run_command("git", &["fetch", "origin"], Some(&dir_path))?;

    Ok(())
}

fn fetch_with_prune() -> Result<()> {
    run_command("git", &["fetch", "origin", "--prune"], None)
}

fn check_worktree_setup() -> Result<()> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .context("failed to run 'git rev-parse'")?;

    if !output.status.success() {
        fatal("not a git repository (or any of the parent directories)");
    }

    let common_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let common_path = PathBuf::from(&common_dir)
        .canonicalize()
        .context("could not resolve git common directory")?;

    let Some(root) = common_path.parent() else {
        fatal("could not determine worktree root");
    };

    if !root.join(".git").is_file() {
        eprintln!("fatal: not a worktree checkout");
        eprintln!("hint: use 'git-wt clone <url>' to create a worktree-based checkout");
        process::exit(128);
    }

    Ok(())
}

fn get_worktree_root() -> Result<PathBuf> {
    // Get the git common dir (where .bare is)
    let output = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .context("failed to run 'git rev-parse'")?;

    if !output.status.success() {
        fatal("not a worktree checkout");
    }

    let git_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let git_path = PathBuf::from(git_dir);

    // Get the parent directory (where worktrees are siblings)
    let root = git_path
        .parent()
        .context("could not determine worktree root")?
        .to_path_buf();

    Ok(root)
}

fn add_worktree(branch: &str, from: Option<&str>) -> Result<()> {
    check_worktree_setup()?;
    let root = get_worktree_root()?;
    let worktree_path = root.join(branch);

    if worktree_path.exists() {
        fatal(&format!("'{}' already exists", worktree_path.display()));
    }

    let branch_exists = Command::new("git")
        .args(["rev-parse", "--verify", &format!("refs/heads/{branch}")])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    let default_ref = format!("origin/{branch}");
    let base_ref = from.unwrap_or(&default_ref);

    let base_ref_exists = Command::new("git")
        .args(["rev-parse", "--verify", base_ref])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    if branch_exists {
        run_command(
            "git",
            &["worktree", "add", worktree_path.to_str().unwrap(), branch],
            None,
        )?;
    } else if base_ref_exists {
        run_command(
            "git",
            &[
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                "-b",
                branch,
                base_ref,
            ],
            None,
        )?;
    } else {
        eprintln!("hint: '{base_ref}' not found, using HEAD");
        run_command(
            "git",
            &[
                "worktree",
                "add",
                worktree_path.to_str().unwrap(),
                "-b",
                branch,
                "HEAD",
            ],
            None,
        )?;
    }

    Ok(())
}

fn get_all_worktrees() -> Result<Vec<(String, String)>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .output()
        .context("failed to run 'git worktree list'")?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let mut worktrees = Vec::new();
    let mut current_worktree_path: Option<String> = None;

    for line in output_str.lines() {
        if line.starts_with("worktree ") {
            current_worktree_path = Some(line.strip_prefix("worktree ").unwrap().to_string());
        } else if line.starts_with("branch ") {
            let branch_name = line
                .strip_prefix("branch ")
                .unwrap()
                .trim_start_matches("refs/heads/")
                .to_string();

            if let Some(path) = current_worktree_path.take() {
                worktrees.push((branch_name, path));
            }
        }
    }

    Ok(worktrees)
}

fn find_worktree_path(branch: &str) -> Result<Option<String>> {
    let worktrees = get_all_worktrees()?;

    if worktrees.is_empty() {
        return Ok(None);
    }

    // First try exact match
    for (wt_branch, wt_path) in &worktrees {
        if wt_branch == branch {
            return Ok(Some(wt_path.clone()));
        }
    }

    // If no exact match, try fuzzy matching
    let mut matcher = Matcher::new(Config::DEFAULT);
    let pattern = Atom::new(
        branch,
        CaseMatching::Smart,
        Normalization::Smart,
        AtomKind::Fuzzy,
        false,
    );

    let mut scored: Vec<(u16, String, String)> = Vec::new();

    for (wt_branch, wt_path) in worktrees {
        let haystack = Utf32Str::Ascii(wt_branch.as_bytes());
        if let Some(score) = pattern.score(haystack, &mut matcher) {
            scored.push((score, wt_branch, wt_path));
        }
    }

    if scored.is_empty() {
        return Ok(None);
    }

    scored.sort_by(|a, b| b.0.cmp(&a.0));

    if scored.len() == 1 {
        return Ok(Some(scored[0].2.clone()));
    }

    let options: Vec<(String, String)> = scored
        .iter()
        .map(|(_, name, path)| (name.clone(), path.clone()))
        .collect();
    let branch_names: Vec<String> = options.iter().map(|(name, _)| name.clone()).collect();

    eprintln!("hint: '{branch}' matched multiple worktrees");
    let selection = Select::new("", branch_names)
        .with_page_size(10)
        .with_render_config(create_select_render_config())
        .without_help_message()
        .prompt_skippable();

    if let Ok(Some(selected)) = selection {
        for (name, path) in options {
            if name == selected {
                return Ok(Some(path));
            }
        }
        Ok(None)
    } else {
        process::exit(0);
    }
}

fn get_current_worktree_branch() -> Result<Option<String>> {
    let current_dir = std::env::current_dir()?.canonicalize()?;

    let worktrees = get_all_worktrees()?;

    for (branch, path) in worktrees {
        if let Ok(worktree_path) = PathBuf::from(&path).canonicalize()
            && worktree_path == current_dir
        {
            return Ok(Some(branch));
        }
    }

    Ok(None)
}

fn remove_worktree(branch: Option<&str>, force: bool) -> Result<()> {
    check_worktree_setup()?;

    let branch = match branch {
        Some(b) => b.to_string(),
        None => {
            if let Some(b) = get_current_worktree_branch()? {
                b
            } else {
                fatal("unable to determine current worktree branch");
            }
        }
    };

    let worktree_path = find_worktree_path(&branch)?;

    if worktree_path.is_none() {
        fatal(&format!("worktree '{branch}' not found"));
    }

    let confirmed = Confirm::new("")
        .with_default(false)
        .with_render_config(create_confirm_render_config("remove worktree?"))
        .prompt_skippable();

    if !matches!(confirmed, Ok(Some(true))) {
        process::exit(0);
    }

    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    let worktree_path = worktree_path.unwrap();
    args.push(&worktree_path);

    run_command("git", &args, None)?;

    Ok(())
}

fn switch_to_worktree(branch: &str) -> Result<()> {
    check_worktree_setup()?;
    let worktree_path = find_worktree_path(branch)?;

    if let Some(path) = worktree_path {
        println!("CD:{path}");
        Ok(())
    } else {
        fatal(&format!("worktree '{branch}' not found"));
    }
}

fn pull_worktree(branch: Option<&str>) -> Result<()> {
    check_worktree_setup()?;

    let branch = match branch {
        Some(b) => b.to_string(),
        None => {
            if let Some(b) = get_current_worktree_branch()? {
                b
            } else {
                fatal("unable to determine current worktree branch");
            }
        }
    };

    let worktree_path = find_worktree_path(&branch)?;

    if worktree_path.is_none() {
        fatal(&format!("worktree '{branch}' not found"));
    }

    let worktree_path = worktree_path.unwrap();
    let worktree_path_buf = PathBuf::from(&worktree_path);

    run_command("git", &["pull"], Some(&worktree_path_buf))
}

fn get_gone_branches() -> Result<Vec<String>> {
    let output = Command::new("git")
        .args([
            "for-each-ref",
            "--format=%(refname:short) %(upstream:track)",
            "refs/heads",
        ])
        .output()
        .context("failed to list branches")?;

    if !output.status.success() {
        return Ok(Vec::new());
    }

    let output_str = String::from_utf8_lossy(&output.stdout);
    let gone_branches: Vec<String> = output_str
        .lines()
        .filter(|line| line.contains("[gone]"))
        .filter_map(|line| line.split_whitespace().next())
        .map(String::from)
        .collect();

    Ok(gone_branches)
}

fn prune_branches(skip_confirm: bool) -> Result<()> {
    check_worktree_setup()?;

    run_command("git", &["fetch", "origin", "--prune"], None)?;

    let gone_branches = get_gone_branches()?;

    if gone_branches.is_empty() {
        eprintln!("nothing to prune");
        return Ok(());
    }

    let worktrees = get_all_worktrees()?;
    let worktree_branches: Vec<&str> = worktrees.iter().map(|(b, _)| b.as_str()).collect();

    eprintln!("branches with gone upstream:");
    for branch in &gone_branches {
        let has_worktree = worktree_branches.contains(&branch.as_str());
        if has_worktree {
            eprintln!("  {branch} (has worktree)");
        } else {
            eprintln!("  {branch}");
        }
    }

    if !skip_confirm {
        let confirmed = Confirm::new("")
            .with_default(false)
            .with_render_config(create_confirm_render_config("Delete these branches?"))
            .prompt_skippable();

        if !matches!(confirmed, Ok(Some(true))) {
            process::exit(0);
        }
    }

    for branch in &gone_branches {
        if let Some((_, path)) = worktrees.iter().find(|(b, _)| b == branch) {
            run_command("git", &["worktree", "remove", "--force", path], None)?;
        }

        run_command("git", &["branch", "-D", branch], None)?;
    }

    Ok(())
}

fn list_worktrees() -> Result<()> {
    check_worktree_setup()?;

    let worktrees = get_all_worktrees()?;

    if worktrees.is_empty() {
        return Ok(());
    }

    let current_branch = get_current_worktree_branch()?;

    for (branch, _path) in &worktrees {
        let marker = if current_branch.as_deref() == Some(branch.as_str()) {
            "* "
        } else {
            "  "
        };
        println!("{marker}{branch}");
    }

    Ok(())
}
