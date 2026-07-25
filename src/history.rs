// history.rs — Manajemen riwayat perintah untuk crush shell
// Menyimpan riwayat di ~/.config/crush/history.txt (atau sesuai XDG/platform)

use std::path::PathBuf;
use directories::ProjectDirs;
use rustyline::history::FileHistory;

/// Dapatkan path ke file history: ~/.config/crush/history.txt
pub fn history_path() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("", "", "crush") {
        let config_dir = proj_dirs.config_dir().to_path_buf();
        // Pastikan direktori ada
        std::fs::create_dir_all(&config_dir).ok();
        config_dir.join("history.txt")
    } else {
        // Fallback ke ~/.config/crush/history.txt secara manual
        let home = std::env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
        let fallback = PathBuf::from(home).join(".config/crush");
        std::fs::create_dir_all(&fallback).ok();
        fallback.join("history.txt")
    }
}

/// Buat rustyline Config — Deprecated: gunakan config::shell::build_rl_config().
#[allow(dead_code)]
pub fn build_rl_config() -> rustyline::Config {
    rustyline::Config::builder()
        .max_history_size(1000)                                   // Maksimal simpan 1000 perintah
        .expect("invalid history size")
        .history_ignore_space(true)                               // Jangan simpan perintah berawali spasi
        .history_ignore_dups(true)                                // Jangan simpan perintah duplikat berturut-turut
        .expect("invalid history_ignore_dups config")
        .completion_type(rustyline::CompletionType::Circular)    // Tab isi top match, Tab lagi = cycle
        .edit_mode(rustyline::EditMode::Emacs)                    // Mode standar (Ctrl+A/E, dll)
        .build()
}

/// Muat FileHistory dari disk; buat file baru jika belum ada.
pub fn load_history(rl: &mut rustyline::Editor<crate::completion::CrushCompleter, FileHistory>) {
    let path = history_path();
    if path.exists() {
        if let Err(e) = rl.load_history(&path) {
            eprintln!("crush: gagal memuat history: {}", e);
        }
    }
}

/// Simpan FileHistory ke disk.
pub fn save_history(rl: &mut rustyline::Editor<crate::completion::CrushCompleter, FileHistory>) {
    let path = history_path();
    if let Err(e) = rl.save_history(&path) {
        eprintln!("crush: gagal menyimpan history: {}", e);
    }
}
