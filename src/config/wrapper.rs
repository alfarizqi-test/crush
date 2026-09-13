// config/wrapper.rs — Shell function/wrapper definitions & runner
//
// Membaca section [functions.*] dari config.toml.
// Setiap fungsi punya:
//   description = "..."
//   body        = """multi-line crush script"""
//
// Fitur interpreter mini crush:
//   • let var = value            — assign variable lokal
//   • let var = $(cmd arg...)    — command substitution
//   • if COND / end              — conditional block (cond = POSIX test expr)
//   • $args                      — semua argumen gabung spasi
//   • $0..$9                     — argumen posisional
//   • $var                       — variable lokal
//   • Baris biasa → executor::execute_line
//   • Komentar (#)

use std::borrow::Cow;
use std::collections::HashMap;
use std::process::Command;
use serde::Deserialize;

// ─────────────────────────────────────────────────────────────────────────────
// Struct
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, Clone, Default)]
#[serde(default)]
pub struct WrapperFunction {
    pub description: String,
    /// Multi-line body script (crush dialect)
    pub body: String,
}

/// Top-level container untuk semua [functions.*] sections
#[derive(Debug, Clone, Default)]
pub struct WrapperConfig {
    pub functions: HashMap<String, WrapperFunction>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Loading dari raw TOML value
// ─────────────────────────────────────────────────────────────────────────────

/// Intermediate struct untuk deserialize raw config yang berisi [functions]
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawConfig {
    functions: HashMap<String, WrapperFunction>,
}

impl WrapperConfig {
    /// Parse [functions] dari raw TOML string.
    pub fn from_str(content: &str) -> Self {
        let raw: RawConfig = basic_toml::from_str(content).unwrap_or_default();
        Self { functions: raw.functions }
    }

    /// Kembalikan function jika nama cocok, atau None.
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
// Runner — interpreter mini untuk body script
// ─────────────────────────────────────────────────────────────────────────────

/// Jalankan sebuah wrapper function dengan argumen yang diberikan.
/// Mengembalikan exit code.
pub fn execute_wrapper(
    func:  &WrapperFunction,
    args:  &[&str],
    ctx:   &mut crate::core::executor::ExecContext<'_>,
) -> i32 {
    let mut locals: HashMap<String, String> = HashMap::new();

    // $args = semua argumen digabung spasi; $1..$9 = per argumen
    locals.insert("args".into(), args.join(" "));
    for (i, a) in args.iter().enumerate().take(9) {
        locals.insert(i.to_string(), a.to_string());
    }

    run_block(func.body.as_str(), &mut locals, args, ctx)
}

/// Jalankan satu blok teks (bisa body utama atau isi if-block).
/// locals di-share (mutable) agar `let` di dalam if visible setelah block.
fn run_block(
    body:   &str,
    locals: &mut HashMap<String, String>,
    args:   &[&str],
    ctx:    &mut crate::core::executor::ExecContext<'_>,
) -> i32 {
    let lines: Vec<&str> = body.lines().collect();
    let mut pc = 0usize;
    let mut last_exit = 0i32;

    while pc < lines.len() {
        let raw = lines[pc].trim();
        pc += 1;

        if raw.is_empty() || raw.starts_with('#') { continue; }

        // ── let var = value | let var = $(cmd) ────────────────────────────────
        if let Some(rest) = raw.strip_prefix("let ") {
            if let Some(eq) = rest.find('=') {
                let var = rest[..eq].trim().to_string();
                let val_raw = rest[eq + 1..].trim();

                let value = if let Some(cmd_expr) = val_raw
                    .strip_prefix("$(")
                    .and_then(|s| s.strip_suffix(')'))
                {
                    run_cmd_substitution(cmd_expr, locals, args)
                } else {
                    // Cow: hanya alokasi jika ada variabel yang perlu diexpand
                    expand_vars(val_raw, locals, args).into_owned()
                };
                locals.insert(var, value);
            }
            continue;
        }

        // ── if COND ────────────────────────────────────────────────────────────
        if let Some(cond_raw) = raw.strip_prefix("if ") {
            // expand_vars mengembalikan Cow: borrowed jika tidak ada '$', owned jika ada.
            let cond_expanded = expand_vars(cond_raw, locals, args);
            let cond_result   = evaluate_condition(&cond_expanded);

            // Scan maju untuk menemukan matching "end"
            // Kumpulkan baris ANTARA "if" dan "end" (tidak termasuk keduanya)
            let block_start = pc;           // baris pertama setelah "if ..."
            let mut depth   = 1usize;
            while pc < lines.len() {
                let l = lines[pc].trim();
                pc += 1;                    // pc sekarang menunjuk SETELAH baris ini
                if l.starts_with("if ") { depth += 1; }
                if l == "end" {
                    depth -= 1;
                    if depth == 0 { break; }
                }
            }
            // pc sekarang = index tepat SETELAH "end" yang cocok
            // baris "end" ada di index pc-1, jadi block = [block_start .. pc-1)
            let block_end = pc - 1;        // index baris "end" (eksklusif)
            let block_body = lines[block_start..block_end].join("\n");

            if cond_result {
                last_exit = run_block(&block_body, locals, args, ctx);
            }
            continue;
        }

        // ── end (sisa yang lolos — seharusnya tidak terjadi) ──────────────────
        if raw == "end" { continue; }

        // ── Perintah biasa ─────────────────────────────────────────────────────
        // Cow: jika baris tidak mengandung '$', tidak ada alokasi sama sekali.
        let expanded = expand_vars(raw, locals, args);

        // Guard khusus: skip `cd ""` agar tidak error "No such file or directory"
        // Bisa terjadi bila $cwd kosong (yazi quit tanpa navigasi)
        {
            let trimmed = expanded.trim();
            if trimmed == "cd" || trimmed.starts_with("cd ") {
                let target = trimmed.trim_start_matches("cd").trim().trim_matches('"').trim_matches('\'');
                if target.is_empty() {
                    // $cwd kosong — abaikan
                    continue;
                }
            }
        }

        last_exit = run_line(&expanded, ctx);
    }

    last_exit
}

// ─────────────────────────────────────────────────────────────────────────────
// Variable expansion
// ─────────────────────────────────────────────────────────────────────────────

/// Expand $args, $0..$9, $var_name, $VAR_NAME (env) dalam sebuah string.
///
/// Zero-copy path: jika `s` tidak mengandung `$` sama sekali, dikembalikan
/// langsung sebagai `Cow::Borrowed` tanpa alokasi heap apapun.
/// Hanya ketika ada variabel yang perlu diexpand barulah `Cow::Owned` dialokasikan.
pub fn expand_vars<'a>(
    s:      &'a str,
    locals: &HashMap<String, String>,
    _args:  &[&str],
) -> Cow<'a, str> {
    // Fast-path: tidak ada '$' → tidak perlu alokasi sama sekali.
    if !s.contains('$') {
        return Cow::Borrowed(s);
    }

    let mut out = String::with_capacity(s.len() + 16);
    let mut chars = s.char_indices().peekable();

    while let Some((_, ch)) = chars.next() {
        if ch != '$' {
            out.push(ch);
            continue;
        }

        // Kumpulkan nama variabel (alfanumerik + _)
        let mut name = String::new();
        while let Some(&(_, c)) = chars.peek() {
            if c.is_alphanumeric() || c == '_' {
                name.push(c);
                chars.next();
            } else {
                break;
            }
        }

        if name.is_empty() {
            out.push('$');
            continue;
        }

        // Prioritas: locals → env
        if let Some(v) = locals.get(&name) {
            out.push_str(v);
        } else if let Ok(v) = std::env::var(&name) {
            out.push_str(&v);
        }
        // Variabel tidak ditemukan → ekspansi jadi string kosong (perilaku POSIX)
    }
    Cow::Owned(out)
}

// ─────────────────────────────────────────────────────────────────────────────
// Command substitution: $(cmd arg...)
// ─────────────────────────────────────────────────────────────────────────────

fn run_cmd_substitution(
    cmd_expr: &str,
    locals:   &HashMap<String, String>,
    _args:    &[&str],
) -> String {
    // Expand variables (lokals + env) di dalam cmd_expr.
    // expand_vars mengembalikan Cow: tidak alokasi jika cmd_expr tidak mengandung '$'.
    let expanded = expand_vars(cmd_expr, locals, &[]);
    let tokens = match shlex::split(&expanded) {
        Some(t) if !t.is_empty() => t,
        _ => return String::new(),
    };

    let output = Command::new(&tokens[0])
        .args(&tokens[1..])
        .output();

    match output {
        Ok(o) => String::from_utf8_lossy(&o.stdout).trim_end().to_string(),
        Err(_) => String::new(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Condition evaluator — native POSIX test (tanpa sh -c)
// ─────────────────────────────────────────────────────────────────────────────

/// Evaluasi kondisi untuk blok `if` secara NATIVE — tidak pernah memanggil shell
/// eksternal, sehingga tidak ada risiko command injection.
///
/// Mendukung subset POSIX test:
///   [ -z "$a" ]       — string kosong
///   [ -n "$a" ]       — string tidak kosong
///   [ -f "$path" ]    — regular file ada
///   [ -d "$path" ]    — direktori ada
///   [ -e "$path" ]    — path (file/dir) ada
///   [ "$a" = "$b" ]   — string equality
///   [ "$a" != "$b" ]  — string inequality
///
/// Kondisi yang tidak dikenali dikembalikan `false` (bukan di-forward ke sh).
fn evaluate_condition(cond: &str) -> bool {
    let inner = cond.trim();

    // Strip [ ... ] wrapper jika ada
    let inner = if inner.starts_with('[') && inner.ends_with(']') {
        inner[1..inner.len() - 1].trim()
    } else {
        inner
    };

    // Tokenize sederhana: pisah berdasarkan whitespace.
    // Catatan: setelah expand_vars dipanggil di atas, nilai variabel sudah
    // disubstitusi sehingga token di sini sudah siap dipakai langsung.
    let tokens: Vec<&str> = inner.split_whitespace().collect();

    match tokens.as_slice() {
        // [ -z "str" ] — true jika string kosong
        ["-z", val] => strip_quotes(val).is_empty(),
        // [ -n "str" ] — true jika string tidak kosong
        ["-n", val] => !strip_quotes(val).is_empty(),
        // [ -f "path" ] — true jika path adalah regular file
        ["-f", path] => std::path::Path::new(strip_quotes(path)).is_file(),
        // [ -d "path" ] — true jika path adalah direktori
        ["-d", path] => std::path::Path::new(strip_quotes(path)).is_dir(),
        // [ -e "path" ] — true jika path ada (file atau dir)
        ["-e", path] => std::path::Path::new(strip_quotes(path)).exists(),
        // [ "a" = "b" ] — string equality
        [a, "=", b]  => strip_quotes(a) == strip_quotes(b),
        // [ "a" != "b" ] — string inequality
        [a, "!=", b] => strip_quotes(a) != strip_quotes(b),
        // Kondisi tidak dikenal: kembalikan false secara aman.
        // KEAMANAN: fallback sh -c DIHAPUS untuk mencegah command injection.
        _ => {
            eprintln!("crush: wrapper: kondisi tidak didukung: {:?}", inner);
            false
        }
    }
}

fn strip_quotes(s: &str) -> &str {
    let s = s.trim();
    if (s.starts_with('"') && s.ends_with('"'))
        || (s.starts_with('\'') && s.ends_with('\''))
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Line runner — kirim baris ke executor
// ─────────────────────────────────────────────────────────────────────────────

/// Jalankan satu baris perintah di dalam executor context.
fn run_line(line: &str, ctx: &mut crate::core::executor::ExecContext<'_>) -> i32 {
    let line = line.trim();
    if line.is_empty() || line.starts_with('#') { return 0; }

    let tokens_raw = match shlex::split(line) {
        Some(t) => t,
        None => {
            eprintln!("crush: wrapper: bad quote in: {}", line);
            return 1;
        }
    };
    if tokens_raw.is_empty() { return 0; }

    let tokens_owned = crate::core::executor::tokenize_operators(tokens_raw);
    let tokens: Vec<&str> = tokens_owned.iter().map(|s| s.as_str()).collect();
    let units = crate::core::executor::parse_input(&tokens);
    if units.is_empty() { return 0; }

    crate::core::executor::execute_line(ctx, units);
    0  // execute_line tidak return exit code langsung; bisa diperbaiki nanti
}
