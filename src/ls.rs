// ls.rs — Builtin ls dengan icon Nerd Font (gaya eza)
//
// Flags:
//   -l   long format (permissions, size, date, owner)
//   -a   tampilkan file tersembunyi (dotfiles)
//   -h   human-readable size
//   -1   satu entri per baris
//   -r   reverse sort
//   -t   sort by modification time
//   -d   list directory itself, bukan isinya
//   --icons / --no-icons   paksa on/off icon

use std::fs::{self, Metadata};
use std::io::{self, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

// ─────────────────────────────────────────────────────────────────────────────
// ANSI color helpers
// ─────────────────────────────────────────────────────────────────────────────

const RST:    &str = "\x1b[0m";
const BOLD:   &str = "\x1b[1m";
const DIM:    &str = "\x1b[2m";
const BLUE:   &str = "\x1b[1;34m";   // direktori
const CYAN:   &str = "\x1b[1;36m";   // symlink
const GREEN:  &str = "\x1b[1;32m";   // executable
const YELLOW: &str = "\x1b[0;33m";   // device / special
const RED:    &str = "\x1b[1;31m";   // broken symlink / permission denied
const WHITE:  &str = "\x1b[0;37m";   // file biasa
const MAGENTA:&str = "\x1b[0;35m";   // archive/compressed
const LBLUE:  &str = "\x1b[0;34m";   // media

// ─────────────────────────────────────────────────────────────────────────────
// Icon map — Nerd Font
// ─────────────────────────────────────────────────────────────────────────────

fn icon_for(name: &str, meta: &Metadata, is_link: bool) -> &'static str {
    if is_link       { return "󱅷 "; }
    if meta.is_dir() { return dir_icon(name); }

    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        // Rust
        "rs"                          => " ",
        // Web
        "html" | "htm"                => " ",
        "css" | "scss" | "sass"       => " ",
        "js" | "mjs" | "cjs"         => " ",
        "ts"                          => " ",
        "jsx" | "tsx"                 => " ",
        "vue"                         => " ",
        "svelte"                      => " ",
        // Config / Data
        "json" | "jsonc"              => " ",
        "yaml" | "yml"                => " ",
        "toml"                        => " ",
        "xml"                         => "󰗀 ",
        "csv"                         => " ",
        "env"                         => " ",
        "ini" | "cfg" | "conf"        => " ",
        // Docs
        "md" | "mdx"                  => " ",
        "txt"                         => " ",
        "pdf"                         => " ",
        "doc" | "docx"                => "󱎒 ",
        "xls" | "xlsx"                => "󱎗 ",
        "ppt" | "pptx"                => "󱎐 ",
        // Archives
        "zip" | "tar" | "gz" | "bz2"
        | "xz" | "zst" | "7z" | "rar"=> " ",
        "deb" | "rpm"                 => " ",
        "pkg"                         => " ",
        // Images
        "png" | "jpg" | "jpeg" | "gif"
        | "webp" | "bmp" | "ico"      => " ",
        "svg"                         => "󰜡 ",
        // Audio / Video
        "mp3" | "flac" | "ogg" | "wav"
        | "aac" | "opus"              => " ",
        "mp4" | "mkv" | "avi" | "mov"
        | "webm" | "flv"              => " ",
        // Code (lain)
        "py" | "pyw"                  => " ",
        "go"                          => " ",
        "java"                        => " ",
        "c" | "h"                     => " ",
        "cpp" | "cc" | "cxx" | "hpp" => " ",
        "cs"                          => "󰌛 ",
        "rb"                          => " ",
        "php"                         => " ",
        "lua"                         => " ",
        "sh" | "bash" | "zsh" | "fish"=> " ",
        "ps1"                         => "󰨊 ",
        "vim" | "nvim"                => " ",
        "el" | "elc"                  => " ",
        "hs"                          => " ",
        "r"                           => " ",
        "swift"                       => " ",
        "kt" | "kts"                  => " ",
        "dart"                        => " ",
        "ex" | "exs"                  => " ",
        "nix"                         => " ",
        // Database
        "sql" | "db" | "sqlite"       => " ",
        // Docker / DevOps
        "dockerfile" | "containerfile"=> " ",
        // Font
        "ttf" | "otf" | "woff" | "woff2" => " ",
        // Binary / object
        "o" | "so" | "a" | "dylib"   => " ",
        "wasm"                        => " ",
        // Lock files
        "lock"                        => " ",
        // Executables & scripts (tidak punya ext tapi executable — ditangani di bawah)
        _ => file_icon_fallback(name, meta),
    }
}

fn dir_icon(name: &str) -> &'static str {
    match name {
        ".git"                       => " ",
        ".github"                    => " ",
        "src" | "source"             => " ",
        "target" | "build" | "dist"
        | "out"                      => " ",
        "node_modules"               => " ",
        "public" | "static"          => " ",
        "assets" | "images" | "img" => " ",
        "docs" | "doc"               => " ",
        "test" | "tests" | "spec"    => " ",
        "config" | ".config"         => " ",
        "bin"                        => " ",
        "lib"                        => " ",
        "scripts" | "script"         => " ",
        "tmp" | "temp" | "cache"     => " ",
        "logs" | "log"               => " ",
        "backup"                     => " ",
        "downloads" | "Download"     => " ",
        "Desktop"                    => " ",
        "Documents" | "document"     => " ",
        "Music"                      => " ",
        "Pictures"                   => " ",
        "Videos"                     => " ",
        "home"                       => " ",
        "etc"                        => " ",
        "usr"                        => " ",
        "var"                        => " ",
        "proc"                       => " ",
        "dev"                        => " ",
        "mnt" | "media"              => " ",
        _                            => " ",
    }
}

fn file_icon_fallback(name: &str, meta: &Metadata) -> &'static str {
    // Executable tanpa ekstensi
    if meta.permissions().mode() & 0o111 != 0 && !meta.is_dir() {
        return " ";
    }
    // Nama khusus
    match name {
        "Makefile" | "makefile" | "GNUmakefile" => " ",
        "Dockerfile" | "Containerfile"          => " ",
        "docker-compose.yml" | "compose.yml"    => " ",
        ".gitignore" | ".gitattributes"         => " ",
        ".editorconfig"                         => " ",
        "LICENSE" | "licence"                   => " ",
        "README" | "readme"                     => " ",
        "Cargo.toml" | "Cargo.lock"             => " ",
        "package.json" | "package-lock.json"    => " ",
        "go.mod" | "go.sum"                     => " ",
        "pyproject.toml" | "setup.py"           => " ",
        ".bashrc" | ".bash_profile" | ".profile"=> " ",
        ".zshrc" | ".zshenv"                    => " ",
        ".vimrc" | "init.vim"                   => " ",
        _                                       => " ",
    }
}

fn color_for(meta: &Metadata, is_link: bool, link_ok: bool) -> &'static str {
    if is_link && !link_ok  { return RED; }
    if is_link              { return CYAN; }
    if meta.is_dir()        { return BLUE; }

    let mode = meta.permissions().mode();
    if mode & 0o111 != 0    { return GREEN; }

    // Deteksi berdasarkan file type bits
    let ftype = mode & 0o170000;
    if ftype == 0o060000 || ftype == 0o020000 { return YELLOW; } // block/char dev

    WHITE
}

fn archive_color(name: &str) -> Option<&'static str> {
    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        "zip"|"tar"|"gz"|"bz2"|"xz"|"zst"|"7z"|"rar"|"deb"|"rpm" => Some(MAGENTA),
        "png"|"jpg"|"jpeg"|"gif"|"webp"|"bmp"|"ico"|"svg" => Some(LBLUE),
        "mp3"|"flac"|"ogg"|"wav"|"mp4"|"mkv"|"avi"|"mov" => Some(LBLUE),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Options parser
// ─────────────────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct LsOptions {
    pub long:      bool,   // -l
    pub all:       bool,   // -a
    pub human:     bool,   // -h
    pub one:       bool,   // -1
    pub reverse:   bool,   // -r
    pub sort_time: bool,   // -t
    pub dir_only:  bool,   // -d
    pub icons:     bool,   // default true; --no-icons = false
    pub paths:     Vec<PathBuf>,
}

impl LsOptions {
    pub fn parse(args: &[&str]) -> Self {
        let mut opt = LsOptions { icons: true, ..Default::default() };
        for &arg in args {
            match arg {
                "--no-icons"  => opt.icons = false,
                "--icons"     => opt.icons = true,
                _ if arg.starts_with('-') && !arg.starts_with("--") => {
                    for ch in arg.chars().skip(1) {
                        match ch {
                            'l' => opt.long      = true,
                            'a' => opt.all       = true,
                            'h' => opt.human     = true,
                            '1' => opt.one       = true,
                            'r' => opt.reverse   = true,
                            't' => opt.sort_time = true,
                            'd' => opt.dir_only  = true,
                            _   => eprintln!("ls: unknown option: -{}", ch),
                        }
                    }
                }
                _ => opt.paths.push(PathBuf::from(arg)),
            }
        }
        if opt.paths.is_empty() {
            opt.paths.push(PathBuf::from("."));
        }
        opt
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry struct
// ─────────────────────────────────────────────────────────────────────────────

struct Entry {
    name:     String,
    path:     PathBuf,
    meta:     Metadata,
    link_meta:Option<Metadata>, // resolved link target meta
    is_link:  bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Size formatting
// ─────────────────────────────────────────────────────────────────────────────

fn fmt_size(bytes: u64, human: bool) -> String {
    if !human { return format!("{:>7}", bytes); }
    const UNITS: &[&str] = &["B", "K", "M", "G", "T", "P"];
    let mut val = bytes as f64;
    let mut idx = 0;
    while val >= 1024.0 && idx < UNITS.len() - 1 {
        val /= 1024.0;
        idx += 1;
    }
    if idx == 0 {
        format!("{:>4}B", bytes)
    } else if val < 10.0 {
        format!("{:>3.1}{}", val, UNITS[idx])
    } else {
        format!("{:>3.0}{}", val, UNITS[idx])
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Permission string (rwxrwxrwx)
// ─────────────────────────────────────────────────────────────────────────────

fn fmt_perms(meta: &Metadata, is_link: bool) -> String {
    let mode = meta.permissions().mode();
    let ftype = if is_link { 'l' }
                else if meta.is_dir() { 'd' }
                else {
                    match mode & 0o170000 {
                        0o140000 => 's',
                        0o060000 => 'b',
                        0o020000 => 'c',
                        0o010000 => 'p',
                        _        => '-',
                    }
                };
    let bits = |shift: u32| -> String {
        let r = if mode & (0o400 >> shift) != 0 { 'r' } else { '-' };
        let w = if mode & (0o200 >> shift) != 0 { 'w' } else { '-' };
        let x = if mode & (0o100 >> shift) != 0 { 'x' } else { '-' };
        format!("{}{}{}", r, w, x)
    };
    format!("{}{}{}{}", ftype, bits(0), bits(3), bits(6))
}

// ─────────────────────────────────────────────────────────────────────────────
// Date formatting
// ─────────────────────────────────────────────────────────────────────────────

fn fmt_time(t: SystemTime) -> String {
    use std::time::UNIX_EPOCH;
    let secs = t.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
    let (y, mo, d, h, mi) = secs_to_ymd(secs);
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, mo, d, h, mi)
}

fn secs_to_ymd(secs: u64) -> (u64, u64, u64, u64, u64) {
    let mi    = (secs / 60) % 60;
    let h     = (secs / 3600) % 24;
    let mut days = secs / 86400;
    let mut y = 1970u64;
    loop {
        let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
        let dy = if leap { 366 } else { 365 };
        if days < dy { break; }
        days -= dy;
        y += 1;
    }
    let leap = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let months = [31u64, if leap {29} else {28}, 31,30,31,30,31,31,30,31,30,31];
    let mut mo = 1u64;
    for &dm in &months {
        if days < dm { break; }
        days -= dm;
        mo += 1;
    }
    (y, mo, days + 1, h, mi)
}

// ─────────────────────────────────────────────────────────────────────────────
// Read entries from directory
// ─────────────────────────────────────────────────────────────────────────────

fn read_entries(dir: &Path, opt: &LsOptions) -> Vec<Entry> {
    let mut entries: Vec<Entry> = Vec::new();

    let read = match fs::read_dir(dir) {
        Ok(r) => r,
        Err(e) => { eprintln!("ls: {}: {}", dir.display(), e); return entries; }
    };

    for item in read.flatten() {
        let name = item.file_name().to_string_lossy().to_string();
        if !opt.all && name.starts_with('.') { continue; }

        let path = item.path();
        let is_link = item.file_type().map(|t| t.is_symlink()).unwrap_or(false);

        // Metadata: bukan follow symlink untuk permissions; follow untuk size dir
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        let link_meta = if is_link {
            fs::metadata(&path).ok() // follow link untuk warna/ikon target
        } else {
            None
        };

        entries.push(Entry { name, path, meta, link_meta, is_link });
    }

    // Sort
    entries.sort_by(|a, b| {
        let ord = if opt.sort_time {
            let ta = a.meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            let tb = b.meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
            tb.cmp(&ta) // lebih baru dulu
        } else {
            // Direktori dulu, lalu alfa case-insensitive
            let da = a.meta.is_dir();
            let db = b.meta.is_dir();
            db.cmp(&da).then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        };
        if opt.reverse { ord.reverse() } else { ord }
    });

    entries
}

// ─────────────────────────────────────────────────────────────────────────────
// Display: grid / long
// ─────────────────────────────────────────────────────────────────────────────

fn entry_color(e: &Entry) -> &'static str {
    let link_ok = e.link_meta.is_some();
    let meta_for_color = e.link_meta.as_ref().unwrap_or(&e.meta);
    archive_color(&e.name)
        .unwrap_or_else(|| color_for(meta_for_color, e.is_link, link_ok))
}

fn print_long(entries: &[Entry], opt: &LsOptions) {
    // Hitung total blocks (du-style)
    let total_blocks: u64 = entries.iter().map(|e| e.meta.blocks()).sum();
    println!("total {}", total_blocks / 2); // 512-byte → 1K blocks

    for e in entries {
        let perm   = fmt_perms(&e.meta, e.is_link);
        let nlink  = e.meta.nlink();
        let uid    = e.meta.uid();
        let gid    = e.meta.gid();
        let size   = fmt_size(e.meta.len(), opt.human);
        let mtime  = e.meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
        let date   = fmt_time(mtime);
        let color  = entry_color(e);
        let icon   = if opt.icons { icon_for(&e.name, &e.meta, e.is_link) } else { "" };

        let suffix = if e.meta.is_dir() { "/" }
                     else if e.is_link  { " →" }
                     else if e.meta.permissions().mode() & 0o111 != 0 { "*" }
                     else { "" };

        print!("{DIM}{:<10} {:>4} {:>5} {:>5}{RST} {size} {DIM}{date}{RST}  {color}{icon}{}{suffix}{RST}",
            perm, nlink, uid, gid, e.name,
            DIM=DIM, RST=RST, size=size, date=date, color=color, icon=icon, suffix=suffix);

        // Symlink target
        if e.is_link {
            if let Ok(target) = fs::read_link(&e.path) {
                let ok = e.link_meta.is_some();
                let tc = if ok { CYAN } else { RED };
                print!(" {tc}{}{RST}", target.display(), tc=tc, RST=RST);
            }
        }
        println!();
    }
}

fn print_grid(entries: &[Entry], opt: &LsOptions) {
    // Hitung lebar terminal (fallback 80)
    let term_w = term_width();

    // Buat string tampilan per entry
    let items: Vec<String> = entries.iter().map(|e| {
        let color  = entry_color(e);
        let icon   = if opt.icons { icon_for(&e.name, &e.meta, e.is_link) } else { "" };
        let suffix = if e.meta.is_dir() { "/" }
                     else if e.meta.permissions().mode() & 0o111 != 0 { "*" }
                     else { "" };
        format!("{}{}{}{}{}", color, icon, e.name, suffix, RST)
    }).collect();

    // Panjang tampilan (tanpa ANSI) per item
    let lens: Vec<usize> = entries.iter().map(|e| {
        let icon_w = if opt.icons { 2 } else { 0 }; // icon + spasi = 2 char
        e.name.chars().count() + icon_w
            + if e.meta.is_dir() || e.meta.permissions().mode() & 0o111 != 0 { 1 } else { 0 }
    }).collect();

    if opt.one || opt.long {
        for s in &items { println!("{}", s); }
        return;
    }

    // Hitung jumlah kolom optimal
    let col_gap = 2usize;
    let max_cols = (term_w / (lens.iter().max().copied().unwrap_or(1) + col_gap)).max(1);

    // Coba dari max_cols turun sampai muat
    let cols = (1..=max_cols).rev().find(|&c| {
        let rows = (items.len() + c - 1) / c;
        let col_widths: Vec<usize> = (0..c).map(|ci| {
            (0..rows).filter_map(|ri| lens.get(ri * c + ci)).copied().max().unwrap_or(0)
        }).collect();
        let total: usize = col_widths.iter().sum::<usize>() + col_gap * (c.saturating_sub(1));
        total <= term_w
    }).unwrap_or(1);

    let rows = (items.len() + cols - 1) / cols;
    let col_widths: Vec<usize> = (0..cols).map(|ci| {
        (0..rows).filter_map(|ri| lens.get(ri * cols + ci)).copied().max().unwrap_or(0)
    }).collect();

    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    for ri in 0..rows {
        for ci in 0..cols {
            let idx = ri * cols + ci;
            if idx >= items.len() { break; }
            let pad = col_widths[ci].saturating_sub(lens[idx]);
            write!(out, "{}", items[idx]).ok();
            if ci < cols - 1 {
                write!(out, "{:pad$}  ", "", pad=pad).ok();
            }
        }
        writeln!(out).ok();
    }
}

fn term_width() -> usize {
    // Baca dari env COLUMNS, atau ioctl, fallback 80
    if let Ok(cols) = std::env::var("COLUMNS") {
        if let Ok(n) = cols.parse::<usize>() { return n; }
    }
    // Coba ioctl TIOCGWINSZ
    #[cfg(target_os = "linux")]
    {
        let mut ws: libc_winsize = unsafe { std::mem::zeroed() };
        unsafe {
            if libc_tiocgwinsz(1, &mut ws) == 0 && ws.ws_col > 0 {
                return ws.ws_col as usize;
            }
        }
    }
    80
}

// ── Minimal ioctl binding tanpa libc crate ────────────────────────────────────
#[cfg(target_os = "linux")]
#[repr(C)]
struct libc_winsize { ws_row: u16, ws_col: u16, ws_xpixel: u16, ws_ypixel: u16 }

#[cfg(target_os = "linux")]
unsafe fn libc_tiocgwinsz(fd: i32, ws: *mut libc_winsize) -> i32 {
    // TIOCGWINSZ = 0x5413 on Linux
    unsafe extern "C" { fn ioctl(fd: i32, request: u64, ...) -> i32; }
    unsafe { ioctl(fd, 0x5413, ws) }
}

// ─────────────────────────────────────────────────────────────────────────────
// Public entry point
// ─────────────────────────────────────────────────────────────────────────────

pub fn run(args: &[&str]) -> i32 {
    let opt = LsOptions::parse(args);

    let multi = opt.paths.len() > 1;

    for (i, path) in opt.paths.iter().enumerate() {
        // Cetak header jika multiple path
        if multi {
            if i > 0 { println!(); }
            println!("{}{}:{}",
                BOLD, path.display(), RST);
        }

        let meta = match fs::symlink_metadata(path) {
            Ok(m) => m,
            Err(e) => { eprintln!("ls: {}: {}", path.display(), e); continue; }
        };

        // -d: tampilkan direktori itu sendiri
        if opt.dir_only || !meta.is_dir() {
            let is_link = meta.file_type().is_symlink();
            let link_meta = if is_link { fs::metadata(path).ok() } else { None };
            let name = path.file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| path.to_string_lossy().to_string());
            let fake = Entry { name, path: path.clone(), meta, link_meta, is_link };
            if opt.long {
                print_long(&[fake], &opt);
            } else {
                print_grid(&[fake], &opt);
            }
            continue;
        }

        // Direktori normal
        let entries = read_entries(path, &opt);
        if opt.long {
            print_long(&entries, &opt);
        } else {
            print_grid(&entries, &opt);
        }
    }

    0
}
