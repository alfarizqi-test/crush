// completion.rs — Builtin + PATH completion untuk crush shell
//
// Tab          → complete satu kata (CompletionType::Circular via default keybinding)
// Shift+Tab    → tampilkan semua suggestion (CompletionType::List)
//
// Urutan completion:
//   1. Builtin commands (echo, cd, pwd, type, exit, history, clear, …)
//   2. Variabel lingkungan ($HOME, $PATH, …)
//   3. File/direktori di CWD
//   4. Executable di $PATH

use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::borrow::Cow;
use std::env;

/// Daftar builtin yang dikenali shell crush.
const BUILTINS: &[&str] = &[
    "echo", "cd", "pwd", "type", "exit", "history", "clear", "export", "unset", "source",
];

/// Helper utama yang menggabungkan: Completer + Hinter + Highlighter + Validator.
pub struct CrushCompleter {
    file_completer: FilenameCompleter,
    hinter: HistoryHinter,
}

impl CrushCompleter {
    pub fn new() -> Self {
        Self {
            file_completer: FilenameCompleter::new(),
            hinter: HistoryHinter::new(),
        }
    }

    /// Kumpulkan semua kandidat: builtins → $VAR → file/dir → PATH executables
    fn collect_candidates(&self, prefix: &str) -> Vec<Pair> {
        let mut candidates: Vec<Pair> = Vec::new();

        // 1. Builtin commands
        for &builtin in BUILTINS {
            if builtin.starts_with(prefix) {
                candidates.push(Pair {
                    display: builtin.to_string(),
                    replacement: builtin.to_string(),
                });
            }
        }

        // 2. Variabel lingkungan: user mengetik $...
        if prefix.starts_with('$') {
            let var_prefix = &prefix[1..];
            for (key, _) in env::vars() {
                if key.starts_with(var_prefix) {
                    let display = format!("${}", key);
                    candidates.push(Pair {
                        display: display.clone(),
                        replacement: display,
                    });
                }
            }
        }

        // 3. Executable di $PATH
        if let Ok(path_var) = env::var("PATH") {
            let mut seen = std::collections::HashSet::new();
            for dir in path_var.split(':') {
                if let Ok(entries) = std::fs::read_dir(dir) {
                    for entry in entries.flatten() {
                        if let Ok(name) = entry.file_name().into_string() {
                            if name.starts_with(prefix) && seen.insert(name.clone()) {
                                // Pastikan executable (kasar: ada perms execute)
                                if let Ok(meta) = entry.metadata() {
                                    use std::os::unix::fs::PermissionsExt;
                                    if meta.permissions().mode() & 0o111 != 0 {
                                        candidates.push(Pair {
                                            display: name.clone(),
                                            replacement: name,
                                        });
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Sort & dedup berdasarkan display
        candidates.sort_by(|a, b| a.display.cmp(&b.display));
        candidates.dedup_by(|a, b| a.display == b.display);
        candidates
    }
}

impl Completer for CrushCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        // Cari kata yang sedang diketik (dari spasi terakhir sebelum cursor)
        let before_cursor = &line[..pos];
        let word_start = before_cursor
            .rfind(|c: char| c.is_whitespace())
            .map(|i| i + 1)
            .unwrap_or(0);
        let current_word = &line[word_start..pos];

        // Jika cursor ada di argumen pertama atau baris kosong → command completion
        let is_first_word = !before_cursor[..word_start].chars().any(|c| !c.is_whitespace());

        if is_first_word || current_word.starts_with('$') || current_word.starts_with('/') || current_word.starts_with('.') {
            // Untuk path atau variabel, gunakan file_completer sebagai tambahan
            let mut candidates = self.collect_candidates(current_word);

            if current_word.starts_with('/') || current_word.starts_with('.') || current_word.starts_with('~') {
                if let Ok((_, file_pairs)) = self.file_completer.complete(line, pos, _ctx) {
                    candidates.extend(file_pairs);
                }
            }

            Ok((word_start, candidates))
        } else {
            // Argumen biasa → file/dir completion
            let (start, pairs) = self.file_completer.complete(line, pos, _ctx)?;
            Ok((start, pairs))
        }
    }
}

// ── Hinter (ghost text dari history) ─────────────────────────────────────────
impl Hinter for CrushCompleter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

// ── Highlighter (warna prompt / teks) ─────────────────────────────────────────
impl Highlighter for CrushCompleter {
    /// Warnai hint (ghost text) dengan abu-abu redup
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        // ANSI: bold dim
        Cow::Owned(format!("\x1b[2m{}\x1b[0m", hint))
    }
}

// ── Validator (wajib untuk Helper) ────────────────────────────────────────────
impl Validator for CrushCompleter {}

// ── Helper trait (menggabungkan semua) ─────────────────────────────────────────
impl Helper for CrushCompleter {}
