// history.rs - Command history management

use std::path::PathBuf;
use directories::ProjectDirs;
use rustyline::history::FileHistory;

pub fn history_path() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("", "", "crush") {
        let config_dir = proj_dirs.config_dir().to_path_buf();
        std::fs::create_dir_all(&config_dir).ok();
        config_dir.join("history.txt")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
        let fallback = PathBuf::from(home).join(".config/crush");
        std::fs::create_dir_all(&fallback).ok();
        fallback.join("history.txt")
    }
}

#[allow(dead_code)]
pub fn build_rl_config() -> rustyline::Config {
    rustyline::Config::builder()
        .max_history_size(1000)
        .expect("invalid history size")
        .history_ignore_space(true)
        .history_ignore_dups(true)
        .expect("invalid history_ignore_dups config")
        .completion_type(rustyline::CompletionType::Circular)
        .edit_mode(rustyline::EditMode::Emacs)
        .build()
}

pub fn load_history(rl: &mut rustyline::Editor<crate::completion::CrushCompleter, FileHistory>) {
    let path = history_path();
    if path.exists() {
        if let Err(e) = rl.load_history(&path) {
            eprintln!("crush: failed to load history: {}", e);
        }
    }
}

pub fn save_history(rl: &mut rustyline::Editor<crate::completion::CrushCompleter, FileHistory>) {
    let path = history_path();
    if let Err(e) = rl.save_history(&path) {
        eprintln!("crush: failed to save history: {}", e);
    }
}
