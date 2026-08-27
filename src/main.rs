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
use rustyline::{Cmd, Editor, KeyEvent, Modifiers, EventHandler, ConditionalEventHandler, Event, EventContext, RepeatCount};

use completion::CrushCompleter;
use config::ShellConfig;
use executor::{parse_input, tokenize_operators, ExecContext};
use jobs::new_job_table;

// ─────────────────────────────────────────────────────────────────────────────
// Prompt
// ─────────────────────────────────────────────────────────────────────────────

// build_prompt dihapus, sekarang menggunakan config::prompt::render_prompt

// ─────────────────────────────────────────────────────────────────────────────
// Main
// ─────────────────────────────────────────────────────────────────────────────

fn main() {
    // Abaikan SIGINT di level shell menggunakan libc.
    // Saat perintah eksternal berjalan (misalnya `sleep 100`), menekan Ctrl+C akan mengirim SIGINT
    // ke shell dan proses anak (karena mereka berada di foreground process group yang sama).
    // Dengan mengabaikannya di sini, shell tetap hidup sementara proses anak akan mati secara default.
    // Saat tidak ada proses berjalan (mengetik prompt), Rustyline yang menangani Ctrl+C.
    unsafe {
        libc::signal(libc::SIGINT, libc::SIG_IGN);
    }

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
                cfg_arc:   &cfg_arc,
            };
            executor::execute_line(&mut ctx, units);
        }
    }

    let mut last_exit_code = 0;
    let mut last_duration_ms = 0;
    let is_ssh = env::var("SSH_CONNECTION").is_ok() || env::var("SSH_CLIENT").is_ok();

    // ── REPL ──────────────────────────────────────────────────────────────────
    loop {
        {
            let mut table = job_table.lock().unwrap();
            table.reap_done();
        }

        let add_newline = cfg_arc.read().unwrap().shell.add_newline_before_prompt;
        if add_newline { println!(); }

        let prompt = {
            let cfg = cfg_arc.read().unwrap();
            let prompt_ctx = config::prompt::PromptContext {
                last_exit_code,
                cmd_duration_ms: last_duration_ms,
                is_ssh,
            };
            config::prompt::render_prompt(&cfg.prompt, &prompt_ctx)
        };
        
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
                    cfg_arc:   &cfg_arc,
                };
                let start_time = std::time::Instant::now();
                last_exit_code = executor::execute_line(&mut ctx, units);
                last_duration_ms = start_time.elapsed().as_millis() as u64;
            }

            Err(ReadlineError::Interrupted) => {
                print!("\x1b[2A\r\x1b[0J");
                use std::io::Write;
                let _ = std::io::stdout().flush();
                continue;
            }

            Err(ReadlineError::Eof) => {
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
// Config-driven keybindings & State Injection
// ─────────────────────────────────────────────────────────────────────────────

struct ExitShellHandler;
impl ConditionalEventHandler for ExitShellHandler {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _pos: bool, ctx: &EventContext) -> Option<Cmd> {
        if ctx.line().is_empty() {
            Some(Cmd::EndOfFile)
        } else {
            Some(Cmd::Kill(rustyline::Movement::ForwardChar(1)))
        }
    }
}

/// State Injection Escape Hatch
/// Digunakan untuk memasukkan string langsung ke input buffer tty via ioctl TIOCSTI.
/// Hal ini "menipu" terminal agar seolah-olah user yang mengetikkannya dengan cepat.
struct InjectAndExecute(String);
impl ConditionalEventHandler for InjectAndExecute {
    fn handle(&self, _evt: &Event, _n: RepeatCount, _pos: bool, _ctx: &EventContext) -> Option<Cmd> {
        unsafe {
            for byte in self.0.as_bytes() {
                let mut c = *byte as libc::c_char;
                libc::ioctl(libc::STDIN_FILENO, libc::TIOCSTI, &mut c as *mut _ as *mut libc::c_void);
            }
            let mut nl = b'\n' as libc::c_char;
            libc::ioctl(libc::STDIN_FILENO, libc::TIOCSTI, &mut nl as *mut _ as *mut libc::c_void);
        }
        // Mengembalikan None agar rustyline tidak melakukan perintah bawaan
        None
    }
}


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

        if action.starts_with("execute:") {
            let cmd_str = action["execute:".len()..].trim().to_string();
            rl.bind_sequence(
                key_event,
                EventHandler::Conditional(Box::new(InjectAndExecute(cmd_str)))
            );
            continue;
        }

        let cmd = match action.as_str() {
            "clear_screen"   => Cmd::ClearScreen,
            "search_history" => Cmd::ReverseSearchHistory,
            "exit_shell"     => {
                rl.bind_sequence(key_event, EventHandler::Conditional(Box::new(ExitShellHandler)));
                continue;
            }
            "accept_line"    => Cmd::AcceptLine,
            "move_home"      => Cmd::Move(rustyline::Movement::BeginningOfLine),
            "move_end"       => Cmd::Move(rustyline::Movement::EndOfLine),
            "complete_hint"  => Cmd::CompleteHint,
            "complete_list"  => Cmd::Complete,
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
        s if s.len() == 1  => Char(s.chars().next()?.to_ascii_lowercase()),
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
