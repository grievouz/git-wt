#![warn(clippy::all, clippy::pedantic)]

use clap::{CommandFactory, Parser, Subcommand, ValueEnum};
use color_eyre::eyre::ContextCompat;
use color_eyre::{Result, eyre::Context};
use crossterm::ExecutableCommand;
use crossterm::style::{Color, Print, ResetColor, SetBackgroundColor, SetForegroundColor};
use inquire::ui::{Attributes, Color as InquireColor, RenderConfig, StyleSheet, Styled};
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
const FISH_INTEGRATION: &str = include_str!("../shell/fish.fish");

#[derive(Parser)]
#[command(name = "git-wt")]
#[command(about = None, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Branch name to switch to (when no subcommand is provided)
    branch: Option<String>,
}

#[derive(Subcommand)]
enum Commands {
    /// Print shell integration script (e.g. git-wt init fish | source)
    Init {
        /// Shell: fish, bash, or zsh
        #[arg(value_enum)]
        shell: Option<Shell>,
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
}

fn main() -> Result<()> {
    color_eyre::install()?;
    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Init { shell }) => init_shell_integration(shell)?,
        Some(Commands::Clone { url, name }) => clone_bare_for_worktrees(&url, name.as_deref())?,
        Some(Commands::Fetch) => fetch_with_prune()?,
        Some(Commands::Add { branch, from }) => add_worktree(&branch, from.as_deref())?,
        Some(Commands::Rm { branch, force }) => remove_worktree(branch.as_deref(), force)?,
        Some(Commands::Switch { branch }) => switch_to_worktree(&branch)?,
        Some(Commands::Pull { branch }) => pull_worktree(branch.as_deref())?,
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
    }

    Ok(())
}

fn init_shell_integration(shell: Option<Shell>) -> Result<()> {
    let Some(shell) = shell else {
        eprintln!("Usage: git-wt init <shell> | source");
        eprintln!("  Shell: fish, bash, zsh");
        eprintln!("  Example: git-wt init fish | source");
        process::exit(1);
    };

    let script = match shell {
        Shell::Fish => FISH_INTEGRATION,
        Shell::Bash | Shell::Zsh => BASH_INTEGRATION,
    };

    io::stdout().write_all(script.as_bytes())?;
    Ok(())
}

fn log_info(message: &str) {
    eprintln!("{message}");
}

fn log_error(message: &str) {
    let mut stdout = io::stderr();
    let _ = stdout
        .execute(SetBackgroundColor(Color::Red))
        .and_then(|s| s.execute(SetForegroundColor(Color::Black)))
        .and_then(|s| s.execute(Print(" ERROR ")))
        .and_then(|s| s.execute(ResetColor))
        .and_then(|s| s.execute(Print(format!(" {message}\n"))));
}

fn create_select_render_config() -> RenderConfig<'static> {
    RenderConfig {
        prompt_prefix: Styled::new("Select:"),
        highlighted_option_prefix: Styled::new(">"),
        answered_prompt_prefix: Styled::new("Select:"),
        prompt: StyleSheet::new(),
        help_message: StyleSheet::new(),
        answer: StyleSheet::new().with_attr(Attributes::BOLD),
        option: StyleSheet::new().with_fg(InquireColor::DarkGrey),
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
        .with_context(|| format!("Failed to execute command: {cmd}"))?;

    if !status.success() {
        log_error("Command failed");
        process::exit(1);
    }

    Ok(())
}

fn clone_bare_for_worktrees(url: &str, name: Option<&str>) -> Result<()> {
    let basename = url.rsplit('/').next().context("Invalid URL")?;
    let default_name = basename.trim_end_matches(".git");
    let dir_name = name.unwrap_or(default_name);

    if let Err(e) = fs::create_dir(dir_name) {
        log_error(&format!("Failed to create directory '{dir_name}': {e}"));
        process::exit(1);
    }

    let dir_path = PathBuf::from(dir_name);

    log_info(&format!("Cloning {url} into {dir_name}/"));

    run_command("git", &["clone", "--bare", url, ".bare"], Some(&dir_path))?;

    // Create .git file pointing to .bare
    fs::write(dir_path.join(".git"), "gitdir: ./.bare\n").context("Failed to create .git file")?;

    // Configure remote origin fetch
    run_command(
        "git",
        &[
            "config",
            "remote.origin.fetch",
            "+refs/heads/*:refs/remotes/origin/*",
        ],
        Some(&dir_path),
    )?;

    // Fetch all branches
    log_info("Fetching branches...");
    run_command("git", &["fetch", "origin"], Some(&dir_path))?;

    log_info("Repository cloned successfully.");

    //let abs_path = std::env::current_dir()?.join(dir_name);
    //println!("CD:{}", abs_path.display());

    Ok(())
}

fn fetch_with_prune() -> Result<()> {
    log_info("Fetching from origin with prune...");
    run_command("git", &["fetch", "origin", "--prune"], None)?;
    log_info("Fetch completed.");
    Ok(())
}

fn check_git_repo() -> Result<()> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .context("Failed to execute git rev-parse")?;

    if !output.status.success() {
        log_error("Not in a git repository");
        process::exit(1);
    }

    Ok(())
}

fn get_worktree_root() -> Result<PathBuf> {
    // Get the git common dir (where .bare is)
    let output = Command::new("git")
        .args(["rev-parse", "--git-common-dir"])
        .output()
        .context("Failed to execute git rev-parse")?;

    if !output.status.success() {
        log_error("Not in a git repository");
        process::exit(1);
    }

    let git_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let git_path = PathBuf::from(git_dir);

    // Get the parent directory (where worktrees are siblings)
    let root = git_path
        .parent()
        .context("Could not determine worktree root")?
        .to_path_buf();

    Ok(root)
}

fn add_worktree(branch: &str, from: Option<&str>) -> Result<()> {
    check_git_repo()?;
    let root = get_worktree_root()?;
    let worktree_path = root.join(branch);

    // Check if worktree already exists
    if worktree_path.exists() {
        log_error(&format!(
            "Directory '{}' already exists",
            worktree_path.display()
        ));
        process::exit(1);
    }

    // Check if branch exists locally
    let branch_exists = Command::new("git")
        .args(["rev-parse", "--verify", &format!("refs/heads/{branch}")])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    let default_ref = format!("origin/{branch}");
    let base_ref = from.unwrap_or(&default_ref);

    // Check if the base ref exists
    let base_ref_exists = Command::new("git")
        .args(["rev-parse", "--verify", base_ref])
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    log_info(&format!("Creating worktree '{branch}'..."));

    if branch_exists {
        run_command(
            "git",
            &["worktree", "add", worktree_path.to_str().unwrap(), branch],
            None,
        )?;
    } else if base_ref_exists {
        // Branch doesn't exist but base ref does, create from it
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
        log_info(&format!(
            "Note: {base_ref} doesn't exist, creating from HEAD"
        ));
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

    log_info("Worktree created.");

    //println!("CD:{}", worktree_path.display());

    Ok(())
}

fn get_all_worktrees() -> Result<Vec<(String, String)>> {
    let output = Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .output()
        .context("Failed to execute git worktree list")?;

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

    eprintln!("'{branch}' matches multiple worktrees.");
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
        eprintln!("Cancelled.");
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
    check_git_repo()?;

    let branch = match branch {
        Some(b) => b.to_string(),
        None => {
            if let Some(b) = get_current_worktree_branch()? {
                b
            } else {
                log_error("Could not determine current worktree branch");
                process::exit(1);
            }
        }
    };

    let worktree_path = find_worktree_path(&branch)?;

    if worktree_path.is_none() {
        log_error(&format!("Worktree for branch '{branch}' not found"));
        process::exit(1);
    }

    let confirmed = Confirm::new("")
        .with_default(false)
        .with_render_config(create_confirm_render_config(
            "Are you sure you want to remove the worktree?",
        ))
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

    log_info(format!("Worktree '{}' removed.", &branch).as_str());

    Ok(())
}

fn switch_to_worktree(branch: &str) -> Result<()> {
    check_git_repo()?;
    let worktree_path = find_worktree_path(branch)?;

    if let Some(path) = worktree_path {
        println!("CD:{path}");
        Ok(())
    } else {
        log_error(&format!("Worktree for branch '{branch}' not found."));
        process::exit(1);
    }
}

fn pull_worktree(branch: Option<&str>) -> Result<()> {
    check_git_repo()?;

    let branch = match branch {
        Some(b) => b.to_string(),
        None => {
            if let Some(b) = get_current_worktree_branch()? {
                b
            } else {
                log_error("Could not determine current worktree branch");
                process::exit(1);
            }
        }
    };

    let worktree_path = find_worktree_path(&branch)?;

    if worktree_path.is_none() {
        log_error(&format!("Worktree for branch '{branch}' not found"));
        process::exit(1);
    }

    let worktree_path = worktree_path.unwrap();
    let worktree_path_buf = PathBuf::from(&worktree_path);

    log_info(&format!("Pulling changes in worktree '{branch}'..."));
    run_command("git", &["pull"], Some(&worktree_path_buf))?;
    log_info("Pull completed.");

    Ok(())
}
