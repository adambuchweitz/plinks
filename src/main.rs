use std::io;

use clap::Parser;
use plinks::cli::Cli;
use plinks::clipboard::SystemClipboard;
use plinks::open_link::SystemOpener;

fn main() {
    if let Err(err) = run() {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let cwd = std::env::current_dir()?;
    let opener = SystemOpener;
    let clipboard = SystemClipboard;
    let mut stdout = io::stdout();
    plinks::run(cli, &cwd, &opener, &clipboard, &mut stdout)
}
