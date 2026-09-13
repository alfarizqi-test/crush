// config/prompt.rs - Tipe data konfigurasi prompt
// (Renderer dipindahkan ke ui/renderer.rs dan ui/prompt.rs)

use serde::Deserialize;
use std::collections::HashMap;

// ─────────────────────────────────────────────────────────────────────────────
// Sub-section structs
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct CharacterSection {
    pub success_symbol: String,
    pub error_symbol:   String,
}

impl Default for CharacterSection {
    fn default() -> Self {
        Self {
            success_symbol: "\x1b[1;32m❯\x1b[0m ".into(),
            error_symbol:   "\x1b[1;31m❯\x1b[0m ".into(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct DirectorySection {
    pub home_symbol:       String,
    pub read_only:         String,
    pub style:             String,
    pub truncation_length: usize,
    pub truncation_symbol: String,
    pub format:            String,
    pub substitutions:     HashMap<String, String>,
}

impl Default for DirectorySection {
    fn default() -> Self {
        Self {
            home_symbol:       "~".into(),
            read_only:         " 󰌾".into(),
            style:             "bold blue".into(),
            truncation_length: 3,
            truncation_symbol: ".../".into(),
            format:            "[$path]($style) ".into(),
            substitutions:     HashMap::new(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct CmdDurationSection {
    pub min_time: u64,
    pub format:   String,
}

impl Default for CmdDurationSection {
    fn default() -> Self {
        Self {
            min_time: 2000,
            format:   "took [$duration]($style) ".into(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct GitBranchSection {
    pub style:             String,
    pub symbol:            String,
    pub truncation_length: usize,
    pub truncation_symbol: String,
    pub format:            String,
}

impl Default for GitBranchSection {
    fn default() -> Self {
        Self {
            style:             "bold purple".into(),
            symbol:            " ".into(),
            truncation_length: usize::MAX,
            truncation_symbol: "…".into(),
            format:            "on [$symbol$branch]($style) ".into(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct GitStatusSection {
    pub conflicted: String,
    pub ahead:      String,
    pub behind:     String,
    pub diverged:   String,
    pub untracked:  String,
    pub stashed:    String,
    pub modified:   String,
    pub staged:     String,
    pub renamed:    String,
    pub deleted:    String,
}

impl Default for GitStatusSection {
    fn default() -> Self {
        Self {
            conflicted: "=".into(),
            ahead:      "⇡".into(),
            behind:     "⇣".into(),
            diverged:   "⇕".into(),
            untracked:  "?".into(),
            stashed:    "$".into(),
            modified:   "!".into(),
            staged:     "+".into(),
            renamed:    "»".into(),
            deleted:    "✘".into(),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct UsernameSection {
    pub style_user:  String,
    pub style_root:  String,
    pub format:      String,
    pub show_always: bool,
    pub disabled:    bool,
}

impl Default for UsernameSection {
    fn default() -> Self {
        Self {
            style_user:  "bold yellow".into(),
            style_root:  "bold red".into(),
            format:      "[$user]($style) ".into(),
            show_always: false,
            disabled:    false,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct HostnameSection {
    pub ssh_only: bool,
    pub format:   String,
    pub trim_at:  String,
    pub disabled: bool,
}

impl Default for HostnameSection {
    fn default() -> Self {
        Self {
            ssh_only: true,
            format:   "on [$hostname]($style) ".into(),
            trim_at:  ".".into(),
            disabled: false,
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Root prompt config
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone)]
#[serde(default)]
pub struct PromptConfig {
    pub format:       String,

    pub character:    CharacterSection,
    pub directory:    DirectorySection,
    pub cmd_duration: CmdDurationSection,
    pub git_branch:   GitBranchSection,
    pub git_status:   GitStatusSection,
    pub username:     UsernameSection,
    pub hostname:     HostnameSection,
}

impl Default for PromptConfig {
    fn default() -> Self {
        Self {
            format:       "$directory$git_branch\n$character".into(),
            character:    Default::default(),
            directory:    Default::default(),
            cmd_duration: Default::default(),
            git_branch:   Default::default(),
            git_status:   Default::default(),
            username:     Default::default(),
            hostname:     Default::default(),
        }
    }
}
