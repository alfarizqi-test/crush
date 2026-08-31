<h1 align="center">Crush</h1>
<p align="center">A highly customizable, fast, and modern shell written in Rust.</p>
<p align="center">
    <a href="https://github.com/username/crush/blob/main/LICENSE"><img alt="GitHub License" src="https://img.shields.io/github/license/username/crush"></a>
    <a href="https://github.com/username/crush/actions"><img alt="Build Status" src="https://img.shields.io/github/actions/workflow/status/username/crush/ci.yml?branch=main"></a>
    <a href="https://github.com/username/crush/releases"><img alt="GitHub Release" src="https://img.shields.io/github/v/release/username/crush?logo=github"></a>
</p>

<p align="center">
    <a href="#about">About</a> | <a href="#features">Features</a> | <a href="#installation">Installation</a> | <a href="#configuration">Configuration</a>
</p>

![Screenshot demo](https://via.placeholder.com/1280x720.png?text=Crush+Shell+Demo)

## About

Crush is a modern, fast, and customizable command-line shell written entirely in Rust. It aims to combine the raw performance and memory safety of Rust with a highly extensible and user-friendly configuration system. 

Designed for power users, Crush provides a built-in Starship-like prompt engine, robust job control, pipeline handling, and complex logic operators, all while maintaining a minimal footprint.

## Features

- **Modern Execution Engine**: Seamless support for pipelines (`|`), logical operators (`&&`, `||`), background jobs (`&`), and redirection (`>`, `>>`, `2>`, `2>>`).
- **Built-in Prompt Engine**: A powerful, Starship-inspired prompt system with Git integration (branch, commit, status), command duration tracking, and rich color/styling support.
- **Advanced Configuration**: Everything is driven by a centralized TOML configuration file, including custom aliases, environment variables, and themes.
- **Directory Aliases**: Easily navigate to frequently used directories using `~alias` syntax (e.g., `cd ~crush`).
- **Custom Keybindings**: Map keyboard shortcuts to built-in actions (like clearing the screen or searching history) or execute complex shell functions directly (e.g., `Ctrl+T` to open a file manager).
- **Wrapper Functions**: Define custom shell functions directly in your TOML config, allowing you to wrap external commands or create complex macros.
- **Event Hooks**: Automate your workflow with lifecycle hooks like `on_cd` and `pre_command`.
- **Rustyline Integration**: Enjoy familiar readline-like features out of the box, including history search, auto-completion, and Emacs/Vi mode support.

## Installation

### Building from Source

Crush is built using Rust's package manager, Cargo. Make sure you have the latest stable Rust toolchain installed.

1. Clone the repository:
   ```bash
   git clone https://github.com/username/crush.git
   cd crush
   ```
2. Build the release binary:
   ```bash
   cargo build --release
   ```
3. Install it to your Cargo bin directory:
   ```bash
   cargo install --path .
   ```

### Dependencies
Crush depends on the following core libraries:
- `rustyline` for advanced CLI input, history, and completions.
- `toml` and `serde` for configuration management.
- `shlex` for POSIX-compliant argument parsing.
- `libc` for Unix process and signal management.

## Usage / Configuration

Crush is configured via a TOML file, typically located in your configuration directory (e.g., `~/.config/crush/config.toml`).

A comprehensive `config-example.toml` is provided in the repository. Key sections include:

- `[env]`: Set environment variables like `EDITOR` or `PATH`.
- `[aliases]`: Define quick command aliases.
- `[dir_aliases]`: Define directory shortcuts.
- `[bindings]`: Map key combinations to actions (e.g., `"Ctrl+L" = "clear_screen"`).
- `[prompt]`: Customize your prompt layout, symbols, and colors.
- `[functions.<name>]`: Create wrapper scripts and custom commands in TOML.

To reload your configuration on the fly, simply run the built-in `reload` command.

## Built-ins

Crush provides essential built-in commands natively for speed and integration:
`cd`, `pwd`, `echo`, `type`, `history`, `clear`, `export`, `unset`, `source`, `jobs`, `ls`, `help`, `rehash`, and `reload`.

## Contributing

We welcome contributions! Whether you're fixing bugs, adding new features, or improving documentation, your help is appreciated. 
Please feel free to open an issue or submit a pull request on our GitHub repository.

## License

This project is open-source and available under the terms of the [MIT License](LICENSE).