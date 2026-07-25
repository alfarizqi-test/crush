mod shell;
mod history;
mod completion;
mod help;
mod jobs;
mod executor;
mod ls;
mod config;
mod state;

use std::env;
use std::sync::{Arc, RwLock};

use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::KeyCode::*;
use rustyline::{Cmd, Editor, KeyEvent, Modifiers};

use completion::CrushCompleter;
use config::ShellConfig;
use executor::{parse_input, tokenize_operators, ExecContext};
use jobs::new_job_table;

// ─────────────────────────────────────────────────────────────────────────────
// Prompt (sementara — akan digantikan config::prompt renderer sesi berikutnya)
// ─────────────────────────────────────────────────────────────────────────────

fn build_prompt() -> String {
    if let Ok(cwd) = env::current_dir() {
        let home = env::var("HOME").unwrap_or_default();
        let path_str = cwd.to_string_lossy();
        let display = if !home.is_empty() && path_str.starts_with(home.as_str()) {
            format!("~{}", &path_str[home.len()..])
        } else {
            path_str.to_string()
        };
        format!("\x1b[1;34m[ {} ]\x1b[0m\n\x1b[1;32m❯\x1b[0m ", display)
    } else {
        "\x1b[1;32m❯\x1b[0m ".to_string()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

fn main() {
    shell::ShellInfo::init_environment();

    if let Some(arg) = std::env::args().nth(1) {
        if arg == "--check" {
            shell::ShellInfo::register_to_system();
            return;
        }
    }

    // ── Load config ───────────────────────────────────────────────────────────
    let cfg = ShellConfig::load();
    cfg.apply_env();

    // Bungkus dalam Arc<RwLock> agar bisa di-share ke CrushCompleter
    let cfg_arc: Arc<RwLock<ShellConfig>> = Arc::new(RwLock::new(cfg));

    // ── Editor rustyline ──────────────────────────────────────────────────────
    let rl_config = {
        let c = cfg_arc.read().unwrap();
        config::shell::build_rl_config(&c)
    };
    let helper   = CrushCompleter::new(Arc::clone(&cfg_arc));
    let mut rl: Editor<CrushCompleter, FileHistory> =
        Editor::with_history(rl_config, FileHistory::new())
            .expect("gagal membuat editor");
    rl.set_helper(Some(helper));

    // ── Keybindings ───────────────────────────────────────────────────────────
    // Hard-coded bindings (rustyline internal)
    rl.bind_sequence(KeyEvent(Char('d'), Modifiers::CTRL),   Cmd::Interrupt);
    rl.bind_sequence(KeyEvent(BackTab, Modifiers::NONE),     Cmd::CompleteHint);
    rl.bind_sequence(KeyEvent(Char(' '), Modifiers::CTRL),   Cmd::Complete);
    rl.bind_sequence(KeyEvent(Char('l'), Modifiers::ALT),    Cmd::Complete);
    rl.bind_sequence(KeyEvent(Char('p'), Modifiers::CTRL),   Cmd::PreviousHistory);
    rl.bind_sequence(KeyEvent(Char('n'), Modifiers::CTRL),   Cmd::NextHistory);

    // Config-driven bindings dari [bindings]
    // Action "clear_screen", "search_history" dipetakan ke rustyline Cmd
    {
        let c = cfg_arc.read().unwrap();
        apply_config_bindings(&mut rl, &c);
    }

    // ── History & Job table ───────────────────────────────────────────────────
    history::load_history(&mut rl);
    let job_table = new_job_table();

    // ── Greeting ──────────────────────────────────────────────────────────────
    {
        let c = cfg_arc.read().unwrap();
        if c.shell.show_greeting {
            println!("\nWelcome to Crush! Type 'help' for reference.\n");
        }
    }

    // ── Startup commands ──────────────────────────────────────────────────────
    let startup_cmds: Vec<String> = {
        cfg_arc.read().unwrap().startup.commands.clone()
    };
    for cmd_str in &startup_cmds {
        let cmd_str = cmd_str.trim();
        if cmd_str.is_empty() { continue; }
        let tokens_raw = match shlex::split(cmd_str) {
            Some(t) => t,
            None    => { eprintln!("crush: startup: bad quote in {:?}", cmd_str); continue; }
        };
        let tokens_owned = tokenize_operators(tokens_raw);
        let tokens: Vec<&str> = tokens_owned.iter().map(|s| s.as_str()).collect();
        let units = parse_input(&tokens);
        if !units.is_empty() {
            let cfg_snap = cfg_arc.read().unwrap().clone();
            let mut ctx = ExecContext {
                jobs:      &job_table,
                rl:        &mut rl,
                raw_input: cmd_str,
                config:    &cfg_snap,
            };
            executor::execute_line(&mut ctx, units);
        }
    }

    // ── REPL ──────────────────────────────────────────────────────────────────
    loop {
        {
            let mut table = job_table.lock().unwrap();
            table.reap_done();
        }

        let add_newline = cfg_arc.read().unwrap().shell.add_newline_before_prompt;
        if add_newline { println!(); }

        let prompt   = build_prompt();
        let readline = rl.readline(&prompt);

        match readline {
            Ok(raw) => {
                let input = raw.trim().to_string();
                if input.is_empty() { continue; }

                rl.add_history_entry(&input).ok();

                // Snapshot config (clone ringan)
                let cfg_snap = cfg_arc.read().unwrap().clone();

                // Resolve alias (kata pertama)
                let effective = resolve_alias_input(&input, &cfg_snap);

                let shlex_tokens = match shlex::split(&effective) {
                    Some(t) => t,
                    None => { eprintln!("crush: unmatched quote"); continue; }
                };
                let tokens_owned = tokenize_operators(shlex_tokens);
                let tokens: Vec<&str> = tokens_owned.iter().map(|s| s.as_str()).collect();

                let units = parse_input(&tokens);
                if units.is_empty() { continue; }

                let mut ctx = ExecContext {
                    jobs:      &job_table,
                    rl:        &mut rl,
                    raw_input: &input,
                    config:    &cfg_snap,
                };
                executor::execute_line(&mut ctx, units);
            }

            Err(ReadlineError::Interrupted) | Err(ReadlineError::Eof) => {
                println!("\nexiting crush. goodbye!");
                break;
            }

            Err(err) => {
                eprintln!("crush: readline error: {}", err);
                break;
            }
        }
    }

    history::save_history(&mut rl);
}

// ─────────────────────────────────────────────────────────────────────────────
// Config-driven keybindings
// ─────────────────────────────────────────────────────────────────────────────

/// Terapkan [bindings] dari config ke rustyline editor.
/// Format key: "Ctrl+L", "Ctrl+R", "Alt+X", "Ctrl+F"
/// Format action: "clear_screen", "search_history", "exit_shell",
///                "execute: <cmd>" (jalankan perintah langsung)
fn apply_config_bindings(
    rl: &mut Editor<CrushCompleter, FileHistory>,
    cfg: &ShellConfig,
) {
    for (key_str, action) in &cfg.bindings {
        let Some(key_event) = parse_key_str(key_str) else { continue; };

        let cmd = match action.as_str() {
            "clear_screen"   => Cmd::ClearScreen,
            "search_history" => Cmd::ReverseSearchHistory,
            "exit_shell"     => Cmd::Interrupt,
            "accept_line"    => Cmd::AcceptLine,
            "move_home"      => Cmd::Move(rustyline::Movement::BeginningOfLine),
            "move_end"       => Cmd::Move(rustyline::Movement::EndOfLine),
            "complete_hint"  => Cmd::CompleteHint,
            "complete_list"  => Cmd::Complete,
            // "execute: <cmd>" → ExternalPrint (tidak bisa langsung, catat untuk REPL)
            // Saat ini: skip action execute karena butuh REPL loop hook
            _ if action.starts_with("execute:") => {
                // TODO sesi berikutnya: inject string ke readline buffer lalu accept
                continue;
            }
            _ => {
                eprintln!("crush: unknown binding action: {:?}", action);
                continue;
            }
        };

        rl.bind_sequence(key_event, cmd);
    }
}

/// Parse string keybinding "Ctrl+L", "Alt+X", "Ctrl+Shift+A" → rustyline KeyEvent
fn parse_key_str(s: &str) -> Option<KeyEvent> {
    let mut mods = Modifiers::NONE;
    let parts: Vec<&str> = s.split('+').collect();
    if parts.is_empty() { return None; }

    let key_part = parts.last()?;
    for mod_part in &parts[..parts.len() - 1] {
        match mod_part.to_lowercase().as_str() {
            "ctrl"  => mods |= Modifiers::CTRL,
            "alt"   => mods |= Modifiers::ALT,
            "shift" => mods |= Modifiers::SHIFT,
            _       => {}
        }
    }

    let key = match *key_part {
        "Enter" | "Return" => Enter,
        "Tab"              => Tab,
        "BackTab"          => BackTab,
        "Esc"              => Esc,
        "Backspace"        => Backspace,
        "Delete"           => Delete,
        "Home"             => Home,
        "End"              => End,
        "PageUp"           => PageUp,
        "PageDown"         => PageDown,
        "Up"               => Up,
        "Down"             => Down,
        "Left"             => Left,
        "Right"            => Right,
        s if s.len() == 1  => Char(s.chars().next()?),
        _                  => return None,
    };

    Some(KeyEvent(key, mods))
}

// ─────────────────────────────────────────────────────────────────────────────
// Alias resolution
// ─────────────────────────────────────────────────────────────────────────────

fn resolve_alias_input(input: &str, cfg: &ShellConfig) -> String {
    let first_space = input.find(char::is_whitespace);
    let cmd  = &input[..first_space.unwrap_or(input.len())];
    let rest = first_space.map(|i| &input[i..]).unwrap_or("");

    if let Some(expanded) = cfg.resolve_alias(cmd) {
        format!("{}{}", expanded, rest)
    } else {
        input.to_string()
    }
}
