//! main — the process entry point: argv in, exit code out.

use std::path::PathBuf;

fn main() {
    let argv: Vec<String> = std::env::args().skip(1).collect();
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    std::process::exit(journeyspec::cli::run(&argv, &cwd));
}
