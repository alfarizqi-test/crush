// completion.rs — Advanced completion engine untuk crush shell
//
// 9 tipe completion:
//   1.  Argument completion  — builtin punya flag/arg khusus (echo -n, history -c, …)
//   2.  Missing completion   — "did you mean X?" via Levenshtein jika tidak ada match
//   3.  Executable completion — binary executable di $PATH
//   4.  Multiple completion  — semua kandidat ditampilkan saat Tab
//   5.  Partial completion   — isi common prefix otomatis sebelum menampilkan daftar
//   6.  File completion      — file/dir di CWD
//   7.  Nested file completion — `src/ma<TAB>` → `src/main.rs`
//   8.  Multiple matches     — dedup + sort semua kandidat yang cocok
//   9.  Multi-argument       — argumen ke-N berbeda sesuai command

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
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipe 2: Levenshtein "did you mean?"
// ─────────────────────────────────────────────────────────────────────────────

fn levenshtein(a: &str, b: &str) -> usize {
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    let (m, n) = (a.len(), b.len());
    let mut dp = vec![vec![0usize; n + 1]; m + 1];
    for i in 0..=m { dp[i][0] = i; }
    for j in 0..=n { dp[0][j] = j; }
    for i in 1..=m {
        for j in 1..=n {
            dp[i][j] = if a[i-1] == b[j-1] { dp[i-1][j-1] }
                       else { 1 + dp[i-1][j].min(dp[i][j-1]).min(dp[i-1][j-1]) };
        }
    }
    dp[m][n]
}

fn suggest_typo(prefix: &str, pool: &[String]) -> Option<String> {
    pool.iter()
        .filter(|s| levenshtein(prefix, s) <= 2)
        .min_by_key(|s| levenshtein(prefix, s))
        .cloned()
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipe 5: Common prefix (partial completion)
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

// ─────────────────────────────────────────────────────────────────────────────
// Tipe 3 & 8: Executable + builtin completion
// ─────────────────────────────────────────────────────────────────────────────

fn complete_executables(prefix: &str) -> Vec<Pair> {
    let mut seen = std::collections::HashSet::new();
    let mut pairs: Vec<Pair> = Vec::new();

    // Builtin dulu
    for &b in BUILTINS {
        if b.starts_with(prefix) && seen.insert(b.to_string()) {
            pairs.push(Pair {
                display: format!("{} (builtin)", b),
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
                Some(Pair { display: name.clone(), replacement: name })
            })
            .collect();
        exe_pairs.sort_by(|a, b| a.replacement.cmp(&b.replacement));
        pairs.extend(exe_pairs);
    }

    // Tipe 5: jika ada >1 kandidat, jadikan replacement = common prefix
    apply_common_prefix(pairs, prefix)
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

    // Pisah menjadi direktori induk + fragment yang diketik
    let p = Path::new(&expanded);
    let (search_dir, name_prefix, dir_prefix) = if expanded.ends_with('/') {
        // "src/" → cari di dalam "src/"
        (p.to_path_buf(), String::new(), prefix.to_string())
    } else if let Some(parent) = p.parent().filter(|par| par != &Path::new("")) {
        // "src/ma" → cari di "src/", prefix "ma"
        let nf = p.file_name().and_then(|f| f.to_str()).unwrap_or("").to_string();
        // dir_prefix = bagian path sampai '/' terakhir (inklusif), pakai original prefix
        let slash_pos = prefix.rfind('/').map(|i| i + 1).unwrap_or(0);
        (parent.to_path_buf(), nf, prefix[..slash_pos].to_string())
    } else {
        // "ma" → cari di "."
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

    // Direktori dulu, lalu alfa
    pairs.sort_by(|a, b| {
        b.display.ends_with('/').cmp(&a.display.ends_with('/'))
            .then(a.display.cmp(&b.display))
    });

    // Tipe 5: common prefix
    apply_common_prefix(pairs, prefix)
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipe 5: Common prefix helper — inject replacement = common prefix saat >1
// ─────────────────────────────────────────────────────────────────────────────

fn apply_common_prefix(mut pairs: Vec<Pair>, prefix: &str) -> Vec<Pair> {
    if pairs.len() > 1 {
        let reps: Vec<&str> = pairs.iter().map(|p| p.replacement.as_str()).collect();
        let cp = common_prefix(&reps);
        // Jika common prefix lebih panjang dari prefix yang sudah diketik,
        // letakkan ia sebagai entry pertama (sentinel) agar rustyline mengisi dulu.
        if cp.len() > prefix.len() {
            pairs.insert(0, Pair {
                display: format!("  → common: {}", &cp[prefix.len()..]),
                replacement: cp,
            });
        }
    }
    pairs
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
    pairs
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipe 1 & 9: Argument-aware completion per command
// ─────────────────────────────────────────────────────────────────────────────

fn complete_args(cmd: &str, word: &str, _arg_n: usize) -> Vec<Pair> {
    match cmd {
        // cd: hanya direktori + alias khusus
        "cd" => {
            let mut extras: Vec<Pair> = ["~", "-", ".."]
                .iter()
                .filter(|&&s| s.starts_with(word))
                .map(|&s| Pair { display: s.to_string(), replacement: s.to_string() })
                .collect();
            let dirs: Vec<Pair> = complete_paths(word)
                .into_iter()
                .filter(|p| p.display.ends_with('/'))
                .collect();
            extras.extend(dirs);
            extras
        }

        // type: nama builtin sebagai argumen
        "type" => BUILTINS.iter()
            .filter(|&&b| b.starts_with(word))
            .map(|&b| Pair { display: b.to_string(), replacement: b.to_string() })
            .collect(),

        // export / unset: nama variabel lingkungan
        "export" | "unset" => {
            let var_prefix = word.trim_start_matches('$');
            let mut pairs: Vec<Pair> = env::vars()
                .filter(|(k, _)| k.starts_with(var_prefix))
                .map(|(k, v)| {
                    let display = if cmd == "export" { format!("{}={}", k, v) } else { k.clone() };
                    Pair { display, replacement: k }
                })
                .collect();
            pairs.sort_by(|a, b| a.replacement.cmp(&b.replacement));
            pairs
        }

        // echo: flag pada arg pertama, file pada arg berikutnya
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
            .filter(|p| {
                p.display.ends_with('/') || p.display.ends_with(".sh")
                    || p.display.ends_with(".crush") || p.display.ends_with(".rc")
            })
            .collect(),

        // Perintah external: flag dengan '-', sisanya file
        _ => {
            if word.starts_with('-') {
                vec![] // Tidak ada info statis untuk flag external command
            } else {
                complete_paths(word)
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tipe 2: Fallback "did you mean?" jika tidak ada kandidat
// ─────────────────────────────────────────────────────────────────────────────

fn fallback_suggestion(prefix: &str) -> Vec<Pair> {
    if prefix.is_empty() || prefix.starts_with('/') || prefix.starts_with('$') {
        return vec![];
    }

    // Kumpulkan semua nama command yang dikenal
    let mut pool: Vec<String> = BUILTINS.iter().map(|s| s.to_string()).collect();
    if let Ok(path_var) = env::var("PATH") {
        use std::os::unix::fs::PermissionsExt;
        for dir in path_var.split(':') {
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.flatten() {
                    if let Ok(name) = entry.file_name().into_string() {
                        if let Ok(meta) = entry.metadata() {
                            if meta.permissions().mode() & 0o111 != 0 {
                                pool.push(name);
                            }
                        }
                    }
                }
            }
        }
    }

    if let Some(suggestion) = suggest_typo(prefix, &pool) {
        vec![Pair {
            display: format!("(did you mean '{}'?)", suggestion),
            replacement: suggestion,
        }]
    } else {
        vec![]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Completer impl — titik masuk utama
// ─────────────────────────────────────────────────────────────────────────────

impl Completer for CrushCompleter {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let before = &line[..pos];

        // ── Cari batas kata saat ini (terbukti benar) ──────────────────────
        let word_start = before
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        let word = &line[word_start..pos];

        // ── Tentukan posisi argumen ─────────────────────────────────────────
        // Berapa banyak token SEBELUM kata saat ini?
        let prefix_before_word = &before[..word_start];
        let prior_tokens: Vec<&str> = prefix_before_word.split_whitespace().collect();
        let arg_position = prior_tokens.len(); // 0 = command, 1 = arg1, dst
        let command = prior_tokens.first().copied().unwrap_or("");

        // ── Routing ────────────────────────────────────────────────────────
        let candidates = if word.starts_with('$') {
            // Env var completion
            complete_env_vars(word)

        } else if word.starts_with('/') || word.starts_with('~')
            || word.starts_with("./") || word.starts_with("../")
            || (arg_position != 0 && word.contains('/'))
        {
            // Tipe 6 & 7: path absolut / relative / nested
            complete_paths(word)

        } else if arg_position == 0 {
            // Tipe 3: command completion
            let pairs = complete_executables(word);
            if pairs.is_empty() {
                // Tipe 2: did you mean?
                fallback_suggestion(word)
            } else {
                pairs
            }

        } else {
            // Tipe 1 & 9: argument completion
            let pairs = complete_args(command, word, arg_position);
            if pairs.is_empty() && !word.is_empty() {
                // Tipe 6: fallback ke file completion
                let file_pairs = complete_paths(word);
                if file_pairs.is_empty() {
                    fallback_suggestion(word)
                } else {
                    file_pairs
                }
            } else {
                pairs
            }
        };

        Ok((word_start, candidates))
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Hinter — ghost text dari history
// ─────────────────────────────────────────────────────────────────────────────

impl Hinter for CrushCompleter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Highlighter — warna command + hint
// ─────────────────────────────────────────────────────────────────────────────

impl Highlighter for CrushCompleter {
    /// Ghost text: abu-abu redup
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("\x1b[2;37m{}\x1b[0m", hint))
    }

    /// Warnai command utama: hijau = dikenal, merah = tidak dikenal
    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        if line.is_empty() { return Cow::Borrowed(line); }

        let end = line.find(char::is_whitespace).unwrap_or(line.len());
        let cmd = &line[..end];
        let rest = &line[end..];

        if cmd.is_empty() { return Cow::Borrowed(line); }

        let is_known = BUILTINS.contains(&cmd) || which::which(cmd).is_ok();
        let colored = if is_known {
            format!("\x1b[1;32m{}\x1b[0m{}", cmd, rest)   // hijau
        } else {
            format!("\x1b[1;31m{}\x1b[0m{}", cmd, rest)   // merah
        };
        Cow::Owned(colored)
    }

    fn highlight_char(&self, _line: &str, _pos: usize, _kind: CmdKind) -> bool {
        true
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Validator & Helper
// ─────────────────────────────────────────────────────────────────────────────

impl Validator for CrushCompleter {}
impl Helper for CrushCompleter {}
