use std::collections::HashMap;
use std::env;
use std::process::Command;

use crate::config::prompt::{
    PromptConfig, DirectorySection, GitBranchSection, GitStatusSection,
    CmdDurationSection, CharacterSection, UsernameSection, HostnameSection
};

pub struct PromptContext {
    pub last_exit_code: i32,
    pub cmd_duration_ms: u64,
    pub is_ssh: bool,
}

pub fn render_prompt(cfg: &PromptConfig, ctx: &PromptContext) -> String {
    let mut out = String::new();
    let format = &cfg.format;
    
    let mut chars = format.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            let mut var_name = String::new();
            while let Some(&nc) = chars.peek() {
                if nc.is_alphanumeric() || nc == '_' {
                    var_name.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            
            let module_output = match var_name.as_str() {
                "directory" => render_directory(&cfg.directory),
                "git_branch" => render_git_branch(&cfg.git_branch),
                "git_status" => render_git_status(&cfg.git_status),
                "cmd_duration" => render_cmd_duration(&cfg.cmd_duration, ctx.cmd_duration_ms),
                "character" => render_character(&cfg.character, ctx.last_exit_code == 0),
                "username" => render_username(&cfg.username),
                "hostname" => render_hostname(&cfg.hostname, ctx.is_ssh),
                _ => String::new(),
            };
            out.push_str(&module_output);
        } else {
            out.push(c);
        }
    }
    
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Format Engine
// ─────────────────────────────────────────────────────────────────────────────

fn apply_style(text: &str, style: &str) -> String {
    if style.is_empty() || text.is_empty() { return text.to_string(); }
    let mut codes = Vec::new();
    for part in style.split_whitespace() {
        match part {
            "bold" => codes.push("1".to_string()),
            "dim" => codes.push("2".to_string()),
            "italic" => codes.push("3".to_string()),
            "underline" => codes.push("4".to_string()),
            "red" => codes.push("31".to_string()),
            "green" => codes.push("32".to_string()),
            "yellow" => codes.push("33".to_string()),
            "blue" => codes.push("34".to_string()),
            "purple" => codes.push("35".to_string()),
            "cyan" => codes.push("36".to_string()),
            "white" => codes.push("37".to_string()),
            _ if part.starts_with("fg:") => {
                if let Ok(c) = part[3..].parse::<u8>() {
                    codes.push(format!("38;5;{}", c));
                }
            }
            _ if part.starts_with("bg:") => {
                if let Ok(c) = part[3..].parse::<u8>() {
                    codes.push(format!("48;5;{}", c));
                }
            }
            _ => {}
        }
    }
    if codes.is_empty() { return text.to_string(); }
    format!("\x1b[{}m{}\x1b[0m", codes.join(";"), text)
}

fn expand_vars(text: &str, vars: &HashMap<&str, String>) -> String {
    let mut out = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '$' {
            let mut var_name = String::new();
            while let Some(&nc) = chars.peek() {
                if nc.is_alphanumeric() || nc == '_' {
                    var_name.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            if let Some(val) = vars.get(var_name.as_str()) {
                out.push_str(val);
            }
        } else if c == '\\' {
            if let Some(nc) = chars.next() {
                if nc == '(' || nc == ')' {
                    out.push(nc);
                } else {
                    out.push('\\');
                    out.push(nc);
                }
            }
        } else {
            out.push(c);
        }
    }
    // Very simple conditional `(:remote_branch)` removal if empty
    out = out.replace("(:)", "");
    out
}

fn render_module_format(format: &str, vars: &HashMap<&str, String>, default_style: &str) -> String {
    let mut out = String::new();
    let mut chars = format.chars().peekable();
    
    // We want to detect `[text](style)` pattern
    while let Some(c) = chars.next() {
        if c == '[' {
            let mut bracket_content = String::new();
            let mut closed = false;
            while let Some(bc) = chars.next() {
                if bc == ']' {
                    closed = true;
                    break;
                }
                bracket_content.push(bc);
            }
            
            if closed && chars.peek() == Some(&'(') {
                chars.next(); // consume '('
                let mut style_ref = String::new();
                while let Some(sc) = chars.next() {
                    if sc == ')' {
                        break;
                    }
                    style_ref.push(sc);
                }
                let actual_style = if style_ref == "$style" { default_style } else { &style_ref };
                let replaced = expand_vars(&bracket_content, vars);
                
                // Only render if there's actual content (ignoring spaces/symbols if variable was empty)
                // Actually Starship skips the block if the variable inside it was empty.
                // Let's do a simple check: if the expanded content has no alphanumeric characters
                // and it had a variable, we might want to skip it.
                // For simplicity, we just check if replaced is empty.
                if !replaced.is_empty() {
                    out.push_str(&apply_style(&replaced, actual_style));
                }
            } else {
                out.push('[');
                out.push_str(&expand_vars(&bracket_content, vars));
                if closed {
                    out.push(']');
                }
            }
        } else if c == '\\' {
             if let Some(nc) = chars.next() {
                 out.push(nc);
             }
        } else if c == '$' {
            let mut var_name = String::new();
            while let Some(&nc) = chars.peek() {
                if nc.is_alphanumeric() || nc == '_' {
                    var_name.push(chars.next().unwrap());
                } else {
                    break;
                }
            }
            if let Some(val) = vars.get(var_name.as_str()) {
                out.push_str(val);
            }
        } else {
            out.push(c);
        }
    }
    
    out
}

// ─────────────────────────────────────────────────────────────────────────────
// Module Renderers
// ─────────────────────────────────────────────────────────────────────────────

fn render_directory(cfg: &DirectorySection) -> String {
    let cwd = env::current_dir().unwrap_or_default();
    let mut path_str = cwd.to_string_lossy().to_string();
    
    let home = env::var("HOME").unwrap_or_default();
    if !home.is_empty() && path_str.starts_with(&home) {
        path_str = path_str.replacen(&home, &cfg.home_symbol, 1);
    }
    
    // Substitutions
    for (k, v) in &cfg.substitutions {
        if path_str.contains(k) {
            path_str = path_str.replace(k, v);
        }
    }
    
    // Truncation
    let parts: Vec<&str> = path_str.split('/').collect();
    if parts.len() > cfg.truncation_length && cfg.truncation_length > 0 {
        let skip = parts.len() - cfg.truncation_length;
        path_str = format!("{}{}", cfg.truncation_symbol, parts[skip..].join("/"));
    }
    
    let mut vars = HashMap::new();
    vars.insert("path", path_str);
    
    // Read only check
    let ro = if std::fs::metadata(&cwd).map(|m| m.permissions().readonly()).unwrap_or(false) {
        cfg.read_only.clone()
    } else {
        "".to_string()
    };
    vars.insert("read_only", ro);
    
    render_module_format(&cfg.format, &vars, &cfg.style)
}

fn render_git_branch(cfg: &GitBranchSection) -> String {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .output();
        
    if let Ok(out) = output {
        if out.status.success() {
            let mut branch = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if branch == "HEAD" {
                // Try to get short hash
                if let Ok(hash_out) = Command::new("git").args(["rev-parse", "--short", "HEAD"]).output() {
                    branch = String::from_utf8_lossy(&hash_out.stdout).trim().to_string();
                }
            }
            
            if branch.is_empty() { return String::new(); }
            
            if cfg.truncation_length > 0 && branch.len() > cfg.truncation_length {
                branch = format!("{}{}", &branch[..cfg.truncation_length], cfg.truncation_symbol);
            }
            
            let mut vars = HashMap::new();
            vars.insert("branch", branch);
            vars.insert("symbol", cfg.symbol.clone());
            
            return render_module_format(&cfg.format, &vars, &cfg.style);
        }
    }
    
    String::new()
}

fn render_git_status(_cfg: &GitStatusSection) -> String {
    // Left as an exercise, simplified for now
    String::new()
}

fn render_cmd_duration(cfg: &CmdDurationSection, ms: u64) -> String {
    if ms < cfg.min_time { return String::new(); }
    
    let duration = if ms < 1000 {
        format!("{}ms", ms)
    } else if ms < 60000 {
        format!("{}s", ms / 1000)
    } else {
        format!("{}m {}s", ms / 60000, (ms % 60000) / 1000)
    };
    
    let mut vars = HashMap::new();
    vars.insert("duration", duration);
    
    render_module_format(&cfg.format, &vars, "")
}

fn render_character(cfg: &CharacterSection, success: bool) -> String {
    if success {
        // Character section format in config is usually just direct string with ansi
        // e.g. success_symbol = "[ ](bold fg:243)"
        // Let's parse it using render_module_format but without variables
        render_module_format(&cfg.success_symbol, &HashMap::new(), "")
    } else {
        render_module_format(&cfg.error_symbol, &HashMap::new(), "")
    }
}

fn render_username(cfg: &UsernameSection) -> String {
    if cfg.disabled { return String::new(); }
    let user = env::var("USER").unwrap_or_default();
    
    if !cfg.show_always && user != "root" {
        // Maybe check if SSH or something, but we respect show_always here
        return String::new();
    }
    
    let style = if user == "root" { &cfg.style_root } else { &cfg.style_user };
    let mut vars = HashMap::new();
    vars.insert("user", user);
    
    render_module_format(&cfg.format, &vars, style)
}

fn render_hostname(cfg: &HostnameSection, is_ssh: bool) -> String {
    if cfg.disabled { return String::new(); }
    if cfg.ssh_only && !is_ssh { return String::new(); }
    
    let hostname = match std::fs::read_to_string("/etc/hostname") {
        Ok(h) => h.trim().to_string(),
        Err(_) => "localhost".to_string(),
    };
    
    let mut vars = HashMap::new();
    vars.insert("hostname", hostname);
    
    render_module_format(&cfg.format, &vars, "")
}
