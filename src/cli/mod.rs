use crate::ui;
use anyhow::Result;
use clap::{Parser, Subcommand};

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
        /// Comma-separated regex patterns for directories to ignore (e.g., "node_modules,\.git")
        #[arg(long, short)]
        ignore: Option<String>,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match &cli.command {
        Commands::Ls {
            pattern,
            path,
            ignore,
        } => {
            // Use TUI with real-time scanning as default behavior
            ui::display_directories_with_scanning(pattern, path, ignore.as_deref().unwrap_or(""))?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
