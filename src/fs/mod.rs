use std::fs;
use std::path::Path;
use anyhow::{Result, bail};

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

/// Lists all directories matching the given pattern in the specified path
/// 
/// # Arguments
/// * `root_path` - The root directory to search in
/// * `pattern` - The directory name pattern to match (e.g., "node_modules")
/// 
/// # Returns
/// * `Result<Vec<String>>` - List of matching directory paths or error
pub fn find_directories(root_path: &str, pattern: &str) -> Result<Vec<String>> {
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
    search_directories_recursive(path, pattern, &mut matches)?;
    
    Ok(matches)
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
    let directories = find_directories(root_path, pattern)?;
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
            .map(|time| format_last_modified(time))
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
    matches: &mut Vec<String>
) -> Result<()> {
    for entry in std::fs::read_dir(current_path)? {
        let entry = entry?;
        let path = entry.path();
        
        if path.is_dir() {
            // Check if this directory matches the pattern
            if let Some(file_name) = path.file_name() {
                if file_name.to_string_lossy() == pattern {
                    matches.push(path.to_string_lossy().to_string());
                }
            }
            
            // Skip nested node_modules to avoid infinite recursion
            if pattern == "node_modules" && path.file_name().map_or(false, |name| name == "node_modules") {
                // If we're already inside a node_modules directory and looking for node_modules,
                // skip recursing into this directory to avoid nested results
                continue;
            }
            
            // Recursively search subdirectories
            search_directories_recursive(&path, pattern, matches)?;
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
                format!("{} weeks ago", weeks)
            }
        } else if duration.num_days() < 365 {
            let months = duration.num_days() / 30;
            if months == 1 {
                "1 month ago".to_string()
            } else {
                format!("{} months ago", months)
            }
        } else {
            let years = duration.num_days() / 365;
            if years == 1 {
                "1 year ago".to_string()
            } else {
                format!("{} years ago", years)
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