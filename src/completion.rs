// completion.rs — Advanced completion engine untuk crush shell
//
// Mendukung 9 tipe completion:
//   1.  Argument completion  — tiap builtin punya daftar argumen/flag khusus
//   2.  Missing completion   — jika tidak ada match, tunjukkan saran "did you mean?"
//   3.  Executable completion — semua binary di $PATH
//   4.  Multiple completion  — beberapa kandidat dengan tampilan rapi (kolom)
//   5.  Partial completion   — isi common prefix secara otomatis
//   6.  File completion      — path file/dir biasa di CWD
//   7.  Nested file completion — path bersarang (a/b/c<TAB>)
//   8.  Multiple matches     — tampilkan semua kandidat jika prefix ambigu
//   9.  Multi-argument       — complete argumen ke-N berdasarkan konteks command
//
// Keybinding (diatur di main.rs):
//   Tab       → complete / cycling kandidat (CompletionType::List)
//   Shift+Tab → ambil saran dari history hinter (CompleteHint)

use rustyline::completion::{Completer, FilenameCompleter, Pair};
use rustyline::highlight::Highlighter;
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::Validator;
use rustyline::highlight::CmdKind;
use rustyline::{Context, Helper};
use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::path::{Path, PathBuf};

// ── Builtin registry ─────────────────────────────────────────────────────────

/// Semua builtin yang dikenali shell crush.
const BUILTINS: &[&str] = &[
    "echo", "cd", "pwd", "type", "exit", "history", "clear", "export", "unset", "source",
];

/// Argumen / flag per builtin untuk completion argumen (tipe 1 & 9).
fn builtin_args() -> HashMap<&'static str, Vec<&'static str>> {
    let mut m: HashMap<&'static str, Vec<&'static str>> = HashMap::new();
    // echo: flag
    m.insert("echo", vec!["-n", "-e", "-E"]);
    // cd: direktori khusus
    m.insert("cd", vec!["~", "-", "..", "/"]);
    // type: semua builtin sebagai argumen valid
    m.insert("type", BUILTINS.to_vec());
    // export: variabel lingkungan (diisi dinamis di runtime)
    m.insert("export", vec![]);
    // unset: variabel lingkungan (diisi dinamis di runtime)
    m.insert("unset", vec![]);
    // history: sub-opsi
    m.insert("history", vec!["-c", "-r", "-w", "-n"]);
    m
}

// ── Tipe 2: Missing completion / "did you mean?" ─────────────────────────────

/// Hitung jarak Levenshtein antara dua string (sederhana, untuk "did you mean?").
fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i - 1] == b[j - 1] {
                dp[i - 1][j - 1]
            } else {
                1 + dp[i - 1][j].min(dp[i][j - 1]).min(dp[i - 1][j - 1])
            };
        }
    }
    dp[m][n]
}

/// Kembalikan saran "did you mean X?" jika prefix tidak cocok dengan kandidat mana pun.
/// Threshold: jarak ≤ 2 karakter.
fn did_you_mean<'a>(prefix: &str, candidates: &[&'a str]) -> Option<&'a str> {
    candidates
        .iter()
        .filter(|&&c| levenshtein(prefix, c) <= 2)
        .min_by_key(|&&c| levenshtein(prefix, c))
        .copied()
}

// ── Tipe 5: Partial completion (common prefix) ───────────────────────────────

/// Hitung common prefix dari sekumpulan string.
fn common_prefix(words: &[String]) -> String {
    if words.is_empty() { return String::new(); }
    let first = &words[0];
    let mut len = first.len();
    for w in words.iter().skip(1) {
        len = len.min(w.len());
        len = first[..len]
            .chars()
            .zip(w.chars())
            .take_while(|(a, b)| a == b)
            .count();
    }
    first[..len].to_string()
}

// ── Tipe 3 & 8: Executable completion ───────────────────────────────────────

/// Kumpulkan semua executable di $PATH yang awalan-nya cocok dengan `prefix`.
/// Mengembalikan Pair berurutan, dedup (tipe 8 = multiple matches).
fn complete_executables(prefix: &str) -> Vec<Pair> {
    let mut seen = std::collections::HashSet::new();
    let mut pairs = Vec::new();

    // Builtin dulu (tipe 3 mencakup builtin sebagai "commands")
    for &b in BUILTINS {
        if b.starts_with(prefix) && seen.insert(b.to_string()) {
            pairs.push(Pair {
                display: format!("{} \x1b[2m(builtin)\x1b[0m", b),
                replacement: b.to_string(),
            });
        }
    }

    // PATH executables
    if let Ok(path_var) = env::var("PATH") {
        use std::os::unix::fs::PermissionsExt;
        let mut exe_pairs: Vec<Pair> = path_var
            .split(':')
            .flat_map(|dir| std::fs::read_dir(dir).ok())
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                if !name.starts_with(prefix) { return None; }
                if !seen.insert(name.clone()) { return None; }
                let meta = entry.metadata().ok()?;
                if meta.permissions().mode() & 0o111 == 0 { return None; }
                Some(Pair {
                    display: name.clone(),
                    replacement: name,
                })
            })
            .collect();
        exe_pairs.sort_by(|a, b| a.display.cmp(&b.display));
        pairs.extend(exe_pairs);
    }

    pairs
}

// ── Tipe 6 & 7: File + Nested file completion ────────────────────────────────

/// Resolve path completion untuk tipe 6 (file biasa di CWD) dan
/// tipe 7 (nested path seperti `src/mai<TAB>` → `src/main.rs`).
///
/// Juga handle `~` expansion dan path absolut.
fn complete_paths(prefix: &str) -> Vec<Pair> {
    // Expand ~ di awal
    let expanded = if prefix.starts_with('~') {
        let home = env::var("HOME").unwrap_or_default();
        prefix.replacen('~', &home, 1)
    } else {
        prefix.to_string()
    };

    // Pisah menjadi direktori induk + fragment yang sedang diketik
    let path = Path::new(&expanded);
    let (search_dir, name_prefix, display_prefix): (PathBuf, &str, &str) =
        if expanded.ends_with('/') {
            // `src/` → masuk ke direktori src, prefix kosong
            (path.to_path_buf(), "", prefix)
        } else if let Some(parent) = path.parent() {
            let parent_str = if parent == Path::new("") {
                PathBuf::from(".")
            } else {
                parent.to_path_buf()
            };
            let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
            // display_prefix = bagian yang sudah diketik sebelum nama file
            let dp = if expanded.contains('/') {
                &prefix[..prefix.rfind('/').map(|i| i + 1).unwrap_or(0)]
            } else {
                ""
            };
            (parent_str, file_name, dp)
        } else {
            (PathBuf::from("."), prefix, "")
        };

    let mut pairs = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&search_dir) {
        let mut ep: Vec<Pair> = entries
            .flatten()
            .filter_map(|entry| {
                let fname = entry.file_name().into_string().ok()?;
                if !fname.starts_with(name_prefix) { return None; }

                let is_dir = entry.metadata().map(|m| m.is_dir()).unwrap_or(false);

                // display: nama file saja (+ "/" jika dir)
                let display = if is_dir {
                    format!("{}/", fname)
                } else {
                    fname.clone()
                };

                // replacement: prefix path + nama file (+ "/" jika dir)
                let replacement = if is_dir {
                    format!("{}{}/", display_prefix, fname)
                } else {
                    format!("{}{}", display_prefix, fname)
                };

                Some(Pair { display, replacement })
            })
            .collect();

        ep.sort_by(|a, b| {
            // Direktori lebih dulu, lalu alfabetis
            let a_dir = a.display.ends_with('/');
            let b_dir = b.display.ends_with('/');
            b_dir.cmp(&a_dir).then(a.display.cmp(&b.display))
        });
        pairs.extend(ep);
    }
    pairs
}

// ── Tipe 2: Variabel lingkungan ───────────────────────────────────────────────

fn complete_env_vars(prefix: &str) -> Vec<Pair> {
    // prefix sudah mengandung '$', misal "$HOM"
    let var_prefix = &prefix[1..];
    let mut pairs: Vec<Pair> = env::vars()
        .filter(|(k, _)| k.starts_with(var_prefix))
        .map(|(k, _)| {
            let replacement = format!("${}", k);
            Pair { display: replacement.clone(), replacement }
        })
        .collect();
    pairs.sort_by(|a, b| a.display.cmp(&b.display));
    pairs
}

// ── Tipe 1 & 9: Argument + Multi-argument completion ─────────────────────────

/// Context lengkap dari baris input yang sudah diparsing.
#[derive(Debug)]
struct LineContext<'a> {
    /// Command utama (token pertama)
    command: &'a str,
    /// Semua token sebelum cursor (tidak termasuk token saat ini) — digunakan untuk multi-arg context masa depan
    #[allow(dead_code)]
    prior_args: Vec<&'a str>,
    /// Token yang sedang diketik (mungkin kosong)
    current_word: &'a str,
    /// Posisi token saat ini (0 = command, 1 = arg1, dst)
    arg_position: usize,
}

impl<'a> LineContext<'a> {
    /// Parse baris sebelum cursor menjadi LineContext.
    fn parse(line: &'a str, pos: usize) -> Self {
        let before = &line[..pos];

        // Tokenisasi sederhana: split by whitespace tapi juga perhatikan quote
        let tokens: Vec<&str> = before
            .split_whitespace()
            .collect();

        // Apakah cursor tepat di belakang spasi → sedang mulai token baru
        let cursor_after_space = before.ends_with(char::is_whitespace) || before.is_empty();

        let (command, prior_args, current_word, arg_position) = if tokens.is_empty() {
            ("", vec![], "", 0)
        } else if cursor_after_space {
            // Semua token sudah lengkap, mulai token baru
            let cmd = tokens[0];
            let prior = tokens[1..].to_vec();
            let pos_idx = tokens.len();
            (cmd, prior, "", pos_idx)
        } else {
            // Token terakhir sedang diketik
            let current = *tokens.last().unwrap();
            let cmd = tokens[0];
            let prior = if tokens.len() > 1 { tokens[1..tokens.len() - 1].to_vec() } else { vec![] };
            let pos_idx = tokens.len() - 1;
            (cmd, prior, current, pos_idx)
        };

        LineContext { command, prior_args, current_word, arg_position }
    }

    fn is_command_position(&self) -> bool {
        self.arg_position == 0
    }
}

// ── CrushCompleter ────────────────────────────────────────────────────────────

pub struct CrushCompleter {
    file_completer: FilenameCompleter,
    hinter: HistoryHinter,
    builtin_arg_map: HashMap<&'static str, Vec<&'static str>>,
}

impl CrushCompleter {
    pub fn new() -> Self {
        Self {
            file_completer: FilenameCompleter::new(),
            hinter: HistoryHinter::new(),
            builtin_arg_map: builtin_args(),
        }
    }

    /// Titik masuk utama: routing completion berdasarkan konteks.
    fn route_completion(
        &self,
        ctx: &LineContext<'_>,
        line: &str,
        pos: usize,
        rl_ctx: &Context<'_>,
    ) -> (usize, Vec<Pair>) {
        let word = ctx.current_word;
        let word_start = if pos >= word.len() { pos - word.len() } else { 0 };

        // ── Tipe 6 & 7: path completion jika mulai '/', '.', '~', atau ada '/' di tengah
        if word.starts_with('/') || word.starts_with('.') || word.starts_with('~')
            || (!ctx.is_command_position() && word.contains('/'))
        {
            let pairs = complete_paths(word);
            return (word_start, self.apply_partial_and_missing(pairs, word, &[]));
        }

        // ── Tipe 2: variabel lingkungan
        if word.starts_with('$') {
            let pairs = complete_env_vars(word);
            return (word_start, pairs);
        }

        // ── Tipe 1 & 9: argument completion untuk builtin
        if !ctx.is_command_position() {
            // Multi-argument: sesuaikan suggestion berdasarkan posisi & command
            let pairs = self.complete_arguments(ctx, line, pos, rl_ctx);
            return (word_start, self.apply_partial_and_missing(pairs, word, &[]));
        }

        // ── Tipe 3 & 4 & 8: command completion (executable + multiple matches)
        let pairs = complete_executables(word);
        (word_start, self.apply_partial_and_missing(pairs, word, BUILTINS))
    }

    /// Tipe 1 & 9: argument-aware completion berdasarkan command dan posisi.
    fn complete_arguments(
        &self,
        ctx: &LineContext<'_>,
        line: &str,
        pos: usize,
        rl_ctx: &Context<'_>,
    ) -> Vec<Pair> {
        let word = ctx.current_word;
        let cmd = ctx.command;

        // Sub-routing berdasarkan command
        match cmd {
            // cd: hanya direktori
            "cd" => {
                let dir_pairs: Vec<Pair> = complete_paths(word)
                    .into_iter()
                    .filter(|p| p.display.ends_with('/') || p.display == ".." || p.display == "~")
                    .collect();

                // Tambah argumen khusus cd: ~, -, ..
                let mut extras: Vec<Pair> = ["~", "-", ".."]
                    .iter()
                    .filter(|&&s| s.starts_with(word))
                    .map(|&s| Pair { display: s.to_string(), replacement: s.to_string() })
                    .collect();
                extras.extend(dir_pairs);
                extras
            }

            // type: builtin names sebagai argumen
            "type" => BUILTINS
                .iter()
                .filter(|&&b| b.starts_with(word))
                .map(|&b| Pair {
                    display: format!("{} \x1b[2m(builtin)\x1b[0m", b),
                    replacement: b.to_string(),
                })
                .collect(),

            // export / unset: variabel lingkungan yang ada
            "export" | "unset" => {
                if word.is_empty() || !word.contains('=') {
                    // Complete nama variabel
                    let var_prefix = word.trim_start_matches('$');
                    let mut pairs: Vec<Pair> = env::vars()
                        .filter(|(k, _)| k.starts_with(var_prefix))
                        .map(|(k, v)| {
                            let display = if cmd == "export" {
                                format!("{}={}", k, v)
                            } else {
                                k.clone()
                            };
                            Pair { display: display.clone(), replacement: k }
                        })
                        .collect();
                    pairs.sort_by(|a, b| a.display.cmp(&b.display));
                    pairs
                } else {
                    vec![]
                }
            }

            // echo: flag -n/-e/-E, lalu file
            "echo" => {
                let mut pairs = Vec::new();
                if word.starts_with('-') {
                    for &flag in &["-n", "-e", "-E"] {
                        if flag.starts_with(word) {
                            pairs.push(Pair {
                                display: flag.to_string(),
                                replacement: flag.to_string(),
                            });
                        }
                    }
                } else {
                    pairs.extend(complete_paths(word));
                }
                pairs
            }

            // history: sub-opsi lalu fallback ke file
            "history" => ["-c", "-r", "-w", "-n"]
                .iter()
                .filter(|&&o| o.starts_with(word))
                .map(|&o| Pair { display: o.to_string(), replacement: o.to_string() })
                .collect(),

            // source: file .sh / .crush / path
            "source" => complete_paths(word)
                .into_iter()
                .filter(|p| {
                    p.display.ends_with('/') ||
                    p.display.ends_with(".sh") ||
                    p.display.ends_with(".crush") ||
                    p.display.ends_with(".rc")
                })
                .collect(),

            // Untuk perintah external: argumen ke-1 adalah flag/opsi (−),
            // argumen lain adalah file
            _ => {
                if word.starts_with('-') {
                    // Opsi/flag: tidak ada info statis → coba builtin arg map
                    if let Some(args) = self.builtin_arg_map.get(cmd) {
                        return args
                            .iter()
                            .filter(|&&a| a.starts_with(word))
                            .map(|&a| Pair { display: a.to_string(), replacement: a.to_string() })
                            .collect();
                    }
                    vec![]
                } else {
                    // File/dir completion untuk argumen external command (tipe 6 & 7)
                    let file_pairs = complete_paths(word);
                    if file_pairs.is_empty() {
                        // Fallback ke rustyline FilenameCompleter (tipe 7 nested)
                        self.file_completer.complete(line, pos, rl_ctx)
                            .map(|(_, p)| p)
                            .unwrap_or_default()
                    } else {
                        file_pairs
                    }
                }
            }
        }
    }

    /// Tipe 5: isi common prefix otomatis jika ada; tipe 2: "did you mean?" jika kosong.
    fn apply_partial_and_missing(
        &self,
        pairs: Vec<Pair>,
        prefix: &str,
        fallback_pool: &[&str],
    ) -> Vec<Pair> {
        if !pairs.is_empty() {
            // Tipe 5: jika ada > 1 kandidat, hitung common prefix dan jadikan
            // replacement-nya common prefix agar Tab mengisi sejauh yang bisa.
            if pairs.len() > 1 {
                let replacements: Vec<String> = pairs.iter().map(|p| p.replacement.clone()).collect();
                let cp = common_prefix(&replacements);
                // Hanya update replacement jika common prefix lebih panjang dari prefix
                if cp.len() > prefix.len() {
                    // Common prefix lebih panjang dari prefix yang sudah diketik.
                    // rustyline akan secara otomatis mengisi common prefix saat Tab.
                    let _ = &cp;
                }
            }
            return pairs;
        }

        // Tipe 2: tidak ada kandidat → "did you mean?"
        if !prefix.is_empty() && !prefix.starts_with('/') && !prefix.starts_with('$') {
            let all_commands: Vec<&str> = {
                let mut v: Vec<&str> = BUILTINS.to_vec();
                v.extend(fallback_pool);
                v
            };
            if let Some(suggestion) = did_you_mean(prefix, &all_commands) {
                return vec![Pair {
                    display: format!("\x1b[33m(did you mean '{}'?)\x1b[0m", suggestion),
                    replacement: suggestion.to_string(),
                }];
            }
        }

        pairs
    }
}

// ── Completer impl ────────────────────────────────────────────────────────────

impl Completer for CrushCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let line_ctx = LineContext::parse(line, pos);
        let (start, candidates) = self.route_completion(&line_ctx, line, pos, ctx);
        Ok((start, candidates))
    }
}

// ── Hinter ────────────────────────────────────────────────────────────────────

impl Hinter for CrushCompleter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

// ── Highlighter ───────────────────────────────────────────────────────────────

impl Highlighter for CrushCompleter {
    /// Ghost text hint: abu-abu redup
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("\x1b[2;37m{}\x1b[0m", hint))
    }

    /// Highlight command utama: biru jika dikenal, merah jika tidak
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        if line.is_empty() { return Cow::Borrowed(line); }

        let first_word_end = line.find(char::is_whitespace).unwrap_or(line.len());
        let first_word = &line[..first_word_end];
        let rest = &line[first_word_end..];

        let is_known = BUILTINS.contains(&first_word)
            || which::which(first_word).is_ok();

        let colored_cmd = if is_known {
            format!("\x1b[1;32m{}\x1b[0m", first_word)   // hijau = dikenal
        } else if !first_word.is_empty() {
            format!("\x1b[1;31m{}\x1b[0m", first_word)   // merah = tidak dikenal
        } else {
            first_word.to_string()
        };

        Cow::Owned(format!("{}{}", colored_cmd, rest))
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _kind: CmdKind) -> bool {
        true // aktifkan highlight setiap karakter
    }
}

// ── Validator ─────────────────────────────────────────────────────────────────

impl Validator for CrushCompleter {}

// ── Helper ────────────────────────────────────────────────────────────────────

impl Helper for CrushCompleter {}
