use rustyline::completion::{Completer, Pair};
use rustyline::highlight::{CmdKind, Highlighter};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::validate::Validator;
use rustyline::{Context, Helper};
use std::borrow::Cow;
use std::env;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use crate::config::ShellConfig;

// ─────────────────────────────────────────────────────────────────────────────
// Constants
// ─────────────────────────────────────────────────────────────────────────────

const BUILTINS: &[&str] = &[
    "echo", "cd", "pwd", "type", "exit", "history", "clear",
    "export", "unset", "source", "jobs", "help", "ls", "reload",
    "rehash",
];

// ─────────────────────────────────────────────────────────────────────────────
// Core Struct
// ─────────────────────────────────────────────────────────────────────────────

pub struct CrushCompleter {
    hinter:  HistoryHinter,
    config:  Arc<RwLock<ShellConfig>>,
}

impl CrushCompleter {
    pub fn new(config: Arc<RwLock<ShellConfig>>) -> Self {
        Self { hinter: HistoryHinter::new(), config }
    }

    fn case_sensitive(&self) -> bool {
        self.config.read()
            .map(|c| c.completion.case_sensitive)
            .unwrap_or(false)
    }

    fn alias_names(&self) -> Vec<String> {
        self.config.read()
            .map(|c| c.aliases.keys().cloned().collect())
            .unwrap_or_default()
    }

    fn dir_aliases(&self) -> Vec<(String, String)> {
        self.config.read()
            .map(|c| c.dir_aliases.iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect())
            .unwrap_or_default()
    }

    fn function_names(&self) -> Vec<String> {
        self.config.read()
            .map(|c| c.functions.names().iter().map(|s| s.to_string()).collect())
            .unwrap_or_default()
    }

    fn user_defined_names(&self) -> Vec<String> {
        let mut names = self.alias_names();
        names.extend(self.function_names());
        names
    }

    fn color_valid(&self) -> String {
        let color = self.config.read()
            .map(|c| c.theme.command_valid.clone())
            .unwrap_or_else(|_| "green".into());
        ansi_color(&color, true)
    }

    fn color_invalid(&self) -> String {
        let color = self.config.read()
            .map(|c| c.theme.command_invalid.clone())
            .unwrap_or_else(|_| "red".into());
        ansi_color(&color, true)
    }

    fn color_string(&self) -> String {
        let color = self.config.read()
            .map(|c| c.theme.string.clone())
            .unwrap_or_else(|_| "yellow".into());
        ansi_color(&color, false)
    }

    // ── Core: Input parsing ──────────────────────────────────────────

    fn parse_line<'a>(&self, line: &'a str, pos: usize) -> (usize, &'a str, &'a str, usize) {
        let before = &line[..pos];
        let word_start = before
            .rfind(char::is_whitespace)
            .map(|i| i + 1)
            .unwrap_or(0);
        let word = &line[word_start..pos];
        let prefix_before = &before[..word_start];
        let prior_tokens: Vec<&str> = prefix_before.split_whitespace().collect();
        let arg_position = prior_tokens.len();
        let command = prior_tokens.first().copied().unwrap_or("");

        (word_start, word, command, arg_position)
    }

    // ── Core: Main completion engine ──────────────────────────────────────

    pub fn get_candidates(&self, line: &str, pos: usize) -> (usize, Vec<Pair>) {
        let case_sensitive = self.case_sensitive();
        let (word_start, word, command, arg_position) = self.parse_line(line, pos);

        let candidates = if word.starts_with('$') {
            complete_env_vars(word)

        } else if is_path_like(word) {
            complete_paths(word, case_sensitive)

        } else if arg_position == 0 {
            let user_names = self.user_defined_names();
            let pairs = complete_executables(word, &user_names, case_sensitive);
            if pairs.is_empty() && !word.is_empty() {
                fallback_typo(word, &user_names)
            } else {
                pairs
            }

        } else {
            // argument completion
            let dir_aliases = self.dir_aliases();
            let pairs = complete_args(command, word, arg_position, &dir_aliases, case_sensitive);
            if pairs.is_empty() {
                let file_pairs = complete_paths(word, case_sensitive);
                if !file_pairs.is_empty() {
                    file_pairs
                } else if !word.is_empty() {
                    fallback_typo(word, &self.user_defined_names())
                } else {
                    vec![]
                }
            } else {
                pairs
            }
        };

        (word_start, candidates)
    }

    pub fn top_candidate(&self, line: &str, pos: usize) -> Option<Pair> {
        let (word_start, candidates) = self.get_candidates(line, pos);
        let word = &line[word_start..pos];
        candidates
            .into_iter()
            .find(|p| {
                !p.display.starts_with("→ ")
                && p.replacement.starts_with(word)
                && p.replacement.len() > word.len()
            })
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Path helper
// ─────────────────────────────────────────────────────────────────────────────

fn is_path_like(word: &str) -> bool {
    word.starts_with('/')
        || word.starts_with('~')
        || word.starts_with("./")
        || word.starts_with("../")
        || word.contains('/')
}

// ─────────────────────────────────────────────────────────────────────────────
// Type 2: Levenshtein
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

fn fallback_typo(prefix: &str, aliases: &[String]) -> Vec<Pair> {
    if prefix.is_empty() || prefix.starts_with('/') || prefix.starts_with('$') {
        return vec![];
    }
    let mut pool: Vec<&str> = BUILTINS.to_vec();
    let alias_refs: Vec<&str> = aliases.iter().map(|s| s.as_str()).collect();
    pool.extend_from_slice(&alias_refs);

    if let Some(&best) = pool.iter().filter(|&&s| levenshtein(prefix, s) <= 2)
                                    .min_by_key(|&&s| levenshtein(prefix, s))
    {
        vec![Pair {
            display:     format!("did you mean: {}", best),
            replacement: best.to_string(),
        }]
    } else {
        vec![]
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Type 5: Common prefix
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
// Type 3 & 8: Executable completion
// ─────────────────────────────────────────────────────────────────────────────

fn complete_executables(prefix: &str, aliases: &[String], case_sensitive: bool) -> Vec<Pair> {
    let mut seen = std::collections::HashSet::new();
    let mut builtin_pairs: Vec<Pair> = Vec::new();

    let matches = |name: &str| -> bool {
        if case_sensitive { name.starts_with(prefix) }
        else { name.to_lowercase().starts_with(&prefix.to_lowercase()) }
    };

    // Builtin priority
    for &b in BUILTINS {
        if matches(b) && seen.insert(b.to_string()) {
            builtin_pairs.push(Pair {
                display:     format!("{} [builtin]", b),
                replacement: b.to_string(),
            });
        }
    }

    // Aliases
    let mut alias_pairs: Vec<Pair> = Vec::new();
    for alias in aliases {
        if matches(alias) && seen.insert(alias.clone()) {
            alias_pairs.push(Pair {
                display:     format!("{} [alias]", alias),
                replacement: alias.clone(),
            });
        }
    }
    alias_pairs.sort_by(|a, b| a.replacement.cmp(&b.replacement));

    // Executable in $PATH
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
                if !matches(&name) || !seen.insert(name.clone()) { return None; }
                let meta = entry.metadata().ok()?;
                if meta.permissions().mode() & 0o111 == 0 { return None; }
                Some(Pair { display: name.clone(), replacement: name })
            })
            .collect();
        exe_pairs.sort_by(|a, b| a.replacement.cmp(&b.replacement));
    }

    let mut all = builtin_pairs;
    all.extend(alias_pairs);
    all.extend(exe_pairs);
    inject_common_prefix(all, prefix)
}

// ─────────────────────────────────────────────────────────────────────────────
// Type 6 & 7: File + nested path completion
// ─────────────────────────────────────────────────────────────────────────────

fn complete_paths(prefix: &str, case_sensitive: bool) -> Vec<Pair> {
    // Expand ~
    let expanded = if prefix.starts_with('~') {
        let home = env::var("HOME").unwrap_or_default();
        prefix.replacen('~', &home, 1)
    } else {
        prefix.to_string()
    };

    let p = Path::new(&expanded);

    let matches_name = |name: &str, pref: &str| -> bool {
        if case_sensitive { name.starts_with(pref) }
        else { name.to_lowercase().starts_with(&pref.to_lowercase()) }
    };

    let (search_dir, name_prefix, dir_prefix): (PathBuf, String, String) =
        if expanded.ends_with('/') {
            (p.to_path_buf(), String::new(), prefix.to_string())
        } else if let Some(parent) = p.parent().filter(|par| *par != Path::new("")) {
            let fname = p.file_name().and_then(|f| f.to_str()).unwrap_or("").to_string();
            let slash_end = prefix.rfind('/').map(|i| i + 1).unwrap_or(0);
            (parent.to_path_buf(), fname, prefix[..slash_end].to_string())
        } else {
            (PathBuf::from("."), expanded.clone(), String::new())
        };

    let mut pairs: Vec<Pair> = std::fs::read_dir(&search_dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let fname = entry.file_name().into_string().ok()?;
            if !matches_name(&fname, &name_prefix) { return None; }
            let is_dir = entry.metadata().map(|m| m.is_dir()).unwrap_or(false);
            let (display, replacement) = if is_dir {
                (format!("{}/", fname), format!("{}{}/", dir_prefix, fname))
            } else {
                (fname.clone(), format!("{}{}", dir_prefix, fname))
            };
            Some(Pair { display, replacement })
        })
        .collect();

    pairs.sort_by(|a, b| {
        b.display.ends_with('/').cmp(&a.display.ends_with('/'))
            .then(a.display.cmp(&b.display))
    });
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
// Type 1 & 9: Argument-aware completion
// ─────────────────────────────────────────────────────────────────────────────

fn complete_args(
    cmd: &str,
    word: &str,
    _arg_n: usize,
    dir_aliases: &[(String, String)],
    case_sensitive: bool,
) -> Vec<Pair> {
    let matches = |s: &str| -> bool {
        if case_sensitive { s.starts_with(word) }
        else { s.to_lowercase().starts_with(&word.to_lowercase()) }
    };

    match cmd {
        "cd" => {
            let mut pairs: Vec<Pair> = ["~", "-", ".."]
                .iter()
                .filter(|&&s| matches(s))
                .map(|&s| Pair { display: s.to_string(), replacement: s.to_string() })
                .collect();

            for (alias, target) in dir_aliases {
                let display_key = format!("~{}", alias);
                if matches(&display_key) {
                    pairs.push(Pair {
                        display:     format!("~{}  (→ {})", alias, target),
                        replacement: display_key,
                    });
                }
            }

            let dirs: Vec<Pair> = complete_paths(word, case_sensitive)
                .into_iter()
                .filter(|p| p.display.ends_with('/') || p.display.starts_with("→ "))
                .collect();
            pairs.extend(dirs);
            pairs
        }

        "type" => BUILTINS.iter()
            .filter(|&&b| matches(b))
            .map(|&b| Pair { display: format!("{} [builtin]", b), replacement: b.to_string() })
            .collect(),

        "export" | "unset" => {
            let var_prefix = word.trim_start_matches('$');
            let mut pairs: Vec<Pair> = env::vars()
                .filter(|(k, _)| {
                    if case_sensitive { k.starts_with(var_prefix) }
                    else { k.to_lowercase().starts_with(&var_prefix.to_lowercase()) }
                })
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

        "echo" => {
            if word.starts_with('-') {
                ["-n", "-e", "-E"].iter()
                    .filter(|&&f| matches(f))
                    .map(|&f| Pair { display: f.to_string(), replacement: f.to_string() })
                    .collect()
            } else {
                complete_paths(word, case_sensitive)
            }
        }

        "history" => ["-c", "-r", "-w", "-n"].iter()
            .filter(|&&o| matches(o))
            .map(|&o| Pair { display: o.to_string(), replacement: o.to_string() })
            .collect(),

        "source" => complete_paths(word, case_sensitive)
            .into_iter()
            .filter(|p| p.display.ends_with('/') || p.display.starts_with("→ ")
                || p.display.ends_with(".sh") || p.display.ends_with(".crush")
                || p.display.ends_with(".rc"))
            .collect(),

        _ => {
            if word.starts_with('-') {
                vec![]
            } else {
                complete_paths(word, case_sensitive)
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
// Hinter
// ─────────────────────────────────────────────────────────────────────────────

impl Hinter for CrushCompleter {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        if pos != line.len() || line.is_empty() {
            return None;
        }

        if let Some(h) = self.hinter.hint(line, pos, ctx) {
            return Some(h);
        }

        if line.trim().is_empty() {
            return None;
        }

        let top = self.top_candidate(line, pos)?;
        let (word_start, _, _, _) = self.parse_line(line, pos);
        let word = &line[word_start..pos];

        if top.replacement.starts_with(word) {
            let remaining = &top.replacement[word.len()..];
            if !remaining.is_empty() {
                return Some(remaining.to_string());
            }
        }

        None
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Highlighter
// ─────────────────────────────────────────────────────────────────────────────

impl Highlighter for CrushCompleter {
    fn highlight_hint<'h>(&self, hint: &'h str) -> Cow<'h, str> {
        Cow::Owned(format!("\x1b[2;37m{}\x1b[0m", hint))
    }

    fn highlight<'l>(&self, line: &'l str, _pos: usize) -> Cow<'l, str> {
        if line.is_empty() { return Cow::Borrowed(line); }

        let end = line.find(char::is_whitespace).unwrap_or(line.len());
        let cmd  = &line[..end];
        let rest = &line[end..];

        if cmd.is_empty() { return Cow::Borrowed(line); }

        let user_names = self.user_defined_names();
        let is_user_defined = user_names.iter().any(|a| a == cmd);
        let known = is_user_defined || BUILTINS.contains(&cmd) || which::which(cmd).is_ok();

        let cv  = self.color_valid();
        let ci  = self.color_invalid();
        let cs  = self.color_string();
        let rst = "\x1b[0m";

        let rest_colored = color_rest(rest, &cs, rst);

        let cmd_colored = if known {
            format!("{}{}{}", cv, cmd, rst)
        } else {
            format!("{}{}{}", ci, cmd, rst)
        };
        Cow::Owned(format!("{}{}", cmd_colored, rest_colored))
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

// ─────────────────────────────────────────────────────────────────────────────
// Theme helpers
// ─────────────────────────────────────────────────────────────────────────────

fn ansi_color(name: &str, bold: bool) -> String {
    let b = if bold { "1;" } else { "" };
    match name.to_lowercase().as_str() {
        "red"     | "r" => format!("\x1b[{}31m", b),
        "green"   | "g" => format!("\x1b[{}32m", b),
        "yellow"  | "y" => format!("\x1b[{}33m", b),
        "blue"    | "b" => format!("\x1b[{}34m", b),
        "magenta" | "m" => format!("\x1b[{}35m", b),
        "cyan"    | "c" => format!("\x1b[{}36m", b),
        "white"   | "w" => format!("\x1b[{}37m", b),
        "gray" | "grey"  => "\x1b[2;37m".into(),
        _ => {
            if let Ok(n) = name.parse::<u8>() {
                format!("\x1b[{}38;5;{}m", b, n)
            } else if name.starts_with('#') && name.len() == 7 {
                if let (Ok(r), Ok(g), Ok(bl)) = (
                    u8::from_str_radix(&name[1..3], 16),
                    u8::from_str_radix(&name[3..5], 16),
                    u8::from_str_radix(&name[5..7], 16),
                ) {
                    return format!("\x1b[{}38;2;{};{};{}m", b, r, g, bl);
                }
                format!("\x1b[{}32m", b)
            } else {
                format!("\x1b[{}32m", b)
            }
        }
    }
}

fn color_rest(rest: &str, string_color: &str, rst: &str) -> String {
    if rest.is_empty() { return String::new(); }

    let mut out = String::new();
    let tokens: Vec<&str> = rest.split_inclusive(char::is_whitespace).collect();
    for token in tokens {
        let trimmed = token.trim();
        let trailing_space = &token[trimmed.len()..];

        let is_string = trimmed.starts_with('"')
            || trimmed.starts_with('\'')
            || trimmed.starts_with('/')
            || trimmed.starts_with('~')
            || trimmed.starts_with("./")
            || trimmed.starts_with("../");

        if is_string && !trimmed.is_empty() {
            out.push_str(string_color);
            out.push_str(trimmed);
            out.push_str(rst);
        } else {
            out.push_str(trimmed);
        }
        out.push_str(trailing_space);
    }
    if out.is_empty() { rest.to_string() } else { out }
}
