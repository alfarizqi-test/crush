// config/wrapper.rs - Shell function/wrapper definitions & runner
//
// Reads [functions.*] from config.toml.
// Features:
//   - let var = value / let var = $(cmd)
//   - if COND / end (POSIX subset)
//   - $args, $0..$9, $var
//   - Fallback to executor::execute_line

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
    /// Multi-line script body
    pub body: String,
}

/// Top-level container for [functions.*]
#[derive(Debug, Clone, Default)]
pub struct WrapperConfig {
    pub functions: HashMap<String, WrapperFunction>,
}

// ─────────────────────────────────────────────────────────────────────────────
// Loading dari raw TOML value
// ─────────────────────────────────────────────────────────────────────────────

/// Intermediate struct for deserializing [functions]
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
struct RawConfig {
    functions: HashMap<String, WrapperFunction>,
}

impl WrapperConfig {
    /// Parse [functions] from raw TOML.
    pub fn from_str(content: &str) -> Self {
        let raw: RawConfig = basic_toml::from_str(content).unwrap_or_default();
        Self { functions: raw.functions }
    }

    /// Return function if name matches.
    pub fn get(&self, name: &str) -> Option<&WrapperFunction> {
        self.functions.get(name)
    }

    /// List all defined function names.
    pub fn names(&self) -> Vec<&str> {
        self.functions.keys().map(|k| k.as_str()).collect()
    }

    pub fn is_empty(&self) -> bool {
        self.functions.is_empty()
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Runner - mini interpreter for script body
// ─────────────────────────────────────────────────────────────────────────────

/// Execute wrapper function with given args, returning exit code.
pub fn execute_wrapper(
    func:  &WrapperFunction,
    args:  &[&str],
    ctx:   &mut crate::core::executor::ExecContext<'_>,
) -> i32 {
    let mut locals: HashMap<String, String> = HashMap::new();

    // $args = all args joined; $1..$9 = positional
    locals.insert("args".into(), args.join(" "));
    for (i, a) in args.iter().enumerate().take(9) {
        locals.insert(i.to_string(), a.to_string());
    }

    run_block(func.body.as_str(), &mut locals, args, ctx)
}

/// Run text block. locals shared mutably so `let` inside if is visible after block.
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
                    // Cow: allocate only if vars need expansion
                    expand_vars(val_raw, locals, args).into_owned()
                };
                locals.insert(var, value);
            }
            continue;
        }

        // ── if COND ────────────────────────────────────────────────────────────
        if let Some(cond_raw) = raw.strip_prefix("if ") {
            // expand_vars returns Cow
            let cond_expanded = expand_vars(cond_raw, locals, args);
            let cond_result   = evaluate_condition(&cond_expanded);

            // Scan forward for matching "end"
            // Collect lines BETWEEN "if" and "end"
            let block_start = pc;           // first line after "if ..."
            let mut depth   = 1usize;
            while pc < lines.len() {
                let l = lines[pc].trim();
                pc += 1;                    // pc points AFTER this line
                if l.starts_with("if ") { depth += 1; }
                if l == "end" {
                    depth -= 1;
                    if depth == 0 { break; }
                }
            }
            // pc is index exactly AFTER matching "end"
            // "end" is at pc-1, so block = [block_start .. pc-1)
            let block_end = pc - 1;        // "end" index (exclusive)
            let block_body = lines[block_start..block_end].join("\n");

            if cond_result {
                last_exit = run_block(&block_body, locals, args, ctx);
            }
            continue;
        }

        // ── end (should not happen) ──────────────────
        if raw == "end" { continue; }

        // ── Normal command ─────────────────────────────────────────────────────
        // Cow: no allocation if no '$'
        let expanded = expand_vars(raw, locals, args);

        // Special guard: skip `cd ""` to avoid errors if $cwd is empty
        {
            let trimmed = expanded.trim();
            if trimmed == "cd" || trimmed.starts_with("cd ") {
                let target = trimmed.trim_start_matches("cd").trim().trim_matches('"').trim_matches('\'');
                if target.is_empty() {
                    // $cwd empty - ignore
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

/// Expand vars ($args, $0..$9, $var). Zero-copy if no '$'.
pub fn expand_vars<'a>(
    s:      &'a str,
    locals: &HashMap<String, String>,
    _args:  &[&str],
) -> Cow<'a, str> {
    // Fast-path: no '$' -> no allocation.
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

        // Collect variable name (alphanumeric + _)
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

        // Priority: locals -> env
        if let Some(v) = locals.get(&name) {
            out.push_str(v);
        } else if let Ok(v) = std::env::var(&name) {
            out.push_str(&v);
        }
        // Variable not found -> empty string (POSIX behavior)
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
    // Expand vars (locals + env) inside cmd_expr.
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
// Condition evaluator - native POSIX test
// ─────────────────────────────────────────────────────────────────────────────

/// Native POSIX test evaluator. Unrecognized conditions return false.
fn evaluate_condition(cond: &str) -> bool {
    let inner = cond.trim();

    // Strip [ ... ] wrapper if present
    let inner = if inner.starts_with('[') && inner.ends_with(']') {
        inner[1..inner.len() - 1].trim()
    } else {
        inner
    };

    // Simple tokenize by whitespace.
    let tokens: Vec<&str> = inner.split_whitespace().collect();

    match tokens.as_slice() {
        // [ -z "str" ] - true if string empty
        ["-z", val] => strip_quotes(val).is_empty(),
        // [ -n "str" ] - true if string not empty
        ["-n", val] => !strip_quotes(val).is_empty(),
        // [ -f "path" ] - true if regular file
        ["-f", path] => std::path::Path::new(strip_quotes(path)).is_file(),
        // [ -d "path" ] - true if directory
        ["-d", path] => std::path::Path::new(strip_quotes(path)).is_dir(),
        // [ -e "path" ] - true if exists (file/dir)
        ["-e", path] => std::path::Path::new(strip_quotes(path)).exists(),
        // [ "a" = "b" ] - string equality
        [a, "=", b]  => strip_quotes(a) == strip_quotes(b),
        // [ "a" != "b" ] - string inequality
        [a, "!=", b] => strip_quotes(a) != strip_quotes(b),
        // Unknown condition: safely return false.
        _ => {
            eprintln!("crush: wrapper: unsupported condition: {:?}", inner);
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
// Line runner - send line to executor
// ─────────────────────────────────────────────────────────────────────────────

/// Execute one line in executor context.
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
    0  // execute_line doesn't return exit code yet
}
