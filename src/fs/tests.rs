use super::*;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_find_directories_empty_result() {
    // Test that non-existent pattern returns empty result
    let result = find_directories(".", "non_existent_pattern");
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), vec![] as Vec<String>);
}

#[test]
fn test_find_directories_invalid_path() {
    // Test that invalid path returns error
    let result = find_directories("/non/existent/path", "pattern");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("does not exist"));
}

#[test]
fn test_find_directories_current_directory() {
    // Test finding directories in current directory
    let result = find_directories_current("src");
    assert!(result.is_ok());
    let paths = result.unwrap();
    assert!(!paths.is_empty());
    assert!(paths.iter().any(|p| p.contains("src")));
}

#[test]
fn test_find_directories_exact_match() {
    // Test exact pattern matching
    let result = find_directories(".", "src");
    assert!(result.is_ok());
    let paths = result.unwrap();
    assert!(!paths.is_empty());
    assert!(paths.iter().all(|p| p.ends_with("src")));
}

#[test]
fn test_find_directories_case_sensitive() {
    // Test that matching is case sensitive
    let result_lower = find_directories(".", "src");
    let result_upper = find_directories(".", "SRC");

    assert_ne!(result_lower.unwrap(), result_upper.unwrap());
}

#[test]
fn test_find_directories_with_temp_dirs() {
    // Test with temporary directories
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create test directories
    let node_modules1 = temp_path.join("node_modules");
    let node_modules2 = temp_path.join("project1").join("node_modules");
    let other_dir = temp_path.join("other");

    fs::create_dir_all(&node_modules1).unwrap();
    fs::create_dir_all(&node_modules2).unwrap();
    fs::create_dir_all(&other_dir).unwrap();

    let result = find_directories(temp_path.to_str().unwrap(), "node_modules");
    assert!(result.is_ok());
    let paths = result.unwrap();

    assert_eq!(paths.len(), 2);
    assert!(paths.iter().any(|p| p.ends_with("node_modules")));
    assert!(paths.iter().any(|p| p.contains("project1/node_modules")));
}

#[test]
fn test_find_directories_ignores_files() {
    // Test that only directories are returned, not files
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create a directory and a file with same name
    let test_dir = temp_path.join("test_pattern");
    let test_file = temp_path.join("test_pattern_file");

    fs::create_dir_all(&test_dir).unwrap();
    fs::write(&test_file, "content").unwrap();

    let result = find_directories(temp_path.to_str().unwrap(), "test_pattern");
    assert!(result.is_ok());
    let paths = result.unwrap();

    assert_eq!(paths.len(), 1);
    assert!(paths[0].ends_with("test_pattern"));
}

#[test]
fn test_find_directories_empty_pattern() {
    // Test that empty pattern returns error
    let result = find_directories(".", "");
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("empty"));
}

// New tests for streaming functionality
#[test]
fn test_stream_directories_basic() {
    // Test basic streaming functionality
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create test directories
    let test_dir1 = temp_path.join("test_pattern");
    let test_dir2 = temp_path.join("subdir").join("test_pattern");
    let other_dir = temp_path.join("other");

    fs::create_dir_all(&test_dir1).unwrap();
    fs::create_dir_all(&test_dir2).unwrap();
    fs::create_dir_all(&other_dir).unwrap();

    let (tx, rx) = std::sync::mpsc::channel::<DiscoveryMessage>();

    // Start streaming in a separate thread
    let temp_path_str = temp_path.to_str().unwrap().to_string();
    let handle = std::thread::spawn(move || {
        stream_directories(&temp_path_str, "test_pattern", tx).unwrap();
    });

    // Collect all messages
    let mut discovered_paths = Vec::new();
    let mut messages = Vec::new();

    while let Ok(message) = rx.recv() {
        messages.push(message.clone());
        match message {
            DiscoveryMessage::DirectoryFound(path) => {
                discovered_paths.push(path);
            }
            DiscoveryMessage::DiscoveryComplete => {
                break;
            }
            DiscoveryMessage::DiscoveryError(_) => {
                panic!("Discovery should not fail");
            }
        }
    }

    handle.join().unwrap();

    // Verify results
    assert_eq!(discovered_paths.len(), 2);
    assert!(discovered_paths.iter().any(|p| p.ends_with("test_pattern")));
    assert!(
        discovered_paths
            .iter()
            .any(|p| p.contains("subdir/test_pattern"))
    );

    // Verify we got completion message
    assert!(
        messages
            .iter()
            .any(|m| matches!(m, DiscoveryMessage::DiscoveryComplete))
    );
}

#[test]
fn test_stream_directories_empty_result() {
    // Test streaming with no matching directories
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create directories that don't match the pattern
    let other_dir1 = temp_path.join("other1");
    let other_dir2 = temp_path.join("other2");

    fs::create_dir_all(&other_dir1).unwrap();
    fs::create_dir_all(&other_dir2).unwrap();

    let (tx, rx) = std::sync::mpsc::channel::<DiscoveryMessage>();

    let temp_path_str = temp_path.to_str().unwrap().to_string();
    let handle = std::thread::spawn(move || {
        stream_directories(&temp_path_str, "nonexistent_pattern", tx).unwrap();
    });

    let mut messages = Vec::new();
    while let Ok(message) = rx.recv() {
        messages.push(message.clone());
        if matches!(message, DiscoveryMessage::DiscoveryComplete) {
            break;
        }
    }

    handle.join().unwrap();

    // Should only get completion message, no directory found messages
    assert_eq!(messages.len(), 1);
    assert!(matches!(messages[0], DiscoveryMessage::DiscoveryComplete));
}

#[test]
fn test_stream_directories_error_handling() {
    // Test streaming with invalid path
    let (tx, _rx) = std::sync::mpsc::channel::<DiscoveryMessage>();

    let handle = std::thread::spawn(move || {
        let result = stream_directories("/non/existent/path", "pattern", tx);
        assert!(result.is_err());
    });

    handle.join().unwrap();
}

#[test]
fn test_stream_directories_progressive_discovery() {
    // Test that directories are discovered progressively
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create multiple test directories at different depths
    let dirs = vec![
        temp_path.join("test_pattern"),
        temp_path.join("level1").join("test_pattern"),
        temp_path.join("level1").join("level2").join("test_pattern"),
        temp_path
            .join("level1")
            .join("level2")
            .join("level3")
            .join("test_pattern"),
    ];

    for dir in &dirs {
        fs::create_dir_all(dir).unwrap();
    }

    let (tx, rx) = std::sync::mpsc::channel::<DiscoveryMessage>();

    let temp_path_str = temp_path.to_str().unwrap().to_string();
    let handle = std::thread::spawn(move || {
        stream_directories(&temp_path_str, "test_pattern", tx).unwrap();
    });

    let mut discovered_paths = Vec::new();
    let mut message_count = 0;

    while let Ok(message) = rx.recv() {
        message_count += 1;
        match message {
            DiscoveryMessage::DirectoryFound(path) => {
                discovered_paths.push(path);
            }
            DiscoveryMessage::DiscoveryComplete => {
                break;
            }
            DiscoveryMessage::DiscoveryError(_) => {
                panic!("Discovery should not fail");
            }
        }
    }

    handle.join().unwrap();

    // Should find all 4 directories
    assert_eq!(discovered_paths.len(), 4);
    assert_eq!(message_count, 5); // 4 directories + 1 completion message

    // Verify all expected paths are found
    for dir in &dirs {
        let dir_str = dir.to_string_lossy().to_string();
        assert!(discovered_paths.iter().any(|p| p == &dir_str));
    }
}

#[test]
fn test_stream_directories_nested_node_modules() {
    // Test that nested node_modules are handled correctly
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create nested node_modules structure
    let node_modules1 = temp_path.join("node_modules");
    let node_modules2 = temp_path.join("project1").join("node_modules");
    let node_modules3 = temp_path
        .join("project1")
        .join("node_modules")
        .join("subpackage")
        .join("node_modules");

    fs::create_dir_all(&node_modules1).unwrap();
    fs::create_dir_all(&node_modules2).unwrap();
    fs::create_dir_all(&node_modules3).unwrap();

    let (tx, rx) = std::sync::mpsc::channel::<DiscoveryMessage>();

    let temp_path_str = temp_path.to_str().unwrap().to_string();
    let handle = std::thread::spawn(move || {
        stream_directories(&temp_path_str, "node_modules", tx).unwrap();
    });

    let mut discovered_paths = Vec::new();

    while let Ok(message) = rx.recv() {
        match message {
            DiscoveryMessage::DirectoryFound(path) => {
                discovered_paths.push(path);
            }
            DiscoveryMessage::DiscoveryComplete => {
                break;
            }
            DiscoveryMessage::DiscoveryError(_) => {
                panic!("Discovery should not fail");
            }
        }
    }

    handle.join().unwrap();

    // Should find exactly 2 node_modules (not 3, because nested ones are ignored)
    assert_eq!(discovered_paths.len(), 2);
    assert!(discovered_paths.iter().any(|p| p.ends_with("node_modules")));
    assert!(
        discovered_paths
            .iter()
            .any(|p| p.contains("project1/node_modules"))
    );
    // The deeply nested one should not be found
    assert!(
        !discovered_paths
            .iter()
            .any(|p| p.contains("subpackage/node_modules"))
    );
}

#[test]
fn test_nested_pattern_avoidance_general() {
    // Test that nested pattern avoidance works for any pattern, not just node_modules
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Test with 'dist' pattern
    let dist1 = temp_path.join("dist");
    let dist2 = temp_path.join("project1").join("dist");
    let dist3 = temp_path
        .join("project1")
        .join("dist")
        .join("build")
        .join("dist");

    fs::create_dir_all(&dist1).unwrap();
    fs::create_dir_all(&dist2).unwrap();
    fs::create_dir_all(&dist3).unwrap();

    let result = find_directories(temp_path.to_str().unwrap(), "dist");
    assert!(result.is_ok());
    let paths = result.unwrap();

    // Should find exactly 2 dist directories (not 3, because nested ones are ignored)
    assert_eq!(paths.len(), 2);
    assert!(paths.iter().any(|p| p.ends_with("dist")));
    assert!(paths.iter().any(|p| p.contains("project1/dist")));
    // The deeply nested one should not be found
    assert!(!paths.iter().any(|p| p.contains("build/dist")));
}

#[test]
fn test_nested_pattern_avoidance_target() {
    // Test that nested pattern avoidance works for 'target' pattern
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Test with 'target' pattern
    let target1 = temp_path.join("target");
    let target2 = temp_path.join("project1").join("target");
    let target3 = temp_path
        .join("project1")
        .join("target")
        .join("debug")
        .join("target");

    fs::create_dir_all(&target1).unwrap();
    fs::create_dir_all(&target2).unwrap();
    fs::create_dir_all(&target3).unwrap();

    let result = find_directories(temp_path.to_str().unwrap(), "target");
    assert!(result.is_ok());
    let paths = result.unwrap();

    // Should find exactly 2 target directories (not 3, because nested ones are ignored)
    assert_eq!(paths.len(), 2);
    assert!(paths.iter().any(|p| p.ends_with("target")));
    assert!(paths.iter().any(|p| p.contains("project1/target")));
    // The deeply nested one should not be found
    assert!(!paths.iter().any(|p| p.contains("debug/target")));
}

#[test]
fn test_nested_pattern_avoidance_with_ignore() {
    // Test that nested pattern avoidance works correctly with ignore patterns
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create structure with both nested patterns and ignored directories
    let node_modules1 = temp_path.join("node_modules");
    let node_modules2 = temp_path.join("project1").join("node_modules");
    let node_modules3 = temp_path
        .join("project1")
        .join("node_modules")
        .join("subpackage")
        .join("node_modules");
    let ignored_dir = temp_path.join("project1").join("node_modules").join("temp");

    fs::create_dir_all(&node_modules1).unwrap();
    fs::create_dir_all(&node_modules2).unwrap();
    fs::create_dir_all(&node_modules3).unwrap();
    fs::create_dir_all(&ignored_dir).unwrap();

    let ignore_patterns = IgnorePatterns::new("temp").unwrap();
    let result = find_directories_with_ignore(
        temp_path.to_str().unwrap(),
        "node_modules",
        &ignore_patterns,
    );
    assert!(result.is_ok());
    let paths = result.unwrap();

    // Should find exactly 2 node_modules (not 3, because nested ones are ignored)
    // The ignored 'temp' directory should not affect the count
    assert_eq!(paths.len(), 2);
    assert!(paths.iter().any(|p| p.ends_with("node_modules")));
    assert!(paths.iter().any(|p| p.contains("project1/node_modules")));
    // The deeply nested one should not be found
    assert!(!paths.iter().any(|p| p.contains("subpackage/node_modules")));
}

#[test]
fn test_performance_optimization() {
    // Test that performance optimizations work correctly
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create a simple directory structure to test optimizations
    let project_path = temp_path.join("project");
    fs::create_dir_all(&project_path).unwrap();

    let target_path = project_path.join("target");
    fs::create_dir_all(&target_path).unwrap();

    let nested_target = target_path.join("debug").join("target");
    fs::create_dir_all(&nested_target).unwrap();

    // Test without ignore patterns (should use fast path)
    let result = find_directories(temp_path.to_str().unwrap(), "target");
    assert!(result.is_ok());
    let paths = result.unwrap();
    assert_eq!(paths.len(), 1); // Only the top-level target, not the nested one

    // Test with ignore patterns
    let ignore_patterns = IgnorePatterns::new("temp").unwrap();
    let result_with_ignore =
        find_directories_with_ignore(temp_path.to_str().unwrap(), "target", &ignore_patterns);
    assert!(result_with_ignore.is_ok());
    let paths_with_ignore = result_with_ignore.unwrap();
    assert_eq!(paths_with_ignore.len(), 1); // Same result
}

#[test]
fn test_format_size() {
    assert_eq!(format_size(0), "0 B");
    assert_eq!(format_size(1023), "1023 B");
    assert_eq!(format_size(1024), "1.0 KB");
    assert_eq!(format_size(1536), "1.5 KB");
    assert_eq!(format_size(1024 * 1024), "1.0 MB");
    assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    assert_eq!(format_size(1024 * 1024 * 1024 + 1024 * 1024), "1.0 GB");
}

#[test]
fn test_calculate_directory_size() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create some test files
    fs::write(dir_path.join("file1.txt"), "Hello, World!").unwrap();
    fs::write(dir_path.join("file2.txt"), "Another file").unwrap();

    // Create a subdirectory with files
    let sub_dir = dir_path.join("subdir");
    fs::create_dir(&sub_dir).unwrap();
    fs::write(sub_dir.join("file3.txt"), "Subdirectory file").unwrap();

    let size = calculate_directory_size(dir_path).unwrap();
    assert!(size > 0);

    // Clean up
    temp_dir.close().unwrap();
}

#[test]
fn test_find_directories_with_size() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create test directories
    let dir1 = dir_path.join("node_modules");
    let dir2 = dir_path.join("subdir").join("node_modules");

    fs::create_dir_all(&dir1).unwrap();
    fs::create_dir_all(&dir2).unwrap();

    // Add some files to make them have different sizes
    fs::write(dir1.join("file1.txt"), "This is a test file").unwrap();
    fs::write(
        dir2.join("file2.txt"),
        "Another test file with different content",
    )
    .unwrap();

    let directories =
        find_directories_with_size(dir_path.to_str().unwrap(), "node_modules").unwrap();

    assert_eq!(directories.len(), 2);
    assert!(directories[0].size >= directories[1].size); // Should be sorted by size (largest first)

    // Clean up
    temp_dir.close().unwrap();
}

#[test]
fn test_last_modified_from_parent_directory() {
    let temp_dir = TempDir::new().unwrap();
    let dir_path = temp_dir.path();

    // Create a project structure: project/node_modules
    let project_dir = dir_path.join("project");
    let node_modules_dir = project_dir.join("node_modules");

    fs::create_dir_all(&project_dir).unwrap();
    fs::create_dir_all(&node_modules_dir).unwrap();

    // Add a file to the project directory (not node_modules)
    fs::write(project_dir.join("package.json"), "{}").unwrap();

    // Wait a moment to ensure different timestamps
    std::thread::sleep(std::time::Duration::from_millis(100));

    // Add a file to node_modules
    fs::write(node_modules_dir.join("some_file.txt"), "content").unwrap();

    // Find node_modules directories
    let directories =
        find_directories_with_size(dir_path.to_str().unwrap(), "node_modules").unwrap();

    assert_eq!(directories.len(), 1);

    let dir_info = &directories[0];
    assert!(dir_info.path.contains("node_modules"));

    // The last modified time should be from the project directory, not node_modules
    assert!(dir_info.last_modified.is_some());

    // Get the actual timestamps to verify
    let project_modified = get_directory_last_modified(&project_dir).unwrap();
    let node_modules_modified = get_directory_last_modified(&node_modules_dir).unwrap();

    // The displayed last modified should match the project directory, not node_modules
    assert_eq!(dir_info.last_modified.unwrap(), project_modified);
    assert_ne!(dir_info.last_modified.unwrap(), node_modules_modified);

    // Clean up
    temp_dir.close().unwrap();
}

#[test]
fn test_ignore_patterns_new_empty() {
    let patterns = IgnorePatterns::new("").unwrap();
    assert!(patterns.is_empty());
    assert_eq!(patterns.len(), 0);
}

#[test]
fn test_ignore_patterns_new_single() {
    let patterns = IgnorePatterns::new("node_modules").unwrap();
    assert!(!patterns.is_empty());
    assert_eq!(patterns.len(), 1);
    assert!(patterns.should_ignore("node_modules"));
    assert!(!patterns.should_ignore("other"));
}

#[test]
fn test_ignore_patterns_new_multiple() {
    let patterns = IgnorePatterns::new("node_modules,.git,target").unwrap();
    assert!(!patterns.is_empty());
    assert_eq!(patterns.len(), 3);
    assert!(patterns.should_ignore("node_modules"));
    assert!(patterns.should_ignore(".git"));
    assert!(patterns.should_ignore("target"));
    assert!(!patterns.should_ignore("other"));
}

#[test]
fn test_ignore_patterns_regex() {
    let patterns = IgnorePatterns::new(".*\\.cache$,^temp.*").unwrap();
    assert!(!patterns.is_empty());
    assert_eq!(patterns.len(), 2);
    assert!(patterns.should_ignore("cache.cache"));
    assert!(patterns.should_ignore("temp_dir"));
    assert!(patterns.should_ignore("something.cache"));
    assert!(!patterns.should_ignore("other"));
}

#[test]
fn test_ignore_patterns_invalid_regex() {
    let result = IgnorePatterns::new("invalid[regex");
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Invalid regex pattern")
    );
}

#[test]
fn test_ignore_patterns_whitespace() {
    let patterns = IgnorePatterns::new("  node_modules  ,  .git  ").unwrap();
    assert_eq!(patterns.len(), 2);
    assert!(patterns.should_ignore("node_modules"));
    assert!(patterns.should_ignore(".git"));
}

#[test]
fn test_find_directories_with_ignore() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create test directories
    let node_modules = temp_path.join("node_modules");
    let git_dir = temp_path.join(".git");
    let target_dir = temp_path.join("target");
    let other_dir = temp_path.join("other");

    fs::create_dir_all(&node_modules).unwrap();
    fs::create_dir_all(&git_dir).unwrap();
    fs::create_dir_all(&target_dir).unwrap();
    fs::create_dir_all(&other_dir).unwrap();

    let ignore_patterns = IgnorePatterns::new("node_modules,.git").unwrap();
    let result =
        find_directories_with_ignore(temp_path.to_str().unwrap(), "other", &ignore_patterns);

    assert!(result.is_ok());
    let paths = result.unwrap();
    assert_eq!(paths.len(), 1);
    assert!(paths[0].ends_with("other"));
}

#[test]
fn test_find_directories_with_ignore_regex() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create test directories
    let cache_dir = temp_path.join("cache");
    let temp_dir1 = temp_path.join("temp_dir");
    let other_dir = temp_path.join("other");

    fs::create_dir_all(&cache_dir).unwrap();
    fs::create_dir_all(&temp_dir1).unwrap();
    fs::create_dir_all(&other_dir).unwrap();

    let ignore_patterns = IgnorePatterns::new(".*cache$,^temp.*").unwrap();
    let result =
        find_directories_with_ignore(temp_path.to_str().unwrap(), "other", &ignore_patterns);

    assert!(result.is_ok());
    let paths = result.unwrap();
    assert_eq!(paths.len(), 1);
    assert!(paths[0].ends_with("other"));
}

#[test]
fn test_stream_directories_with_ignore() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path().to_path_buf();

    // Create test directories
    let node_modules = temp_path.join("node_modules");
    let other_dir = temp_path.join("other");

    fs::create_dir_all(&node_modules).unwrap();
    fs::create_dir_all(&other_dir).unwrap();

    let ignore_patterns = IgnorePatterns::new("node_modules").unwrap();
    let (tx, rx) = std::sync::mpsc::channel();

    let temp_path_str = temp_path.to_str().unwrap().to_string();
    let handle = std::thread::spawn(move || {
        stream_directories_with_ignore(&temp_path_str, "other", &ignore_patterns, tx)
    });

    let mut messages = Vec::new();
    while let Ok(msg) = rx.recv() {
        messages.push(msg);
    }

    handle.join().unwrap().unwrap();

    // Should find the "other" directory but not "node_modules"
    let found_dirs: Vec<String> = messages
        .iter()
        .filter_map(|msg| {
            if let DiscoveryMessage::DirectoryFound(path) = msg {
                Some(path.clone())
            } else {
                None
            }
        })
        .collect();

    assert_eq!(found_dirs.len(), 1);
    assert!(found_dirs[0].ends_with("other"));
}

#[test]
fn test_find_directories_with_size_and_ignore() {
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create test directories with files
    let node_modules = temp_path.join("node_modules");
    let other_dir = temp_path.join("other");

    fs::create_dir_all(&node_modules).unwrap();
    fs::create_dir_all(&other_dir).unwrap();

    // Add a file to make the directory have size
    fs::write(other_dir.join("test.txt"), "content").unwrap();

    let ignore_patterns = IgnorePatterns::new("node_modules").unwrap();
    let result = find_directories_with_size_and_ignore(
        temp_path.to_str().unwrap(),
        "other",
        &ignore_patterns,
    );

    assert!(result.is_ok());
    let directories = result.unwrap();
    assert_eq!(directories.len(), 1);
    assert!(directories[0].path.ends_with("other"));
    assert!(directories[0].size > 0);
}

#[cfg(test)]
mod benchmarks {
    use super::*;
    use std::fs;
    use std::time::Instant;
    use tempfile::TempDir;

    /// Benchmark different directory size calculation methods
    #[test]
    fn benchmark_directory_size_calculation() {
        // Create a test directory structure
        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        // Create a complex directory structure for testing
        create_test_directory_structure(dir_path);

        println!("\n=== Directory Size Calculation Benchmark ===");
        println!("Test directory: {:?}", dir_path);

        // Benchmark original method
        let start = Instant::now();
        let original_size = calculate_directory_size(dir_path).unwrap();
        let original_duration = start.elapsed();
        println!(
            "Original method: {} bytes in {:?}",
            original_size, original_duration
        );

        // Benchmark optimized method
        let start = Instant::now();
        let optimized_size = calculate_directory_size_optimized(dir_path).unwrap();
        let optimized_duration = start.elapsed();
        println!(
            "Optimized method: {} bytes in {:?}",
            optimized_size, optimized_duration
        );

        // Benchmark parallel method
        let start = Instant::now();
        let parallel_size = calculate_directory_size_parallel(dir_path).unwrap();
        let parallel_duration = start.elapsed();
        println!(
            "Parallel method: {} bytes in {:?}",
            parallel_size, parallel_duration
        );

        // Benchmark jwalk method
        let start = Instant::now();
        let jwalk_size = calculate_directory_size_jwalk(dir_path).unwrap();
        let jwalk_duration = start.elapsed();
        println!("Jwalk method: {} bytes in {:?}", jwalk_size, jwalk_duration);

        // Verify optimized methods return consistent results (they may differ from original due to disk size vs logical size)
        assert_eq!(
            optimized_size, parallel_size,
            "Optimized and parallel sizes should match"
        );
        assert_eq!(
            optimized_size, jwalk_size,
            "Optimized and jwalk sizes should match"
        );

        // Note: Original method uses logical file size, optimized methods use disk size
        // This is why they may differ - optimized methods are more accurate

        // Performance comparison
        println!("\n=== Performance Comparison ===");
        println!("Original method: {:?}", original_duration);
        println!(
            "Optimized method: {:?} ({:.1}x faster)",
            optimized_duration,
            original_duration.as_nanos() as f64 / optimized_duration.as_nanos() as f64
        );
        println!(
            "Parallel method: {:?} ({:.1}x faster)",
            parallel_duration,
            original_duration.as_nanos() as f64 / parallel_duration.as_nanos() as f64
        );
        println!(
            "Jwalk method: {:?} ({:.1}x faster)",
            jwalk_duration,
            original_duration.as_nanos() as f64 / jwalk_duration.as_nanos() as f64
        );

        // Clean up
        temp_dir.close().unwrap();
    }

    /// Create a test directory structure for benchmarking
    fn create_test_directory_structure(root: &std::path::Path) {
        // Create main directories
        let dirs = vec![
            "src",
            "docs",
            "tests",
            "assets",
            "config",
            "src/components",
            "src/utils",
            "src/api",
            "docs/api",
            "docs/user-guide",
            "docs/examples",
            "tests/unit",
            "tests/integration",
            "tests/e2e",
            "assets/images",
            "assets/fonts",
            "assets/icons",
            "config/dev",
            "config/prod",
            "config/test",
        ];

        for dir in dirs {
            let dir_path = root.join(dir);
            fs::create_dir_all(&dir_path).unwrap();
        }

        // Create files with different sizes
        let files = vec![
            ("README.md", "This is a test README file with some content."),
            ("src/main.rs", "fn main() { println!(\"Hello, world!\"); }"),
            (
                "src/lib.rs",
                "pub fn hello() { println!(\"Hello from lib!\"); }",
            ),
            (
                "Cargo.toml",
                "[package]\nname = \"test\"\nversion = \"0.1.0\"\nedition = \"2021\"",
            ),
            ("docs/README.md", "Documentation for the project."),
            (
                "tests/test.rs",
                "#[test]\nfn test_function() {\n    assert!(true);\n}",
            ),
            (
                "config/settings.json",
                "{\n  \"debug\": true,\n  \"port\": 8080\n}",
            ),
            ("assets/logo.png", "fake_png_data_here"),
            (
                "src/components/button.rs",
                "pub struct Button {\n    pub text: String,\n}",
            ),
            (
                "src/utils/helpers.rs",
                "pub fn helper_function() {\n    // Helper logic\n}",
            ),
        ];

        for (file_path, content) in files {
            let full_path = root.join(file_path);
            fs::write(&full_path, content).unwrap();
        }

        // Create some larger files to make the benchmark more realistic
        for i in 0..5 {
            let large_file = root.join(format!("large_file_{}.txt", i));
            let content = "x".repeat(1024 * 1024); // 1MB file
            fs::write(large_file, content).unwrap();
        }
    }
}
