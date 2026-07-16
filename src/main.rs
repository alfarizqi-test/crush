mod shell;
mod history;
mod completion;
mod help;

#[allow(unused_imports)]
use std::env;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;
use which::which;

use rustyline::error::ReadlineError;
use rustyline::history::FileHistory;
use rustyline::KeyCode::*;
use rustyline::KeyEvent;
use rustyline::Modifiers;
use rustyline::Cmd;
use rustyline::Editor;

use completion::CrushCompleter;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn expand_variables(arg: &str) -> String {
    if arg == "$$" {
        return std::process::id().to_string();
    }
    if arg.starts_with('$') && arg.len() > 1 {
        let var_name = &arg[1..];
        return std::env::var(var_name).unwrap_or_default();
    }
    arg.to_string()
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with('~') {
        if let Ok(home) = env::var("HOME") {
            return path.replacen('~', &home, 1);
        }
    }
    path.to_string()
}

/// Buat string prompt dinamis berdasarkan direktori kerja saat ini.
fn build_prompt() -> String {
    if let Ok(cwd) = env::current_dir() {
        // Singkat: ganti $HOME dengan ~
        let home = env::var("HOME").unwrap_or_default();
        let path_str = cwd.to_string_lossy();
        let display = if !home.is_empty() && path_str.starts_with(&home) {
            format!("~{}", &path_str[home.len()..])
        } else {
            path_str.to_string()
        };
        format!("\x1b[1;34m[ {} ]\x1b[0m\n\x1b[1;32m❯\x1b[0m ", display)
    } else {
        "\x1b[1;32m❯\x1b[0m ".to_string()
    }
}

// ── Main ──────────────────────────────────────────────────────────────────────

fn main() {
    shell::ShellInfo::init_environment();

    if let Some(arg) = std::env::args().nth(1) {
        if arg == "--check" {
            shell::ShellInfo::register_to_system();
            return;
        }
    }

    // Bangun editor rustyline dengan config + helper (completer + hinter)
    let rl_config = history::build_rl_config();
    let helper = CrushCompleter::new();
    let mut rl: Editor<CrushCompleter, FileHistory> =
        Editor::with_history(rl_config, FileHistory::new()).expect("gagal membuat editor");
    rl.set_helper(Some(helper));

    // ── Keybindings ───────────────────────────────────────────────────────────
    //
    // Ghost text (suggestion dim):
    //   → muncul otomatis dari Hinter: history hint atau top completion candidate
    //   → terima ghost text dengan: Right-arrow / End / Shift+Tab
    //
    // Tab behaviour (CompletionType::Circular):
    //   • 1x Tab → isi common prefix (tipe 5) atau top match langsung
    //   • 2x Tab → tampilkan semua kandidat sebagai daftar + cycle
    //   • Alt+l  → paksa tampilkan daftar semua kandidat
    //
    // ─────────────────────────────────────────────────────────────────────────

    // Ctrl+D → exit shell
    rl.bind_sequence(
        KeyEvent(Char('d'), Modifiers::CTRL),
        Cmd::Interrupt,
    );

    // Shift+Tab → terima ghost text (accept top suggestion)
    rl.bind_sequence(
        KeyEvent(BackTab, Modifiers::NONE),
        Cmd::CompleteHint,
    );

    // Alt+l → paksa tampilkan semua completion list
    // (pada Circular mode, Cmd::Complete pada Tab ke-2 sudah memunculkan list;
    //  binding ini memungkinkan akses list secara eksplisit tanpa Tab pertama)
    rl.bind_sequence(
        KeyEvent(Char('l'), Modifiers::ALT),
        Cmd::Complete,
    );

    // Ctrl+P / Ctrl+N → alias Up/Down history
    rl.bind_sequence(
        KeyEvent(Char('p'), Modifiers::CTRL),
        Cmd::PreviousHistory,
    );
    rl.bind_sequence(
        KeyEvent(Char('n'), Modifiers::CTRL),
        Cmd::NextHistory,
    );

    // Muat history dari disk
    history::load_history(&mut rl);

    println!("\nWelcome to Crush!\nType 'exit' or press Ctrl+D to quit.\nType 'help' for a list of commands.\n");

    // ── REPL ──────────────────────────────────────────────────────────────────
    loop {
        let prompt = build_prompt();
        let readline = rl.readline(&prompt);

        match readline {
            Ok(input) => {
                let input = input.trim().to_string();
                if input.is_empty() {
                    continue;
                }

                // Tambah ke history (rustyline otomatis ikuti ignore_space & ignore_dups)
                rl.add_history_entry(&input).ok();

                run_command_line(&input, &mut rl);
            }

            // Ctrl+D → EOF → exit
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

    // Simpan history sebelum keluar
    history::save_history(&mut rl);
}

// ── Command dispatcher ────────────────────────────────────────────────────────

fn run_command_line(
    input: &str,
    rl: &mut Editor<CrushCompleter, FileHistory>,
) {
    let parsed_args = match shlex::split(input) {
        Some(args) => args,
        None => {
            eprintln!("Error: quotation marks aren't closed properly!");
            return;
        }
    };

    let args: Vec<&str> = parsed_args.iter().map(|s| s.as_str()).collect();

    // ── Deteksi output redirection ────────────────────────────────────────────
    let mut redirect_idx = None;
    let mut is_stderr = false;
    let mut is_append = false;

    for (i, &arg) in args.iter().enumerate() {
        match arg {
            ">" | "1>" => { redirect_idx = Some(i); is_stderr = false; is_append = false; break; }
            "2>"       => { redirect_idx = Some(i); is_stderr = true;  is_append = false; break; }
            ">>" | "1>>" => { redirect_idx = Some(i); is_stderr = false; is_append = true; break; }
            "2>>"      => { redirect_idx = Some(i); is_stderr = true;  is_append = true;  break; }
            _ => {}
        }
    }

    let mut clean_args = args.clone();
    let mut target_file: Option<&str> = None;

    if let Some(idx) = redirect_idx {
        if idx + 1 < args.len() {
            target_file = Some(args[idx + 1]);
        }
        clean_args.truncate(idx);
    }

    // ── Expand variabel ───────────────────────────────────────────────────────
    let expanded_args: Vec<String> = clean_args.iter().map(|&arg| expand_variables(arg)).collect();
    let clean_args_refs: Vec<&str> = expanded_args.iter().map(|s| s.as_str()).collect();

    // ── Dispatch ──────────────────────────────────────────────────────────────
    match clean_args_refs.as_slice() {
        // Help
        ["help"] => help::print_help(None),
        ["help", topic] => help::print_help(Some(topic)),

        // Keluar
        ["exit"] | ["exit", ..] => {
            history::save_history(rl);
            println!("exiting crush. goodbye!");
            std::process::exit(0);
        }

        // History builtin: tampilkan daftar perintah
        ["history"] => {
            for (i, entry) in rl.history().iter().enumerate() {
                println!("{:>5}  {}", i + 1, entry);
            }
        }

        // Clear screen
        ["clear"] => {
            print!("\x1b[2J\x1b[H");
            std::io::stdout().flush().ok();
        }

        // Echo
        ["echo"] => println!(),
        ["echo", echo_args @ ..] => {
            let output = echo_args.join(" ");
            write_or_print(&output, target_file, is_stderr, is_append);
        }

        // PWD
        ["pwd", ..] => match env::current_dir() {
            Ok(dir) => println!("{}", dir.display()),
            Err(e) => eprintln!("pwd: failed to get directory {}", e),
        },

        // CD
        ["cd"] => {
            let home = env::var("HOME").unwrap_or_else(|_| String::from("/"));
            if let Err(e) = env::set_current_dir(&home) {
                eprintln!("cd: {}", e);
            }
        }
        ["cd", path] => {
            let expanded_path = expand_tilde(path);
            let root = Path::new(&expanded_path);
            if let Err(_) = env::set_current_dir(root) {
                eprintln!("cd: {}: No such file or directory", path);
            }
        }
        ["cd", ..] => eprintln!("cd: too many arguments"),

        // Type
        ["type", args @ ("echo" | "exit" | "type" | "pwd" | "history" | "clear" | "help" | "export" | "unset" | "source")] => {
            println!("{} is a shell builtin", args);
        }
        ["type", arg] if which(arg).is_ok() => {
            let path = which(arg).unwrap();
            println!("{} is {}", arg, path.display());
        }
        ["type", args @ ..] => println!("{}: not found", args[0]),

        // External commands
        [cmd, ext_args @ ..] => {
            let mut process = Command::new(cmd);
            process.args(ext_args);

            if let Some(file_path) = target_file {
                let file_result = if is_append {
                    OpenOptions::new().create(true).append(true).open(file_path)
                } else {
                    File::create(file_path)
                };
                if let Ok(file) = file_result {
                    if is_stderr {
                        process.stderr(file);
                    } else {
                        process.stdout(file);
                    }
                }
            }

            match process.spawn() {
                Ok(mut child) => { child.wait().unwrap(); }
                Err(_) => println!("{}: command not found", cmd),
            }
        }

        _ => {}
    }
}

// ── Utility ───────────────────────────────────────────────────────────────────

fn write_or_print(output: &str, target_file: Option<&str>, is_stderr: bool, is_append: bool) {
    if let Some(file_path) = target_file {
        let file_result = if is_append {
            OpenOptions::new().create(true).append(true).open(file_path)
        } else {
            File::create(file_path)
        };
        if let Ok(mut file) = file_result {
            if is_stderr {
                println!("{}", output);
            } else {
                writeln!(file, "{}", output).unwrap();
            }
        }
    } else {
        println!("{}", output);
    }
}
