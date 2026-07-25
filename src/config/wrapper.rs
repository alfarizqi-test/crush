// config/wrapper.rs — Shell function/wrapper definitions
//
// Membaca section [functions.*] dari config.toml.
// Setiap fungsi punya:
//   description = "..."
//   body        = """multi-line crush script"""
//
// STATUS: Stub — akan diimplementasikan sesi berikutnya.
// Yang sudah disiapkan:
//   - Struct WrapperFunction dan WrapperConfig
//   - load() dari ShellConfig (sections [functions.*])
//   - Trait runner interface (belum diimplementasikan)
//
// Rencana implementasi:
//   1. Parse body menjadi Vec<Statement> menggunakan mini-parser crush
//   2. Ekspansi $args, $1, $2, dst
//   3. Dukungan: let, if, end, perintah biasa
//   4. Integrasi dengan executor::execute_line()

use std::collections::HashMap;
use serde::Deserialize;

// ─────────────────────────────────────────────────────────────────────────────
// Struct
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct WrapperFunction {
    pub description: String,
    /// Multi-line body script (crush dialect)
    pub body:        String,
}

/// Top-level container untuk semua [functions.*] sections
#[derive(Debug, Clone, Default)]
pub struct WrapperConfig {
    pub functions: HashMap<String, WrapperFunction>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Loading dari raw TOML value
// ─────────────────────────────────────────────────────────────────────────────

impl WrapperConfig {
    /// Parse [functions] dari raw toml::Table yang sudah dibaca.
    /// Dipanggil oleh config::shell::ShellConfig::load() setelah parsing utama.
    pub fn from_toml(table: &toml::Table) -> Self {
        let mut functions = HashMap::new();

        if let Some(toml::Value::Table(funcs)) = table.get("functions") {
            for (name, val) in funcs {
                if let Ok(func) = val.clone().try_into::<WrapperFunction>() {
                    functions.insert(name.clone(), func);
                }
            }
        }

        Self { functions }
    }

    /// Kembalikan body function jika nama cocok, atau None.
    pub fn get(&self, name: &str) -> Option<&WrapperFunction> {
        self.functions.get(name)
    }

    /// Daftar semua nama function yang terdefinisi.
    pub fn names(&self) -> Vec<&str> {
        self.functions.keys().map(|k| k.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// TODO (sesi berikutnya): Runner
// ─────────────────────────────────────────────────────────────────────────────
//
// pub fn execute_wrapper(
//     func: &WrapperFunction,
//     args: &[&str],
//     ctx:  &mut crate::executor::ExecContext<'_>,
// ) -> i32 {
//     // 1. Tokenize func.body baris per baris
//     // 2. Expand $args → args.join(" "), $1 → args[0], dst
//     // 3. Kirim tiap baris ke executor::execute_line()
//     0
// }
