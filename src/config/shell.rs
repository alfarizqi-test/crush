// config/shell.rs — Main shell configuration
//
// Membaca dari:
//   1. ~/.config/crush/config.toml        (user config, prioritas utama)
//   2. <workspace>/src/config-example.toml (dev fallback saat debugging)
//
// Sections:
//   [shell]      — greeting, syntax_highlighting, add_newline
//   [startup]    — perintah yang dijalankan saat shell dimulai
//   [env]        — variabel environment (termasuk PATH extension)
//   [aliases]    — alias perintah
//   [bindings]   — keybinding (Ctrl+L, Ctrl+R, dll) — applied ke rustyline
//   [completion] — opsi completion engine
//   [dir_aliases]— alias direktori untuk cd
//   [hooks]      — on_cd, on_clear, pre_command
//   [history]    — max_entries, ignore_space, ignore_dups, timestamp_format
//   [theme]      — warna command valid/invalid/string

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use serde::Deserialize;
use crate::config::wrapper::WrapperConfig;

// ─────────────────────────────────────────────────────────────────────────────
// Struct definitions
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct ShellSection {
    pub show_greeting:            bool,
    pub add_newline_before_prompt: bool,
    pub syntax_highlighting:      bool,
}

impl Default for ShellSection {
    fn default() -> Self {
        Self {
            show_greeting:             true,
            add_newline_before_prompt: false,
            syntax_highlighting:       true,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct StartupSection {
    /// Daftar perintah shell yang dijalankan saat crush pertama kali dimulai.
    pub commands: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────

/// [env] section: nilai bisa berupa String atau Vec<String> (untuk PATH)
#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct EnvSection {
    #[serde(rename = "EDITOR")]
    pub editor: Option<String>,

    #[serde(rename = "VISUAL")]
    pub visual: Option<String>,

    #[serde(rename = "LANG")]
    pub lang: Option<String>,

    /// PATH extensions: di-prepend ke PATH yang ada, tiap entry di-expand ~
    #[serde(rename = "PATH")]
    pub path_extra: Vec<String>,

    /// Semua env lain yang tidak dikenal (key-value bebas)
    #[serde(flatten)]
    pub extra: HashMap<String, toml::Value>,
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct CompletionSection {
    pub case_sensitive:    bool,
    pub fuzzy_search:      bool,
    pub inline_suggestions: bool,
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct HooksSection {
    /// Perintah yang dijalankan setiap kali cd berhasil (string kosong = nonaktif)
    pub on_cd:       String,
    /// Perintah yang dijalankan setelah clear (string kosong = nonaktif)
    pub on_clear:    String,
    /// Perintah yang dijalankan sebelum setiap command (string kosong = nonaktif)
    pub pre_command: String,
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct HistorySection {
    pub max_entries:      usize,
    pub ignore_space:     bool,
    pub ignore_dups:      bool,
    pub timestamp_format: String,
}

impl Default for HistorySection {
    fn default() -> Self {
        Self {
            max_entries:      10_000,
            ignore_space:     true,
            ignore_dups:      true,
            timestamp_format: "%Y-%m-%d %H:%M:%S".into(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct ThemeSection {
    pub command_valid:   String,
    pub command_invalid: String,
    pub string:          String,
}

impl Default for ThemeSection {
    fn default() -> Self {
        Self {
            command_valid:   "green".into(),
            command_invalid: "red".into(),
            string:          "yellow".into(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Root config struct
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct ShellConfig {
    pub shell:       ShellSection,
    pub startup:     StartupSection,
    pub env:         EnvSection,

    /// [aliases] — map nama → string perintah
    pub aliases:     HashMap<String, String>,

    /// [bindings] — map "Ctrl+L" → action string
    pub bindings:    HashMap<String, String>,

    pub completion:  CompletionSection,

    /// [dir_aliases] — map nama → path (~ akan di-expand)
    pub dir_aliases: HashMap<String, String>,

    pub hooks:       HooksSection,
    pub history:     HistorySection,
    pub theme:       ThemeSection,

    /// [functions.*] — wrapper functions (skip serde, diisi manual)
    #[serde(skip)]
    pub functions:   WrapperConfig,

    pub prompt:      crate::config::prompt::PromptConfig,
}

// ─────────────────────────────────────────────────────────────────────────────
// Loading
// ─────────────────────────────────────────────────────────────────────────────

impl ShellConfig {
    /// Load config dari path tertentu.
    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;

        // Parse sebagai raw toml::Table untuk dua tujuan:
        //   1. Deserialize bagian shell via serde
        //   2. Extract [functions.*] manual via WrapperConfig::from_toml
        let table: toml::Table = toml::from_str(&content)?;

        let mut cfg: ShellConfig = toml::Value::Table(table.clone())
            .try_into()
            .map_err(|e: toml::de::Error| anyhow::anyhow!(e))?;

        // Load functions section
        cfg.functions = WrapperConfig::from_toml(&table);

        Ok(cfg)
    }

    /// Load config dengan strategi fallback:
    ///   1. ~/.config/crush/config.toml
    ///   2. <workspace>/src/config-example.toml  (dev mode)
    ///   3. Default kosong jika tidak ada file
    pub fn load() -> Self {
        let user_path = super::config_path();

        if user_path.exists() {
            match Self::load_from(&user_path) {
                Ok(cfg) => {
                    return cfg;
                }
                Err(e) => {
                    eprintln!("crush: config error ({}): {}", user_path.display(), e);
                    eprintln!("crush: using default config.");
                }
            }
        }

        // Dev fallback: pakai config-example.toml
        if let Some(example_path) = super::example_config_path() {
            if example_path.exists() {
                match Self::load_from(&example_path) {
                    Ok(cfg) => {
                        eprintln!(
                            "crush: \x1b[2m[dev] loaded config from {}\x1b[0m",
                            example_path.display()
                        );
                        return cfg;
                    }
                    Err(e) => {
                        eprintln!("crush: config-example error: {}", e);
                    }
                }
            }
        }

        Self::default()
    }

    // ── Applicators ────────────────────────────────────────────────────────────

    /// Terapkan [env] ke environment proses saat ini.
    /// PATH entries di-prepend (bukan replace).
    pub fn apply_env(&self) {
        unsafe {
            // Field yang dikenal
            if let Some(ref v) = self.env.editor {
                std::env::set_var("EDITOR", v);
            }
            if let Some(ref v) = self.env.visual {
                std::env::set_var("VISUAL", v);
            }
            if let Some(ref v) = self.env.lang {
                std::env::set_var("LANG", v);
            }

            // PATH: prepend entries baru
            if !self.env.path_extra.is_empty() {
                let current = std::env::var("PATH").unwrap_or_default();
                let new_entries: Vec<String> = self
                    .env
                    .path_extra
                    .iter()
                    .map(|p| expand_tilde(p))
                    .collect();
                let combined = format!("{}:{}", new_entries.join(":"), current);
                std::env::set_var("PATH", &combined);
            }

            // Extra env vars (toml::Value::String only; skip table/array)
            for (k, v) in &self.env.extra {
                // Hindari override PATH/EDITOR/VISUAL/LANG yang sudah ditangani
                match k.as_str() {
                    "EDITOR" | "VISUAL" | "LANG" | "PATH" => {}
                    _ => {
                        if let toml::Value::String(s) = v {
                            std::env::set_var(k, s);
                        }
                    }
                }
            }
        }
    }

    /// Resolve alias: jika `cmd` cocok dengan alias, kembalikan string gantinya.
    /// Caller harus re-tokenize string alias tersebut.
    pub fn resolve_alias<'a>(&'a self, cmd: &str) -> Option<&'a str> {
        self.aliases.get(cmd).map(|s| s.as_str())
    }

    /// Resolve dir_alias: jika path dimulai dengan ~<name>, expand ke target.
    /// Contoh: ~crush/src → ~/Templates/TUI/crush/src
    pub fn resolve_dir_alias(&self, path: &str) -> String {
        if path.starts_with('~') {
            let rest = &path[1..]; // hapus ~
            for (alias, target) in &self.dir_aliases {
                if rest == alias || rest.starts_with(&format!("{}/", alias)) {
                    let expanded_target = expand_tilde(target);
                    let suffix = &rest[alias.len()..]; // "" atau "/..."
                    return format!("{}{}", expanded_target, suffix);
                }
            }
        }
        path.to_string()
    }

    /// Kembalikan path absolut untuk dir_alias tertentu, atau None.
    pub fn dir_alias_path(&self, name: &str) -> Option<PathBuf> {
        self.dir_aliases
            .get(name)
            .map(|p| PathBuf::from(expand_tilde(p)))
    }

    /// Kembalikan daftar semua alias untuk ditampilkan / completion.
    pub fn alias_names(&self) -> Vec<&str> {
        self.aliases.keys().map(|k| k.as_str()).collect()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper
// ─────────────────────────────────────────────────────────────────────────────

pub fn expand_tilde(path: &str) -> String {
    if path == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return home;
        }
    } else if path.starts_with("~/") {
        if let Ok(home) = std::env::var("HOME") {
            return path.replacen("~/", &format!("{}/", home), 1);
        }
    }
    path.to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Build rustyline Config dari ShellConfig.history
// ─────────────────────────────────────────────────────────────────────────────

/// Buat rustyline::Config menggunakan nilai dari [history] section.
/// Dipanggil dari main.rs menggantikan history::build_rl_config().
pub fn build_rl_config(cfg: &ShellConfig) -> rustyline::Config {
    let h = &cfg.history;

    let max = h.max_entries.min(65535) as usize; // rustyline max ~65535

    rustyline::Config::builder()
        .max_history_size(max)
        .expect("invalid history size")
        .history_ignore_space(h.ignore_space)
        .history_ignore_dups(h.ignore_dups)
        .expect("invalid history_ignore_dups")
        .completion_type(rustyline::CompletionType::Circular)
        .edit_mode(rustyline::EditMode::Emacs)
        .build()
}
