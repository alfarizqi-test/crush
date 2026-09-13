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
// Icon map — Nerd Font v3 (codepoints U+E000–U+F8FF, BMP Private Use Area)
//
// Referensi: https://www.nerdfonts.com/cheat-sheet
//   nf-dev-*    E600–E6FF   (DevIcons)
//   nf-fa-*     E000–E0FF, F000–F2FF  (Font Awesome)
//   nf-seti-*   E5FA–E62A  (Seti-UI)
//   nf-cod-*    EA60–EBEB  (Codicons)
//   nf-md-*     F0000+     (Material — TIDAK dipakai, di luar BMP)
// ─────────────────────────────────────────────────────────────────────────────

fn icon_for(name: &str, meta: &Metadata, is_link: bool) -> &'static str {
    if is_link       { return "\u{f0c1} "; }  // nf-fa-link
    if meta.is_dir() { return dir_icon(name); }

    let ext = name.rsplit('.').next().unwrap_or("").to_lowercase();
    match ext.as_str() {
        // Rust
        "rs"                          => "\u{e7a8} ", // nf-dev-rust
        // Web
        "html" | "htm"                => "\u{e736} ", // nf-dev-html5
        "css"                         => "\u{e749} ", // nf-dev-css3
        "scss" | "sass"               => "\u{e603} ", // nf-dev-sass
        "js" | "mjs" | "cjs"         => "\u{e74e} ", // nf-dev-javascript
        "ts"                          => "\u{e628} ", // nf-dev-typescript
        "jsx" | "tsx"                 => "\u{e7ba} ", // nf-dev-react
        "vue"                         => "\u{e6a0} ", // nf-dev-vue
        "svelte"                      => "\u{e697} ", // nf-dev-svelte (fallback file)
        // Config / Data
        "json" | "jsonc"              => "\u{e60b} ", // nf-seti-json
        "yaml" | "yml"                => "\u{e601} ", // nf-seti-yaml
        "toml"                        => "\u{e615} ", // nf-seti-config (settings)
        "xml"                         => "\u{e619} ", // nf-seti-xml
        "csv"                         => "\u{f1c3} ", // nf-fa-file_excel_o
        "env"                         => "\u{f462} ", // nf-fa-shield (env vars)
        "ini" | "cfg" | "conf"        => "\u{e615} ", // nf-seti-config
        // Docs
        "md" | "mdx"                  => "\u{e609} ", // nf-seti-markdown
        "txt"                         => "\u{f15c} ", // nf-fa-file_text
        "pdf"                         => "\u{f1c1} ", // nf-fa-file_pdf_o
        "doc" | "docx"                => "\u{f1c2} ", // nf-fa-file_word_o
        "xls" | "xlsx"                => "\u{f1c3} ", // nf-fa-file_excel_o
        "ppt" | "pptx"                => "\u{f1c4} ", // nf-fa-file_powerpoint_o
        // Archives
        "zip" | "tar" | "gz" | "bz2"
        | "xz" | "zst" | "7z" | "rar"=> "\u{f1c6} ", // nf-fa-file_archive_o
        "deb"                         => "\u{e77d} ", // nf-dev-debian
        "rpm"                         => "\u{e7bb} ", // nf-dev-redhat
        "pkg"                         => "\u{f468} ", // nf-fa-cube
        // Images
        "png" | "jpg" | "jpeg" | "gif"
        | "webp" | "bmp" | "ico"      => "\u{f1c5} ", // nf-fa-file_image_o
        "svg"                         => "\u{e698} ", // nf-dev-svg
        // Audio / Video
        "mp3" | "flac" | "ogg" | "wav"
        | "aac" | "opus"              => "\u{f001} ", // nf-fa-music
        "mp4" | "mkv" | "avi" | "mov"
        | "webm" | "flv"              => "\u{f03d} ", // nf-fa-film
        // Code (lain)
        "py" | "pyw"                  => "\u{e606} ", // nf-dev-python
        "go"                          => "\u{e626} ", // nf-dev-go
        "java"                        => "\u{e738} ", // nf-dev-java
        "c" | "h"                     => "\u{e61e} ", // nf-dev-c
        "cpp" | "cc" | "cxx" | "hpp" => "\u{e61d} ", // nf-dev-cplusplus
        "cs"                          => "\u{e648} ", // nf-dev-csharp
        "rb"                          => "\u{e739} ", // nf-dev-ruby
        "php"                         => "\u{e73d} ", // nf-dev-php
        "lua"                         => "\u{e620} ", // nf-dev-lua
        "sh" | "bash"                 => "\u{e691} ", // nf-dev-bash (terminal)
        "zsh" | "fish"                => "\u{e615} ", // nf-seti-config
        "ps1"                         => "\u{e6a2} ", // nf-dev-windows (powershell)
        "vim" | "nvim"                => "\u{e62b} ", // nf-dev-vim
        "el" | "elc"                  => "\u{e616} ", // nf-seti-elisp
        "hs"                          => "\u{e777} ", // nf-dev-haskell
        "r"                           => "\u{e68a} ", // nf-dev-r
        "swift"                       => "\u{e755} ", // nf-dev-swift
        "kt" | "kts"                  => "\u{e634} ", // nf-dev-kotlin
        "dart"                        => "\u{e798} ", // nf-dev-dart
        "ex" | "exs"                  => "\u{e62d} ", // nf-dev-elixir
        "nix"                         => "\u{e7a5} ", // nf-dev-nixos
        // Database
        "sql"                         => "\u{e706} ", // nf-dev-database
        "db" | "sqlite"               => "\u{e706} ", // nf-dev-database
        // Docker / DevOps
        "dockerfile" | "containerfile"=> "\u{e650} ", // nf-dev-docker (whale)
        // Font
        "ttf" | "otf" | "woff" | "woff2" => "\u{f031} ", // nf-fa-font
        // Binary / object
        "o" | "so" | "a" | "dylib"   => "\u{f17c} ", // nf-fa-linux
        "wasm"                        => "\u{e738} ", // nf-dev-java (reuse)
        // Lock
        "lock"                        => "\u{f023} ", // nf-fa-lock
        _ => file_icon_fallback(name, meta),
    }
}

fn dir_icon(name: &str) -> &'static str {
    match name {
        ".git"                       => "\u{e702} ", // nf-dev-git
        ".github"                    => "\u{e65b} ", // nf-dev-github_badge
        "src" | "source"             => "\u{e5fe} ", // nf-seti-folder_src
        "target" | "build" | "dist"
        | "out"                      => "\u{e5fe} ", // nf-seti-folder (build)
        "node_modules"               => "\u{e74e} ", // nf-dev-javascript
        "public" | "static"          => "\u{f0c2} ", // nf-fa-cloud
        "assets" | "images" | "img"  => "\u{f03e} ", // nf-fa-picture_o
        "docs" | "doc"               => "\u{f02d} ", // nf-fa-book
        "test" | "tests" | "spec"    => "\u{f0ae} ", // nf-fa-tasks
        "config" | ".config"         => "\u{e615} ", // nf-seti-config
        "bin"                        => "\u{e5fc} ", // nf-seti-folder_bin
        "lib"                        => "\u{f121} ", // nf-fa-code
        "scripts" | "script"         => "\u{f489} ", // nf-fa-terminal
        "tmp" | "temp" | "cache"     => "\u{f07b} ", // nf-fa-folder
        "logs" | "log"               => "\u{f18d} ", // nf-fa-file_text_o
        "backup"                     => "\u{f0c7} ", // nf-fa-floppy_o
        "downloads" | "Download"     => "\u{f019} ", // nf-fa-download
        "Desktop"                    => "\u{f108} ", // nf-fa-desktop
        "Documents" | "document"     => "\u{f02d} ", // nf-fa-book
        "Music"                      => "\u{f001} ", // nf-fa-music
        "Pictures"                   => "\u{f03e} ", // nf-fa-picture_o
        "Videos"                     => "\u{f03d} ", // nf-fa-film
        "home"                       => "\u{f015} ", // nf-fa-home
        "etc"                        => "\u{f013} ", // nf-fa-cog
        "usr"                        => "\u{f007} ", // nf-fa-user
        "var"                        => "\u{f1b2} ", // nf-fa-cube
        "proc"                       => "\u{f085} ", // nf-fa-cogs
        "dev"                        => "\u{e601} ", // nf-seti-default (devices)
        "mnt" | "media"              => "\u{f0a0} ", // nf-fa-hdd_o
        _                            => "\u{f07b} ", // nf-fa-folder (default)
    }
}

fn file_icon_fallback(name: &str, meta: &Metadata) -> &'static str {
    // Executable tanpa ekstensi
    if meta.permissions().mode() & 0o111 != 0 && !meta.is_dir() {
        return "\u{f489} "; // nf-fa-terminal
    }
    // Nama file khusus
    match name {
        "Makefile" | "makefile" | "GNUmakefile" => "\u{f013} ", // nf-fa-cog
        "Dockerfile" | "Containerfile"          => "\u{e650} ", // nf-dev-docker
        "docker-compose.yml" | "compose.yml"    => "\u{e650} ", // nf-dev-docker
        ".gitignore" | ".gitattributes"         => "\u{e702} ", // nf-dev-git
        ".editorconfig"                         => "\u{e652} ", // nf-dev-aptana
        "LICENSE" | "LICENCE"                   => "\u{f0e3} ", // nf-fa-legal
        "README" | "readme"                     => "\u{f48a} ", // nf-fa-book (README)
        "Cargo.toml" | "Cargo.lock"             => "\u{e7a8} ", // nf-dev-rust
        "package.json" | "package-lock.json"    => "\u{e74e} ", // nf-dev-javascript
        "go.mod" | "go.sum"                     => "\u{e626} ", // nf-dev-go
        "pyproject.toml" | "setup.py"           => "\u{e606} ", // nf-dev-python
        ".bashrc" | ".bash_profile" | ".profile"=> "\u{e691} ", // nf-dev-bash
        ".zshrc" | ".zshenv"                    => "\u{e615} ", // nf-seti-config
        ".vimrc" | "init.vim"                   => "\u{e62b} ", // nf-dev-vim
        "PKGBUILD"                              => "\u{f303} ", // nf-md-arch
        _                                       => "\u{f15b} ", // nf-fa-file (default)
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
