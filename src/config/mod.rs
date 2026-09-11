// config/mod.rs - Config module entry point

#[allow(dead_code)]
pub mod shell;
#[allow(dead_code)]
pub mod wrapper;
#[allow(dead_code)]
pub mod prompt;

pub use shell::ShellConfig;

use std::path::PathBuf;
use directories::ProjectDirs;

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

pub fn example_config_path() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let workspace = exe.ancestors().find(|p| p.join("Cargo.toml").exists())?;
    let example = workspace.join("src/config-example.toml");
    if example.exists() { Some(example) } else { None }
}
