// help.rs - Shell help and documentation

use std::io::{self, Write};
use crate::ui::history::history_path;
use crate::config::ShellConfig;

// ─────────────────────────────────────────────────────────────────────────────
// ANSI helpers
// ─────────────────────────────────────────────────────────────────────────────

const RST: &str = "\x1b[0m";
const BOLD: &str = "\x1b[1m";
const DIM: &str = "\x1b[2m";
const CYAN: &str = "\x1b[1;36m";
const GREEN: &str = "\x1b[1;32m";
const YELLOW: &str = "\x1b[1;33m";
const BLUE: &str = "\x1b[1;34m";
const WHITE: &str = "\x1b[1;37m";

fn header(title: &str) {
    println!();
    println!("{}{}  {}  {}{}", BOLD, BLUE, title, RST, DIM);
    println!("{}  {}{}", DIM, "─".repeat(52), RST);
}

fn row(key: &str, desc: &str) {
    println!("    {}{:<22}{} {}{}{}", CYAN, key, RST, DIM, desc, RST);
}

fn row2(key: &str, arg: &str, desc: &str) {
    let cell = format!("{} {}{}{}", key, YELLOW, arg, CYAN);
    println!("    {}{:<33}{} {}{}{}", CYAN, cell, RST, DIM, desc, RST);
}

// ─────────────────────────────────────────────────────────────────────────────
// Topics
// ─────────────────────────────────────────────────────────────────────────────

fn print_banner() {
    println!();
    println!("{}{}    ██████╗██████╗ ██╗   ██╗███████╗██╗  ██╗{}", BOLD, CYAN, RST);
    println!("{}{}   ██╔════╝██╔══██╗██║   ██║██╔════╝██║  ██║{}", BOLD, CYAN, RST);
    println!("{}{}   ██║     ██████╔╝██║   ██║███████╗███████║{}", BOLD, CYAN, RST);
    println!("{}{}   ██║     ██╔══██╗██║   ██║╚════██║██╔══██║{}", BOLD, CYAN, RST);
    println!("{}{}   ╚██████╗██║  ██║╚██████╔╝███████║██║  ██║{}", BOLD, CYAN, RST);
    println!("{}{}    ╚═════╝╚═╝  ╚═╝ ╚═════╝ ╚══════╝╚═╝  ╚═╝{}", BOLD, CYAN, RST);
    println!();
    println!("    {}A minimal shell written in Rust{}", DIM, RST);
    println!();
    println!("    {}Usage:{} {}help <topic>{}", WHITE, RST, GREEN, RST);
    println!("    {}Topics:{} builtins  bindings  config{}", WHITE, RST, DIM);
}

fn print_config() {
    header("CONFIG & PATHS");

    let history_file = history_path();
    let config_dir = history_file
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| "~/.config/crush".to_string());

    row("Config directory", &config_dir);
    row("History file",     &history_file.to_string_lossy());
    row("Config file",      &format!("{}/config.toml", config_dir));
    row("History size",     "10000 entries (configurable)");
    row("History dedup",    "enabled — duplicates & space-prefixed skipped");
}

fn print_builtins() {
    header("BUILTIN COMMANDS");

    row2("echo",    "[-n|-e|-E] <text>",  "Print text to stdout");
    row2("cd",      "[dir|~|-|..]",       "Change working directory");
    row2("pwd",     "",                   "Print current directory");
    row2("type",    "<cmd>",              "Show if cmd is builtin or external");
    row2("exit",    "[code]",             "Exit the shell");
    row2("history", "[-c|-r|-w|-n]",      "Show or manage command history");
    row2("clear",   "",                   "Clear the terminal screen");
    row2("export",  "NAME[=value]",       "Set environment variable");
    row2("unset",   "NAME",               "Unset environment variable");
    row2("source",  "<file.sh>",          "Execute script in current shell");
    row2("jobs",    "[%N ...]",           "List background jobs (optional: specific job IDs)");
    row2("ls",      "[-lahrtd1] [path]",  "List directory with icons (Nerd Font, eza-style)");
    row2("help",    "[topic]",            "Show this help (topics: builtins bindings config)");

    println!();
    println!("    {}Redirection:{}", WHITE, RST);
    row("cmd > file",   "Redirect stdout to file (overwrite)");
    row("cmd >> file",  "Redirect stdout to file (append)");
    row("cmd 2> file",  "Redirect stderr to file");
    row("cmd 2>> file", "Redirect stderr to file (append)");
    row("cmd 1> file",  "Explicit stdout redirect");

    println!();
    println!("    {}Background jobs:{}", WHITE, RST);
    row("cmd &",          "Run command in background");
    row("jobs",           "List all background jobs");
    row("jobs %1 %2",     "Show specific background jobs");
    println!("    {}  Jobs are reaped automatically before each prompt.{}", DIM, RST);

    println!();
    println!("    {}Pipelines:{}", WHITE, RST);
    row("cmd1 | cmd2",         "Pipe stdout of cmd1 to stdin of cmd2");
    row("echo hi | wc -c",    "Pipeline with builtin");
    row("cmd1 | cmd2 | cmd3", "Multi-command pipeline");

    println!();
    println!("    {}Conditional execution:{}", WHITE, RST);
    row("cmd1 && cmd2",  "Run cmd2 only if cmd1 succeeds (exit 0)");
    row("cmd1 || cmd2",  "Run cmd2 only if cmd1 fails (exit ≠ 0)");
}

fn print_bindings() {
    header("KEYBOARD BINDINGS");

    println!("    {}Completion:{}", WHITE, RST);
    row("Tab",           "Complete / cycle (Circular mode)");
    row("Tab  (×2)",     "Show full candidate list + cycle");
    row("Ctrl+Space",    "Force show all candidates as list");
    row("Alt+l",         "Force show all candidates as list");
    row("Shift+Tab",     "Accept ghost text suggestion");
    row("→  /  End",     "Accept ghost text (history or top completion)");

    println!();
    println!("    {}Navigation:{}", WHITE, RST);
    row("↑ / Ctrl+P",   "Previous command in history");
    row("↓ / Ctrl+N",   "Next command in history");
    row("← / →",        "Move cursor left / right");
    row("Ctrl+A",        "Move cursor to beginning of line");
    row("Ctrl+E",        "Move cursor to end of line");
    row("Alt+B",         "Move cursor one word back");
    row("Alt+F",         "Move cursor one word forward");

    println!();
    println!("    {}Editing:{}", WHITE, RST);
    row("Ctrl+W",        "Delete word before cursor");
    row("Ctrl+U",        "Delete from cursor to line start");
    row("Ctrl+K",        "Delete from cursor to line end");
    row("Ctrl+Y",        "Paste last deleted text (yank)");
    row("Ctrl+L",        "Clear screen (same as 'clear')");
    row("Alt+D",         "Delete word after cursor");

    println!();
    println!("    {}Shell control:{}", WHITE, RST);
    row("Ctrl+C",        "Cancel current line / interrupt");
    row("Ctrl+D",        "Exit shell (EOF)");

    println!();
    println!("    {}Ghost text suggestion:{}", WHITE, RST);
    println!("    {}  Appears automatically while typing — dim gray to the right of cursor.{}", DIM, RST);
    println!("    {}  Source: history (priority) → top completion candidate.{}", DIM, RST);
    println!("    {}  Use → or Shift+Tab to accept it.{}", DIM, RST);
}

fn print_functions(cfg: &ShellConfig) {
    header("WRAPPER FUNCTIONS  [functions.*]");

    if cfg.functions.is_empty() {
        println!("    {}No functions defined in config.toml{}", DIM, RST);
        println!();
        println!("    {}Add functions in your config.toml:{}", WHITE, RST);
        println!("    {}[functions.y]{}", DIM, RST);
        println!("    {}description = \"Yazi with cd-on-exit\"{}", DIM, RST);
        println!("    {}body = \"\"\"...body...\"\"\"{}", DIM, RST);
        return;
    }

    for name in {
        let mut names = cfg.functions.names();
        names.sort();
        names
    } {
        if let Some(func) = cfg.functions.get(name) {
            let desc = if func.description.is_empty() {
                "(no description)"
            } else {
                &func.description
            };
            row(name, desc);
            let preview: Vec<&str> = func.body.lines()
                .map(str::trim)
                .filter(|l| !l.is_empty() && !l.starts_with('#'))
                .take(3)
                .collect();
            for l in &preview {
                println!("      {}  {}{}\n{}", DIM, l, RST, "");
            }
            if func.body.lines().filter(|l| !l.trim().is_empty()).count() > 3 {
                println!("      {}  ...(truncated){}", DIM, RST);
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry point
// ─────────────────────────────────────────────────────────────────────────────

pub fn print_help(topic: Option<&str>, cfg: &ShellConfig) {
    let stdout = io::stdout();
    let _lock = stdout.lock();

    match topic {
        None => {
            print_banner();
            print_config();
            print_builtins();
            if !cfg.functions.is_empty() {
                print_functions(cfg);
            }
            print_bindings();
            println!();
        }
        Some("builtins") | Some("builtin") | Some("commands") => {
            print_builtins();
            println!();
        }
        Some("bindings") | Some("binding") | Some("keys") | Some("keybindings") => {
            print_bindings();
            println!();
        }
        Some("config") | Some("paths") | Some("path") => {
            print_config();
            println!();
        }
        Some("functions") | Some("function") | Some("fn") | Some("wrapper") | Some("wrappers") => {
            print_functions(cfg);
            println!();
        }
        Some(unknown) => {
            eprintln!(
                "help: unknown topic '{}'. Try: {}help builtins{}, {}help bindings{}, {}help functions{}, {}help config{}",
                unknown, GREEN, RST, GREEN, RST, GREEN, RST, GREEN, RST
            );
        }
    }

    io::stdout().flush().ok();
}
