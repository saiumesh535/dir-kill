pub mod cli;
pub mod fs;
pub mod ui;

use anyhow::Result;

fn main() -> Result<()> {
    if let Err(e) = cli::run() {
        eprintln!("Error: {:#}", e);
        std::process::exit(1);
    }
    Ok(())
}
