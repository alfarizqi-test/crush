mod shell;

#[allow(unused_imports)]
use std::env;
use std::fs::{File, OpenOptions};
use std::io::{self, Write};
use std::path::Path;
use std::process::Command;
use which::which;

fn expand_variables(arg: &str) -> String {
    if arg == "$$" {
        return std::process::id().to_string();
    }

    if arg.starts_with('$') && arg.len() > 1 {
        let var_name = &arg[1..];
        
        if let Ok(value) = std::env::var(var_name) {
            return value;
        } else {
            return String::new();
        }
    }

    arg.to_string()
}

fn expand_tilde(path: &str) -> String {
    if path.starts_with('~') {
        if let Ok(home) = env::var("HOME") {
            return path.replacen('~', &home, 1);
        }
    }
    path.to_string()
}

fn main() {
    shell::ShellInfo::init_environment();

    if let Some(arg) = std::env::args().nth(1) {
        if arg == "--check" {
            shell::ShellInfo::register_to_system();
            return;
        }
    }

    loop {
        
        if let Ok(current_dir) = env::current_dir() {
            println!("[ {} ]", current_dir.display());
            print!("   ");
        } else {
            print!("$ ");
        }
        
        // print!("$ ");
        io::stdout().flush().unwrap();

        // Wait for user input
        let mut input = String::new();
        io::stdin().read_line(&mut input).unwrap();

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        let parsed_args = match shlex::split(input) {
            Some(args) => args,
            None => {
                eprintln!("Error: quotation marks aren't closed properly!");
                continue;
            }
        };

        let args: Vec<&str> = parsed_args.iter().map(|s| s.as_str()).collect();

        let mut redirect_idx = None;
        let mut is_stderr = false;
        let mut is_append = false;

        for (i, &arg) in args.iter().enumerate() {
            if arg == ">" || arg == "1>" {
                redirect_idx = Some(i);
                is_stderr = false;
                is_append = false;
                break;
            } else if arg =="2>" {
                redirect_idx = Some(i);
                is_stderr = true;
                is_append = false;
                break;
            } else if arg == ">>" || arg =="1>>" {
                redirect_idx = Some(i);
                is_stderr = false;
                is_append = true;
                break;
            } else if arg =="2>>" {
                redirect_idx = Some(i);
                is_stderr = true;
                is_append = true;
                break;
            }
        }

        let mut clean_args = args.clone();
        let mut target_file = None;

        if let Some(idx) = redirect_idx {
            if idx + 1 < args.len() {
                target_file = Some(args[idx + 1]);
            }
            clean_args.truncate(idx);
        }

        let expanded_args: Vec<String> = clean_args
            .iter()
            .map(|&arg| expand_variables(arg))
            .collect();
        
        let clean_args_refs: Vec<&str> = expanded_args
            .iter()
            .map(|s| s.as_str())
            .collect();

        match clean_args_refs.as_slice() {
            ["exit"] => break,
            ["echo"] => println!(),
            ["echo", echo_args @ ..] => {
                let output = echo_args.join(" ");

                if let Some(file_path) = target_file {
                    let file_result = if is_append {
                        OpenOptions::new().create(true).append(true).open(file_path)
                    } else {
                        File::create(file_path)
                    };

                    if let Ok(mut file) = file_result {
                        if is_stderr {
                            println!("{}", output);
                        } else {
                            writeln!(file, "{}", output).unwrap();
                        }
                    }
                } else {
                    println!("{}", output);
                }
            }
            ["pwd", ..] => {
                match env::current_dir() {
                    Ok(dir) => println!("{}", dir.display()),
                    Err(e) => eprintln!("pwd: failed to get directory {}", e),
                }
            }
            ["cd"] => {
                let home = env::var("HOME").unwrap_or_else(|_| String::from("/"));
                let root = Path::new(&home);
                if let Err(e) = env::set_current_dir(&root) {
                    eprintln!("cd: {}", e);
                }
            }
            ["cd", path] => {
                let expanded_path = expand_tilde(path);
                let root = Path::new(&expanded_path);

                if let Err(_) = env::set_current_dir(&root) {
                    eprintln!("cd: {}: No such file or directory", path);
                }
            }
            ["cd", ..] => {
                eprintln!("cd: too many arguments");
            }
            ["type", args @ ("echo" | "exit" | "type" | "pwd")] => println!("{args} is a shell builtin"),
            ["type", arg] if which(arg).is_ok() => {
                let path = which(arg).unwrap();
                println!("{arg} is {}", path.display());
            },
            ["type", args @ ..] => println!("{}: not found", args[0]),
            [cmd, ext_args @ ..] => {
                let mut process = Command::new(cmd);
                process.args(ext_args);

                if let Some(file_path) = target_file {
                    let file_result = if is_append {
                        OpenOptions::new().create(true).append(true).open(file_path)
                    } else {
                        File::create(file_path)
                    };

                    if let Ok(file) = file_result {
                        if is_stderr {
                            process.stderr(file);
                        } else {
                            process.stdout(file);
                        }
                    }
                }

                match process.spawn() {
                    Ok(mut child) => {
                        child.wait().unwrap();
                    }
                    Err(_) => {
                        println!("{}: command not found", cmd);
                    }
                }
            }
            _ => {}
        }
    }
}
