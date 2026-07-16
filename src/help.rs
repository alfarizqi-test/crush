// help.rs — Menampilkan informasi lengkap shell crush
//
// Dipanggil via builtin: `help` atau `help <topic>`

use std::io::{self, Write};
use crate::history::history_path;

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
// Topik
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
    row("History size",     "1000 entries (max)");
    row("History dedup",    "enabled — duplicates & space-prefixed skipped");
    row("Config file",      &format!("{}/config.toml  (planned)", config_dir));
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
    row2("help",    "[topic]",            "Show this help (topics: builtins bindings config)");

    println!();
    println!("    {}Redirection:{}", WHITE, RST);
    row("cmd > file",   "Redirect stdout to file (overwrite)");
    row("cmd >> file",  "Redirect stdout to file (append)");
    row("cmd 2> file",  "Redirect stderr to file");
    row("cmd 2>> file", "Redirect stderr to file (append)");
    row("cmd 1> file",  "Explicit stdout redirect");
}

fn print_bindings() {
    header("KEYBOARD BINDINGS");

    println!("    {}Completion:{}", WHITE, RST);
    row("Tab",          "Complete / cycle through candidates (Circular)");
    row("Tab  (×2)",    "Show full candidate list + cycle");
    row("Shift+Tab",    "Accept ghost text suggestion");
    row("Alt+l",        "Force show all candidates as list");
    row("→  /  End",    "Accept ghost text (history or top completion)");

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
    println!("    {}  Muncul otomatis saat mengetik — abu-abu redup di kanan kursor.{}", DIM, RST);
    println!("    {}  Sumber: history (prioritas) → top completion candidate.{}", DIM, RST);
    println!("    {}  Gunakan → atau Shift+Tab untuk menerimanya.{}", DIM, RST);
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry point
// ─────────────────────────────────────────────────────────────────────────────

pub fn print_help(topic: Option<&str>) {
    let stdout = io::stdout();
    let _lock = stdout.lock(); // buffer output agar tidak terpotong

    match topic {
        None => {
            print_banner();
            print_config();
            print_builtins();
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
        Some(unknown) => {
            eprintln!(
                "help: topic '{}' tidak dikenal. Coba: {}help builtins{}, {}help bindings{}, {}help config{}",
                unknown, GREEN, RST, GREEN, RST, GREEN, RST
            );
        }
    }

    io::stdout().flush().ok();
}
