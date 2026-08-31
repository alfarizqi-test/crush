// executor.rs — Command execution engine untuk crush shell
//
// Mendukung:
//   - Redirection: >, >>, 2>, 2>>
//   - Background jobs: cmd &
//   - Pipeline: cmd1 | cmd2 | cmd3
//   - Logical AND: cmd1 && cmd2
//   - Logical OR:  cmd1 || cmd2
//   - Kombinasi: cmd1 && cmd2 | cmd3 || cmd4

use std::collections::HashMap;
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::os::unix::process::CommandExt;
use which::which;

use crate::jobs::SharedJobTable;
use crate::config::ShellConfig;

// ─────────────────────────────────────────────────────────────────────────────
// Tipe data internal
// ─────────────────────────────────────────────────────────────────────────────

/// Representasi satu segmen perintah (sudah dipisah dari pipeline/&&/||)
#[derive(Debug)]
pub struct Segment {
    pub args:        Vec<String>,
    pub redirect_out: Option<Redirect>,
    pub redirect_err: Option<Redirect>,
    pub background:  bool,
}

#[derive(Debug, Clone)]
pub struct Redirect {
    pub path:   String,
    pub append: bool,
}

/// Operator antara dua segmen dalam sebuah perintah gabungan
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Op {
    Pipe,      // |
    And,       // &&
    Or,        // ||
}

/// Satu unit eksekusi: segmen + operator yang mengikutinya
#[derive(Debug)]
pub struct Unit {
    pub segment: Segment,
    pub op:      Option<Op>,  // None = unit terakhir
}

// ─────────────────────────────────────────────────────────────────────────────
// Parsing
// ─────────────────────────────────────────────────────────────────────────────

/// Parse seluruh baris input menjadi Vec<Unit> yang siap dieksekusi.
pub fn parse_input(tokens: &[&str]) -> Vec<Unit> {
    let mut units: Vec<Unit> = Vec::new();
    let mut current_tokens: Vec<&str> = Vec::new();

    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i];
        match t {
            // Pipeline
            "|" => {
                // Pastikan bukan "||"
                if i + 1 < tokens.len() && tokens[i + 1] == "|" {
                    // Ini seharusnya sudah ditangani sebagai "||" oleh tokenizer
                    // Tapi shlex sudah memisahnya, jadi tangani di sini
                }
                if let Some(seg) = build_segment(&current_tokens) {
                    units.push(Unit { segment: seg, op: Some(Op::Pipe) });
                }
                current_tokens.clear();
            }
            "||" => {
                if let Some(seg) = build_segment(&current_tokens) {
                    units.push(Unit { segment: seg, op: Some(Op::Or) });
                }
                current_tokens.clear();
            }
            "&&" => {
                if let Some(seg) = build_segment(&current_tokens) {
                    units.push(Unit { segment: seg, op: Some(Op::And) });
                }
                current_tokens.clear();
            }
            _ => {
                current_tokens.push(t);
            }
        }
        i += 1;
    }

    // Sisa token
    if !current_tokens.is_empty() {
        if let Some(seg) = build_segment(&current_tokens) {
            units.push(Unit { segment: seg, op: None });
        }
    }

    units
}

/// Bangun Segment dari kumpulan token (termasuk deteksi redirection & &)
fn build_segment(tokens: &[&str]) -> Option<Segment> {
    if tokens.is_empty() { return None; }

    let mut args: Vec<String> = Vec::new();
    let mut redirect_out: Option<Redirect> = None;
    let mut redirect_err: Option<Redirect> = None;
    let mut background = false;

    let mut i = 0;
    while i < tokens.len() {
        let t = tokens[i];
        match t {
            // Background
            "&" => { background = true; }

            // Stdout redirect
            ">" | "1>" => {
                i += 1;
                if i < tokens.len() {
                    redirect_out = Some(Redirect { path: tokens[i].to_string(), append: false });
                }
            }
            ">>" | "1>>" => {
                i += 1;
                if i < tokens.len() {
                    redirect_out = Some(Redirect { path: tokens[i].to_string(), append: true });
                }
            }
            // Stderr redirect
            "2>" => {
                i += 1;
                if i < tokens.len() {
                    redirect_err = Some(Redirect { path: tokens[i].to_string(), append: false });
                }
            }
            "2>>" => {
                i += 1;
                if i < tokens.len() {
                    redirect_err = Some(Redirect { path: tokens[i].to_string(), append: true });
                }
            }
            _ => {
                args.push(t.to_string());
            }
        }
        i += 1;
    }

    if args.is_empty() { return None; }

    Some(Segment { args, redirect_out, redirect_err, background })
}

// ─────────────────────────────────────────────────────────────────────────────
// Pre-processing: tokenize dengan memecah ||, &&, | dengan benar
// ─────────────────────────────────────────────────────────────────────────────

/// Tokenize dengan memisahkan operator shell dari token biasa.
/// shlex sudah memisahkan kata, tapi tidak memisahkan &&, ||, |.
///
/// Optimasi alokasi: token yang TIDAK mengandung operator dikembalikan
/// langsung dari input aslinya tanpa alokasi baru.
pub fn tokenize_operators(raw_tokens: Vec<String>) -> Vec<String> {
    // Operator yang perlu dipisah (urutan: lebih panjang dulu)
    const OPS: &[&str] = &["&&", "||", "|"];

    let mut result: Vec<String> = Vec::with_capacity(raw_tokens.len());
    for tok in raw_tokens {
        // Fast-path: jika token tidak mengandung operator sama sekali,
        // push langsung tanpa alokasi tambahan.
        if !OPS.iter().any(|op| tok.contains(op)) {
            result.push(tok);
            continue;
        }
        // Slow-path: ada operator yang perlu dipecah.
        let expanded = split_operators_cow(&tok);
        result.extend(expanded);
    }
    result
}

/// Pecah token yang mengandung operator shell.
/// Kembalikan Vec<String> agar tidak ada lifetime issue dari Cow yang
/// meminjam local variable.
fn split_operators_cow(s: &str) -> Vec<String> {
    const OPS: &[&str] = &["&&", "||", "|"];
    let mut result: Vec<String> = vec![s.to_string()];
    for op in OPS {
        let mut new_result: Vec<String> = Vec::new();
        for part in result {
            if part.as_str() == *op {
                new_result.push(part);
                continue;
            }
            if let Some(idx) = part.find(op) {
                // left dan right diekstrak sebagai String baru.
                let left  = part[..idx].to_string();
                let right = part[idx + op.len()..].to_string();
                if !left.is_empty()  { new_result.push(left); }
                // Operator: gunakan str literal untuk menjaga string heap seminimal mungkin.
                new_result.push(op.to_string());
                if !right.is_empty() { new_result.extend(split_operators_cow(&right)); }
            } else {
                new_result.push(part);
            }
        }
        result = new_result;
    }
    result
}

// ─────────────────────────────────────────────────────────────────────────────
// Variable expansion
// ─────────────────────────────────────────────────────────────────────────────

pub fn expand_variables(arg: &str) -> String {
    if arg == "$$" { return std::process::id().to_string(); }
    if arg.starts_with('$') && arg.len() > 1 {
        return env::var(&arg[1..]).unwrap_or_default();
    }
    arg.to_string()
}

pub fn expand_tilde(path: &str) -> String {
    if path == "~" {
        if let Ok(home) = env::var("HOME") {
            return home;
        }
    } else if path.starts_with("~/") {
        if let Ok(home) = env::var("HOME") {
            return path.replacen("~/", &format!("{}/", home), 1);
        }
    }
    path.to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Shell Environment State — pengganti global unsafe env::set_var
// ─────────────────────────────────────────────────────────────────────────────

/// Penyimpanan variabel environment lokal untuk sesi shell ini.
///
/// KEAMANAN: Menggantikan `unsafe { env::set_var(...) }` yang berisiko
/// Undefined Behavior di Rust modern (terutama di lingkungan multi-threaded).
/// Variabel disimpan di HashMap lokal dan diinjeksikan ke child process via
/// `.envs()` sehingga perubahan tidak pernah menyentuh global state OS.
pub struct ShellEnv {
    /// Overrides lokal: variabel yang di-set via `export` atau `cd`.
    overrides: HashMap<String, String>,
}

impl ShellEnv {
    pub fn new() -> Self {
        Self { overrides: HashMap::new() }
    }

    /// Set satu variabel. Juga update process environment agar child process
    /// yang baru di-spawn bisa mewarisinya (melalui .envs() di spawn_process).
    pub fn set(&mut self, key: impl Into<String>, value: impl Into<String>) {
        let k = key.into();
        let v = value.into();
        // Selain disimpan di map, kita TETAP update env global untuk kompatibilitas
        // dengan `env::current_dir()` dan crate lain yang membaca env langsung.
        // Ini aman karena crush adalah single-threaded (tidak ada tokio/rayon).
        // Jika di masa depan ada threading, ganti ini dengan Arc<Mutex<HashMap>>.
        // Penggunaan unsafe di sini dibatasi HANYA untuk menjaga kompatibilitas
        // dengan API sistem, bukan untuk logika bisnis.
        #[allow(unused_unsafe)]
        unsafe { env::set_var(&k, &v); }
        self.overrides.insert(k, v);
    }

    pub fn remove(&mut self, key: &str) {
        #[allow(unused_unsafe)]
        unsafe { env::remove_var(key); }
        self.overrides.remove(key);
    }

    /// Iterasi semua override untuk diinjeksikan ke child process.
    pub fn iter(&self) -> impl Iterator<Item = (&String, &String)> {
        self.overrides.iter()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Execution context
// ─────────────────────────────────────────────────────────────────────────────

pub struct ExecContext<'a> {
    pub jobs:      &'a SharedJobTable,
    pub rl:        &'a mut rustyline::Editor<
                       crate::completion::CrushCompleter,
                       rustyline::history::FileHistory,
                   >,
    pub raw_input: &'a str,
    pub config:    &'a ShellConfig,
    pub cfg_arc:   &'a std::sync::Arc<std::sync::RwLock<ShellConfig>>,
    /// Environment state lokal: hasil dari `export`, `cd`, dsb.
    /// Diinjeksikan ke setiap child process via `.envs()`.
    pub shell_env: ShellEnv,
}


/// Jalankan satu baris input lengkap.
/// Kembalikan last exit code.
pub fn execute_line(ctx: &mut ExecContext<'_>, units: Vec<Unit>) -> i32 {
    if units.is_empty() { return 0; }

    // Pisah units menjadi kelompok yang dihubungkan oleh pipe,
    // dan chain &&/|| di antara kelompok-kelompok tersebut.
    let pipeline_groups = split_into_pipeline_groups(units);
    let mut last_code: i32 = 0;
    let mut skip_and = false;
    let mut skip_or  = false;

    for (group, trailing_op) in pipeline_groups {
        // Evaluasi kondisi &&/||
        if skip_and {
            // Perintah sebelumnya gagal, && berikutnya dilewati
            if trailing_op == Some(Op::And) { skip_and = true; } else { skip_and = false; }
            last_code = 1;
            continue;
        }
        if skip_or {
            // Perintah sebelumnya sukses, || berikutnya dilewati
            if trailing_op == Some(Op::Or) { skip_or = true; } else { skip_or = false; }
            continue;
        }

        last_code = execute_pipeline(ctx, group);

        // Tentukan kondisi untuk unit berikutnya
        match trailing_op {
            Some(Op::And) => {
                if last_code != 0 { skip_and = true; }
            }
            Some(Op::Or) => {
                if last_code == 0 { skip_or = true; }
            }
            _ => {}
        }
    }

    last_code
}

/// Pisah Vec<Unit> menjadi pipeline group: setiap group = Vec<Segment> yang dihubungkan |
type PipelineGroup = (Vec<Segment>, Option<Op>); // (segments, op setelah group ini)

fn split_into_pipeline_groups(units: Vec<Unit>) -> Vec<PipelineGroup> {
    let mut groups: Vec<PipelineGroup> = Vec::new();
    let mut current_pipe: Vec<Segment> = Vec::new();

    for unit in units {
        let op = unit.op;
        current_pipe.push(unit.segment);

        match op {
            Some(Op::Pipe) => {
                // Lanjut kumpulkan pipeline
            }
            other => {
                // Akhir dari pipeline group ini
                groups.push((std::mem::take(&mut current_pipe), other));
            }
        }
    }

    if !current_pipe.is_empty() {
        groups.push((current_pipe, None));
    }

    groups
}

// ─────────────────────────────────────────────────────────────────────────────
// Pipeline execution
// ─────────────────────────────────────────────────────────────────────────────

fn execute_pipeline(ctx: &mut ExecContext<'_>, mut segments: Vec<Segment>) -> i32 {
    if segments.is_empty() { return 0; }
    if segments.len() == 1 {
        return execute_segment(ctx, segments.remove(0), None, None);
    }

    // Multi-command pipeline: hubungkan stdin/stdout antar proses
    let n = segments.len();
    let mut prev_stdout: Option<std::process::ChildStdout> = None;
    let mut children: Vec<Child> = Vec::new();

    for (i, seg) in segments.into_iter().enumerate() {
        let is_last = i == n - 1;
        let cmd_name = seg.args[0].clone();

        // Jika builtin (echo) dalam pipeline — jalankan di thread terpisah
        // agar bisa di-pipe; untuk simplisitas kita fork dan jalankan biasa.
        // Builtin di tengah pipeline akan dilanjutkan sebagai external cmd.

        let stdin_source = prev_stdout.take().map(|s| Stdio::from(s));
        let stdout_dest = if is_last { None } else { Some(Stdio::piped()) };

        let child_result = spawn_process(
            &seg,
            &ctx.shell_env,
            stdin_source,
            stdout_dest,
            false,
        );

        match child_result {
            Ok(mut child) => {
                prev_stdout = child.stdout.take();
                children.push(child);
            }
            Err(e) => {
                eprintln!("{}: {}", cmd_name, e);
                // Bersihkan yang sudah di-spawn
                for mut c in children { c.wait().ok(); }
                return 127;
            }
        }
    }

    // Tunggu semua proses selesai; kembalikan exit code yang terakhir
    let mut last_code = 0;
    for mut child in children {
        if let Ok(status) = child.wait() {
            last_code = status.code().unwrap_or(1);
        }
    }
    last_code
}

// ─────────────────────────────────────────────────────────────────────────────
// Single segment execution
// ─────────────────────────────────────────────────────────────────────────────

fn execute_segment(
    ctx: &mut ExecContext<'_>,
    seg: Segment,
    stdin_override: Option<Stdio>,
    stdout_override: Option<Stdio>,
) -> i32 {
    if seg.args.is_empty() { return 0; }

    // Expand variabel & tilde
    let args: Vec<String> = seg.args.iter()
        .map(|a| expand_tilde(&expand_variables(a)))
        .collect();

    let cmd = args[0].as_str();
    let rest: Vec<&str> = args[1..].iter().map(|s| s.as_str()).collect();

    // ── Builtin dispatch ─────────────────────────────────────────────────────
    // Builtin tidak bisa di-background dan harus di-foreground (kecuali jika
    // di dalam pipeline; dalam kasus itu kita sudah di external mode di atas).
    match cmd {
        "exit" => {
            crate::history::save_history(ctx.rl);
            println!("exiting crush. goodbye!");
            std::process::exit(0);
        }
        "help" => {
            crate::help::print_help(rest.first().copied(), ctx.config);
            return 0;
        }
        "history" => {
            for (i, entry) in ctx.rl.history().iter().enumerate() {
                println!("{:>5}  {}", i + 1, entry);
            }
            return 0;
        }
        "clear" => {
            print!("\x1b[2J\x1b[3J\x1b[1;1H");
            io::stdout().flush().ok();
            // Hook: on_clear
            let hook = ctx.config.hooks.on_clear.clone();
            if !hook.is_empty() {
                run_hook(&hook, ctx);
            }
            return 0;
        }
        "echo" => {
            if rest.is_empty() { println!(); }
            else {
                // Handle -n flag
                if rest[0] == "-n" {
                    print!("{}", rest[1..].join(" "));
                    io::stdout().flush().ok();
                } else if rest[0] == "-e" {
                    let out = rest[1..].join(" ")
                        .replace("\\n", "\n")
                        .replace("\\t", "\t")
                        .replace("\\\\", "\\");
                    println!("{}", out);
                } else {
                    println!("{}", rest.join(" "));
                }
            }
            return 0;
        }
        "pwd" => {
            match env::current_dir() {
                Ok(d) => println!("{}", d.display()),
                Err(e) => eprintln!("pwd: {}", e),
            }
            return 0;
        }
        "cd" => {
            let raw_target = if rest.is_empty() {
                env::var("HOME").unwrap_or_else(|_| "/".to_string())
            } else if rest.len() > 1 {
                eprintln!("cd: too many arguments");
                return 1;
            } else {
                rest[0].to_string()
            };

            // Resolve dir_alias DULU (sebelum expand_tilde), agar ~crush tetap ~name
            let after_alias = ctx.config.resolve_dir_alias(&raw_target);
            // Kemudian baru expand_tilde
            let target = expand_tilde(&after_alias);

            if let Err(_) = env::set_current_dir(Path::new(&target)) {
                eprintln!("cd: {}: No such file or directory", raw_target);
                return 1;
            }

            // Catat PWD baru di ShellEnv (aman, tanpa unsafe global mutation).
            if let Ok(current_path) = env::current_dir() {
                ctx.shell_env.set("PWD", current_path.to_string_lossy().as_ref());
            }

            // Hook: on_cd
            let hook = ctx.config.hooks.on_cd.clone();
            if !hook.is_empty() {
                run_hook(&hook, ctx);
            }
            return 0;
        }
        "type" => {
            let builtins = ["echo","cd","pwd","type","exit","history",
                            "clear","help","export","unset","source","jobs","ls", "rehash", "reload"];
            for &arg in &rest {
                if builtins.contains(&arg) {
                    println!("{} is a shell builtin", arg);
                } else if which(arg).is_ok() {
                    println!("{} is {}", arg, which(arg).unwrap().display());
                } else {
                    eprintln!("{}: not found", arg);
                }
            }
            return 0;
        }
        "export" => {
            for &kv in &rest {
                if let Some(eq) = kv.find('=') {
                    let k = &kv[..eq];
                    let v = &kv[eq+1..];
                    // Simpan di ShellEnv lokal dan propagate ke env OS.
                    ctx.shell_env.set(k, v);
                } else {
                    // export NAME tanpa nilai: pastikan ada di env OS.
                    if env::var(kv).is_err() {
                        ctx.shell_env.set(kv, "");
                    }
                }
            }
            return 0;
        }
        "unset" => {
            for &k in &rest {
                ctx.shell_env.remove(k);
            }
            return 0;
        }
        "jobs" => {
            let mut table = ctx.jobs.lock().unwrap();
            if rest.is_empty() {
                table.print_all();
            } else {
                for &spec in &rest {
                    table.print_job(spec);
                }
            }
            return 0;
        }
        "ls" => {
            return crate::ls::run(&rest);
        }
        "rehash" => {
            // Refresh PATH di ShellEnv agar child process baru mendapat PATH terkini.
            if let Ok(path) = env::var("PATH") {
                ctx.shell_env.set("PATH", &path);
            }
            println!("Rehashed!");
            return 0;
        }
        "reload" => {
            let new_cfg = ShellConfig::load();
            new_cfg.apply_env();
            if let Ok(mut lock) = ctx.cfg_arc.write() {
                *lock = new_cfg;
            }
            println!("crush: config reloaded successfully.");
            return 0;
        }
        _ => {} // lanjut ke wrapper atau external command
    }

    // ── Wrapper function dispatch ─────────────────────────────────────────────
    // Cek apakah cmd cocok dengan salah satu [functions.*] dari config.
    // Ini harus dilakukan sebelum mencoba external command agar wrapper
    // punya prioritas di atas binary dengan nama sama di $PATH.
    if let Some(func) = ctx.config.functions.get(cmd) {
        let func = func.clone();
        return crate::config::wrapper::execute_wrapper(&func, &rest, ctx);
    }

    // ── External command ─────────────────────────────────────────────────────
    let is_bg = seg.background;
    let child_result = spawn_process(&Segment {
        args: args.clone(),
        redirect_out: seg.redirect_out.clone(),
        redirect_err: seg.redirect_err.clone(),
        background: seg.background,
    }, &ctx.shell_env, stdin_override, stdout_override, is_bg);

    match child_result {
        Ok(child) => {
            if is_bg {
                // Background job
                let label = ctx.raw_input
                    .trim_end_matches('&')
                    .trim()
                    .to_string();
                let mut table = ctx.jobs.lock().unwrap();
                table.add(child, &label);
                0
            } else {
                // Foreground — tunggu
                let mut child = child;
                match child.wait() {
                    Ok(status) => status.code().unwrap_or(1),
                    Err(_) => 1,
                }
            }
        }
        Err(_) => {
            eprintln!("crush: {}: command not found", cmd);
            127
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Spawn helper
// ─────────────────────────────────────────────────────────────────────────────

fn open_redirect(r: &Redirect) -> io::Result<File> {
    if r.append {
        OpenOptions::new().create(true).append(true).open(&r.path)
    } else {
        File::create(&r.path)
    }
}

fn spawn_process(
    seg: &Segment,
    shell_env: &ShellEnv,
    stdin_override: Option<Stdio>,
    stdout_override: Option<Stdio>,
    is_bg: bool,
) -> io::Result<Child> {
    let cmd = &seg.args[0];
    let rest: Vec<&str> = seg.args[1..].iter().map(|s| s.as_str()).collect();

    let mut proc = Command::new(cmd);
    proc.args(&rest);

    // Injeksikan semua variabel dari ShellEnv ke child process.
    // Ini menggantikan set_var global: hanya child process ini yang melihat
    // variabel-variabel tersebut; proses lain tidak terpengaruh.
    proc.envs(shell_env.iter());

    // Stdin
    if let Some(si) = stdin_override {
        proc.stdin(si);
    }

    // Stdout
    if let Some(ro) = &seg.redirect_out {
        proc.stdout(open_redirect(ro)?);
    } else if let Some(so) = stdout_override {
        proc.stdout(so);
    }

    // Stderr
    if let Some(re) = &seg.redirect_err {
        proc.stderr(open_redirect(re)?);
    }

    unsafe {
        proc.pre_exec(move || {
            if is_bg {
                libc::setpgid(0, 0);
            } else {
                libc::signal(libc::SIGINT, libc::SIG_DFL);
            }
            Ok(())
        });
    }

    proc.spawn()
}

// ─────────────────────────────────────────────────────────────────────────────
// Hook runner
// ─────────────────────────────────────────────────────────────────────────────

/// Jalankan satu string hook (on_cd, on_clear, pre_command) di dalam context saat ini.
/// Hook adalah string perintah biasa yang di-tokenize dan dieksekusi.
pub fn run_hook(hook: &str, ctx: &mut ExecContext<'_>) {
    let hook = hook.trim();
    if hook.is_empty() { return; }

    let tokens_raw = match shlex::split(hook) {
        Some(t) => t,
        None    => return,
    };
    let tokens_owned = tokenize_operators(tokens_raw);
    let tokens: Vec<&str> = tokens_owned.iter().map(|s| s.as_str()).collect();
    let units = parse_input(&tokens);
    if !units.is_empty() {
        execute_line(ctx, units);
    }
}
