use super::*;
use clap::CommandFactory;

#[test]
fn test_cli_creation() {
    // Test that CLI can be created without errors
    let _cli = Cli::command();
}

#[test]
fn test_ls_command_exists() {
    // Test that ls command exists and has correct name
    let cmd = Cli::command();
    let ls_cmd = cmd.get_subcommands()
        .find(|cmd| cmd.get_name() == "ls")
        .unwrap();
    
    assert_eq!(ls_cmd.get_name(), "ls");
}

#[test]
fn test_ls_default_arguments() {
    // Test ls command with default arguments
    let args = vec!["dir-kill", "ls", "test_pattern"];
    let cli = Cli::try_parse_from(args).unwrap();
    
    match cli.command {
        Commands::Ls { pattern, path } => {
            assert_eq!(pattern, "test_pattern");
            assert_eq!(path, ".");
        }
    }
}

#[test]
fn test_ls_with_path() {
    // Test ls command with custom path
    let args = vec!["dir-kill", "ls", "test_pattern", "/tmp"];
    let cli = Cli::try_parse_from(args).unwrap();
    
    match cli.command {
        Commands::Ls { pattern, path } => {
            assert_eq!(pattern, "test_pattern");
            assert_eq!(path, "/tmp");
        }
    }
}



#[test]
fn test_invalid_command() {
    // Test that invalid command returns error
    let args = vec!["dir-kill", "invalid"];
    let result = Cli::try_parse_from(args);
    assert!(result.is_err());
}

#[test]
fn test_missing_subcommand() {
    // Test that missing subcommand returns error
    let args = vec!["dir-kill"];
    let result = Cli::try_parse_from(args);
    assert!(result.is_err());
}

#[test]
fn test_help_flag() {
    // Test that help flag works
    let args = vec!["dir-kill", "--help"];
    let result = Cli::try_parse_from(args);
    assert!(result.is_err()); // Help should exit with error code
}

#[test]
fn test_ls_help_flag() {
    // Test that ls help flag works
    let args = vec!["dir-kill", "ls", "--help"];
    let result = Cli::try_parse_from(args);
    assert!(result.is_err()); // Help should exit with error code
}

#[test]
fn test_ls_with_pattern() {
    // Test ls command with pattern argument
    let args = vec!["dir-kill", "ls", "node_modules"];
    let cli = Cli::try_parse_from(args).unwrap();
    
    match cli.command {
        Commands::Ls { path, pattern } => {
            assert_eq!(path, ".");
            assert_eq!(pattern, "node_modules");
        }
    }
}

#[test]
fn test_ls_with_short_pattern() {
    // Test ls command with pattern argument
    let args = vec!["dir-kill", "ls", "src"];
    let cli = Cli::try_parse_from(args).unwrap();
    
    match cli.command {
        Commands::Ls { path, pattern } => {
            assert_eq!(path, ".");
            assert_eq!(pattern, "src");
        }
    }
} 