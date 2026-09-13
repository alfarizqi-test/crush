use std::env;
use std::path::PathBuf;

pub struct ShellInfo;

impl ShellInfo {
    pub fn init_environment() {
        let exe_path = match env::current_exe() {
            Ok(path) => path,
            Err(_) => {
                let prefix = env::var("PREFIX").unwrap_or_default();
                if prefix.is_empty() {
                    PathBuf::from("/usr/bin/omnishell")
                } else {
                    PathBuf::from(format!("{}/bin/omnishell", prefix))
                }
            }
        };

        let exe_string = exe_path.to_string_lossy().to_string();
        
        unsafe {
            env::set_var("SHELL", &exe_string);
            env::set_var("0", &exe_string);
        }
    }

    pub fn register_to_system() {
        let exe_path = env::current_exe().unwrap_or_default().to_string_lossy().to_string();
        
        // Detect Termux
        let prefix = env::var("PREFIX").unwrap_or_default();
        let etc_shells = if prefix.is_empty() {
            String::from("/etc/shells") // Standard Linux
        } else {
            format!("{}/etc/shells", prefix) // Android Termux
        };

        let content = std::fs::read_to_string(&etc_shells).unwrap_or_default();

        if !content.contains(&exe_path) {
            println!("To make the system fully recognize {}, add the following line:", exe_path);
            
            if prefix.is_empty() {
                println!("sudo sh -c 'echo \"{}\" >> {}'", exe_path, etc_shells);
            } else {
                println!("echo \"{}\" >> {}", exe_path, etc_shells);
            }
        } else {
            println!("Shell is already registered in {}!", etc_shells);
        }
    }
}
