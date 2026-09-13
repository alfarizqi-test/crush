// state.rs - Shared application state (config + runtime)

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

    pub fn config(&self) -> std::sync::RwLockReadGuard<'_, ShellConfig> {
        self.inner.read().unwrap()
    }

    #[allow(dead_code)]
    pub fn reload(&self, cfg: ShellConfig) {
        *self.inner.write().unwrap() = cfg;
    }
}
