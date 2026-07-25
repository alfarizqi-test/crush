// state.rs — Shared application state (config + runtime)
//
// ShellConfig dimuat sekali di main() dan diteruskan via AppState
// ke executor, completion, dan highlighter.
//
// Menggunakan Arc<RwLock<>> agar bisa dibagikan lintas thread (job reaper),
// tapi dalam REPL single-thread cukup clone atau borrow langsung.

use std::sync::{Arc, RwLock};
use crate::config::ShellConfig;

#[allow(dead_code)]
pub struct AppState {
    inner: Arc<RwLock<ShellConfig>>,
}

#[allow(dead_code)]
impl AppState {
    pub fn new(cfg: ShellConfig) -> Self {
        Self { inner: Arc::new(RwLock::new(cfg)) }
    }

    /// Baca config (non-blocking read)
    pub fn config(&self) -> std::sync::RwLockReadGuard<'_, ShellConfig> {
        self.inner.read().unwrap()
    }

    /// Ganti config (misal: `config reload`)
    #[allow(dead_code)]
    pub fn reload(&self, cfg: ShellConfig) {
        *self.inner.write().unwrap() = cfg;
    }
}
