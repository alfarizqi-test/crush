// completion.rs — Advanced completion engine untuk crush shell
//
// Strategi UX:
//   • Ghost text (dim)  : Hinter gabungan — history hint atau top completion candidate
//   • Tab               : CompletionType::Circular → isi top match, Tab lagi = cycle
//   • Shift+Tab         : CompleteHint → terima ghost text langsung
//   • Alt+l             : Cmd::Complete (trigger list di binding main.rs)
//
// 9 Tipe Completion:
//   1. Argument     — flag/arg khusus per builtin (echo -n, history -c, …)
//   2. Missing      — "did you mean X?" via Levenshtein distance ≤ 2
//   3. Executable   — binary di $PATH yang punya bit executable
//   4. Multiple     — semua kandidat diurutkan, ditampilkan saat list mode
//   5. Partial      — common prefix diisi CompletionType::Circular secara otomatis
//   6. File         — file/dir di CWD
//   7. Nested file  — src/ma<TAB> → src/main.rs
//   8. Multi-match  — dedup + sort, top candidate tampil sebagai ghost text
//   9. Multi-arg    — argumen ke-N berbeda per command (cd→dir, type→builtin, …)

use rustyline::completion::{Completer, Pair};
use rustyline::highlight::{CmdKind, Highlighter};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::borrow::Cow;
use std::env;
use std::path::{Path, PathBuf};

// ─────────────────────────────────────────────────────────────────────────────
// Konstanta
// ─────────────────────────────────────────────────────────────────────────────

const BUILTINS: &[&str] = &[
    "echo", "cd", "pwd", "type", "exit", "history", "clear",
    "export", "unset", "source",
];

// ─────────────────────────────────────────────────────────────────────────────
// Struct utama
// ─────────────────────────────────────────────────────────────────────────────

pub struct CrushCompleter {
    hinter: HistoryHinter,
}

impl CrushCompleter {
    pub fn new() -> Self {
        Self { hinter: HistoryHinter::new() }
    }

    // ── Core: parsing baris input ──────────────────────────────────────────

    /// Parse baris sebelum kursor, kembalikan (word_start, current_word, command, arg_position).
    /// Ini adalah satu-satunya tempat parsing dilakukan.
    fn parse_line<'a>(&self, line: &'a str, pos: usize) -> (usize, &'a str, &'a str, usize) {
        let before = &line[..pos];

        // Batas kata saat ini: cari whitespace terakhir sebelum kursor
        let word_start = before
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        let word = &line[word_start..pos];

        // Token sebelum kata saat ini → tentukan posisi argumen
        let prefix_before = &before[..word_start];
        let prior_tokens: Vec<&str> = prefix_before.split_whitespace().collect();
        let arg_position = prior_tokens.len(); // 0=command, 1=arg1, …
        let command = prior_tokens.first().copied().unwrap_or("");

        (word_start, word, command, arg_position)
    }

    // ── Core: engine completion utama ──────────────────────────────────────

    /// Kembalikan (word_start, Vec<Pair>) — dipakai oleh BOTH Completer dan Hinter.
    pub fn get_candidates(&self, line: &str, pos: usize) -> (usize, Vec<Pair>) {
        let (word_start, word, command, arg_position) = self.parse_line(line, pos);

        let candidates = if word.starts_with('$') {
            // Env var: $HOME, $PATH, …
            complete_env_vars(word)

        } else if is_path_like(word) {
            // Tipe 6 & 7: file/nested path
            complete_paths(word)

        } else if arg_position == 0 {
            // Tipe 3 & 8: command/executable completion
            let pairs = complete_executables(word);
            if pairs.is_empty() && !word.is_empty() {
                // Tipe 2: did you mean?
                fallback_typo(word)
            } else {
                pairs
            }

        } else {
            // Tipe 1 & 9: argument completion berdasarkan command
            let pairs = complete_args(command, word, arg_position);
            if pairs.is_empty() {
                // Fallback: file completion, lalu typo suggestion
                let file_pairs = complete_paths(word);
                if !file_pairs.is_empty() {
                    file_pairs
                } else if !word.is_empty() {
                    fallback_typo(word)
                } else {
                    vec![]
                }
            } else {
                pairs
            }
        };

        (word_start, candidates)
    }

    /// Ambil TOP candidate dari get_candidates() untuk ghost text.
    /// Filter sentinel common-prefix entry.
    pub fn top_candidate(&self, line: &str, pos: usize) -> Option<Pair> {
        let (word_start, candidates) = self.get_candidates(line, pos);
        let word = &line[word_start..pos];
        candidates
            .into_iter()
            .find(|p| {
                // Skip sentinel entry (common prefix display marker)
                !p.display.starts_with("→ ")
                // Hanya candidate yang replacement-nya memulai dengan kata yang diketik
                && p.replacement.starts_with(word)
                // Dan lebih panjang dari yang sudah diketik (ada sesuatu untuk di-complete)
                && p.replacement.len() > word.len()
            })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper: apakah kata ini berbentuk path?
// ─────────────────────────────────────────────────────────────────────────────

fn is_path_like(word: &str) -> bool {
    word.starts_with('/')
        || word.starts_with('~')
        || word.starts_with("./")
        || word.starts_with("../")
        || word.contains('/')
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipe 2: Levenshtein "did you mean?"
// ─────────────────────────────────────────────────────────────────────────────

fn levenshtein(a: &str, b: &str) -> usize {
    let av: Vec<char> = a.chars().collect();
    let bv: Vec<char> = b.chars().collect();
    let (m, n) = (av.len(), bv.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if av[i-1] == bv[j-1] { dp[i-1][j-1] }
                       else { 1 + dp[i-1][j].min(dp[i][j-1]).min(dp[i-1][j-1]) };
        }
    }
    dp[m][n]
}

/// Hanya pakai BUILTINS untuk typo suggestion (cepat, tidak scan $PATH).
fn fallback_typo(prefix: &str) -> Vec<Pair> {
    if prefix.is_empty() || prefix.starts_with('/') || prefix.starts_with('$') {
        return vec![];
    }
    let pool: Vec<&str> = BUILTINS.to_vec();
    if let Some(&best) = pool.iter().filter(|&&s| levenshtein(prefix, s) <= 2)
                                    .min_by_key(|&&s| levenshtein(prefix, s))
    {
        vec![Pair {
            display: format!("did you mean: {}", best),
            replacement: best.to_string(),
        }]
    } else {
        vec![]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipe 5: Common prefix
// ─────────────────────────────────────────────────────────────────────────────

fn common_prefix(words: &[&str]) -> String {
    if words.is_empty() { return String::new(); }
    let first = words[0];
    let mut len = first.len();
    for w in &words[1..] {
        len = first[..len].chars().zip(w.chars())
            .take_while(|(a, b)| a == b)
            .count();
        if len == 0 { break; }
    }
    first.chars().take(len).collect()
}

/// Jika ada >1 kandidat dan ada common prefix lebih panjang dari `prefix`,
/// sisipkan sentinel Pair di index 0 dengan replacement = common prefix.
/// Dengan CompletionType::Circular, rustyline akan memakai replacement[0] pada Tab pertama.
fn inject_common_prefix(mut pairs: Vec<Pair>, prefix: &str) -> Vec<Pair> {
    if pairs.len() > 1 {
        let reps: Vec<&str> = pairs.iter().map(|p| p.replacement.as_str()).collect();
        let cp = common_prefix(&reps);
        if cp.len() > prefix.len() {
            pairs.insert(0, Pair {
                display: format!("→ {}", cp),   // sentinel — tidak ditampilkan sebagai scoped entry
                replacement: cp,
            });
        }
    }
    pairs
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipe 3 & 8: Executable completion
// ─────────────────────────────────────────────────────────────────────────────

fn complete_executables(prefix: &str) -> Vec<Pair> {
    let mut seen = std::collections::HashSet::new();
    let mut builtin_pairs: Vec<Pair> = Vec::new();

    // Builtin selalu muncul dulu
    for &b in BUILTINS {
        if b.starts_with(prefix) && seen.insert(b.to_string()) {
            builtin_pairs.push(Pair {
                display: format!("{} [builtin]", b),
                replacement: b.to_string(),
            });
        }
    }

    // Executable di $PATH
    let mut exe_pairs: Vec<Pair> = Vec::new();
    if let Ok(path_var) = env::var("PATH") {
        use std::os::unix::fs::PermissionsExt;
        exe_pairs = path_var
            .split(':')
            .flat_map(|dir| std::fs::read_dir(dir).ok())
            .flatten()
            .flatten()
            .filter_map(|entry| {
                let name = entry.file_name().into_string().ok()?;
                if !name.starts_with(prefix) || !seen.insert(name.clone()) { return None; }
                let meta = entry.metadata().ok()?;
                if meta.permissions().mode() & 0o111 == 0 { return None; }
                Some(Pair { display: name.clone(), replacement: name })
            })
            .collect();
        exe_pairs.sort_by(|a, b| a.replacement.cmp(&b.replacement));
    }

    let mut all = builtin_pairs;
    all.extend(exe_pairs);

    // Tipe 5: inject common prefix sentinel
    inject_common_prefix(all, prefix)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipe 6 & 7: File + nested path completion
// ─────────────────────────────────────────────────────────────────────────────

fn complete_paths(prefix: &str) -> Vec<Pair> {
    // Expand ~
    let expanded = if prefix.starts_with('~') {
        let home = env::var("HOME").unwrap_or_default();
        prefix.replacen('~', &home, 1)
    } else {
        prefix.to_string()
    };

    let p = Path::new(&expanded);

    // Tentukan direktori pencarian dan nama prefix yang dicari
    let (search_dir, name_prefix, dir_prefix): (PathBuf, String, String) =
        if expanded.ends_with('/') {
            // "src/" → masuk ke dalam src/
            (p.to_path_buf(), String::new(), prefix.to_string())
        } else if let Some(parent) = p.parent().filter(|par| *par != Path::new("")) {
            // "src/ma" → cari di src/, prefix = "ma"
            let fname = p.file_name().and_then(|f| f.to_str()).unwrap_or("").to_string();
            let slash_end = prefix.rfind('/').map(|i| i + 1).unwrap_or(0);
            (parent.to_path_buf(), fname, prefix[..slash_end].to_string())
        } else {
            // "ma" → cari di CWD
            (PathBuf::from("."), expanded.clone(), String::new())
        };

    let mut pairs: Vec<Pair> = std::fs::read_dir(&search_dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let fname = entry.file_name().into_string().ok()?;
            if !fname.starts_with(name_prefix.as_str()) { return None; }
            let is_dir = entry.metadata().map(|m| m.is_dir()).unwrap_or(false);
            let (display, replacement) = if is_dir {
                (format!("{}/", fname), format!("{}{}/", dir_prefix, fname))
            } else {
                (fname.clone(), format!("{}{}", dir_prefix, fname))
            };
            Some(Pair { display, replacement })
        })
        .collect();

    // Direktori dulu, lalu alfabetis
    pairs.sort_by(|a, b| {
        b.display.ends_with('/').cmp(&a.display.ends_with('/'))
            .then(a.display.cmp(&b.display))
    });

    // Tipe 5: inject common prefix sentinel
    inject_common_prefix(pairs, prefix)
}

// ─────────────────────────────────────────────────────────────────────────────
// Env var completion
// ─────────────────────────────────────────────────────────────────────────────

fn complete_env_vars(prefix: &str) -> Vec<Pair> {
    let var_prefix = prefix.trim_start_matches('$');
    let mut pairs: Vec<Pair> = env::vars()
        .filter(|(k, _)| k.starts_with(var_prefix))
        .map(|(k, _)| {
            let rep = format!("${}", k);
            Pair { display: rep.clone(), replacement: rep }
        })
        .collect();
    pairs.sort_by(|a, b| a.replacement.cmp(&b.replacement));
    inject_common_prefix(pairs, prefix)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipe 1 & 9: Argument-aware completion per command
// ─────────────────────────────────────────────────────────────────────────────

fn complete_args(cmd: &str, word: &str, _arg_n: usize) -> Vec<Pair> {
    match cmd {
        // cd: direktori saja + alias khusus ~, -, ..
        "cd" => {
            let mut pairs: Vec<Pair> = ["~", "-", ".."]
                .iter()
                .filter(|&&s| s.starts_with(word))
                .map(|&s| Pair { display: s.to_string(), replacement: s.to_string() })
                .collect();
            let dirs: Vec<Pair> = complete_paths(word)
                .into_iter()
                .filter(|p| p.display.ends_with('/') || p.display.starts_with("→ "))
                .collect();
            pairs.extend(dirs);
            pairs
        }

        // type: nama builtin sebagai argumen
        "type" => BUILTINS.iter()
            .filter(|&&b| b.starts_with(word))
            .map(|&b| Pair { display: format!("{} [builtin]", b), replacement: b.to_string() })
            .collect(),

        // export / unset: variabel lingkungan
        "export" | "unset" => {
            let var_prefix = word.trim_start_matches('$');
            let mut pairs: Vec<Pair> = env::vars()
                .filter(|(k, _)| k.starts_with(var_prefix))
                .map(|(k, v)| {
                    let display = if cmd == "export" {
                        format!("{}  (={})", k, if v.len() > 20 { &v[..20] } else { &v })
                    } else {
                        k.clone()
                    };
                    Pair { display, replacement: k }
                })
                .collect();
            pairs.sort_by(|a, b| a.replacement.cmp(&b.replacement));
            pairs
        }

        // echo: flag -n/-e/-E atau file
        "echo" => {
            if word.starts_with('-') {
                ["-n", "-e", "-E"].iter()
                    .filter(|&&f| f.starts_with(word))
                    .map(|&f| Pair { display: f.to_string(), replacement: f.to_string() })
                    .collect()
            } else {
                complete_paths(word)
            }
        }

        // history: sub-opsi
        "history" => ["-c", "-r", "-w", "-n"].iter()
            .filter(|&&o| o.starts_with(word))
            .map(|&o| Pair { display: o.to_string(), replacement: o.to_string() })
            .collect(),

        // source: file script
        "source" => complete_paths(word)
            .into_iter()
            .filter(|p| p.display.ends_with('/') || p.display.starts_with("→ ")
                || p.display.ends_with(".sh") || p.display.ends_with(".crush")
                || p.display.ends_with(".rc"))
            .collect(),

        // Perintah lain: flag dengan '-' tidak ada info, selain itu file
        _ => {
            if word.starts_with('-') {
                vec![]
            } else {
                complete_paths(word)
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Completer impl
// ─────────────────────────────────────────────────────────────────────────────

impl Completer for CrushCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        Ok(self.get_candidates(line, pos))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Hinter — ghost text: history hint ATAU top completion suggestion
// ─────────────────────────────────────────────────────────────────────────────

impl Hinter for CrushCompleter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        // Ghost text hanya muncul jika kursor di akhir baris
        if pos != line.len() || line.is_empty() {
            return None;
        }

        // 1. History hint (prioritas utama — lebih relevan secara konteks)
        if let Some(h) = self.hinter.hint(line, pos, ctx) {
            return Some(h);
        }

        // 2. Top completion candidate sebagai ghost text
        //    Hanya jika sudah mengetik minimal 1 karakter
        if line.trim().is_empty() {
            return None;
        }

        let top = self.top_candidate(line, pos)?;
        let (word_start, _, _, _) = self.parse_line(line, pos);
        let word = &line[word_start..pos];

        if top.replacement.starts_with(word) {
            // Tampilkan sisa yang belum diketik: "ech" → ghost "o"
            let remaining = &top.replacement[word.len()..];
            if !remaining.is_empty() {
                return Some(remaining.to_string());
            }
        }

        None
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Highlighter — warna command + ghost text
// ─────────────────────────────────────────────────────────────────────────────

impl Highlighter for CrushCompleter {
    /// Ghost text: abu-abu redup
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("\x1b[2;37m{}\x1b[0m", hint))
    }

    /// Command hijau (dikenal) atau merah (tidak dikenal)
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        if line.is_empty() { return Cow::Borrowed(line); }

        let end = line.find(char::is_whitespace).unwrap_or(line.len());
        let cmd  = &line[..end];
        let rest = &line[end..];

        if cmd.is_empty() { return Cow::Borrowed(line); }

        let known = BUILTINS.contains(&cmd) || which::which(cmd).is_ok();
        let out = if known {
            format!("\x1b[1;32m{}\x1b[0m{}", cmd, rest)   // hijau
        } else {
            format!("\x1b[1;31m{}\x1b[0m{}", cmd, rest)   // merah
        };
        Cow::Owned(out)
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _kind: CmdKind) -> bool {
        true // redraw setiap karakter agar highlight selalu update
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Validator & Helper
// ─────────────────────────────────────────────────────────────────────────────

impl Validator for CrushCompleter {}
impl Helper for CrushCompleter {}
