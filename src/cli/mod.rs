use clap::{Parser, Subcommand};
use crate::ui;
use anyhow::Result;

#[derive(Parser)]
#[command(name = "dir-kill")]
#[command(about = "A directory management tool")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// List files and directories
    Ls {
        /// Pattern to match directory names (e.g., "node_modules")
        pattern: String,
        /// Directory to list (defaults to current directory)
        #[arg(default_value = ".")]
        path: String,
        /// Show hidden files
        #[arg(short, long)]
        all: bool,
        /// Use long listing format
        #[arg(short, long)]
        long: bool,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Ls { pattern, path, all: _, long: _ } => {
            // Use TUI with real-time scanning as default behavior
            ui::display_directories_with_scanning(pattern, path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests; 