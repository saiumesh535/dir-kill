use anyhow::{Context, Result, bail};
#[cfg(test)]
use filesize::PathExt;
use rayon::prelude::*;
use regex::Regex;
use std::ffi::OsStr;
use std::fs;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, mpsc};

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

    /// Check if a directory name should be ignored (OsStr variant avoids allocation on hot path)
    pub fn should_ignore_name(&self, dir_name: &OsStr) -> bool {
        if self.patterns.is_empty() {
            return false;
        }
        self.should_ignore(&dir_name.to_string_lossy())
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
    pub calculation_time: Option<std::time::Duration>,
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

    walk_matching_directories_collect(path, pattern, ignore_patterns, &mut matches)?;

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

    // Start streaming discovery (parallel walk, like npkill's worker pool)
    walk_matching_directories_streaming(path, pattern, ignore_patterns, &sender)?;

    // Send completion message
    let _ = sender.send(DiscoveryMessage::DiscoveryComplete);

    Ok(())
}

/// Parallel discovery / concurrent size-calc thread budget.
///
/// Sizing many trees is IO-bound on SSD; a slightly higher concurrency than dua's
/// interactive default (3 on macOS) finishes large `~/Developer` scans sooner.
pub fn scan_thread_count() -> usize {
    std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(4)
        .clamp(4, 8)
}

fn discovery_parallelism() -> usize {
    scan_thread_count()
}

/// Long-lived pool for discovery walks (avoids RayonNewPool create/teardown per scan).
fn discovery_pool() -> std::sync::Arc<rayon::ThreadPool> {
    use std::sync::{Arc, OnceLock};
    static POOL: OnceLock<Arc<rayon::ThreadPool>> = OnceLock::new();
    POOL.get_or_init(|| {
        Arc::new(
            rayon::ThreadPoolBuilder::new()
                .num_threads(discovery_parallelism())
                .thread_name(|i| format!("dir-kill-discover-{i}"))
                .build()
                .expect("discovery pool"),
        )
    })
    .clone()
}

/// Heavy directories we never need to descend into while searching for `pattern`
/// (unless `pattern` itself is that name). Skipping these is the biggest discovery win
/// under trees like `~/Developer` (Rust `target`, Next `.next`, etc.).
fn should_prune_unrelated(name: &OsStr, pattern: &str) -> bool {
    if name == OsStr::new(pattern) {
        return false;
    }
    // .git is handled separately so we can still search for it
    matches!(
        name.to_str(),
        Some("target")
            | Some("dist")
            | Some("build")
            | Some("out")
            | Some("coverage")
            | Some("__pycache__")
            | Some("site-packages")
            | Some("bower_components")
            | Some(".next")
            | Some(".nuxt")
            | Some(".output")
            | Some(".cache")
            | Some(".turbo")
            | Some(".parcel-cache")
            | Some(".gradle")
            | Some(".yarn")
            | Some(".pnpm-store")
            | Some(".cargo")
            | Some(".rustup")
            | Some(".tox")
            | Some(".mypy_cache")
            | Some("Pods")
            | Some("DerivedData")
            | Some("vendor")
            | Some(".venv")
            | Some("venv")
            | Some("node_modules") // when searching for something else
    )
}

fn walk_matching_directories_collect(
    root: &Path,
    pattern: &str,
    ignore_patterns: &IgnorePatterns,
    matches: &mut Vec<String>,
) -> Result<()> {
    let (tx, rx) = mpsc::channel::<DiscoveryMessage>();
    walk_matching_directories_parallel(root, pattern, ignore_patterns, &tx)?;
    let _ = tx.send(DiscoveryMessage::DiscoveryComplete);
    for msg in rx {
        match msg {
            DiscoveryMessage::DirectoryFound(path) => matches.push(path),
            DiscoveryMessage::DiscoveryComplete => break,
            DiscoveryMessage::DiscoveryError(_) => break,
        }
    }
    Ok(())
}

fn walk_matching_directories_streaming(
    root: &Path,
    pattern: &str,
    ignore_patterns: &IgnorePatterns,
    sender: &Sender<DiscoveryMessage>,
) -> Result<()> {
    walk_matching_directories_parallel(root, pattern, ignore_patterns, sender)
}

fn walk_matching_directories_parallel(
    root: &Path,
    pattern: &str,
    ignore_patterns: &IgnorePatterns,
    sender: &Sender<DiscoveryMessage>,
) -> Result<()> {
    let root_buf = root.to_path_buf();
    let ignore = ignore_patterns.clone();
    let pattern_for_prune = pattern.to_string();
    let stop = Arc::new(AtomicBool::new(false));
    let sender = sender.clone();
    // Searching for a non-dot name (e.g. node_modules): skip hidden dirs entirely.
    let skip_hidden = !pattern.starts_with('.');

    let walker = jwalk::WalkDir::new(root)
        .follow_links(false)
        .skip_hidden(skip_hidden)
        .parallelism(jwalk::Parallelism::RayonExistingPool {
            pool: discovery_pool(),
            busy_timeout: None,
        })
        .process_read_dir(move |_depth, _path, _read_dir, children| {
            children.retain(|entry| {
                let Ok(entry) = entry else {
                    return true;
                };

                if !entry.file_type().is_dir() {
                    return false;
                }

                let name = entry.file_name();
                if ignore.should_ignore_name(name) {
                    return false;
                }

                // Skip VCS metadata dirs unless that is the search target
                if name == OsStr::new(".git") && pattern_for_prune != ".git" {
                    return false;
                }

                if should_prune_unrelated(name, &pattern_for_prune) {
                    return false;
                }

                if name == OsStr::new(&pattern_for_prune) {
                    if entry.path() != root_buf
                        && !stop.load(Ordering::Relaxed)
                        && sender
                            .send(DiscoveryMessage::DirectoryFound(
                                entry.path().to_string_lossy().into_owned(),
                            ))
                            .is_err()
                    {
                        stop.store(true, Ordering::Relaxed);
                    }
                    return false;
                }

                true
            })
        });

    for entry in walker {
        if entry.is_err() {
            continue;
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

    // Process directories in parallel for better performance
    let mut directory_infos: Vec<DirectoryInfo> = directories
        .par_iter()
        .map(|dir_path| {
            let path = Path::new(dir_path);
            // Use optimized size calculation with timing
            let (size, calculation_time) = calculate_directory_size_with_timing(path)
                .unwrap_or((0, std::time::Duration::ZERO));
            let formatted_size = format_size(size);
            // Get last modified time for the parent directory (not the matching directory itself)
            let parent_path = path.parent().unwrap_or(path);
            let last_modified = get_directory_last_modified(parent_path);
            let formatted_last_modified = last_modified
                .as_ref()
                .map(format_last_modified)
                .unwrap_or_else(|| "Unknown".to_string());

            DirectoryInfo {
                path: dir_path.clone(),
                size,
                formatted_size,
                last_modified,
                formatted_last_modified,
                selected: false,
                deletion_status: DeletionStatus::Normal,
                calculation_status: CalculationStatus::Completed,
                calculation_time: Some(calculation_time),
            }
        })
        .collect();

    // Sort by size (largest first)
    directory_infos.sort_by_key(|b| std::cmp::Reverse(b.size));

    Ok(directory_infos)
}

/// Lists all directories matching the given pattern in the current directory
pub fn find_directories_current(pattern: &str) -> Result<Vec<String>> {
    find_directories(".", pattern)
}

/// Calculate the total size of a directory in bytes (recursive read_dir baseline).
#[cfg(test)]
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

/// Calculate the total size of a directory using disk-allocated size (recursive).
#[cfg(test)]
pub fn calculate_directory_size_optimized(path: &Path) -> Result<u64> {
    let mut total_size = 0u64;

    if path.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let entry_path = entry.path();

            if entry_path.is_file() {
                if let Ok(metadata) = entry.metadata() {
                    total_size += entry_path
                        .size_on_disk_fast(&metadata)
                        .unwrap_or(metadata.len());
                }
            } else if entry_path.is_dir() {
                total_size += calculate_directory_size_optimized(&entry_path)?;
            }
        }
    }

    Ok(total_size)
}

/// Calculate directory size with nested rayon (test/benchmark only — pathological on wide trees).
#[cfg(test)]
pub fn calculate_directory_size_parallel(path: &Path) -> Result<u64> {
    if !path.is_dir() {
        return Ok(0);
    }

    let entries: Vec<_> = fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;

    let total_size: u64 = entries
        .par_iter()
        .map(|entry| {
            let entry_path = &entry.path();

            if entry_path.is_file() {
                if let Ok(metadata) = entry.metadata() {
                    entry_path
                        .size_on_disk_fast(&metadata)
                        .unwrap_or(metadata.len())
                } else {
                    0
                }
            } else if entry_path.is_dir() {
                calculate_directory_size_parallel(entry_path).unwrap_or(0)
            } else {
                0
            }
        })
        .sum();

    Ok(total_size)
}

/// Calculate directory size (apparent / logical size).
///
/// On macOS this uses `getattrlistbulk` for batched metadata. Elsewhere it uses
/// jwalk with sizes accumulated in `process_read_dir` (files are not yielded).
pub fn calculate_directory_size_jwalk(path: &Path) -> Result<u64> {
    #[cfg(target_os = "macos")]
    {
        size_macos::calculate_directory_size(path)
    }
    #[cfg(not(target_os = "macos"))]
    {
        calculate_directory_size_jwalk_fallback(path)
    }
}

/// Cross-platform jwalk size path: sum file lengths inside `process_read_dir`
/// and only descend into directories (no per-file iterator traffic).
#[cfg(any(test, not(target_os = "macos")))]
pub(crate) fn calculate_directory_size_jwalk_fallback(path: &Path) -> Result<u64> {
    if !path.is_dir() {
        return Ok(0);
    }

    let total_size = Arc::new(std::sync::atomic::AtomicU64::new(0));

    let walker = jwalk::WalkDir::new(path)
        .follow_links(false)
        .skip_hidden(false)
        .parallelism(jwalk::Parallelism::Serial)
        .process_read_dir({
            let total_size = Arc::clone(&total_size);
            move |_depth, _path, _state, children| {
                children.retain(|entry| {
                    let Ok(entry) = entry else {
                        return true;
                    };
                    if entry.file_type().is_file() {
                        if let Ok(meta) = entry.metadata() {
                            total_size.fetch_add(meta.len(), Ordering::Relaxed);
                        }
                        return false;
                    }
                    entry.file_type().is_dir()
                });
            }
        });

    // Drive recursion only — file sizes were already accumulated above.
    for entry in walker {
        let _ = entry;
    }

    Ok(total_size.load(Ordering::Relaxed))
}

/// Calculate directory size with timing information
pub fn calculate_directory_size_with_timing(path: &Path) -> Result<(u64, std::time::Duration)> {
    let start_time = std::time::Instant::now();
    let size = calculate_directory_size_jwalk(path)?;
    let duration = start_time.elapsed();
    Ok((size, duration))
}

/// Format duration in a human-readable format
pub fn format_duration(duration: &std::time::Duration) -> String {
    if duration.as_secs() > 0 {
        if duration.as_secs() == 1 {
            "1 second".to_string()
        } else {
            format!("{} seconds", duration.as_secs())
        }
    } else if duration.as_millis() > 0 {
        if duration.as_millis() == 1 {
            "1 millisecond".to_string()
        } else {
            format!("{} milliseconds", duration.as_millis())
        }
    } else if duration.as_micros() > 0 {
        if duration.as_micros() == 1 {
            "1 microsecond".to_string()
        } else {
            format!("{} microseconds", duration.as_micros())
        }
    } else {
        format!("{} nanoseconds", duration.as_nanos())
    }
}

/// Format duration using fractional seconds (for total wall-clock timing)
pub fn format_duration_in_seconds(duration: &std::time::Duration) -> String {
    let secs = duration.as_secs_f64();
    if secs >= 10.0 {
        format!("{secs:.0} seconds")
    } else if secs >= 1.0 {
        format!("{secs:.1} seconds")
    } else if secs > 0.0 {
        format!("{secs:.2} seconds")
    } else {
        "0 seconds".to_string()
    }
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

#[cfg(target_os = "macos")]
mod size_macos;

#[cfg(test)]
mod tests;
