use anyhow::{Context, Result, bail};
use regex::Regex;
use std::fs;
use std::path::Path;

/// Ignore patterns for directory filtering
#[derive(Debug, Clone)]
pub struct IgnorePatterns {
    patterns: Vec<Regex>,
}

impl IgnorePatterns {
    /// Create new ignore patterns from comma-separated string
    ///
    /// # Arguments
    /// * `patterns_str` - Comma-separated regex patterns (e.g., "node_modules,\.git")
    ///
    /// # Returns
    /// * `Result<Self>` - Compiled ignore patterns or error
    pub fn new(patterns_str: &str) -> Result<Self> {
        if patterns_str.trim().is_empty() {
            return Ok(Self {
                patterns: Vec::new(),
            });
        }

        let mut patterns = Vec::new();
        for pattern in patterns_str.split(',') {
            let pattern = pattern.trim();
            if !pattern.is_empty() {
                let regex = Regex::new(pattern)
                    .with_context(|| format!("Invalid regex pattern: '{pattern}'"))?;
                patterns.push(regex);
            }
        }

        Ok(Self { patterns })
    }

    /// Check if a directory name should be ignored
    ///
    /// # Arguments
    /// * `dir_name` - Directory name to check
    ///
    /// # Returns
    /// * `bool` - True if directory should be ignored
    pub fn should_ignore(&self, dir_name: &str) -> bool {
        // Fast path for empty patterns
        if self.patterns.is_empty() {
            return false;
        }

        self.patterns
            .iter()
            .any(|pattern| pattern.is_match(dir_name))
    }

    /// Check if ignore patterns are empty
    ///
    /// # Returns
    /// * `bool` - True if no ignore patterns are set
    pub fn is_empty(&self) -> bool {
        self.patterns.is_empty()
    }

    /// Get the number of ignore patterns
    ///
    /// # Returns
    /// * `usize` - Number of ignore patterns
    pub fn len(&self) -> usize {
        self.patterns.len()
    }
}

/// Directory information with path, size, and last modified date
#[derive(Debug, Clone, PartialEq)]
pub struct DirectoryInfo {
    pub path: String,
    pub size: u64,
    pub formatted_size: String,
    pub last_modified: Option<std::time::SystemTime>,
    pub formatted_last_modified: String,
    pub selected: bool,
    pub deletion_status: DeletionStatus,
    pub calculation_status: CalculationStatus,
}

/// Status of directory deletion
#[derive(Debug, Clone, PartialEq)]
pub enum DeletionStatus {
    Normal,
    Deleting,
    Deleted,
    Error(String),
}

/// Status of directory size calculation
///
/// - NotStarted: waiting to be calculated (shows hourglass)
/// - Calculating: in progress (shows spinner)
/// - Completed: done (shows no icon)
/// - Error: failed (shows error icon)
#[derive(Debug, Clone, PartialEq)]
pub enum CalculationStatus {
    NotStarted,
    Calculating,
    Completed,
    Error(String),
}

/// Message for streaming directory discovery
#[derive(Debug, Clone)]
pub enum DiscoveryMessage {
    /// A new directory was found
    DirectoryFound(String),
    /// Discovery is complete
    DiscoveryComplete,
    /// An error occurred during discovery
    DiscoveryError(String),
}

/// Lists all directories matching the given pattern in the specified path
///
/// # Arguments
/// * `root_path` - The root directory to search in
/// * `pattern` - The directory name pattern to match (e.g., "node_modules")
///
/// # Returns
/// * `Result<Vec<String>>` - List of matching directory paths or error
pub fn find_directories(root_path: &str, pattern: &str) -> Result<Vec<String>> {
    find_directories_with_ignore(root_path, pattern, &IgnorePatterns::new("")?)
}

/// Lists all directories matching the given pattern in the specified path with ignore patterns
///
/// # Arguments
/// * `root_path` - The root directory to search in
/// * `pattern` - The directory name pattern to match (e.g., "node_modules")
/// * `ignore_patterns` - Patterns for directories to ignore
///
/// # Returns
/// * `Result<Vec<String>>` - List of matching directory paths or error
pub fn find_directories_with_ignore(
    root_path: &str,
    pattern: &str,
    ignore_patterns: &IgnorePatterns,
) -> Result<Vec<String>> {
    // Validate inputs
    if pattern.is_empty() {
        bail!("Pattern cannot be empty");
    }

    let path = Path::new(root_path);

    // Check if path exists
    if !path.exists() {
        bail!("Path '{}' does not exist", root_path);
    }

    // Check if path is a directory
    if !path.is_dir() {
        bail!("Path '{}' is not a directory", root_path);
    }

    let mut matches = Vec::new();

    // Recursively search for directories
    search_directories_recursive(path, pattern, ignore_patterns, &mut matches)?;

    Ok(matches)
}

/// Streams directory discovery results as they're found
///
/// # Arguments
/// * `root_path` - The root directory to search in
/// * `pattern` - The directory name pattern to match
/// * `sender` - Channel sender for streaming results
///
/// # Returns
/// * `Result<()>` - Success or error
pub fn stream_directories(
    root_path: &str,
    pattern: &str,
    sender: std::sync::mpsc::Sender<DiscoveryMessage>,
) -> Result<()> {
    stream_directories_with_ignore(root_path, pattern, &IgnorePatterns::new("")?, sender)
}

/// Streams directory discovery results as they're found with ignore patterns
///
/// # Arguments
/// * `root_path` - The root directory to search in
/// * `pattern` - The directory name pattern to match
/// * `ignore_patterns` - Patterns for directories to ignore
/// * `sender` - Channel sender for streaming results
///
/// # Returns
/// * `Result<()>` - Success or error
pub fn stream_directories_with_ignore(
    root_path: &str,
    pattern: &str,
    ignore_patterns: &IgnorePatterns,
    sender: std::sync::mpsc::Sender<DiscoveryMessage>,
) -> Result<()> {
    // Validate inputs
    if pattern.is_empty() {
        bail!("Pattern cannot be empty");
    }

    let path = Path::new(root_path);

    // Check if path exists
    if !path.exists() {
        bail!("Path '{}' does not exist", root_path);
    }

    // Check if path is a directory
    if !path.is_dir() {
        bail!("Path '{}' is not a directory", root_path);
    }

    // Start streaming discovery
    stream_directories_recursive(path, pattern, ignore_patterns, &sender)?;

    // Send completion message
    let _ = sender.send(DiscoveryMessage::DiscoveryComplete);

    Ok(())
}

/// Recursively streams directory discovery with immediate results
fn stream_directories_recursive(
    current_path: &Path,
    pattern: &str,
    ignore_patterns: &IgnorePatterns,
    sender: &std::sync::mpsc::Sender<DiscoveryMessage>,
) -> Result<()> {
    stream_directories_recursive_with_depth(current_path, pattern, ignore_patterns, sender, 0)
}

/// Recursively streams directory discovery with depth tracking to avoid nested pattern matches
fn stream_directories_recursive_with_depth(
    current_path: &Path,
    pattern: &str,
    ignore_patterns: &IgnorePatterns,
    sender: &std::sync::mpsc::Sender<DiscoveryMessage>,
    _depth: usize,
) -> Result<()> {
    for entry in std::fs::read_dir(current_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Get file name once to avoid redundant calls
            if let Some(file_name) = path.file_name() {
                let dir_name = file_name.to_string_lossy();

                // Skip if directory matches ignore patterns
                if ignore_patterns.should_ignore(&dir_name) {
                    continue;
                }

                // Check if this directory matches the pattern
                if dir_name == pattern {
                    let dir_path = path.to_string_lossy().to_string();
                    // Send immediately as found
                    if sender
                        .send(DiscoveryMessage::DirectoryFound(dir_path))
                        .is_err()
                    {
                        // Channel closed, stop discovery
                        return Ok(());
                    }
                }

                // Skip nested pattern matches to avoid infinite recursion and redundant results
                // If we're already inside a directory that matches our pattern, don't recurse into it
                if dir_name == pattern {
                    // Skip recursing into this directory to avoid nested results
                    continue;
                }
            }

            // Recursively search subdirectories
            stream_directories_recursive_with_depth(
                &path,
                pattern,
                ignore_patterns,
                sender,
                _depth + 1,
            )?;
        }
    }

    Ok(())
}

/// Lists all directories matching the given pattern with size information
///
/// # Arguments
/// * `root_path` - The root directory to search in
/// * `pattern` - The directory name pattern to match (e.g., "node_modules")
///
/// # Returns
/// * `Result<Vec<DirectoryInfo>>` - List of matching directories with size info or error
pub fn find_directories_with_size(root_path: &str, pattern: &str) -> Result<Vec<DirectoryInfo>> {
    find_directories_with_size_and_ignore(root_path, pattern, &IgnorePatterns::new("")?)
}

/// Lists all directories matching the given pattern with size information and ignore patterns
///
/// # Arguments
/// * `root_path` - The root directory to search in
/// * `pattern` - The directory name pattern to match (e.g., "node_modules")
/// * `ignore_patterns` - Patterns for directories to ignore
///
/// # Returns
/// * `Result<Vec<DirectoryInfo>>` - List of matching directories with size info or error
pub fn find_directories_with_size_and_ignore(
    root_path: &str,
    pattern: &str,
    ignore_patterns: &IgnorePatterns,
) -> Result<Vec<DirectoryInfo>> {
    let directories = find_directories_with_ignore(root_path, pattern, ignore_patterns)?;
    let mut directory_infos = Vec::new();

    for dir_path in directories {
        let path = Path::new(&dir_path);
        let size = calculate_directory_size(path).unwrap_or(0);
        let formatted_size = format_size(size);
        // Get last modified time for the parent directory (not the matching directory itself)
        let parent_path = path.parent().unwrap_or(path);
        let last_modified = get_directory_last_modified(parent_path);
        let formatted_last_modified = last_modified
            .as_ref()
            .map(format_last_modified)
            .unwrap_or_else(|| "Unknown".to_string());

        directory_infos.push(DirectoryInfo {
            path: dir_path,
            size,
            formatted_size,
            last_modified,
            formatted_last_modified,
            selected: false,
            deletion_status: DeletionStatus::Normal,
            calculation_status: CalculationStatus::Completed,
        });
    }

    // Sort by size (largest first)
    directory_infos.sort_by(|a, b| b.size.cmp(&a.size));

    Ok(directory_infos)
}

fn search_directories_recursive(
    current_path: &Path,
    pattern: &str,
    ignore_patterns: &IgnorePatterns,
    matches: &mut Vec<String>,
) -> Result<()> {
    search_directories_recursive_with_depth(current_path, pattern, ignore_patterns, matches, 0)
}

/// Recursively searches directories with depth tracking to avoid nested pattern matches
fn search_directories_recursive_with_depth(
    current_path: &Path,
    pattern: &str,
    ignore_patterns: &IgnorePatterns,
    matches: &mut Vec<String>,
    _depth: usize,
) -> Result<()> {
    for entry in std::fs::read_dir(current_path)? {
        let entry = entry?;
        let path = entry.path();

        if path.is_dir() {
            // Get file name once to avoid redundant calls
            if let Some(file_name) = path.file_name() {
                let dir_name = file_name.to_string_lossy();

                // Skip if directory matches ignore patterns
                if ignore_patterns.should_ignore(&dir_name) {
                    continue;
                }

                // Check if this directory matches the pattern
                if dir_name == pattern {
                    matches.push(path.to_string_lossy().to_string());
                }

                // Skip nested pattern matches to avoid infinite recursion and redundant results
                // If we're already inside a directory that matches our pattern, don't recurse into it
                if dir_name == pattern {
                    // Skip recursing into this directory to avoid nested results
                    continue;
                }
            }

            // Recursively search subdirectories
            search_directories_recursive_with_depth(
                &path,
                pattern,
                ignore_patterns,
                matches,
                _depth + 1,
            )?;
        }
    }

    Ok(())
}

/// Lists all directories matching the given pattern in the current directory
pub fn find_directories_current(pattern: &str) -> Result<Vec<String>> {
    find_directories(".", pattern)
}

/// Calculate the total size of a directory in bytes
pub fn calculate_directory_size(path: &Path) -> Result<u64> {
    let mut total_size = 0u64;

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();

            if entry_path.is_file() {
                total_size += entry.metadata()?.len();
            } else if entry_path.is_dir() {
                total_size += calculate_directory_size(&entry_path)?;
            }
        }
    }

    Ok(total_size)
}

/// Get the last modified time of a directory
pub fn get_directory_last_modified(path: &Path) -> Option<std::time::SystemTime> {
    match fs::metadata(path) {
        Ok(metadata) => metadata.modified().ok(),
        Err(_) => None,
    }
}

/// Format last modified time in a human-readable format
pub fn format_last_modified(time: &std::time::SystemTime) -> String {
    use chrono::{DateTime, Local};

    let datetime: DateTime<Local> = DateTime::from(*time);
    let now = Local::now();
    let duration = now.signed_duration_since(datetime);

    if duration.num_days() > 0 {
        if duration.num_days() == 1 {
            "1 day ago".to_string()
        } else if duration.num_days() < 7 {
            format!("{} days ago", duration.num_days())
        } else if duration.num_days() < 30 {
            let weeks = duration.num_days() / 7;
            if weeks == 1 {
                "1 week ago".to_string()
            } else {
                format!("{weeks} weeks ago")
            }
        } else if duration.num_days() < 365 {
            let months = duration.num_days() / 30;
            if months == 1 {
                "1 month ago".to_string()
            } else {
                format!("{months} months ago")
            }
        } else {
            let years = duration.num_days() / 365;
            if years == 1 {
                "1 year ago".to_string()
            } else {
                format!("{years} years ago")
            }
        }
    } else if duration.num_hours() > 0 {
        if duration.num_hours() == 1 {
            "1 hour ago".to_string()
        } else {
            format!("{} hours ago", duration.num_hours())
        }
    } else if duration.num_minutes() > 0 {
        if duration.num_minutes() == 1 {
            "1 minute ago".to_string()
        } else {
            format!("{} minutes ago", duration.num_minutes())
        }
    } else {
        "Just now".to_string()
    }
}

/// Format bytes into human-readable format
pub fn format_size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", bytes, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}

#[cfg(test)]
mod tests;
