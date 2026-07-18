// executor.rs — Command execution engine untuk crush shell
//
// Mendukung:
//   - Redirection: >, >>, 2>, 2>>
//   - Background jobs: cmd &
//   - Pipeline: cmd1 | cmd2 | cmd3
//   - Logical AND: cmd1 && cmd2
//   - Logical OR:  cmd1 || cmd2
//   - Kombinasi: cmd1 && cmd2 | cmd3 || cmd4

use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use which::which;

use crate::jobs::SharedJobTable;

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
pub fn tokenize_operators(raw_tokens: Vec<String>) -> Vec<String> {
    let mut result = Vec::new();
    for tok in raw_tokens {
        // Pecah token yang mengandung operator shell
        // Contoh: "&&" → ["&&"], "build&&run" → ["build", "&&", "run"]
        let expanded = split_operators(&tok);
        result.extend(expanded);
    }
    result
}

fn split_operators(s: &str) -> Vec<String> {
    // Operator yang perlu dipisah (urutan: lebih panjang dulu)
    const OPS: &[&str] = &["&&", "||", "|"];
    let mut result = vec![s.to_string()];
    for op in OPS {
        let mut new_result = Vec::new();
        for part in result {
            if part == *op {
                new_result.push(part);
                continue;
            }
            // Pisah berdasarkan operator ini
            let pieces: Vec<&str> = part.splitn(2, op).collect();
            if pieces.len() == 2 {
                if !pieces[0].is_empty() { new_result.push(pieces[0].to_string()); }
                new_result.push(op.to_string());
                if !pieces[1].is_empty() { new_result.extend(split_operators(pieces[1])); }
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
    if path.starts_with('~') {
        if let Ok(home) = env::var("HOME") {
            return path.replacen('~', &home, 1);
        }
    }
    path.to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// Execution context
// ─────────────────────────────────────────────────────────────────────────────

pub struct ExecContext<'a> {
    pub jobs:   &'a SharedJobTable,
    pub rl:     &'a mut rustyline::Editor<
                    crate::completion::CrushCompleter,
                    rustyline::history::FileHistory,
                >,
    pub raw_input: &'a str, // teks asli baris input (untuk job command label)
}

// ─────────────────────────────────────────────────────────────────────────────
// Titik masuk utama
// ─────────────────────────────────────────────────────────────────────────────

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
            stdin_source,
            stdout_dest,
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
            crate::help::print_help(rest.first().copied());
            return 0;
        }
        "history" => {
            for (i, entry) in ctx.rl.history().iter().enumerate() {
                println!("{:>5}  {}", i + 1, entry);
            }
            return 0;
        }
        "clear" => {
            print!("\x1b[2J\x1b[H");
            io::stdout().flush().ok();
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
            let target = if rest.is_empty() {
                env::var("HOME").unwrap_or_else(|_| "/".to_string())
            } else if rest.len() > 1 {
                eprintln!("cd: too many arguments");
                return 1;
            } else {
                expand_tilde(rest[0])
            };
            if let Err(_) = env::set_current_dir(Path::new(&target)) {
                eprintln!("cd: {}: No such file or directory", rest[0]);
                return 1;
            }
            return 0;
        }
        "type" => {
            let builtins = ["echo","cd","pwd","type","exit","history",
                            "clear","help","export","unset","source","jobs","ls"];
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
                    unsafe { env::set_var(k, v); }
                } else {
                    // export NAME tanpa nilai — pastikan ada di env
                    if env::var(kv).is_err() {
                        unsafe { env::set_var(kv, ""); }
                    }
                }
            }
            return 0;
        }
        "unset" => {
            for &k in &rest {
                unsafe { env::remove_var(k); }
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
        _ => {} // lanjut ke external command
    }

    // ── External command ─────────────────────────────────────────────────────
    let is_bg = seg.background;
    let child_result = spawn_process(&Segment {
        args: args.clone(),
        redirect_out: seg.redirect_out.clone(),
        redirect_err: seg.redirect_err.clone(),
        background: seg.background,
    }, stdin_override, stdout_override);

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
            eprintln!("{}: command not found", cmd);
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
    stdin_override: Option<Stdio>,
    stdout_override: Option<Stdio>,
) -> io::Result<Child> {
    let cmd = &seg.args[0];
    let rest: Vec<&str> = seg.args[1..].iter().map(|s| s.as_str()).collect();

    let mut proc = Command::new(cmd);
    proc.args(&rest);

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

    proc.spawn()
}
