use crate::fs::DirectoryInfo;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc;
use std::sync::{Arc, Condvar, Mutex};
use std::thread::{self, JoinHandle};

/// Concurrent directory size jobs (each walk is serial; throughput comes from
/// sizing many trees at once — typical `node_modules` cleanup workload).
pub fn max_concurrent_size_calcs() -> usize {
    crate::fs::scan_thread_count()
}

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
    // Discovery timing
    pub discovery_start_time: Option<std::time::Instant>,
    pub discovery_end_time: Option<std::time::Instant>,
    pub discovery_duration: Option<std::time::Duration>,
    // Total completion timing (discovery + all calculations)
    pub total_completion_time: Option<std::time::Duration>,
    // Parallel deletion system
    pub deletion_thread_pool: Option<DeletionThreadPool>,
    // Size calculation backpressure and fast lookups
    pub path_index: HashMap<String, usize>,
    pub cached_total_size: u64,
    pub cached_calculated_count: usize,
    pub cached_total_formatted: String,
    pub cached_selected_count: usize,
    pub cached_selected_size: u64,
    pub cached_in_flight_count: usize,
    // Pre-computed lowercased paths for fast filter matching
    lowercased_paths: Vec<String>,
    cached_filter_query: String,
    pub display_indices_dirty: bool,
    // O(1) unsized directory lookup — indices of directories not yet calculated
    unsized_indices: Vec<usize>,
    // UI state
    pub display_indices: Vec<usize>,
    pub sort_column: SortColumn,
    pub sort_direction: SortDirection,
    pub filter_query: String,
    pub applied_filter: String,
    pub filter_input_active: bool,
    pub show_details_panel: bool,
    pub show_help: bool,
    pub delete_confirmation: Option<DeleteConfirmation>,
    pub status_toast: Option<StatusToast>,
    pub user_sort_locked: bool,
    pub auto_sort_by_size_done: bool,
    pub auto_sort_by_size: bool,
    pub items_per_page: usize,
}

/// Temporary status message shown in the status bar
#[derive(Debug, Clone)]
pub struct StatusToast {
    pub message: String,
    pub until: std::time::Instant,
}

/// Column used for sorting the results table
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortColumn {
    Size,
    Path,
    Age,
}

/// Sort direction for results
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortDirection {
    Asc,
    Desc,
}

/// Pending destructive action awaiting confirmation
#[derive(Debug, Clone)]
pub struct DeleteConfirmation {
    pub action: DeleteConfirmAction,
    pub count: usize,
    pub total_size: u64,
    pub preview_paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeleteConfirmAction {
    Current,
    Selected,
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
    pub work_queue: Arc<(Mutex<VecDeque<DeletionTask>>, Condvar)>,
    pub sender: mpsc::Sender<DeletionMessage>,
    pub active_tasks: Arc<Mutex<usize>>,
    pub max_workers: usize,
}

impl DeletionThreadPool {
    /// Create a new deletion thread pool
    pub fn new(sender: mpsc::Sender<DeletionMessage>, max_workers: usize) -> Self {
        let work_queue = Arc::new((Mutex::new(VecDeque::new()), Condvar::new()));
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
        work_queue: Arc<(Mutex<VecDeque<DeletionTask>>, Condvar)>,
        sender: mpsc::Sender<DeletionMessage>,
        active_tasks: Arc<Mutex<usize>>,
    ) {
        let (lock, cvar) = &*work_queue;
        loop {
            // Wait for a task using Condvar (no busy-spin)
            let task = {
                let mut queue = lock.lock().unwrap();
                loop {
                    if let Some(task) = queue.pop_front() {
                        break Some(task);
                    }
                    // Wait until notified or 100ms timeout (for graceful shutdown)
                    let result = cvar.wait_timeout(queue, std::time::Duration::from_millis(100)).unwrap();
                    queue = result.0;
                }
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
                // Timeout — check again (graceful shutdown path)
            }
        }
    }

    /// Add a deletion task to the queue with priority
    pub fn add_task(&self, task: DeletionTask) -> Result<(), std::io::Error> {
        let (lock, cvar) = &*self.work_queue;
        let mut queue = lock.lock().unwrap();

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

        cvar.notify_one();
        Ok(())
    }

    /// Get the number of active tasks
    pub fn active_task_count(&self) -> usize {
        *self.active_tasks.lock().unwrap()
    }

    /// Get the number of queued tasks
    pub fn queued_task_count(&self) -> usize {
        self.work_queue.0.lock().unwrap().len()
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
        let config = if cfg!(test) {
            crate::config::Config::default()
        } else {
            crate::config::Config::load()
        };
        let sort_column = match config.sort_column.as_str() {
            "size" => SortColumn::Size,
            "age" => SortColumn::Age,
            _ => SortColumn::Path,
        };
        let sort_direction = match config.sort_direction.as_str() {
            "desc" => SortDirection::Desc,
            _ => SortDirection::Asc,
        };

        let mut app = Self {
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
            batch_size: 1, // Show matches immediately as they are discovered
            total_discovered: 0,
            discovery_start_time: None,
            discovery_end_time: None,
            discovery_duration: None,
            total_completion_time: None,
            deletion_thread_pool: None,
            path_index: HashMap::new(),
            cached_total_size: 0,
            cached_calculated_count: 0,
            cached_total_formatted: "0 B".to_string(),
            cached_selected_count: 0,
            cached_selected_size: 0,
            cached_in_flight_count: 0,
            lowercased_paths: Vec::new(),
            cached_filter_query: String::new(),
            display_indices_dirty: true,
            unsized_indices: Vec::new(),
            display_indices: Vec::new(),
            sort_column,
            sort_direction,
            filter_query: String::new(),
            applied_filter: String::new(),
            filter_input_active: false,
            show_details_panel: config.show_details_panel,
            show_help: false,
            delete_confirmation: None,
            status_toast: None,
            user_sort_locked: false,
            auto_sort_by_size_done: false,
            auto_sort_by_size: config.auto_sort_by_size,
            items_per_page: 20,
        };
        app.rebuild_aggregates_from_directories();
        app.rebuild_display_indices();
        app
    }

    /// Rebuild path index and cached size totals from the directory list.
    pub fn rebuild_aggregates_from_directories(&mut self) {
        self.path_index.clear();
        self.cached_total_size = 0;
        self.cached_calculated_count = 0;
        self.cached_selected_count = 0;
        self.cached_selected_size = 0;
        self.lowercased_paths.clear();
        self.unsized_indices.clear();

        for (idx, dir) in self.directories.iter().enumerate() {
            self.path_index.insert(dir.path.clone(), idx);
            self.lowercased_paths.push(dir.path.to_lowercase());
            if matches!(
                dir.calculation_status,
                crate::fs::CalculationStatus::Completed
            ) {
                self.cached_total_size += dir.size;
                self.cached_calculated_count += 1;
            } else {
                self.unsized_indices.push(idx);
            }
            if dir.selected {
                self.cached_selected_count += 1;
                self.cached_selected_size += dir.size;
            }
        }

        self.cached_total_formatted = crate::fs::format_size(self.cached_total_size);
    }

    /// Cached aggregate size stats for the UI (avoids per-frame O(n) scans).
    pub fn size_stats(&self) -> (u64, &str, usize, usize) {
        (
            self.cached_total_size,
            &self.cached_total_formatted,
            self.cached_calculated_count,
            self.directories.len(),
        )
    }

    /// Apply a completed size calculation and update cached totals.
    pub fn apply_size_update(
        &mut self,
        path: &str,
        size: u64,
        formatted_size: String,
        last_modified: Option<std::time::SystemTime>,
        formatted_last_modified: String,
    ) -> bool {
        let index = match self.path_index.get(path) {
            Some(&idx) if idx < self.directories.len() && self.directories[idx].path == path => idx,
            _ => return false,
        };

        let dir = &mut self.directories[index];
        if matches!(
            dir.calculation_status,
            crate::fs::CalculationStatus::Completed
        ) {
            return false;
        }

        dir.size = size;
        dir.formatted_size = formatted_size;
        dir.last_modified = last_modified;
        dir.formatted_last_modified = formatted_last_modified;
        dir.calculation_status = crate::fs::CalculationStatus::Completed;
        self.cached_total_size += size;
        self.cached_calculated_count += 1;
        self.cached_in_flight_count = self.cached_in_flight_count.saturating_sub(1);
        self.cached_total_formatted = crate::fs::format_size(self.cached_total_size);

        // Remove from unsized_indices (O(n) scan but rare — only happens once per dir)
        if let Some(pos) = self.unsized_indices.iter().position(|&i| i == index) {
            self.unsized_indices.swap_remove(pos);
        }

        if self.sort_column == SortColumn::Size || self.sort_column == SortColumn::Age {
            self.display_indices_dirty = true;
        }
        self.maybe_auto_sort_by_size();
        true
    }

    pub fn set_status_toast(&mut self, message: String) {
        self.status_toast = Some(StatusToast {
            message,
            until: std::time::Instant::now() + std::time::Duration::from_secs(2),
        });
    }

    pub fn active_toast_message(&self) -> Option<&str> {
        self.status_toast.as_ref().and_then(|toast| {
            if std::time::Instant::now() < toast.until {
                Some(toast.message.as_str())
            } else {
                None
            }
        })
    }

    pub fn clear_expired_toast(&mut self) -> bool {
        if let Some(toast) = &self.status_toast
            && std::time::Instant::now() >= toast.until
        {
            self.status_toast = None;
            return true;
        }
        false
    }

    pub fn filter_status_label(&self) -> Option<String> {
        let filter_text = if self.filter_input_active {
            self.filter_query.as_str()
        } else if self.has_active_filter() {
            self.applied_filter.as_str()
        } else {
            return None;
        };
        if filter_text.is_empty() {
            return None;
        }
        Some(format!(
            "filter: \"{filter_text}\" · {}/{}",
            self.view_len(),
            self.directories.len()
        ))
    }

    pub fn save_preferences(&self) {
        let (sort_column, sort_direction) = match self.sort_column {
            SortColumn::Size => ("size", match self.sort_direction {
                SortDirection::Asc => "asc",
                SortDirection::Desc => "desc",
            }),
            SortColumn::Path => ("path", match self.sort_direction {
                SortDirection::Asc => "asc",
                SortDirection::Desc => "desc",
            }),
            SortColumn::Age => ("age", match self.sort_direction {
                SortDirection::Asc => "asc",
                SortDirection::Desc => "desc",
            }),
        };
        let config = crate::config::Config {
            sort_column: sort_column.to_string(),
            sort_direction: sort_direction.to_string(),
            show_details_panel: self.show_details_panel,
            auto_sort_by_size: self.auto_sort_by_size,
        };
        let _ = config.save();
    }

    fn maybe_auto_sort_by_size(&mut self) {
        if !self.auto_sort_by_size
            || self.auto_sort_by_size_done
            || self.user_sort_locked
            || self.cached_calculated_count < self.directories.len()
            || self.directories.is_empty()
        {
            return;
        }
        self.sort_column = SortColumn::Size;
        self.sort_direction = SortDirection::Desc;
        self.auto_sort_by_size_done = true;
        self.rebuild_display_indices();
    }

    pub fn selected_size_percent(&self) -> Option<f64> {
        let dir = self.get_selected_directory()?;
        if self.cached_total_size == 0 || dir.size == 0 {
            return None;
        }
        Some((dir.size as f64 / self.cached_total_size as f64) * 100.0)
    }

    pub fn copy_selected_path(&self) -> Result<(), std::io::Error> {
        let Some(dir) = self.get_selected_directory() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No directory selected",
            ));
        };
        copy_to_clipboard(&dir.path)
    }

    /// Number of rows visible in the current filtered/sorted view
    pub fn view_len(&self) -> usize {
        self.display_indices.len()
    }

    /// Filter string currently driving the table (draft while editing, otherwise applied)
    fn effective_filter(&self) -> &str {
        if self.filter_input_active {
            &self.filter_query
        } else {
            &self.applied_filter
        }
    }

    pub fn has_active_filter(&self) -> bool {
        !self.applied_filter.is_empty()
    }

    pub fn is_filtering(&self) -> bool {
        !self.effective_filter().is_empty()
    }

    /// Rebuild filtered + sorted view indices after data or UI changes
    pub fn rebuild_display_indices(&mut self) {
        let query = self.effective_filter().to_lowercase();
        let had_view = !self.display_indices.is_empty();
        let selected_path = if had_view {
            self.get_selected_directory().map(|dir| dir.path.clone())
        } else {
            None
        };

        self.cached_filter_query = query.clone();

        let mut indices: Vec<usize> = self
            .directories
            .iter()
            .enumerate()
            .filter(|(idx, _)| {
                query.is_empty()
                    || self
                        .lowercased_paths
                        .get(*idx)
                        .is_some_and(|lp| lp.contains(&query))
            })
            .map(|(idx, _)| idx)
            .collect();

        indices.sort_by(|&a, &b| {
            let ordering = match self.sort_column {
                SortColumn::Size => self.directories[a].size.cmp(&self.directories[b].size),
                SortColumn::Path => natural_path_cmp(&self.directories[a].path, &self.directories[b].path),
                SortColumn::Age => match (
                    self.directories[a].last_modified,
                    self.directories[b].last_modified,
                ) {
                    (Some(left), Some(right)) => left.cmp(&right),
                    (None, Some(_)) => std::cmp::Ordering::Greater,
                    (Some(_), None) => std::cmp::Ordering::Less,
                    (None, None) => std::cmp::Ordering::Equal,
                },
            };
            if self.sort_direction == SortDirection::Desc {
                ordering.reverse()
            } else {
                ordering
            }
        });

        self.display_indices = indices;

        if self.display_indices.is_empty() {
            self.selected = 0;
            self.current_page = 0;
            return;
        }

        if let Some(path) = selected_path {
            if let Some(new_sel) = self
                .display_indices
                .iter()
                .position(|&idx| self.directories[idx].path == path)
            {
                self.selected = new_sel;
            } else {
                self.selected = self
                    .selected
                    .min(self.display_indices.len().saturating_sub(1));
            }
        } else {
            self.selected = self
                .selected
                .min(self.display_indices.len().saturating_sub(1));
        }

        if let Some(page) = self.selected.checked_div(self.items_per_page) {
            self.current_page = page;
        }
    }

    pub fn toggle_sort(&mut self, column: SortColumn) {
        self.user_sort_locked = true;
        if self.sort_column == column {
            self.sort_direction = match self.sort_direction {
                SortDirection::Asc => SortDirection::Desc,
                SortDirection::Desc => SortDirection::Asc,
            };
        } else {
            self.sort_column = column;
            self.sort_direction = match column {
                SortColumn::Size => SortDirection::Desc,
                SortColumn::Path => SortDirection::Asc,
                SortColumn::Age => SortDirection::Desc,
            };
        }
        self.rebuild_display_indices();
        self.save_preferences();
    }

    pub fn sort_label(&self) -> String {
        let col = match self.sort_column {
            SortColumn::Size => "size",
            SortColumn::Path => "path",
            SortColumn::Age => "age",
        };
        let arrow = match self.sort_direction {
            SortDirection::Asc => "↑",
            SortDirection::Desc => "↓",
        };
        format!("sort: {col}{arrow}")
    }

    pub fn status_line_summary(&self) -> String {
        let total = self.directories.len();
        let calculated = self.cached_calculated_count;
        let sizing = if calculated < total {
            format!("sizing {calculated}/{total}")
        } else if total > 0 {
            format!(
                "~{} releasable",
                self.cached_total_formatted
            )
        } else {
            String::new()
        };
        let timing = self.format_status_timing_label();

        match self.discovery_status {
            DiscoveryStatus::Discovering => {
                if self.total_discovered == 0 {
                    "scanning…".to_string()
                } else {
                    format!("{} found · {timing} · {sizing}", self.total_discovered)
                }
            }
            DiscoveryStatus::Complete => {
                if total == 0 {
                    format!("0 found · {timing}")
                } else {
                    format!("{total} found · {timing} · {sizing}")
                }
            }
            DiscoveryStatus::Error(ref err) => format!("error: {err}"),
            DiscoveryStatus::NotStarted => "ready".to_string(),
        }
    }

    pub fn sizing_progress_bar(&self, width: usize) -> String {
        let total = self.directories.len();
        if total == 0 {
            return String::new();
        }
        let done = self.cached_calculated_count;
        let filled = (done * width / total).min(width);
        format!(
            "[{}{}] {done}/{total}",
            "█".repeat(filled),
            "░".repeat(width - filled),
        )
    }

    pub fn begin_filter_input(&mut self) {
        if !self.filter_input_active {
            self.filter_query = self.applied_filter.clone();
        }
        self.filter_input_active = true;
    }

    pub fn commit_filter(&mut self) {
        self.applied_filter = self.filter_query.clone();
        self.filter_input_active = false;
        self.rebuild_display_indices();
    }

    pub fn cancel_filter(&mut self) {
        self.filter_input_active = false;
        if self.filter_query.is_empty() {
            self.applied_filter.clear();
            self.filter_query.clear();
        } else {
            self.filter_query = self.applied_filter.clone();
        }
        self.rebuild_display_indices();
    }

    pub fn clear_filter(&mut self) {
        self.filter_query.clear();
        self.applied_filter.clear();
        self.filter_input_active = false;
        self.rebuild_display_indices();
    }

    pub fn push_filter_char(&mut self, ch: char) {
        self.filter_query.push(ch);
        self.rebuild_display_indices();
    }

    pub fn pop_filter_char(&mut self) {
        self.filter_query.pop();
        self.rebuild_display_indices();
    }

    pub fn toggle_details_panel(&mut self) {
        self.show_details_panel = !self.show_details_panel;
        self.save_preferences();
    }

    pub fn toggle_help(&mut self) {
        self.show_help = !self.show_help;
    }

    pub fn request_delete_current(&mut self) {
        let Some(dir) = self.get_selected_directory() else {
            return;
        };
        self.delete_confirmation = Some(DeleteConfirmation {
            action: DeleteConfirmAction::Current,
            count: 1,
            total_size: dir.size,
            preview_paths: vec![dir.path.clone()],
        });
    }

    pub fn request_delete_selected(&mut self) {
        let selected: Vec<_> = self
            .directories
            .iter()
            .filter(|dir| dir.selected)
            .collect();
        if selected.is_empty() {
            return;
        }
        let total_size: u64 = selected.iter().map(|dir| dir.size).sum();
        self.delete_confirmation = Some(DeleteConfirmation {
            action: DeleteConfirmAction::Selected,
            count: selected.len(),
            total_size,
            preview_paths: selected.iter().map(|dir| dir.path.clone()).collect(),
        });
    }

    pub fn cancel_delete_confirmation(&mut self) {
        self.delete_confirmation = None;
    }

    pub fn confirm_delete(&mut self) -> DeleteConfirmAction {
        let action = self
            .delete_confirmation
            .as_ref()
            .map(|c| c.action)
            .unwrap_or(DeleteConfirmAction::Current);
        self.delete_confirmation = None;
        action
    }

    pub fn open_selected_in_file_manager(&self) -> Result<(), std::io::Error> {
        let Some(dir) = self.get_selected_directory() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No directory selected",
            ));
        };
        let path = std::path::Path::new(&dir.path);
        let open_path = path.parent().unwrap_or(path);
        open_path_in_file_manager(open_path)
    }

    /// Mark up to `max_concurrent` not-started directories as calculating.
    /// Prefers visible (current page) rows, then falls back to `unsized_indices`.
    pub fn dequeue_size_calculations(
        &mut self,
        max_concurrent: usize,
        items_per_page: usize,
    ) -> Vec<String> {
        let slots = max_concurrent.saturating_sub(self.cached_in_flight_count);
        if slots == 0 || self.unsized_indices.is_empty() {
            return Vec::new();
        }

        if self.display_indices_dirty {
            self.rebuild_display_indices();
        }

        let mut paths = Vec::with_capacity(slots);
        let page_size = if items_per_page == 0 {
            20
        } else {
            items_per_page
        };
        let start = self.current_page * page_size;
        let end = std::cmp::min(start + page_size, self.display_indices.len());

        // Prefer directories currently visible on the page
        let mut preferred = Vec::new();
        for &dir_idx in &self.display_indices[start..end] {
            if matches!(
                self.directories[dir_idx].calculation_status,
                crate::fs::CalculationStatus::NotStarted
            ) {
                preferred.push(dir_idx);
                if preferred.len() >= slots {
                    break;
                }
            }
        }

        for dir_idx in preferred {
            if let Some(pos) = self.unsized_indices.iter().position(|&i| i == dir_idx) {
                self.unsized_indices.swap_remove(pos);
            }
            self.directories[dir_idx].calculation_status =
                crate::fs::CalculationStatus::Calculating;
            self.cached_in_flight_count += 1;
            paths.push(self.directories[dir_idx].path.clone());
        }

        // Fill remaining slots from unsized_indices (LIFO)
        while paths.len() < slots && !self.unsized_indices.is_empty() {
            let idx = self.unsized_indices.pop().unwrap();
            if idx < self.directories.len()
                && matches!(
                    self.directories[idx].calculation_status,
                    crate::fs::CalculationStatus::NotStarted
                )
            {
                self.directories[idx].calculation_status =
                    crate::fs::CalculationStatus::Calculating;
                self.cached_in_flight_count += 1;
                paths.push(self.directories[idx].path.clone());
            }
        }

        paths
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

        // Convert to DirectoryInfo and add to main list.
        // Defer parent mtime to the size worker so discovery stays off the UI hot path.
        for dir_path in batch {
            let directory_info = DirectoryInfo {
                path: dir_path,
                size: 0,
                formatted_size: "Calculating...".to_string(),
                last_modified: None,
                formatted_last_modified: "…".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::NotStarted,
                calculation_time: None,
            };

            let index = self.directories.len();
            self.path_index
                .insert(directory_info.path.clone(), index);
            self.lowercased_paths.push(directory_info.path.to_lowercase());
            self.unsized_indices.push(index);
            self.directories.push(directory_info);
        }
        self.display_indices_dirty = true;
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

    /// Start discovery timing
    pub fn start_discovery_timing(&mut self) {
        self.discovery_start_time = Some(std::time::Instant::now());
        self.discovery_end_time = None;
        self.discovery_duration = None;
    }

    /// End discovery timing
    pub fn end_discovery_timing(&mut self) {
        if let Some(start_time) = self.discovery_start_time {
            self.discovery_end_time = Some(std::time::Instant::now());
            self.discovery_duration =
                Some(self.discovery_end_time.unwrap().duration_since(start_time));
        }
    }

    /// Get discovery duration
    pub fn get_discovery_duration(&self) -> Option<std::time::Duration> {
        if let Some(duration) = self.discovery_duration {
            Some(duration)
        } else {
            // Discovery in progress — return live elapsed time
            self.discovery_start_time
                .map(|start_time| std::time::Instant::now().duration_since(start_time))
        }
    }

    /// Get formatted discovery duration
    pub fn get_formatted_discovery_duration(&self) -> String {
        if let Some(duration) = self.get_discovery_duration() {
            crate::fs::format_duration(&duration)
        } else {
            "Not started".to_string()
        }
    }

    /// Label for discovery-only timing (comparable to npkill "search completed")
    pub fn format_discovery_timing_label(&self) -> String {
        if let Some(duration) = self.discovery_duration {
            format!("search: {}", crate::fs::format_duration(&duration))
        } else if let Some(duration) = self.get_discovery_duration() {
            format!("search: {}", crate::fs::format_duration(&duration))
        } else {
            "search: unknown".to_string()
        }
    }

    /// Wall-clock elapsed since scan start (live while sizing, fixed when complete)
    pub fn get_total_elapsed(&self) -> Option<std::time::Duration> {
        if let Some(duration) = self.total_completion_time {
            Some(duration)
        } else {
            self.discovery_start_time
                .map(|start| std::time::Instant::now().duration_since(start))
        }
    }

    pub fn format_total_timing_label(&self) -> String {
        match self.get_total_elapsed() {
            Some(duration) => format!(
                "total: {}",
                crate::fs::format_duration_in_seconds(&duration)
            ),
            None => "total: unknown".to_string(),
        }
    }

    pub fn format_status_timing_label(&self) -> String {
        format!(
            "{} · {}",
            self.format_discovery_timing_label(),
            self.format_total_timing_label()
        )
    }

    /// Update total completion time if all calculations are done
    pub fn update_total_completion_time(&mut self) {
        if self.total_completion_time.is_none() {
            let total_count = self.directories.len();
            if self.cached_calculated_count == total_count && total_count > 0 {
                if let Some(start_time) = self.discovery_start_time {
                    self.total_completion_time =
                        Some(std::time::Instant::now().duration_since(start_time));
                }
                self.maybe_auto_sort_by_size();
            }
        }
    }

    /// Check if discovery is still in progress
    pub fn is_discovering(&self) -> bool {
        matches!(self.discovery_status, DiscoveryStatus::Discovering)
    }

    /// Get the current total size of all directories
    pub fn get_current_total_size(&self) -> (u64, &str) {
        (self.cached_total_size, &self.cached_total_formatted)
    }

    /// Get discovery progress information
    pub fn get_discovery_progress(&self) -> String {
        match self.discovery_status {
            DiscoveryStatus::NotStarted => "Ready to scan...".to_string(),
            DiscoveryStatus::Discovering => {
                let timing_info = if let Some(duration) = self.get_discovery_duration() {
                    format!(" ({} elapsed)", crate::fs::format_duration(&duration))
                } else {
                    String::new()
                };

                if self.total_discovered == 0 {
                    format!("Scanning directories...{timing_info}")
                } else {
                    let (_, size_formatted) = self.get_current_total_size();
                    format!(
                        "Found {} directories ({}), showing {}...{}",
                        self.total_discovered,
                        size_formatted,
                        self.directories.len(),
                        timing_info
                    )
                }
            }
            DiscoveryStatus::Complete => {
                let total_count = self.directories.len();
                let (_, size_formatted) = self.get_current_total_size();

                if self.cached_calculated_count == total_count && total_count > 0 {
                    let discovery_timing = self.format_discovery_timing_label();
                    let sizing_timing = self
                        .total_completion_time
                        .map(|duration| {
                            format!(
                                ", sizes: {}",
                                crate::fs::format_duration(&duration.saturating_sub(
                                    self.discovery_duration.unwrap_or(duration)
                                ))
                            )
                        })
                        .unwrap_or_default();
                    format!(
                        "Search complete: {} directories, {} total ({}{})",
                        self.total_discovered, size_formatted, discovery_timing, sizing_timing
                    )
                } else if total_count == 0 {
                    format!(
                        "Search complete: {} directories found ({})",
                        self.total_discovered,
                        self.format_discovery_timing_label()
                    )
                } else {
                    format!(
                        "Search complete: {} directories, {} total ({}; sizing {}/{})",
                        self.total_discovered,
                        size_formatted,
                        self.format_discovery_timing_label(),
                        self.cached_calculated_count,
                        total_count
                    )
                }
            }
            DiscoveryStatus::Error(ref error) => {
                let timing_info = if let Some(duration) = self.get_discovery_duration() {
                    format!(" after {}", crate::fs::format_duration(&duration))
                } else {
                    String::new()
                };
                format!("Scan error: {error}{timing_info}")
            }
        }
    }

    pub fn next(&mut self, items_per_page: usize) {
        if self.view_len() > 0 {
            self.selected = (self.selected + 1) % self.view_len();
            self.update_selection_for_pagination(items_per_page);
        }
    }

    pub fn previous(&mut self, items_per_page: usize) {
        if self.view_len() > 0 {
            self.selected = if self.selected == 0 {
                self.view_len() - 1
            } else {
                self.selected - 1
            };
            self.update_selection_for_pagination(items_per_page);
        }
    }

    pub fn select_first(&mut self) {
        if self.view_len() > 0 {
            self.selected = 0;
        }
    }

    pub fn select_last(&mut self) {
        if self.view_len() > 0 {
            self.selected = self.view_len() - 1;
        }
    }

    pub fn get_selected_directory(&self) -> Option<&DirectoryInfo> {
        self.display_indices
            .get(self.selected)
            .and_then(|&idx| self.directories.get(idx))
    }

    fn selected_directory_index(&self) -> Option<usize> {
        self.display_indices.get(self.selected).copied()
    }

    pub fn directory_count(&self) -> usize {
        self.directories.len()
    }

    pub fn is_empty(&self) -> bool {
        self.directories.is_empty()
    }

    // Pagination methods
    pub fn total_pages(&self, items_per_page: usize) -> usize {
        if self.display_indices.is_empty() || items_per_page == 0 {
            0
        } else {
            (self.display_indices.len() - 1) / items_per_page + 1
        }
    }

    pub fn clamp_pagination(&mut self) {
        if self.items_per_page == 0 || self.display_indices.is_empty() {
            self.current_page = 0;
            self.selected = 0;
            return;
        }
        let max_page = self.total_pages(self.items_per_page).saturating_sub(1);
        if self.current_page > max_page {
            self.current_page = max_page;
        }
        let max_sel = self.display_indices.len().saturating_sub(1);
        if self.selected > max_sel {
            self.selected = max_sel;
        }
    }

    pub fn visible_items(&self, items_per_page: usize) -> Vec<&DirectoryInfo> {
        let start = self.current_page * items_per_page;
        let end = std::cmp::min(start + items_per_page, self.display_indices.len());
        self.display_indices
            .get(start..end)
            .unwrap_or(&[])
            .iter()
            .filter_map(|&idx| self.directories.get(idx))
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
        if self.view_len() == 0 {
            self.selected = 0;
            self.current_page = 0;
            return;
        }

        if self.selected >= self.view_len() {
            self.selected = self.view_len().saturating_sub(1);
        }

        self.current_page = self.selected / items_per_page;
    }

    // Selection methods
    pub fn toggle_selection_mode(&mut self) {
        self.selection_mode = !self.selection_mode;
    }

    pub fn toggle_current_selection(&mut self) {
        if let Some(idx) = self.selected_directory_index() {
            let dir = &mut self.directories[idx];
            dir.selected = !dir.selected;
            if dir.selected {
                self.cached_selected_count += 1;
                self.cached_selected_size += dir.size;
            } else {
                self.cached_selected_count -= 1;
                self.cached_selected_size -= dir.size;
            }
        }
    }

    pub fn select_all(&mut self) {
        for &idx in &self.display_indices {
            if !self.directories[idx].selected {
                self.directories[idx].selected = true;
                self.cached_selected_count += 1;
                self.cached_selected_size += self.directories[idx].size;
            }
        }
    }

    pub fn deselect_all(&mut self) {
        for dir in &mut self.directories {
            dir.selected = false;
        }
        self.cached_selected_count = 0;
        self.cached_selected_size = 0;
    }

    pub fn select_current(&mut self) {
        if let Some(idx) = self.selected_directory_index()
            && !self.directories[idx].selected
        {
            self.directories[idx].selected = true;
            self.cached_selected_count += 1;
            self.cached_selected_size += self.directories[idx].size;
        }
    }

    pub fn deselect_current(&mut self) {
        if let Some(idx) = self.selected_directory_index()
            && self.directories[idx].selected
        {
            self.directories[idx].selected = false;
            self.cached_selected_count -= 1;
            self.cached_selected_size -= self.directories[idx].size;
        }
    }

    pub fn get_selected_count(&self) -> usize {
        self.cached_selected_count
    }

    pub fn get_selected_directories(&self) -> Vec<&DirectoryInfo> {
        self.directories.iter().filter(|dir| dir.selected).collect()
    }

    pub fn get_selected_total_size(&self) -> u64 {
        self.cached_selected_size
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

    pub fn deletion_status_label(&self) -> Option<String> {
        let progress = self.deletion_progress.as_ref()?;
        let path = progress
            .current_path
            .strip_prefix("./")
            .unwrap_or(&progress.current_path);
        if progress.total_items > 1 {
            Some(format!(
                "Deleting {}/{}: {path}",
                progress.completed_items.min(progress.total_items) + 1,
                progress.total_items
            ))
        } else if path.is_empty() {
            Some("Deleting…".to_string())
        } else {
            Some(format!("Deleting: {path}"))
        }
    }

    fn mark_directory_deleting(&mut self, index: usize) {
        if index < self.directories.len() {
            self.directories[index].deletion_status = crate::fs::DeletionStatus::Deleting;
            if let Some(progress) = &mut self.deletion_progress {
                progress.current_path = self.directories[index].path.clone();
            }
        }
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

    /// Process any pending deletion messages. Returns true if UI state changed.
    pub fn process_deletion_messages(&mut self) -> bool {
        let mut changed = false;
        let mut toast_message: Option<String> = None;
        if let Some(receiver) = &self.deletion_receiver {
            while let Ok(message) = receiver.try_recv() {
                changed = true;
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
                        if index < self.directories.len() {
                            self.directories[index].deletion_status = status;
                            if let Some(progress) = &mut self.deletion_progress {
                                progress.current_path = self.directories[index].path.clone();
                            }
                        }
                    }
                    DeletionMessage::Complete { results } => {
                        let mut freed = 0u64;
                        let mut deleted_count = 0usize;
                        // Process completion
                        for result in results {
                            if result.index < self.directories.len() {
                                if result.success {
                                    if let Some(progress) = &mut self.deletion_progress {
                                        progress.completed_items += 1;
                                        progress
                                            .deleted_paths
                                            .push(result.path.clone());
                                    }
                                    // Track freed space
                                    let freed_size = self.directories[result.index].size;
                                    self.total_freed_space += freed_size;
                                    freed += freed_size;
                                    deleted_count += 1;
                                    self.cached_total_size =
                                        self.cached_total_size.saturating_sub(freed_size);
                                    self.cached_calculated_count =
                                        self.cached_calculated_count.saturating_sub(1);
                                    self.cached_total_formatted =
                                        crate::fs::format_size(self.cached_total_size);

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
                                    if let Some(progress) = &mut self.deletion_progress {
                                        progress.completed_items += 1;
                                        progress.errors.push(format!(
                                            "{}: {}",
                                            result.path,
                                            result
                                                .error
                                                .as_deref()
                                                .unwrap_or("Unknown error")
                                        ));
                                    }
                                    self.directories[result.index].deletion_status =
                                        crate::fs::DeletionStatus::Error(
                                            result
                                                .error
                                                .unwrap_or_else(|| "Unknown error".to_string()),
                                        );
                                }
                            }
                        }
                        if deleted_count > 0 {
                            let releasable = self.cached_total_formatted.clone();
                            toast_message = Some(format!(
                                "Deleted {} · freed {} · ~{} releasable",
                                if deleted_count == 1 {
                                    "1 directory".to_string()
                                } else {
                                    format!("{deleted_count} directories")
                                },
                                crate::fs::format_size(freed),
                                releasable
                            ));
                        } else if let Some(progress) = &self.deletion_progress
                            && let Some(err) = progress.errors.first()
                        {
                            toast_message = Some(format!("Delete failed: {err}"));
                        }
                        // Clear progress
                        self.deletion_progress = None;
                    }
                }
            }
        }
        if let Some(message) = toast_message {
            self.set_status_toast(message);
        }
        changed
    }
    pub fn start_delete_current_directory(&mut self) -> Result<(), std::io::Error> {
        if let Some(dir) = self.get_selected_directory() {
            let path = dir.path.clone();
            let Some(index) = self.selected_directory_index() else {
                return Ok(());
            };

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

            self.mark_directory_deleting(index);

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

        let paths: Vec<String> = selected_indices
            .iter()
            .map(|&i| self.directories[i].path.clone())
            .collect();

        // Initialize channel if not already done
        if self.deletion_sender.is_none() {
            self.init_deletion_channel();
        }

        // Initialize progress tracking
        self.deletion_progress = Some(DeletionProgress {
            total_items: selected_indices.len(),
            completed_items: 0,
            current_path: paths.first().cloned().unwrap_or_default(),
            deleted_paths: Vec::new(),
            errors: Vec::new(),
            freed_space: 0,
            freed_space_this_session: self.total_freed_space,
        });

        for &index in &selected_indices {
            self.mark_directory_deleting(index);
        }

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
            if let Some(index) = self.selected_directory_index() {
                self.directories[index].deletion_status = crate::fs::DeletionStatus::Deleting;
            }

            match std::fs::remove_dir_all(&path) {
                Ok(_) => {
                    // Update progress
                    if let Some(progress) = &mut self.deletion_progress {
                        progress.completed_items = 1;
                        progress.deleted_paths.push(path.clone());
                    }

                    // Mark as deleted (but keep in list)
                    if let Some(index) = self.selected_directory_index() {
                        self.directories[index].deletion_status =
                            crate::fs::DeletionStatus::Deleted;
                    }

                    let freed_size = self
                        .get_selected_directory()
                        .map(|d| d.size)
                        .unwrap_or(0);
                    self.total_freed_space += freed_size;
                    self.cached_total_size = self.cached_total_size.saturating_sub(freed_size);
                    self.cached_calculated_count = self.cached_calculated_count.saturating_sub(1);
                    self.cached_total_formatted = crate::fs::format_size(self.cached_total_size);
                    self.set_status_toast(format!(
                        "Deleted · freed {} · ~{} releasable",
                        crate::fs::format_size(freed_size),
                        self.cached_total_formatted
                    ));

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
                    if let Some(index) = self.selected_directory_index() {
                        self.directories[index].deletion_status =
                            crate::fs::DeletionStatus::Error(e.to_string());
                    }

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

fn natural_path_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let mut a_chars = a.chars().peekable();
    let mut b_chars = b.chars().peekable();

    loop {
        match (a_chars.peek(), b_chars.peek()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(a_peek), Some(b_peek)) => {
                if a_peek.is_ascii_digit() && b_peek.is_ascii_digit() {
                    let a_num: String = a_chars.by_ref().take_while(|c| c.is_ascii_digit()).collect();
                    let b_num: String = b_chars.by_ref().take_while(|c| c.is_ascii_digit()).collect();
                    let ordering = a_num
                        .parse::<u128>()
                        .unwrap_or(0)
                        .cmp(&b_num.parse::<u128>().unwrap_or(0));
                    if ordering != Ordering::Equal {
                        return ordering;
                    }
                } else {
                    let a_ch = a_chars.next().unwrap();
                    let b_ch = b_chars.next().unwrap();
                    let ordering = a_ch
                        .to_ascii_lowercase()
                        .cmp(&b_ch.to_ascii_lowercase());
                    if ordering != Ordering::Equal {
                        return ordering;
                    }
                }
            }
        }
    }
}

fn copy_to_clipboard(text: &str) -> Result<(), std::io::Error> {
    use std::io::Write;

    #[cfg(target_os = "macos")]
    {
        let mut child = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(text.as_bytes())?;
        }
        child.wait()?;
        return Ok(());
    }

    #[cfg(target_os = "linux")]
    {
        if std::process::Command::new("wl-copy")
            .stdin(std::process::Stdio::piped())
            .spawn()
            .and_then(|mut child| {
                if let Some(stdin) = child.stdin.as_mut() {
                    stdin.write_all(text.as_bytes())?;
                }
                child.wait()?;
                Ok(())
            })
            .is_ok()
        {
            return Ok(());
        }
        let mut child = std::process::Command::new("xclip")
            .args(["-selection", "clipboard"])
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(text.as_bytes())?;
        }
        child.wait()?;
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    {
        let mut child = std::process::Command::new("clip")
            .stdin(std::process::Stdio::piped())
            .spawn()?;
        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(text.as_bytes())?;
        }
        child.wait()?;
        return Ok(());
    }

    #[allow(unreachable_code)]
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Clipboard copy is not supported on this platform",
    ))
}

fn open_path_in_file_manager(path: &std::path::Path) -> Result<(), std::io::Error> {
    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open").arg(path).spawn()?;
        return Ok(());
    }
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(path).spawn()?;
        return Ok(());
    }
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer").arg(path).spawn()?;
        return Ok(());
    }
    #[allow(unreachable_code)]
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "Opening paths is not supported on this platform",
    ))
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
            calculation_time: None,
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
            .map(|i| create_test_directory(&format!("dir{i:02}"), i as u64 * 100))
            .collect();
        let mut app = App::new(directories, "test".to_string(), ".".to_string());
        let items_per_page = 20;

        // Test total pages (25 items, 20 per page = 2 pages)
        assert_eq!(app.total_pages(items_per_page), 2);

        // Test visible items on first page
        let visible = app.visible_items(items_per_page);
        assert_eq!(visible.len(), 20);
        assert_eq!(visible[0].path, "dir00");
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
    fn test_filter_esc_clears_applied_results() {
        let directories = vec![
            create_test_directory("alpha/node_modules", 100),
            create_test_directory("beta/node_modules", 200),
        ];
        let mut app = App::new(directories, "node_modules".to_string(), ".".to_string());

        app.begin_filter_input();
        app.push_filter_char('a');
        app.push_filter_char('l');
        app.push_filter_char('p');
        app.push_filter_char('h');
        app.push_filter_char('a');
        app.commit_filter();
        assert_eq!(app.view_len(), 1);
        assert_eq!(app.directories[app.display_indices[0]].path, "alpha/node_modules");

        app.begin_filter_input();
        while !app.filter_query.is_empty() {
            app.pop_filter_char();
        }
        app.cancel_filter();
        assert!(!app.has_active_filter());
        assert_eq!(app.view_len(), 2);

        app.begin_filter_input();
        app.push_filter_char('b');
        app.push_filter_char('e');
        app.push_filter_char('t');
        app.push_filter_char('a');
        app.commit_filter();
        assert_eq!(app.view_len(), 1);

        app.clear_filter();
        assert_eq!(app.view_len(), 2);
    }

    #[test]
    fn test_natural_path_sort_order() {
        let directories = vec![
            create_test_directory("dir2", 100),
            create_test_directory("dir10", 200),
            create_test_directory("dir1", 50),
        ];
        let mut app = App::new(directories, "test".to_string(), ".".to_string());
        app.sort_column = SortColumn::Path;
        app.sort_direction = SortDirection::Asc;
        app.rebuild_display_indices();
        assert_eq!(app.directories[app.display_indices[0]].path, "dir1");
        assert_eq!(app.directories[app.display_indices[1]].path, "dir2");
        assert_eq!(app.directories[app.display_indices[2]].path, "dir10");
    }

    #[test]
    fn test_pagination_bounds() {
        let directories = (0..5)
            .map(|i| create_test_directory(&format!("dir{i}"), i as u64 * 100))
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
            .map(|i| create_test_directory(&format!("dir{i}"), i as u64 * 100))
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
        assert!(!app.directories[0].selected);
        app.toggle_current_selection();
        assert!(app.directories[0].selected);
        app.toggle_current_selection();
        assert!(!app.directories[0].selected);
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
        app.toggle_current_selection(); // selects dir1
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
        app.toggle_current_selection(); // selects dir1
        app.selected = 1;
        app.select_current(); // selects dir2
        assert_eq!(app.get_selected_total_size(), 300);
    }

    #[test]
    fn test_toggle_selection_mode() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        assert!(!app.selection_mode);
        app.toggle_selection_mode();
        assert!(app.selection_mode);
        app.toggle_selection_mode();
        assert!(!app.selection_mode);
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
    fn test_display_indices_tracks_all_discovered_directories() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        for i in 1..=15 {
            app.add_discovered_directory(format!("dir{i}"));
            if app.display_indices_dirty {
                app.rebuild_display_indices();
                app.display_indices_dirty = false;
            }
        }
        assert_eq!(app.directories.len(), 15);
        assert_eq!(app.view_len(), 15);

        for i in 16..=59 {
            app.add_discovered_directory(format!("dir{i}"));
        }
        app.rebuild_display_indices();
        assert_eq!(app.directories.len(), 59);
        assert_eq!(app.view_len(), 59);
        assert_eq!(app.total_pages(10), 6);
    }

    #[test]
    fn test_add_discovered_directory() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Add directories one by one
        app.add_discovered_directory("dir1".to_string());
        app.add_discovered_directory("dir2".to_string());
        app.add_discovered_directory("dir3".to_string());

        // Default batch size is 1 — each directory appears immediately
        assert_eq!(app.directories.len(), 3);
        assert_eq!(app.pending_directories.len(), 0);
        assert_eq!(app.total_discovered, 3);
    }

    #[test]
    fn test_process_pending_batch() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        app.batch_size = 5;

        // Add 6 directories (more than batch size)
        for i in 1..=6 {
            app.add_discovered_directory(format!("dir{i}"));
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
            app.add_discovered_directory(format!("dir{i}"));
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
        assert!(progress.contains("Found 2 directories") && progress.contains("showing 2"));

        // Complete
        app.set_discovery_status(DiscoveryStatus::Complete);
        let progress = app.get_discovery_progress();
        assert!(progress.contains("Search complete: 2 directories"));

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
            app.add_discovered_directory(format!("dir{i}"));
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
            app.add_discovered_directory(format!("dir{i}"));
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
                .contains("Search complete: 3 directories")
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

    #[test]
    fn test_dequeue_size_calculations_respects_concurrency_limit() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        for i in 0..8 {
            app.directories.push(DirectoryInfo {
                path: format!("dir{i}"),
                size: 0,
                formatted_size: "Calculating...".to_string(),
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::NotStarted,
                calculation_time: None,
            });
            app.path_index.insert(format!("dir{i}"), i);
            app.lowercased_paths.push(format!("dir{i}"));
            app.unsized_indices.push(i);
        }

        let first_batch = app.dequeue_size_calculations(4, 20);
        assert_eq!(first_batch.len(), 4);
        assert_eq!(app.cached_in_flight_count, 4);

        let second_batch = app.dequeue_size_calculations(4, 20);
        assert!(second_batch.is_empty());

        // Simulate completion via apply_size_update (which removes from unsized_indices)
        for path in &first_batch {
            app.apply_size_update(path, 1024, "1.0 KB".to_string(), None, "Unknown".to_string());
        }

        let third_batch = app.dequeue_size_calculations(4, 20);
        assert_eq!(third_batch.len(), 4);
    }

    #[test]
    fn test_dequeue_size_calculations_prefers_visible_page() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        app.items_per_page = 2;
        app.current_page = 0;

        for i in 0..6 {
            app.directories.push(DirectoryInfo {
                path: format!("dir{i}"),
                size: 0,
                formatted_size: "Calculating...".to_string(),
                last_modified: None,
                formatted_last_modified: "…".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::NotStarted,
                calculation_time: None,
            });
            app.path_index.insert(format!("dir{i}"), i);
            app.lowercased_paths.push(format!("dir{i}"));
            app.unsized_indices.push(i);
        }
        app.display_indices_dirty = true;

        let batch = app.dequeue_size_calculations(2, 2);
        assert_eq!(batch.len(), 2);
        // Visible page (indices 0,1 in discovery/sort-by-path order) should be preferred
        assert!(batch.contains(&"dir0".to_string()) || batch.contains(&"dir1".to_string()));
        assert_eq!(app.cached_in_flight_count, 2);
    }

    #[test]
    fn test_apply_size_update_updates_cached_totals() {
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        app.directories.push(DirectoryInfo {
            path: "dir1".to_string(),
            size: 0,
            formatted_size: "Calculating...".to_string(),
            last_modified: None,
            formatted_last_modified: "Unknown".to_string(),
            selected: false,
            deletion_status: crate::fs::DeletionStatus::Normal,
            calculation_status: crate::fs::CalculationStatus::Calculating,
            calculation_time: None,
        });
        app.path_index.insert("dir1".to_string(), 0);

        assert!(app.apply_size_update(
            "dir1",
            1024,
            "1.0 KB".to_string(),
            None,
            "Unknown".to_string()
        ));
        assert_eq!(app.cached_total_size, 1024);
        assert_eq!(app.cached_calculated_count, 1);
        assert_eq!(app.cached_total_formatted, "1.0 KB");
        assert!(!app.apply_size_update(
            "dir1",
            2048,
            "2.0 KB".to_string(),
            None,
            "Unknown".to_string()
        ));
        assert_eq!(app.cached_total_size, 1024);
    }
}
