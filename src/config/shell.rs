// config/shell.rs - Main shell configuration

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
    pub commands: Vec<String>,
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct EnvVal(pub Option<String>);

impl<'de> serde::Deserialize<'de> for EnvVal {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        use serde::de::{IgnoredAny, MapAccess, SeqAccess, Visitor};
        use std::fmt;

        struct V;
        impl<'de> Visitor<'de> for V {
            type Value = EnvVal;
            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                write!(f, "any TOML value")
            }
            fn visit_str<E: serde::de::Error>(self, v: &str) -> Result<EnvVal, E> {
                Ok(EnvVal(Some(v.to_owned())))
            }
            fn visit_string<E: serde::de::Error>(self, v: String) -> Result<EnvVal, E> {
                Ok(EnvVal(Some(v)))
            }
            fn visit_i64<E: serde::de::Error>(self, v: i64) -> Result<EnvVal, E> {
                Ok(EnvVal(Some(v.to_string())))
            }
            fn visit_u64<E: serde::de::Error>(self, v: u64) -> Result<EnvVal, E> {
                Ok(EnvVal(Some(v.to_string())))
            }
            fn visit_f64<E: serde::de::Error>(self, v: f64) -> Result<EnvVal, E> {
                Ok(EnvVal(Some(v.to_string())))
            }
            fn visit_bool<E: serde::de::Error>(self, v: bool) -> Result<EnvVal, E> {
                Ok(EnvVal(Some(v.to_string())))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut m: A) -> Result<EnvVal, A::Error> {
                while m.next_entry::<IgnoredAny, IgnoredAny>()?.is_some() {}
                Ok(EnvVal(None))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut s: A) -> Result<EnvVal, A::Error> {
                while s.next_element::<IgnoredAny>()?.is_some() {}
                Ok(EnvVal(None))
            }
        }
        d.deserialize_any(V)
    }
}

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct EnvSection {
    #[serde(rename = "EDITOR")]
    pub editor: Option<String>,

    #[serde(rename = "VISUAL")]
    pub visual: Option<String>,

    #[serde(rename = "LANG")]
    pub lang: Option<String>,

    #[serde(rename = "PATH")]
    pub path_extra: Vec<String>,

    #[serde(flatten)]
    pub extra: HashMap<String, EnvVal>,
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
    pub on_cd:       String,
    pub on_clear:    String,
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

    pub aliases:     HashMap<String, String>,

    pub bindings:    HashMap<String, String>,

    pub completion:  CompletionSection,

    pub dir_aliases: HashMap<String, String>,

    pub hooks:       HooksSection,
    pub history:     HistorySection,
    pub theme:       ThemeSection,

    #[serde(skip)]
    pub functions:   WrapperConfig,

    pub prompt:      crate::config::prompt::PromptConfig,
}

// ─────────────────────────────────────────────────────────────────────────────
// Loading
// ─────────────────────────────────────────────────────────────────────────────

impl ShellConfig {
    pub fn load_from(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;

        let mut cfg: ShellConfig = basic_toml::from_str(&content)
            .map_err(|e| anyhow::anyhow!(e))?;

        cfg.functions = WrapperConfig::from_str(&content);

        Ok(cfg)
    }

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

    pub fn apply_env(&self) {
        unsafe {
            if let Some(ref v) = self.env.editor {
                std::env::set_var("EDITOR", v);
            }
            if let Some(ref v) = self.env.visual {
                std::env::set_var("VISUAL", v);
            }
            if let Some(ref v) = self.env.lang {
                std::env::set_var("LANG", v);
            }

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

            for (k, v) in &self.env.extra {
                match k.as_str() {
                    "EDITOR" | "VISUAL" | "LANG" | "PATH" => {}
                    _ => {
                        if let Some(s) = &v.0 {
                            std::env::set_var(k, s);
                        }
                    }
                }
            }
        }
    }

    pub fn resolve_alias<'a>(&'a self, cmd: &str) -> Option<&'a str> {
        self.aliases.get(cmd).map(|s| s.as_str())
    }

    pub fn resolve_dir_alias(&self, path: &str) -> String {
        if path.starts_with('~') {
            let rest = &path[1..];
            for (alias, target) in &self.dir_aliases {
                if rest == alias || rest.starts_with(&format!("{}/", alias)) {
                    let expanded_target = expand_tilde(target);
                    let suffix = &rest[alias.len()..];
                    return format!("{}{}", expanded_target, suffix);
                }
            }
        }
        path.to_string()
    }

    pub fn dir_alias_path(&self, name: &str) -> Option<PathBuf> {
        self.dir_aliases
            .get(name)
            .map(|p| PathBuf::from(expand_tilde(p)))
    }

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
// Build rustyline Config
// ─────────────────────────────────────────────────────────────────────────────

pub fn build_rl_config(cfg: &ShellConfig) -> rustyline::Config {
    let h = &cfg.history;

    let max = h.max_entries.min(65535) as usize;

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
