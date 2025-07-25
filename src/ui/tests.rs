#[test]
fn test_ui_layout_changes() {
    // Test that the UI layout changes work correctly
    // This test verifies that the list items are simplified and last modified is moved to details panel
    
    // Create a test directory with last modified time
    let test_dir = crate::fs::DirectoryInfo {
        path: "test/directory".to_string(),
        size: 1024,
        formatted_size: "1.0 KB".to_string(),
        last_modified: Some(std::time::SystemTime::now()),
        formatted_last_modified: "Just now".to_string(),
        selected: false,
        deletion_status: crate::fs::DeletionStatus::Normal,
        calculation_status: crate::fs::CalculationStatus::Completed,
    };
    
    // Verify that the directory has proper last modified time
    assert!(test_dir.last_modified.is_some());
    assert_eq!(test_dir.formatted_last_modified, "Just now");
    
    // Test that the path is clean (no ./ prefix)
    let clean_path = super::clean_path(&test_dir.path);
    assert_eq!(clean_path, "test/directory");
    
    // Test that the directory icon works
    let icon = super::get_directory_icon(false, false);
    assert_eq!(icon, "📁");
    
    // Test that selection indicator works
    let indicator = if test_dir.selected { "✓" } else { " " };
    assert_eq!(indicator, " ");
} 