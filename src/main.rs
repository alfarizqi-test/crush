mod shell;
mod history;
mod completion;
mod help;
mod jobs;
mod executor;
mod ls;

use std::env;

use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::KeyCode::*;
use rustyline::{Cmd, Editor, KeyEvent, Modifiers};

use completion::CrushCompleter;
use executor::{parse_input, tokenize_operators, ExecContext};
use jobs::new_job_table;

// ─────────────────────────────────────────────────────────────────────────────
// Prompt
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

    // ── Editor rustyline ──────────────────────────────────────────────────────
    let rl_config = history::build_rl_config();
    let helper    = CrushCompleter::new();
    let mut rl: Editor<CrushCompleter, FileHistory> =
        Editor::with_history(rl_config, FileHistory::new())
            .expect("gagal membuat editor");
    rl.set_helper(Some(helper));

    // ── Keybindings ───────────────────────────────────────────────────────────
    //
    // Ghost text (dim abu-abu):
    //   muncul otomatis dari Hinter — history atau top completion candidate.
    //   Terima dengan: →  /  End  /  Shift+Tab
    //
    // Tab (CompletionType::Circular):
    //   1×Tab  → isi common prefix atau top match langsung
    //   2×Tab  → tampilkan semua kandidat + cycle
    //
    // Ctrl+Tab → paksa tampilkan full list (Cmd::Complete ulang)
    // Alt+l    → sama: paksa tampilkan full list
    // ─────────────────────────────────────────────────────────────────────────

    // Ctrl+D → exit
    rl.bind_sequence(KeyEvent(Char('d'), Modifiers::CTRL), Cmd::Interrupt);

    // Shift+Tab → terima ghost text (CompleteHint)
    rl.bind_sequence(KeyEvent(BackTab, Modifiers::NONE), Cmd::CompleteHint);

    // Ctrl+Space → paksa tampilkan semua completion list
    // (Ctrl+Tab tidak bisa di-bind karena terminal mengirimnya sebagai Tab biasa,
    //  tetapi Ctrl+Space dapat dibind dan hasilnya sama.)
    rl.bind_sequence(
        KeyEvent(Char(' '), Modifiers::CTRL),
        Cmd::Complete,
    );

    // Alt+l → juga tampilkan semua completion list
    rl.bind_sequence(KeyEvent(Char('l'), Modifiers::ALT), Cmd::Complete);

    // Ctrl+P / Ctrl+N → alias ↑/↓ history
    rl.bind_sequence(KeyEvent(Char('p'), Modifiers::CTRL), Cmd::PreviousHistory);
    rl.bind_sequence(KeyEvent(Char('n'), Modifiers::CTRL), Cmd::NextHistory);

    // ── History & Job table ───────────────────────────────────────────────────
    history::load_history(&mut rl);
    let job_table = new_job_table();

    println!("\nWelcome to Crush!\nType 'exit' or Ctrl+D to quit. Type 'help' for reference.\n");

    // ── REPL ──────────────────────────────────────────────────────────────────
    loop {
        // Reap background jobs yang sudah selesai sebelum mencetak prompt
        {
            let mut table = job_table.lock().unwrap();
            table.reap_done();
        }

        let prompt   = build_prompt();
        let readline = rl.readline(&prompt);

        match readline {
            Ok(raw) => {
                let input = raw.trim().to_string();
                if input.is_empty() { continue; }

                rl.add_history_entry(&input).ok();

                // Tokenize dengan operator splitting (&&, ||, |, &)
                let shlex_tokens = match shlex::split(&input) {
                    Some(t) => t,
                    None => {
                        eprintln!("crush: unmatched quote");
                        continue;
                    }
                };
                let tokens_owned = tokenize_operators(shlex_tokens);
                let tokens: Vec<&str> = tokens_owned.iter().map(|s| s.as_str()).collect();

                let units = parse_input(&tokens);
                if units.is_empty() { continue; }

                let mut ctx = ExecContext {
                    jobs:      &job_table,
                    rl:        &mut rl,
                    raw_input: &input,
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
