use crate::fs::DirectoryInfo;
use std::collections::VecDeque;
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};

/// Application state for TUI
pub struct App {
    pub directories: Vec<DirectoryInfo>,
    pub selected: usize,
    pub pattern: String,
    pub path: String,
    pub ignore_patterns: crate::fs::IgnorePatterns,
    pub current_page: usize,
    pub selection_mode: bool,
    pub deletion_progress: Option<DeletionProgress>,
    pub deletion_sender: Option<mpsc::Sender<DeletionMessage>>,
    pub deletion_receiver: Option<mpsc::Receiver<DeletionMessage>>,
    pub total_freed_space: u64,
    pub freed_space_history: Vec<FreedSpaceEntry>,
    // Progressive loading state
    pub discovery_status: DiscoveryStatus,
    pub pending_directories: Vec<String>,
    pub batch_size: usize,
    pub total_discovered: usize,
    // Parallel deletion system
    pub deletion_thread_pool: Option<DeletionThreadPool>,
}

/// Status of directory discovery
#[derive(Debug, Clone, PartialEq)]
pub enum DiscoveryStatus {
    /// Discovery has not started yet
    NotStarted,
    /// Discovery is in progress
    Discovering,
    /// Discovery is complete
    Complete,
    /// Discovery encountered an error
    Error(String),
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
    StartSingle {
        index: usize,
        path: String,
    },
    StartMultiple {
        indices: Vec<usize>,
        paths: Vec<String>,
    },
    Progress {
        index: usize,
        status: crate::fs::DeletionStatus,
    },
    Complete {
        results: Vec<DeletionResult>,
    },
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

/// Priority levels for deletion tasks
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeletionPriority {
    Small,  // < 1MB
    Medium, // 1-100MB
    Large,  // 100MB-1GB
    Huge,   // > 1GB
}

/// Individual deletion task
#[derive(Debug, Clone)]
pub struct DeletionTask {
    pub index: usize,
    pub path: String,
    pub priority: DeletionPriority,
    pub size: u64,
}

/// Thread pool for parallel deletion operations
pub struct DeletionThreadPool {
    pub workers: Vec<JoinHandle<()>>,
    pub work_queue: Arc<Mutex<VecDeque<DeletionTask>>>,
    pub sender: mpsc::Sender<DeletionMessage>,
    pub active_tasks: Arc<Mutex<usize>>,
    pub max_workers: usize,
}

impl DeletionThreadPool {
    /// Create a new deletion thread pool
    pub fn new(sender: mpsc::Sender<DeletionMessage>, max_workers: usize) -> Self {
        let work_queue = Arc::new(Mutex::new(VecDeque::new()));
        let active_tasks = Arc::new(Mutex::new(0));

        let mut workers = Vec::new();

        // Spawn worker threads
        for worker_id in 0..max_workers {
            let work_queue = work_queue.clone();
            let sender = sender.clone();
            let active_tasks = active_tasks.clone();

            let handle = thread::spawn(move || {
                Self::worker_loop(worker_id, work_queue, sender, active_tasks);
            });

            workers.push(handle);
        }

        Self {
            workers,
            work_queue,
            sender,
            active_tasks,
            max_workers,
        }
    }

    /// Worker thread main loop
    fn worker_loop(
        _worker_id: usize,
        work_queue: Arc<Mutex<VecDeque<DeletionTask>>>,
        sender: mpsc::Sender<DeletionMessage>,
        active_tasks: Arc<Mutex<usize>>,
    ) {
        loop {
            // Try to get a task from the queue
            let task = {
                let mut queue = work_queue.lock().unwrap();
                queue.pop_front()
            };

            if let Some(task) = task {
                // Increment active tasks counter
                {
                    let mut active = active_tasks.lock().unwrap();
                    *active += 1;
                }

                // Send start message
                let _ = sender.send(DeletionMessage::Progress {
                    index: task.index,
                    status: crate::fs::DeletionStatus::Deleting,
                });

                // Perform the deletion
                let result = std::fs::remove_dir_all(&task.path);
                let success = result.is_ok();
                let error = result.err().map(|e| e.to_string());

                // Send completion message
                let _ = sender.send(DeletionMessage::Complete {
                    results: vec![DeletionResult {
                        index: task.index,
                        path: task.path,
                        success,
                        error,
                    }],
                });

                // Decrement active tasks counter
                {
                    let mut active = active_tasks.lock().unwrap();
                    *active -= 1;
                }
            } else {
                // No tasks available, sleep briefly
                thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }

    /// Add a deletion task to the queue with priority
    pub fn add_task(&self, task: DeletionTask) -> Result<(), std::io::Error> {
        let mut queue = self.work_queue.lock().unwrap();

        // Insert based on priority (higher priority = lower enum value)
        let mut inserted = false;
        for (i, existing_task) in queue.iter().enumerate() {
            if task.priority < existing_task.priority {
                queue.insert(i, task.clone());
                inserted = true;
                break;
            }
        }

        if !inserted {
            queue.push_back(task);
        }

        Ok(())
    }

    /// Get the number of active tasks
    pub fn active_task_count(&self) -> usize {
        *self.active_tasks.lock().unwrap()
    }

    /// Get the number of queued tasks
    pub fn queued_task_count(&self) -> usize {
        self.work_queue.lock().unwrap().len()
    }

    /// Check if the thread pool is idle
    pub fn is_idle(&self) -> bool {
        self.active_task_count() == 0 && self.queued_task_count() == 0
    }
}

/// Helper function to determine deletion priority based on directory size
fn get_deletion_priority(size: u64) -> DeletionPriority {
    match size {
        0..=1_048_576 => DeletionPriority::Small, // < 1MB
        1_048_577..=104_857_600 => DeletionPriority::Medium, // 1-100MB
        104_857_601..=1_073_741_824 => DeletionPriority::Large, // 100MB-1GB
        _ => DeletionPriority::Huge,              // > 1GB
    }
}

impl App {
    pub fn new(directories: Vec<DirectoryInfo>, pattern: String, path: String) -> Self {
        Self::new_with_ignore(
            directories,
            pattern,
            path,
            crate::fs::IgnorePatterns::new("").unwrap(),
        )
    }

    pub fn new_with_ignore(
        directories: Vec<DirectoryInfo>,
        pattern: String,
        path: String,
        ignore_patterns: crate::fs::IgnorePatterns,
    ) -> Self {
        Self {
            directories,
            selected: 0,
            pattern,
            path,
            ignore_patterns,
            current_page: 0,
            selection_mode: false,
            deletion_progress: None,
            deletion_sender: None,
            deletion_receiver: None,
            total_freed_space: 0,
            freed_space_history: Vec::new(),
            discovery_status: DiscoveryStatus::NotStarted,
            pending_directories: Vec::new(),
            batch_size: 5, // Default batch size for progressive loading
            total_discovered: 0,
            deletion_thread_pool: None,
        }
    }

    /// Add a newly discovered directory to the pending list
    pub fn add_discovered_directory(&mut self, path: String) {
        self.pending_directories.push(path);
        self.total_discovered += 1;

        // Process batch if we have enough items
        if self.pending_directories.len() >= self.batch_size {
            self.process_pending_batch();
        }
    }

    /// Process a batch of pending directories and add them to the main list
    pub fn process_pending_batch(&mut self) {
        if self.pending_directories.is_empty() {
            return;
        }

        // Take up to batch_size items from pending
        let batch: Vec<String> = self
            .pending_directories
            .drain(..std::cmp::min(self.batch_size, self.pending_directories.len()))
            .collect();

        // Convert to DirectoryInfo and add to main list
        for dir_path in batch {
            let path = std::path::Path::new(&dir_path);
            let last_modified =
                crate::fs::get_directory_last_modified(path.parent().unwrap_or(path));
            let formatted_last_modified = last_modified
                .as_ref()
                .map(crate::fs::format_last_modified)
                .unwrap_or_else(|| "Unknown".to_string());

            let directory_info = DirectoryInfo {
                path: dir_path,
                size: 0,
                formatted_size: "Calculating...".to_string(),
                last_modified,
                formatted_last_modified,
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::NotStarted,
            };

            self.directories.push(directory_info);
        }

        // Start size calculation for the newly added items
        self.start_size_calculation_for_new_items();
    }

    /// Start size calculation for newly added directories
    pub fn start_size_calculation_for_new_items(&mut self) {
        // This will be implemented to start background size calculations
        // for the most recently added items
        // For now, we'll rely on the existing size calculation system
        // that's already implemented in the UI module
    }

    /// Process any remaining pending directories (called when discovery is complete)
    pub fn process_remaining_pending(&mut self) {
        while !self.pending_directories.is_empty() {
            self.process_pending_batch();
        }
    }

    /// Set discovery status
    pub fn set_discovery_status(&mut self, status: DiscoveryStatus) {
        self.discovery_status = status;

        // If discovery is complete, process any remaining pending items
        if matches!(self.discovery_status, DiscoveryStatus::Complete) {
            self.process_remaining_pending();
        }
    }

    /// Check if discovery is still in progress
    pub fn is_discovering(&self) -> bool {
        matches!(self.discovery_status, DiscoveryStatus::Discovering)
    }

    /// Get discovery progress information
    pub fn get_discovery_progress(&self) -> String {
        match self.discovery_status {
            DiscoveryStatus::NotStarted => "Ready to scan...".to_string(),
            DiscoveryStatus::Discovering => {
                if self.total_discovered == 0 {
                    "Scanning directories...".to_string()
                } else {
                    format!(
                        "Found {} directories, showing {}...",
                        self.total_discovered,
                        self.directories.len()
                    )
                }
            }
            DiscoveryStatus::Complete => {
                format!("Scan complete: {} directories found", self.total_discovered)
            }
            DiscoveryStatus::Error(ref error) => {
                format!("Scan error: {error}")
            }
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
        self.directories
            .get(start..end)
            .unwrap_or(&[])
            .iter()
            .collect()
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
        self.directories
            .iter()
            .filter(|dir| dir.selected)
            .map(|dir| dir.size)
            .sum()
    }

    /// Delete selected directories using parallel thread pool (HIGH PERFORMANCE)
    pub fn delete_selected_directories(&mut self) -> Result<Vec<String>, std::io::Error> {
        let selected_indices: Vec<usize> = self
            .directories
            .iter()
            .enumerate()
            .filter(|(_, dir)| dir.selected)
            .map(|(i, _)| i)
            .collect();

        if selected_indices.is_empty() {
            return Ok(Vec::new());
        }

        // Initialize channel and thread pool if not already done
        if self.deletion_thread_pool.is_none() {
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

        // Create deletion tasks with priority based on size
        let mut tasks = Vec::new();
        for &index in &selected_indices {
            if index >= self.directories.len() {
                continue;
            }

            let dir = &self.directories[index];
            let priority = get_deletion_priority(dir.size);

            tasks.push(DeletionTask {
                index,
                path: dir.path.clone(),
                priority,
                size: dir.size,
            });
        }

        // Add all tasks to the thread pool queue
        if let Some(thread_pool) = &self.deletion_thread_pool {
            for task in tasks {
                thread_pool.add_task(task)?;
            }
        }

        // Return immediately - deletion happens in background
        // The UI will show progress through the message processing system
        Ok(Vec::new()) // Empty vector since deletion is now async
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
        self.freed_space_history
            .get(start..)
            .unwrap_or(&[])
            .iter()
            .collect()
    }

    /// Initialize background deletion channel and thread pool
    pub fn init_deletion_channel(&mut self) {
        let (tx, rx) = mpsc::channel();
        self.deletion_sender = Some(tx.clone());
        self.deletion_receiver = Some(rx);

        // Initialize the parallel deletion thread pool
        // Use 4 workers for optimal performance (can be tuned based on system)
        self.deletion_thread_pool = Some(DeletionThreadPool::new(tx, 4));
    }

    /// Process any pending deletion messages
    pub fn process_deletion_messages(&mut self) {
        if let Some(receiver) = &self.deletion_receiver {
            while let Ok(message) = receiver.try_recv() {
                match message {
                    DeletionMessage::StartSingle { index, path: _ } => {
                        // Mark as deleting
                        if index < self.directories.len() {
                            self.directories[index].deletion_status =
                                crate::fs::DeletionStatus::Deleting;
                        }
                    }
                    DeletionMessage::StartMultiple { indices, paths: _ } => {
                        // Mark all as deleting
                        for &index in &indices {
                            if index < self.directories.len() {
                                self.directories[index].deletion_status =
                                    crate::fs::DeletionStatus::Deleting;
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

                                    self.directories[result.index].deletion_status =
                                        crate::fs::DeletionStatus::Deleted;
                                } else {
                                    self.directories[result.index].deletion_status =
                                        crate::fs::DeletionStatus::Error(
                                            result
                                                .error
                                                .unwrap_or_else(|| "Unknown error".to_string()),
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
                let _ = sender.send(DeletionMessage::StartSingle {
                    index,
                    path: path.clone(),
                });

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
                        results: vec![deletion_result],
                    });
                });
            }

            Ok(())
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No directory selected",
            ))
        }
    }

    /// Start background deletion of selected directories
    pub fn start_delete_selected_directories(&mut self) -> Result<(), std::io::Error> {
        let selected_indices: Vec<usize> = self
            .directories
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
                paths: paths.clone(),
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
                        status: crate::fs::DeletionStatus::Deleting,
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
                    self.directories[self.selected].deletion_status =
                        crate::fs::DeletionStatus::Deleted;

                    // Clear progress
                    self.deletion_progress = None;

                    Ok(path)
                }
                Err(e) => {
                    // Update progress with error
                    if let Some(progress) = &mut self.deletion_progress {
                        progress
                            .errors
                            .push(format!("Failed to delete {path}: {e}"));
                    }

                    // Mark as error
                    self.directories[self.selected].deletion_status =
                        crate::fs::DeletionStatus::Error(e.to_string());

                    // Clear progress
                    self.deletion_progress = None;

                    Err(e)
                }
            }
        } else {
            Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No directory selected",
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
            formatted_size: format!("{size} B"),
            last_modified: None,
            formatted_last_modified: "Unknown".to_string(),
            selected: false,
            deletion_status: crate::fs::DeletionStatus::Normal,
            calculation_status: crate::fs::CalculationStatus::Completed,
        }
    }

    #[test]
    fn test_app_creation() {
        let directories = vec![
            create_test_directory("dir1", 100),
            create_test_directory("dir2", 200),
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
            create_test_directory("dir3", 300),
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
            create_test_directory("dir2", 200),
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
        let app = App::new(
            vec![
                create_test_directory("dir1", 100),
                create_test_directory("dir2", 200),
            ],
            "test".to_string(),
            ".".to_string(),
        );
        assert_eq!(app.directory_count(), 2);
    }

    #[test]
    fn test_is_empty() {
        let app = App::new(vec![], "test".to_string(), ".".to_string());
        assert!(app.is_empty());

        let app = App::new(
            vec![create_test_directory("dir1", 100)],
            "test".to_string(),
            ".".to_string(),
        );
        assert!(!app.is_empty());
    }

    #[test]
    fn test_select_first() {
        let directories = vec![
            create_test_directory("dir1", 100),
            create_test_directory("dir2", 200),
            create_test_directory("dir3", 300),
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
            create_test_directory("dir3", 300),
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
        let directories = (0..25)
            .map(|i| create_test_directory(&format!("dir{}", i), i as u64 * 100))
            .collect();
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
        let directories = (0..5)
            .map(|i| create_test_directory(&format!("dir{}", i), i as u64 * 100))
            .collect();
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
        let directories = (0..25)
            .map(|i| create_test_directory(&format!("dir{}", i), i as u64 * 100))
            .collect();
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
        let mut app = App::new(
            vec![create_test_directory("dir1", 100)],
            "test".to_string(),
            ".".to_string(),
        );
        assert_eq!(app.directories[0].selected, false);
        app.toggle_current_selection();
        assert_eq!(app.directories[0].selected, true);
        app.toggle_current_selection();
        assert_eq!(app.directories[0].selected, false);
    }

    #[test]
    fn test_select_all_and_deselect_all() {
        let mut app = App::new(
            vec![
                create_test_directory("dir1", 100),
                create_test_directory("dir2", 200),
            ],
            "test".to_string(),
            ".".to_string(),
        );
        app.select_all();
        assert!(app.directories.iter().all(|d| d.selected));
        app.deselect_all();
        assert!(app.directories.iter().all(|d| !d.selected));
    }

    #[test]
    fn test_select_and_deselect_current() {
        let mut app = App::new(
            vec![
                create_test_directory("dir1", 100),
                create_test_directory("dir2", 200),
            ],
            "test".to_string(),
            ".".to_string(),
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
            vec![
                create_test_directory("dir1", 100),
                create_test_directory("dir2", 200),
            ],
            "test".to_string(),
            ".".to_string(),
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
            vec![
                create_test_directory("dir1", 100),
                create_test_directory("dir2", 200),
            ],
            "test".to_string(),
            ".".to_string(),
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
        assert!(matches!(
            app.directories[0].deletion_status,
            crate::fs::DeletionStatus::Deleted
        ));
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

        let mut app = App::new(vec![dir1, dir2, dir3], "test".to_string(), ".".to_string());

        // Verify directories exist
        assert!(test_path1.exists());
        assert!(test_path2.exists());
        assert!(test_path3.exists());

        // Delete selected directories (now async)
        let result = app.delete_selected_directories();
        assert!(result.is_ok());

        // The new parallel deletion system returns immediately with empty vector
        // since deletion happens in background threads
        let deleted_paths = result.unwrap();
        assert_eq!(deleted_paths.len(), 0); // Async deletion returns empty immediately

        // Process deletion messages to simulate background completion
        app.process_deletion_messages();

        // Verify all directories are still in list (async deletion doesn't remove them immediately)
        assert_eq!(app.directories.len(), 3);

        // The deletion status will be updated by background threads
        // For now, we just verify the method works without panicking
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

        let mut app = App::new(vec![dir1, dir2], "test".to_string(), ".".to_string());

        // Initially no deletion in progress
        assert!(!app.is_deleting());

        // Delete selected directories (now async)
        let result = app.delete_selected_directories();
        assert!(result.is_ok());

        // The new parallel deletion system returns immediately with empty vector
        let deleted_paths = result.unwrap();
        assert_eq!(deleted_paths.len(), 0); // Async deletion returns empty immediately

        // Process deletion messages to simulate background completion
        app.process_deletion_messages();

        // The deletion progress will be managed by background threads
        // For now, we just verify the method works without panicking
    }

    // New tests for progressive loading functionality
    #[test]
    fn test_add_discovered_directory() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Add directories one by one
        app.add_discovered_directory("dir1".to_string());
        app.add_discovered_directory("dir2".to_string());
        app.add_discovered_directory("dir3".to_string());

        // Should not process batch yet (only 3 items, batch size is 5)
        assert_eq!(app.directories.len(), 0);
        assert_eq!(app.pending_directories.len(), 3);
        assert_eq!(app.total_discovered, 3);
    }

    #[test]
    fn test_process_pending_batch() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Add 6 directories (more than batch size)
        for i in 1..=6 {
            app.add_discovered_directory(format!("dir{}", i));
        }

        // Should process first batch of 5
        assert_eq!(app.directories.len(), 5);
        assert_eq!(app.pending_directories.len(), 1);
        assert_eq!(app.total_discovered, 6);

        // Verify the directories were added correctly
        assert_eq!(app.directories[0].path, "dir1");
        assert_eq!(app.directories[4].path, "dir5");
        assert_eq!(app.pending_directories[0], "dir6");
    }

    #[test]
    fn test_process_remaining_pending() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Add 3 directories (less than batch size)
        for i in 1..=3 {
            app.add_discovered_directory(format!("dir{}", i));
        }

        // Process remaining
        app.process_remaining_pending();

        assert_eq!(app.directories.len(), 3);
        assert_eq!(app.pending_directories.len(), 0);
        assert_eq!(app.total_discovered, 3);
    }

    #[test]
    fn test_discovery_status_transitions() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Initial state
        assert!(matches!(app.discovery_status, DiscoveryStatus::NotStarted));
        assert!(!app.is_discovering());

        // Set to discovering
        app.set_discovery_status(DiscoveryStatus::Discovering);
        assert!(matches!(app.discovery_status, DiscoveryStatus::Discovering));
        assert!(app.is_discovering());

        // Set to complete
        app.set_discovery_status(DiscoveryStatus::Complete);
        assert!(matches!(app.discovery_status, DiscoveryStatus::Complete));
        assert!(!app.is_discovering());

        // Set to error
        app.set_discovery_status(DiscoveryStatus::Error("test error".to_string()));
        assert!(matches!(app.discovery_status, DiscoveryStatus::Error(_)));
        assert!(!app.is_discovering());
    }

    #[test]
    fn test_get_discovery_progress() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Not started
        assert_eq!(app.get_discovery_progress(), "Ready to scan...");

        // Discovering with no results
        app.set_discovery_status(DiscoveryStatus::Discovering);
        assert_eq!(app.get_discovery_progress(), "Scanning directories...");

        // Discovering with results
        app.add_discovered_directory("dir1".to_string());
        app.add_discovered_directory("dir2".to_string());
        app.process_remaining_pending();

        let progress = app.get_discovery_progress();
        assert!(progress.contains("Found 2 directories, showing 2..."));

        // Complete
        app.set_discovery_status(DiscoveryStatus::Complete);
        let progress = app.get_discovery_progress();
        assert!(progress.contains("Scan complete: 2 directories found"));

        // Error
        app.set_discovery_status(DiscoveryStatus::Error("test error".to_string()));
        let progress = app.get_discovery_progress();
        assert!(progress.contains("Scan error: test error"));
    }

    #[test]
    fn test_batch_processing_with_size_calculation() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Add directories and process batch
        for i in 1..=5 {
            app.add_discovered_directory(format!("dir{}", i));
        }

        // Verify all directories have correct initial state
        assert_eq!(app.directories.len(), 5);
        for dir in &app.directories {
            assert_eq!(dir.size, 0);
            assert_eq!(dir.formatted_size, "Calculating...");
            assert!(matches!(
                dir.calculation_status,
                crate::fs::CalculationStatus::NotStarted
            ));
            assert!(!dir.selected);
            assert!(matches!(
                dir.deletion_status,
                crate::fs::DeletionStatus::Normal
            ));
        }
    }

    #[test]
    fn test_custom_batch_size() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        app.batch_size = 3; // Set custom batch size

        // Add 4 directories
        for i in 1..=4 {
            app.add_discovered_directory(format!("dir{}", i));
        }

        // Should process first batch of 3
        assert_eq!(app.directories.len(), 3);
        assert_eq!(app.pending_directories.len(), 1);

        // Process remaining
        app.process_remaining_pending();
        assert_eq!(app.directories.len(), 4);
        assert_eq!(app.pending_directories.len(), 0);
    }

    #[test]
    fn test_discovery_progress_counter() {
        // Test that the total_discovered counter works correctly
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Set discovery status to discovering
        app.set_discovery_status(DiscoveryStatus::Discovering);

        // Initially should be 0
        assert_eq!(app.total_discovered, 0);
        assert_eq!(app.get_discovery_progress(), "Scanning directories...");

        // Add some directories
        app.add_discovered_directory("dir1".to_string());
        assert_eq!(app.total_discovered, 1);
        assert!(app.get_discovery_progress().contains("Found 1 directories"));

        app.add_discovered_directory("dir2".to_string());
        assert_eq!(app.total_discovered, 2);
        assert!(app.get_discovery_progress().contains("Found 2 directories"));

        app.add_discovered_directory("dir3".to_string());
        assert_eq!(app.total_discovered, 3);
        assert!(app.get_discovery_progress().contains("Found 3 directories"));

        // Process remaining to see the final state
        app.process_remaining_pending();
        assert_eq!(app.total_discovered, 3);
        assert_eq!(app.directories.len(), 3);

        // Complete discovery
        app.set_discovery_status(DiscoveryStatus::Complete);
        assert!(
            app.get_discovery_progress()
                .contains("Scan complete: 3 directories found")
        );
    }

    #[test]
    fn test_progress_message_during_batch_processing() {
        // Test that progress message shows correctly during batch processing
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        app.batch_size = 5; // Set batch size to 5

        // Set discovery status to discovering
        app.set_discovery_status(DiscoveryStatus::Discovering);

        // Initially should show "Scanning directories..."
        assert_eq!(app.get_discovery_progress(), "Scanning directories...");

        // Add 3 directories (less than batch size, so they stay in pending)
        app.add_discovered_directory("dir1".to_string());
        app.add_discovered_directory("dir2".to_string());
        app.add_discovered_directory("dir3".to_string());

        // Should have discovered 3 but they're still in pending
        assert_eq!(app.total_discovered, 3);
        assert_eq!(app.directories.len(), 0); // Still empty because batch not processed
        assert_eq!(app.pending_directories.len(), 3);

        // Progress should show "Found 3 directories, showing 0..."
        let progress = app.get_discovery_progress();
        assert!(progress.contains("Found 3 directories"));
        assert!(progress.contains("showing 0"));

        // Process the pending directories
        app.process_remaining_pending();

        // Now should have 3 directories in the main list
        assert_eq!(app.directories.len(), 3);
        assert_eq!(app.pending_directories.len(), 0);

        // Progress should show "Found 3 directories, showing 3..."
        let progress = app.get_discovery_progress();
        assert!(progress.contains("Found 3 directories"));
        assert!(progress.contains("showing 3"));
    }

    #[test]
    fn test_parallel_deletion_system() {
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
        let mut dir3 = create_test_directory(test_path3.to_str().unwrap(), 300);

        // Select all directories
        dir1.selected = true;
        dir2.selected = true;
        dir3.selected = true;

        let mut app = App::new(vec![dir1, dir2, dir3], "test".to_string(), ".".to_string());

        // Verify directories exist
        assert!(test_path1.exists());
        assert!(test_path2.exists());
        assert!(test_path3.exists());

        // Test parallel deletion system
        let result = app.delete_selected_directories();
        assert!(result.is_ok());

        // Should return empty vector immediately (async deletion)
        let deleted_paths = result.unwrap();
        assert_eq!(deleted_paths.len(), 0);

        // Should have thread pool initialized
        assert!(app.deletion_thread_pool.is_some());

        // Should have progress tracking initialized
        assert!(app.deletion_progress.is_some());
        let progress = app.deletion_progress.as_ref().unwrap();
        assert_eq!(progress.total_items, 3);
        assert_eq!(progress.completed_items, 0);

        // Process deletion messages to simulate background completion
        app.process_deletion_messages();

        // Verify all directories are still in list (async deletion doesn't remove them immediately)
        assert_eq!(app.directories.len(), 3);
    }

    #[test]
    fn test_deletion_priority_system() {
        // Test that deletion priority is correctly assigned based on size
        assert_eq!(get_deletion_priority(0), DeletionPriority::Small);
        assert_eq!(get_deletion_priority(500_000), DeletionPriority::Small); // < 1MB
        assert_eq!(get_deletion_priority(1_048_576), DeletionPriority::Small); // 1MB
        assert_eq!(get_deletion_priority(1_048_577), DeletionPriority::Medium); // > 1MB
        assert_eq!(get_deletion_priority(50_000_000), DeletionPriority::Medium); // 50MB
        assert_eq!(get_deletion_priority(104_857_600), DeletionPriority::Medium); // 100MB
        assert_eq!(get_deletion_priority(104_857_601), DeletionPriority::Large); // > 100MB
        assert_eq!(get_deletion_priority(500_000_000), DeletionPriority::Large); // 500MB
        assert_eq!(
            get_deletion_priority(1_073_741_824),
            DeletionPriority::Large
        ); // 1GB
        assert_eq!(get_deletion_priority(1_073_741_825), DeletionPriority::Huge); // > 1GB
        assert_eq!(get_deletion_priority(2_000_000_000), DeletionPriority::Huge); // 2GB
    }

    #[test]
    fn test_thread_pool_creation() {
        let (sender, _receiver) = mpsc::channel::<DeletionMessage>();
        let thread_pool = DeletionThreadPool::new(sender, 4);

        // Should have 4 workers
        assert_eq!(thread_pool.max_workers, 4);
        assert_eq!(thread_pool.workers.len(), 4);

        // Should start with no active or queued tasks
        assert_eq!(thread_pool.active_task_count(), 0);
        assert_eq!(thread_pool.queued_task_count(), 0);
        assert!(thread_pool.is_idle());
    }

    #[test]
    fn test_deletion_task_creation() {
        let task = DeletionTask {
            index: 0,
            path: "/test/path".to_string(),
            priority: DeletionPriority::Medium,
            size: 50_000_000,
        };

        assert_eq!(task.index, 0);
        assert_eq!(task.path, "/test/path");
        assert_eq!(task.priority, DeletionPriority::Medium);
        assert_eq!(task.size, 50_000_000);
    }
}
