<h1 align="center">Crush</h1>
<p align="center">An ultra-lightweight, fast, and modern shell written in Rust.</p>
<p align="center">
    <a href="https://github.com/alfarizqi-test/crush/blob/main/LICENSE"><img alt="GitHub License" src="https://img.shields.io/github/license/alfarizqi-test/crush"></a>
    <a href="https://github.com/alfarizqi-test/crush/actions"><img alt="Build Status" src="https://img.shields.io/github/actions/workflow/status/alfarizqi-test/crush/ci.yml?branch=main"></a>
    <a href="https://github.com/alfarizqi-test/crush/releases"><img alt="GitHub Release" src="https://img.shields.io/github/v/release/alfarizqi-test/crush?logo=github"></a>
</p>

<p align="center">
    <a href="#about">About</a> | <a href="#features">Features</a> | <a href="#installation">Installation</a> | <a href="#configuration">Configuration</a>
</p>

![Screenshot demo](https://via.placeholder.com/1280x720.png?text=Crush+Shell+Demo)

## About

Crush is a modern, fast, and customizable command-line shell written entirely in Rust. Built with a strong **suckless** philosophy, it aims to combine the raw performance and memory safety of Rust with a highly extensible configuration system—all while maintaining an extreme minimal footprint.

Designed for power users, Crush provides a built-in Starship-like prompt engine, robust job control, and pipeline handling, yet it consumes **only ~2.7 MB of RAM** and compiles to a binary size of **less than 2 MB**.

## Features

- **Extreme Minimalist & Suckless**: Idles at ~2.7 MB of RAM. Uses the strict `toml v0.5` parser to avoid the bloat of modern document-editing parsers.
- **Modern Execution Engine**: Seamless support for pipelines (`|`), logical operators (`&&`, `||`), background jobs (`&`), and redirection (`>`, `>>`, `2>`, `2>>`) powered by a non-blocking Polling Wait executor.
- **Built-in Prompt Engine**: A powerful, Starship-inspired prompt system with Git integration (branch, commit, status), command duration tracking, and rich color/styling support (including Catppuccin palettes).
- **Advanced Configuration**: Everything is driven by a centralized TOML configuration file, including custom aliases, environment variables, and themes.
- **Directory Aliases**: Easily navigate to frequently used directories using `~alias` syntax (e.g., `cd ~crush`).
- **Custom Keybindings**: Map keyboard shortcuts to built-in actions (like clearing the screen or searching history) or execute complex shell functions directly (e.g., `Ctrl+T` to open a file manager like Yazi).
- **Wrapper Functions**: Define custom shell functions directly in your TOML config, allowing you to wrap external commands or create complex macros.
- **Event Hooks**: Automate your workflow with lifecycle hooks like `on_cd` and `pre_command`.
- **Rustyline Integration**: Enjoy familiar readline-like features out of the box, including history search, auto-completion, and Emacs/Vi mode support.

## Installation

### Arch Linux / Artix (Recommended)

For Arch-based distributions, you can install the pre-compiled binary and automatically register Crush to `/etc/shells` using the provided `PKGBUILD`.

1. Download the `PKGBUILD` and `crush.install` files from the source tree.
2. Build and install the package using `makepkg`:
   ```bash
   makepkg -si
   ```

### Building from Source

Crush is built using Rust's package manager, Cargo. Make sure you have the latest stable Rust toolchain installed.

1. Clone the repository:
   ```bash
   git clone https://github.com/alfarizqi-test/crush.git
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

- `[shell]`: Core shell behavior settings (e.g., greeting, newline before prompt).
- `[env]`: Set environment variables like `EDITOR` or `PATH`.
- `[aliases]`: Define quick command aliases.
- `[dir_aliases]`: Define directory shortcuts.
- `[bindings]`: Map key combinations to actions. Available actions are: `clear_screen`, `search_history`, `exit_shell`, `accept_line`, `move_home`, `move_end`, `complete_hint`, `complete_list`, and `execute: <cmd>`. (e.g., `"Ctrl+L" = "clear_screen"`).
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