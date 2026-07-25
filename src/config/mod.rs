// config/mod.rs — Config module entry point
//
// Tiga sub-modul:
//   shell   — konfigurasi utama shell (env, aliases, history, theme, hooks, ...)
//   wrapper — fungsi/wrapper shell yang didefinisikan user (functions.y, dst)
//   prompt  — mesin prompt builtin bergaya starship (sesi berikutnya)

#[allow(dead_code)]
pub mod shell;
#[allow(dead_code)]
pub mod wrapper;
#[allow(dead_code)]
pub mod prompt;

pub use shell::ShellConfig;

use std::path::PathBuf;
use directories::ProjectDirs;

/// Path ke config.toml yang sesungguhnya: ~/.config/crush/config.toml
pub fn config_path() -> PathBuf {
    if let Some(proj) = ProjectDirs::from("", "", "crush") {
        let dir = proj.config_dir().to_path_buf();
        std::fs::create_dir_all(&dir).ok();
        dir.join("config.toml")
    } else {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
        PathBuf::from(home).join(".config/crush/config.toml")
    }
}

/// Path ke config-example.toml di direktori source (untuk debugging).
/// Hanya ada saat development; pada rilis pakai config_path().
pub fn example_config_path() -> Option<PathBuf> {
    // Cari relatif terhadap binary saat dev (target/debug/crush → src/)
    let exe = std::env::current_exe().ok()?;
    let workspace = exe.ancestors().find(|p| p.join("Cargo.toml").exists())?;
    let example = workspace.join("src/config-example.toml");
    if example.exists() { Some(example) } else { None }
}
