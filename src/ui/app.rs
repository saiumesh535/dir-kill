use crate::fs::DirectoryInfo;
use std::sync::mpsc;

/// Application state for TUI
pub struct App {
    pub directories: Vec<DirectoryInfo>,
    pub selected: usize,
    pub pattern: String,
    pub path: String,
    pub current_page: usize,
    pub selection_mode: bool,
    pub deletion_progress: Option<DeletionProgress>,
    pub deletion_sender: Option<mpsc::Sender<DeletionMessage>>,
    pub deletion_receiver: Option<mpsc::Receiver<DeletionMessage>>,
    pub total_freed_space: u64,
    pub freed_space_history: Vec<FreedSpaceEntry>,
}

/// Entry for tracking freed space
#[derive(Debug, Clone)]
pub struct FreedSpaceEntry {
    pub path: String,
    pub size: u64,
    pub timestamp: std::time::Instant,
}

/// Messages for background deletion operations
#[derive(Debug)]
pub enum DeletionMessage {
    StartSingle { index: usize, path: String },
    StartMultiple { indices: Vec<usize>, paths: Vec<String> },
    Progress { index: usize, status: crate::fs::DeletionStatus },
    Complete { results: Vec<DeletionResult> },
}

/// Result of a deletion operation
#[derive(Debug)]
pub struct DeletionResult {
    pub index: usize,
    pub path: String,
    pub success: bool,
    pub error: Option<String>,
}

/// Progress tracking for deletion operations
pub struct DeletionProgress {
    pub total_items: usize,
    pub completed_items: usize,
    pub current_path: String,
    pub deleted_paths: Vec<String>,
    pub errors: Vec<String>,
    pub freed_space: u64,
    pub freed_space_this_session: u64,
}

impl App {
    pub fn new(directories: Vec<DirectoryInfo>, pattern: String, path: String) -> Self {
        Self {
            directories,
            selected: 0,
            pattern,
            path,
            current_page: 0,
            selection_mode: false,
            deletion_progress: None,
            deletion_sender: None,
            deletion_receiver: None,
            total_freed_space: 0,
            freed_space_history: Vec::new(),
        }
    }

    pub fn next(&mut self, items_per_page: usize) {
        if !self.directories.is_empty() {
            self.selected = (self.selected + 1) % self.directories.len();
            self.update_selection_for_pagination(items_per_page);
        }
    }

    pub fn previous(&mut self, items_per_page: usize) {
        if !self.directories.is_empty() {
            self.selected = if self.selected == 0 {
                self.directories.len() - 1
            } else {
                self.selected - 1
            };
            self.update_selection_for_pagination(items_per_page);
        }
    }

    pub fn select_first(&mut self) {
        if !self.directories.is_empty() {
            self.selected = 0;
        }
    }

    pub fn select_last(&mut self) {
        if !self.directories.is_empty() {
            self.selected = self.directories.len() - 1;
        }
    }

    pub fn get_selected_directory(&self) -> Option<&DirectoryInfo> {
        self.directories.get(self.selected)
    }

    pub fn directory_count(&self) -> usize {
        self.directories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.directories.is_empty()
    }

    // Pagination methods
    pub fn total_pages(&self, items_per_page: usize) -> usize {
        if self.directories.is_empty() || items_per_page == 0 {
            0
        } else {
            (self.directories.len() - 1) / items_per_page + 1
        }
    }

    pub fn visible_items(&self, items_per_page: usize) -> Vec<&DirectoryInfo> {
        let start = self.current_page * items_per_page;
        let end = std::cmp::min(start + items_per_page, self.directories.len());
        self.directories.get(start..end).unwrap_or(&[]).iter().collect()
    }

    pub fn visible_selected_index(&self, items_per_page: usize) -> usize {
        self.selected % items_per_page
    }

    pub fn next_page(&mut self, items_per_page: usize) {
        if self.current_page < self.total_pages(items_per_page).saturating_sub(1) {
            self.current_page += 1;
            // Adjust selected to stay within visible range
            self.selected = self.current_page * items_per_page;
        }
    }

    pub fn previous_page(&mut self, items_per_page: usize) {
        if self.current_page > 0 {
            self.current_page -= 1;
            // Adjust selected to stay within visible range
            self.selected = self.current_page * items_per_page;
        }
    }

    pub fn go_to_page(&mut self, page: usize, items_per_page: usize) {
        if page < self.total_pages(items_per_page) {
            self.current_page = page;
            self.selected = page * items_per_page;
        }
    }

    pub fn update_selection_for_pagination(&mut self, items_per_page: usize) {
        // Ensure selected index stays within bounds
        if self.selected >= self.directories.len() {
            self.selected = self.directories.len().saturating_sub(1);
        }
        
        // Update current page based on selection
        self.current_page = self.selected / items_per_page;
    }

    // Selection methods
    pub fn toggle_selection_mode(&mut self) {
        self.selection_mode = !self.selection_mode;
    }

    pub fn toggle_current_selection(&mut self) {
        if !self.directories.is_empty() && self.selected < self.directories.len() {
            self.directories[self.selected].selected = !self.directories[self.selected].selected;
        }
    }

    pub fn select_all(&mut self) {
        for dir in &mut self.directories {
            dir.selected = true;
        }
    }

    pub fn deselect_all(&mut self) {
        for dir in &mut self.directories {
            dir.selected = false;
        }
    }

    pub fn select_current(&mut self) {
        if !self.directories.is_empty() && self.selected < self.directories.len() {
            self.directories[self.selected].selected = true;
        }
    }

    pub fn deselect_current(&mut self) {
        if !self.directories.is_empty() && self.selected < self.directories.len() {
            self.directories[self.selected].selected = false;
        }
    }

    pub fn get_selected_count(&self) -> usize {
        self.directories.iter().filter(|dir| dir.selected).count()
    }

    pub fn get_selected_directories(&self) -> Vec<&DirectoryInfo> {
        self.directories.iter().filter(|dir| dir.selected).collect()
    }

    pub fn get_selected_total_size(&self) -> u64 {
        self.directories.iter()
            .filter(|dir| dir.selected)
            .map(|dir| dir.size)
            .sum()
    }

    /// Delete selected directories from the file system with progressive visual feedback
    pub fn delete_selected_directories(&mut self) -> Result<Vec<String>, std::io::Error> {
        let selected_indices: Vec<usize> = self.directories
            .iter()
            .enumerate()
            .filter(|(_, dir)| dir.selected)
            .map(|(i, _)| i)
            .collect();

        if selected_indices.is_empty() {
            return Ok(Vec::new());
        }

        // Initialize progress tracking
        self.deletion_progress = Some(DeletionProgress {
            total_items: selected_indices.len(),
            completed_items: 0,
            current_path: String::new(),
            deleted_paths: Vec::new(),
            errors: Vec::new(),
            freed_space: 0,
            freed_space_this_session: self.total_freed_space,
        });

        let mut deleted_paths = Vec::new();
        let mut errors = Vec::new();

        for (i, &index) in selected_indices.iter().enumerate() {
            if index >= self.directories.len() {
                continue; // Skip if index is out of bounds
            }

            let path = self.directories[index].path.clone();
            
            // Update progress
            if let Some(progress) = &mut self.deletion_progress {
                progress.current_path = path.clone();
                progress.completed_items = i;
            }

            // Mark as deleting
            self.directories[index].deletion_status = crate::fs::DeletionStatus::Deleting;

            match std::fs::remove_dir_all(&path) {
                Ok(_) => {
                    deleted_paths.push(path.clone());
                    if let Some(progress) = &mut self.deletion_progress {
                        progress.deleted_paths.push(path.clone());
                    }
                    // Mark as deleted (but keep in list)
                    self.directories[index].deletion_status = crate::fs::DeletionStatus::Deleted;
                }
                Err(e) => {
                    let error_msg = format!("Failed to delete {}: {}", path, e);
                    errors.push(error_msg.clone());
                    if let Some(progress) = &mut self.deletion_progress {
                        progress.errors.push(error_msg);
                    }
                    // Mark as error
                    self.directories[index].deletion_status = crate::fs::DeletionStatus::Error(e.to_string());
                }
            }
        }

        // Finalize progress
        if let Some(progress) = &mut self.deletion_progress {
            progress.completed_items = selected_indices.len();
            progress.current_path = String::new();
        }

        // Clear progress after completion
        self.deletion_progress = None;

        if errors.is_empty() {
            Ok(deleted_paths)
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Some deletions failed: {}", errors.join("; "))
            ))
        }
    }

    /// Get deletion progress information
    pub fn get_deletion_progress(&self) -> Option<&DeletionProgress> {
        self.deletion_progress.as_ref()
    }

    /// Check if deletion is in progress
    pub fn is_deleting(&self) -> bool {
        self.deletion_progress.is_some()
    }

    /// Get total freed space
    pub fn get_total_freed_space(&self) -> u64 {
        self.total_freed_space
    }

    /// Get freed space in this session
    pub fn get_session_freed_space(&self) -> u64 {
        if let Some(progress) = &self.deletion_progress {
            progress.freed_space
        } else {
            0
        }
    }

    /// Get recent freed space history (last 5 entries)
    pub fn get_recent_freed_space_history(&self) -> Vec<&FreedSpaceEntry> {
        let start = if self.freed_space_history.len() > 5 {
            self.freed_space_history.len() - 5
        } else {
            0
        };
        self.freed_space_history.get(start..).unwrap_or(&[]).iter().collect()
    }

    /// Initialize background deletion channel
    pub fn init_deletion_channel(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.deletion_sender = Some(tx);
        self.deletion_receiver = Some(rx);
    }

    /// Process any pending deletion messages
    pub fn process_deletion_messages(&mut self) {
        if let Some(receiver) = &self.deletion_receiver {
            while let Ok(message) = receiver.try_recv() {
                match message {
                    DeletionMessage::StartSingle { index, path: _ } => {
                        // Mark as deleting
                        if index < self.directories.len() {
                            self.directories[index].deletion_status = crate::fs::DeletionStatus::Deleting;
                        }
                    }
                    DeletionMessage::StartMultiple { indices, paths: _ } => {
                        // Mark all as deleting
                        for &index in &indices {
                            if index < self.directories.len() {
                                self.directories[index].deletion_status = crate::fs::DeletionStatus::Deleting;
                            }
                        }
                    }
                    DeletionMessage::Progress { index, status } => {
                        // Update status
                        if index < self.directories.len() {
                            self.directories[index].deletion_status = status;
                        }
                    }
                    DeletionMessage::Complete { results } => {
                        // Process completion
                        for result in results {
                            if result.index < self.directories.len() {
                                if result.success {
                                    // Track freed space
                                    let freed_size = self.directories[result.index].size;
                                    self.total_freed_space += freed_size;
                                    
                                    // Add to history
                                    self.freed_space_history.push(FreedSpaceEntry {
                                        path: result.path.clone(),
                                        size: freed_size,
                                        timestamp: std::time::Instant::now(),
                                    });
                                    
                                    // Update progress freed space
                                    if let Some(progress) = &mut self.deletion_progress {
                                        progress.freed_space += freed_size;
                                    }
                                    
                                    self.directories[result.index].deletion_status = crate::fs::DeletionStatus::Deleted;
                                } else {
                                    self.directories[result.index].deletion_status = crate::fs::DeletionStatus::Error(
                                        result.error.unwrap_or_else(|| "Unknown error".to_string())
                                    );
                                }
                            }
                        }
                        // Clear progress
                        self.deletion_progress = None;
                    }
                }
            }
        }
    }

    /// Start background deletion of current directory
    pub fn start_delete_current_directory(&mut self) -> Result<(), std::io::Error> {
        if let Some(dir) = self.get_selected_directory() {
            let path = dir.path.clone();
            let index = self.selected;
            
            // Initialize channel if not already done
            if self.deletion_sender.is_none() {
                self.init_deletion_channel();
            }
            
            // Initialize progress tracking
            self.deletion_progress = Some(DeletionProgress {
                total_items: 1,
                completed_items: 0,
                current_path: path.clone(),
                deleted_paths: Vec::new(),
                errors: Vec::new(),
                freed_space: 0,
                freed_space_this_session: self.total_freed_space,
            });

            // Send start message
            if let Some(sender) = &self.deletion_sender {
                let _ = sender.send(DeletionMessage::StartSingle { index, path: path.clone() });
                
                // Start background deletion
                let sender_clone = sender.clone();
                std::thread::spawn(move || {
                    let result = std::fs::remove_dir_all(&path);
                    let deletion_result = DeletionResult {
                        index,
                        path: path.clone(),
                        success: result.is_ok(),
                        error: result.err().map(|e| e.to_string()),
                    };
                    
                    // Send completion message
                    let _ = sender_clone.send(DeletionMessage::Complete { 
                        results: vec![deletion_result] 
                    });
                });
            }
            
            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No directory selected"
            ))
        }
    }

    /// Start background deletion of selected directories
    pub fn start_delete_selected_directories(&mut self) -> Result<(), std::io::Error> {
        let selected_indices: Vec<usize> = self.directories
            .iter()
            .enumerate()
            .filter(|(_, dir)| dir.selected)
            .map(|(i, _)| i)
            .collect();

        if selected_indices.is_empty() {
            return Ok(());
        }

        // Initialize channel if not already done
        if self.deletion_sender.is_none() {
            self.init_deletion_channel();
        }

        // Initialize progress tracking
        self.deletion_progress = Some(DeletionProgress {
            total_items: selected_indices.len(),
            completed_items: 0,
            current_path: String::new(),
            deleted_paths: Vec::new(),
            errors: Vec::new(),
            freed_space: 0,
            freed_space_this_session: self.total_freed_space,
        });

        // Collect paths
        let paths: Vec<String> = selected_indices
            .iter()
            .map(|&i| self.directories[i].path.clone())
            .collect();

        // Send start message
        if let Some(sender) = &self.deletion_sender {
            let _ = sender.send(DeletionMessage::StartMultiple { 
                indices: selected_indices.clone(), 
                paths: paths.clone() 
            });
            
            // Start background deletion
            let sender_clone = sender.clone();
            std::thread::spawn(move || {
                let mut results = Vec::new();
                
                for (i, &index) in selected_indices.iter().enumerate() {
                    let path = &paths[i];
                    
                    // Send progress update
                    let _ = sender_clone.send(DeletionMessage::Progress { 
                        index, 
                        status: crate::fs::DeletionStatus::Deleting 
                    });
                    
                    let result = std::fs::remove_dir_all(path);
                    let deletion_result = DeletionResult {
                        index,
                        path: path.clone(),
                        success: result.is_ok(),
                        error: result.err().map(|e| e.to_string()),
                    };
                    results.push(deletion_result);
                }
                
                // Send completion message
                let _ = sender_clone.send(DeletionMessage::Complete { results });
            });
        }
        
        Ok(())
    }

    /// Delete the currently selected directory with progressive visual feedback
    pub fn delete_current_directory(&mut self) -> Result<String, std::io::Error> {
        if let Some(dir) = self.get_selected_directory() {
            let path = dir.path.clone();
            
            // Initialize progress tracking
            self.deletion_progress = Some(DeletionProgress {
                total_items: 1,
                completed_items: 0,
                current_path: path.clone(),
                deleted_paths: Vec::new(),
                errors: Vec::new(),
                freed_space: 0,
                freed_space_this_session: self.total_freed_space,
            });

            // Mark as deleting
            self.directories[self.selected].deletion_status = crate::fs::DeletionStatus::Deleting;

            match std::fs::remove_dir_all(&path) {
                Ok(_) => {
                    // Update progress
                    if let Some(progress) = &mut self.deletion_progress {
                        progress.completed_items = 1;
                        progress.deleted_paths.push(path.clone());
                    }
                    
                    // Mark as deleted (but keep in list)
                    self.directories[self.selected].deletion_status = crate::fs::DeletionStatus::Deleted;
                    
                    // Clear progress
                    self.deletion_progress = None;
                    
                    Ok(path)
                }
                Err(e) => {
                    // Update progress with error
                    if let Some(progress) = &mut self.deletion_progress {
                        progress.errors.push(format!("Failed to delete {}: {}", path, e));
                    }
                    
                    // Mark as error
                    self.directories[self.selected].deletion_status = crate::fs::DeletionStatus::Error(e.to_string());
                    
                    // Clear progress
                    self.deletion_progress = None;
                    
                    Err(e)
                }
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No directory selected"
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    fn create_test_directory(path: &str, size: u64) -> DirectoryInfo {
        DirectoryInfo {
            path: path.to_string(),
            size,
            formatted_size: format!("{} B", size),
            selected: false,
            deletion_status: crate::fs::DeletionStatus::Normal,
            calculation_status: crate::fs::CalculationStatus::Completed,
        }
    }

    #[test]
    fn test_app_creation() {
        let directories = vec![
            create_test_directory("dir1", 100),
            create_test_directory("dir2", 200)
        ];
        let app = App::new(directories.clone(), "test".to_string(), ".".to_string());
        assert_eq!(app.directories.len(), directories.len());
        assert_eq!(app.selected, 0);
        assert_eq!(app.pattern, "test");
        assert_eq!(app.path, ".");
    }

               #[test]
           fn test_navigation_with_items() {
               let directories = vec![
                   create_test_directory("dir1", 100),
                   create_test_directory("dir2", 200),
                   create_test_directory("dir3", 300)
               ];
               let mut app = App::new(directories, "test".to_string(), ".".to_string());
               let items_per_page = 20;
               
               // Test next
               app.next(items_per_page);
               assert_eq!(app.selected, 1);
               
               app.next(items_per_page);
               assert_eq!(app.selected, 2);
               
               // Test wrapping
               app.next(items_per_page);
               assert_eq!(app.selected, 0);
               
               // Test previous
               app.previous(items_per_page);
               assert_eq!(app.selected, 2);
               
               app.previous(items_per_page);
               assert_eq!(app.selected, 1);
           }

               #[test]
           fn test_navigation_empty_list() {
               let mut app = App::new(vec![], "test".to_string(), ".".to_string());
               let items_per_page = 20;
               
               // Navigation should not panic with empty list
               app.next(items_per_page);
               assert_eq!(app.selected, 0);
               
               app.previous(items_per_page);
               assert_eq!(app.selected, 0);
           }

    #[test]
    fn test_get_selected_directory() {
        let directories = vec![
            create_test_directory("dir1", 100),
            create_test_directory("dir2", 200)
        ];
        let mut app = App::new(directories, "test".to_string(), ".".to_string());
        let items_per_page = 20;
        
        assert_eq!(app.get_selected_directory().unwrap().path, "dir1");
        
        app.next(items_per_page);
        assert_eq!(app.get_selected_directory().unwrap().path, "dir2");
    }

    #[test]
    fn test_get_selected_directory_empty() {
        let app = App::new(vec![], "test".to_string(), ".".to_string());
        assert_eq!(app.get_selected_directory(), None);
    }

    #[test]
    fn test_directory_count() {
        let app = App::new(vec![
            create_test_directory("dir1", 100),
            create_test_directory("dir2", 200)
        ], "test".to_string(), ".".to_string());
        assert_eq!(app.directory_count(), 2);
    }

    #[test]
    fn test_is_empty() {
        let app = App::new(vec![], "test".to_string(), ".".to_string());
        assert!(app.is_empty());
        
        let app = App::new(vec![create_test_directory("dir1", 100)], "test".to_string(), ".".to_string());
        assert!(!app.is_empty());
    }

    #[test]
    fn test_select_first() {
        let directories = vec![
            create_test_directory("dir1", 100),
            create_test_directory("dir2", 200),
            create_test_directory("dir3", 300)
        ];
        let mut app = App::new(directories, "test".to_string(), ".".to_string());
        let items_per_page = 20;
        
        // Move to middle
        app.next(items_per_page);
        assert_eq!(app.selected, 1);
        
        // Select first
        app.select_first();
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn test_select_last() {
        let directories = vec![
            create_test_directory("dir1", 100),
            create_test_directory("dir2", 200),
            create_test_directory("dir3", 300)
        ];
        let mut app = App::new(directories, "test".to_string(), ".".to_string());
        
        // Select last
        app.select_last();
        assert_eq!(app.selected, 2);
    }

    #[test]
    fn test_select_first_empty() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        app.select_first();
        assert_eq!(app.selected, 0);
    }

               #[test]
           fn test_select_last_empty() {
               let mut app = App::new(vec![], "test".to_string(), ".".to_string());
               app.select_last();
               assert_eq!(app.selected, 0);
           }

           #[test]
           fn test_pagination() {
               let directories = (0..25).map(|i| create_test_directory(&format!("dir{}", i), i as u64 * 100)).collect();
               let mut app = App::new(directories, "test".to_string(), ".".to_string());
               let items_per_page = 20;
               
               // Test total pages (25 items, 20 per page = 2 pages)
               assert_eq!(app.total_pages(items_per_page), 2);
               
               // Test visible items on first page
               let visible = app.visible_items(items_per_page);
               assert_eq!(visible.len(), 20);
               assert_eq!(visible[0].path, "dir0");
               assert_eq!(visible[19].path, "dir19");
               
               // Test next page
               app.next_page(items_per_page);
               assert_eq!(app.current_page, 1);
               assert_eq!(app.selected, 20);
               
               let visible = app.visible_items(items_per_page);
               assert_eq!(visible.len(), 5); // Last page has 5 items
               assert_eq!(visible[0].path, "dir20");
               assert_eq!(visible[4].path, "dir24");
               
               // Test previous page
               app.previous_page(items_per_page);
               assert_eq!(app.current_page, 0);
               assert_eq!(app.selected, 0);
               
               // Test go to specific page
               app.go_to_page(1, items_per_page);
               assert_eq!(app.current_page, 1);
               assert_eq!(app.selected, 20);
               
               let visible = app.visible_items(items_per_page);
               assert_eq!(visible.len(), 5); // Last page has 5 items
               assert_eq!(visible[0].path, "dir20");
               assert_eq!(visible[4].path, "dir24");
           }

           #[test]
           fn test_pagination_bounds() {
               let directories = (0..5).map(|i| create_test_directory(&format!("dir{}", i), i as u64 * 100)).collect();
               let mut app = App::new(directories, "test".to_string(), ".".to_string());
               let items_per_page = 20;
               
               // Test that we can't go beyond bounds
               app.previous_page(items_per_page); // Should not change anything
               assert_eq!(app.current_page, 0);
               
               app.next_page(items_per_page); // Should not change anything (only 1 page)
               assert_eq!(app.current_page, 0);
               
               app.go_to_page(5, items_per_page); // Should not change anything
               assert_eq!(app.current_page, 0);
           }

           #[test]
           fn test_visible_selected_index() {
               let directories = (0..25).map(|i| create_test_directory(&format!("dir{}", i), i as u64 * 100)).collect();
               let mut app = App::new(directories, "test".to_string(), ".".to_string());
               let items_per_page = 20;
               
               // First page, first item
               assert_eq!(app.visible_selected_index(items_per_page), 0);
               
               // First page, last item
               app.selected = 19;
               assert_eq!(app.visible_selected_index(items_per_page), 19);
               
               // Second page, first item
               app.selected = 20;
               app.current_page = 1;
               assert_eq!(app.visible_selected_index(items_per_page), 0);
               
               // Second page, last item
               app.selected = 24;
               assert_eq!(app.visible_selected_index(items_per_page), 4);
           }

    #[test]
    fn test_toggle_current_selection() {
        let mut app = App::new(vec![create_test_directory("dir1", 100)], "test".to_string(), ".".to_string());
        assert_eq!(app.directories[0].selected, false);
        app.toggle_current_selection();
        assert_eq!(app.directories[0].selected, true);
        app.toggle_current_selection();
        assert_eq!(app.directories[0].selected, false);
    }

    #[test]
    fn test_select_all_and_deselect_all() {
        let mut app = App::new(
            vec![create_test_directory("dir1", 100), create_test_directory("dir2", 200)],
            "test".to_string(), ".".to_string(),
        );
        app.select_all();
        assert!(app.directories.iter().all(|d| d.selected));
        app.deselect_all();
        assert!(app.directories.iter().all(|d| !d.selected));
    }

    #[test]
    fn test_select_and_deselect_current() {
        let mut app = App::new(
            vec![create_test_directory("dir1", 100), create_test_directory("dir2", 200)],
            "test".to_string(), ".".to_string(),
        );
        app.select_current();
        assert!(app.directories[0].selected);
        app.deselect_current();
        assert!(!app.directories[0].selected);
        app.selected = 1;
        app.select_current();
        assert!(app.directories[1].selected);
    }

    #[test]
    fn test_get_selected_count_and_directories() {
        let mut app = App::new(
            vec![create_test_directory("dir1", 100), create_test_directory("dir2", 200)],
            "test".to_string(), ".".to_string(),
        );
        assert_eq!(app.get_selected_count(), 0);
        app.directories[0].selected = true;
        assert_eq!(app.get_selected_count(), 1);
        let selected_dirs = app.get_selected_directories();
        assert_eq!(selected_dirs.len(), 1);
        assert_eq!(selected_dirs[0].path, "dir1");
    }

    #[test]
    fn test_get_selected_total_size() {
        let mut app = App::new(
            vec![create_test_directory("dir1", 100), create_test_directory("dir2", 200)],
            "test".to_string(), ".".to_string(),
        );
        assert_eq!(app.get_selected_total_size(), 0);
        app.directories[0].selected = true;
        app.directories[1].selected = true;
        assert_eq!(app.get_selected_total_size(), 300);
    }

    #[test]
    fn test_toggle_selection_mode() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        assert_eq!(app.selection_mode, false);
        app.toggle_selection_mode();
        assert_eq!(app.selection_mode, true);
        app.toggle_selection_mode();
        assert_eq!(app.selection_mode, false);
    }

    #[test]
    fn test_delete_current_directory() {
        use tempfile::tempdir;
        
        // Create a temporary directory
        let temp_dir = tempdir().unwrap();
        let test_path = temp_dir.path().join("test_dir");
        std::fs::create_dir(&test_path).unwrap();
        
        let mut app = App::new(
            vec![create_test_directory(test_path.to_str().unwrap(), 100)],
            "test".to_string(),
            ".".to_string(),
        );
        
        // Verify directory exists
        assert!(test_path.exists());
        
        // Delete the directory
        let result = app.delete_current_directory();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), test_path.to_str().unwrap());
        
        // Verify directory is deleted
        assert!(!test_path.exists());
        
        // Verify it's still in the list but marked as deleted
        assert_eq!(app.directories.len(), 1);
        assert!(matches!(app.directories[0].deletion_status, crate::fs::DeletionStatus::Deleted));
    }

    #[test]
    fn test_delete_selected_directories() {
        use tempfile::tempdir;
        
        // Create temporary directories
        let temp_dir = tempdir().unwrap();
        let test_path1 = temp_dir.path().join("test_dir1");
        let test_path2 = temp_dir.path().join("test_dir2");
        let test_path3 = temp_dir.path().join("test_dir3");
        
        std::fs::create_dir(&test_path1).unwrap();
        std::fs::create_dir(&test_path2).unwrap();
        std::fs::create_dir(&test_path3).unwrap();
        
        let mut dir1 = create_test_directory(test_path1.to_str().unwrap(), 100);
        let mut dir2 = create_test_directory(test_path2.to_str().unwrap(), 200);
        let dir3 = create_test_directory(test_path3.to_str().unwrap(), 300);
        
        // Select first two directories
        dir1.selected = true;
        dir2.selected = true;
        
        let mut app = App::new(
            vec![dir1, dir2, dir3],
            "test".to_string(),
            ".".to_string(),
        );
        
        // Verify directories exist
        assert!(test_path1.exists());
        assert!(test_path2.exists());
        assert!(test_path3.exists());
        
        // Delete selected directories
        let result = app.delete_selected_directories();
        assert!(result.is_ok());
        let deleted_paths = result.unwrap();
        assert_eq!(deleted_paths.len(), 2);
        assert!(deleted_paths.contains(&test_path1.to_str().unwrap().to_string()));
        assert!(deleted_paths.contains(&test_path2.to_str().unwrap().to_string()));
        
        // Verify directories are deleted
        assert!(!test_path1.exists());
        assert!(!test_path2.exists());
        assert!(test_path3.exists()); // This one should remain
        
        // Verify all directories are still in list but deleted ones are marked as deleted
        assert_eq!(app.directories.len(), 3);
        assert!(matches!(app.directories[0].deletion_status, crate::fs::DeletionStatus::Deleted));
        assert!(matches!(app.directories[1].deletion_status, crate::fs::DeletionStatus::Deleted));
        assert!(matches!(app.directories[2].deletion_status, crate::fs::DeletionStatus::Normal));
        assert_eq!(app.directories[2].path, test_path3.to_str().unwrap());
    }

    #[test]
    fn test_delete_nonexistent_directory() {
        let mut app = App::new(
            vec![create_test_directory("/nonexistent/path", 100)],
            "test".to_string(),
            ".".to_string(),
        );
        
        let result = app.delete_current_directory();
        assert!(result.is_err());
        
        // Directory should still be in the list since deletion failed
        assert_eq!(app.directories.len(), 1);
    }

    #[test]
    fn test_deletion_progress_tracking() {
        use tempfile::tempdir;
        
        // Create a temporary directory
        let temp_dir = tempdir().unwrap();
        let test_path = temp_dir.path().join("test_dir");
        std::fs::create_dir(&test_path).unwrap();
        
        let mut app = App::new(
            vec![create_test_directory(test_path.to_str().unwrap(), 100)],
            "test".to_string(),
            ".".to_string(),
        );
        
        // Initially no deletion in progress
        assert!(!app.is_deleting());
        assert!(app.get_deletion_progress().is_none());
        
        // Start deletion
        let result = app.delete_current_directory();
        assert!(result.is_ok());
        
        // After deletion, progress should be cleared
        assert!(!app.is_deleting());
        assert!(app.get_deletion_progress().is_none());
    }

    #[test]
    fn test_deletion_progress_with_multiple_items() {
        use tempfile::tempdir;
        
        // Create temporary directories
        let temp_dir = tempdir().unwrap();
        let test_path1 = temp_dir.path().join("test_dir1");
        let test_path2 = temp_dir.path().join("test_dir2");
        
        std::fs::create_dir(&test_path1).unwrap();
        std::fs::create_dir(&test_path2).unwrap();
        
        let mut dir1 = create_test_directory(test_path1.to_str().unwrap(), 100);
        let mut dir2 = create_test_directory(test_path2.to_str().unwrap(), 200);
        
        // Select both directories
        dir1.selected = true;
        dir2.selected = true;
        
        let mut app = App::new(
            vec![dir1, dir2],
            "test".to_string(),
            ".".to_string(),
        );
        
        // Initially no deletion in progress
        assert!(!app.is_deleting());
        
        // Delete selected directories
        let result = app.delete_selected_directories();
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
        
        // After deletion, progress should be cleared
        assert!(!app.is_deleting());
        assert!(app.get_deletion_progress().is_none());
    }
} 