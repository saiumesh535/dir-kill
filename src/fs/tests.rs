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

#[test]
fn test_find_directories_special_characters() {
    // Test with special characters in pattern
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    let special_dir = temp_path.join("test-dir_123");
    fs::create_dir_all(&special_dir).unwrap();

    let result = find_directories(temp_path.to_str().unwrap(), "test-dir_123");
    assert!(result.is_ok());
    let paths = result.unwrap();

    assert_eq!(paths.len(), 1);
    assert!(paths[0].ends_with("test-dir_123"));
}

#[test]
fn test_find_directories_relative_paths() {
    // Test with relative paths
    let result = find_directories("src", "cli");
    assert!(result.is_ok());
    let paths = result.unwrap();

    assert!(!paths.is_empty());
    assert!(paths.iter().all(|p| p.contains("cli")));
}

#[test]
fn test_find_directories_absolute_paths() {
    // Test with absolute paths
    let current_dir = std::env::current_dir().unwrap();
    let result = find_directories(current_dir.to_str().unwrap(), "src");
    assert!(result.is_ok());
    let paths = result.unwrap();

    assert!(!paths.is_empty());
    assert!(paths.iter().all(|p| p.contains("src")));
}

#[test]
fn test_find_directories_ignore_nested_node_modules() {
    // Test that nested node_modules are ignored
    let temp_dir = TempDir::new().unwrap();
    let temp_path = temp_dir.path();

    // Create nested structure: node_modules/node_modules/node_modules
    let node_modules1 = temp_path.join("node_modules");
    let node_modules2 = node_modules1.join("node_modules");
    let node_modules3 = node_modules2.join("node_modules");

    fs::create_dir_all(&node_modules1).unwrap();
    fs::create_dir_all(&node_modules2).unwrap();
    fs::create_dir_all(&node_modules3).unwrap();

    let result = find_directories(temp_path.to_str().unwrap(), "node_modules");
    assert!(result.is_ok());
    let paths = result.unwrap();

    // Should only find the top-level node_modules, not nested ones
    assert_eq!(paths.len(), 1);
    assert!(paths[0].ends_with("node_modules"));
    assert!(!paths[0].contains("node_modules/node_modules"));
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
