//! xtask — build and release automation for ZapTerm.
//!
//! Invoke subcommands as:  `cargo xtask <subcommand> [args]`
//!
//! Available subcommands:
//!   build        — compile the workspace in debug or release mode
//!   dev          — start the Vite dev server and the Tauri app together
//!   package      — produce a distributable installer (MSI, dmg, AppImage)
//!   release      — full release pipeline: bump version, tag, build, package
//!   gen-bindings — generate TypeScript types from Rust types

mod build;
mod dev;
mod gen_bindings;
mod package;
mod release;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let subcommand = args.get(1).map(String::as_str).unwrap_or("help");

    let result = match subcommand {
        "build" => build::run(&args[2..]),
        "dev" => dev::run(&args[2..]),
        "package" => package::run(&args[2..]),
        "release" => release::run(&args[2..]),
        "gen-bindings" => gen_bindings::run(&args[2..]),
        "help" | "--help" | "-h" => {
            print_usage();
            return;
        }
        _ => {
            eprintln!("unknown subcommand: {subcommand}");
            print_usage();
            return;
        }
    };

    if let Err(e) = result {
        eprintln!("xtask error: {e}");
        std::process::exit(1);
    }
}

fn print_usage() {
    println!(
        "cargo xtask <subcommand>\n\
         \n\
         Subcommands:\n\
         \n\
         \x20 build        Compile the workspace (--release flag supported)\n\
         \x20 dev          Start Vite dev server + Tauri app in development mode\n\
         \x20 package      Produce a distributable installer for the current platform\n\
         \x20 release      Full release pipeline: bump, tag, build, package\n\
         \x20 gen-bindings Generate TypeScript types from Rust (requires specta or similar)\n\
         \x20 help         Print this message"
    );
}
