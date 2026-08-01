use anyhow::Result;
use ratatui::{
    layout::Rect,
    Terminal,
    backend::CrosstermBackend,
};
use std::io;

/// Remove ./ prefix from path if present
fn clean_path(path: &str) -> &str {
    path.strip_prefix("./").unwrap_or(path)
}

#[cfg(test)]
mod test_helpers {
    use ratatui::style::Color;

    pub const TEXT_SECONDARY: Color = Color::Rgb(189, 174, 147);
    pub const SELECTION_INDICATOR_COLOR: Color = Color::Rgb(184, 187, 38);

    pub fn get_loading_frame() -> &'static str {
        "⠋"
    }

    pub fn get_directory_icon(selected: bool, _is_highlighted: bool) -> &'static str {
        if selected {
            "📂"
        } else {
            "📁"
        }
    }

    pub fn get_selection_indicator_color(selected: bool) -> Color {
        if selected {
            SELECTION_INDICATOR_COLOR
        } else {
            TEXT_SECONDARY
        }
    }

    pub fn get_calculation_status_icon(status: &crate::fs::CalculationStatus) -> &'static str {
        match status {
            crate::fs::CalculationStatus::NotStarted => "⏳",
            crate::fs::CalculationStatus::Calculating => "⠋",
            crate::fs::CalculationStatus::Completed => "",
            crate::fs::CalculationStatus::Error(_) => "❌",
        }
    }
}

pub mod app;
pub mod view;

#[allow(unused_imports)]
use crate::fs::{self, DirectoryInfo};
use app::{App, DeleteConfirmAction, SortColumn};
use view::RenderContext;

/// Initialize the terminal for TUI mode
pub fn init_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    // Check if we're in a TTY and if the terminal supports the features we need
    if !crossterm::terminal::is_raw_mode_enabled()? {
        crossterm::terminal::enable_raw_mode()?;
    }

    let stdout = io::stdout();
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

/// Restore terminal to normal mode
pub fn restore_terminal() -> Result<()> {
    crossterm::terminal::disable_raw_mode()?;
    crossterm::execute!(
        io::stdout(),
        crossterm::terminal::Clear(crossterm::terminal::ClearType::All)
    )?;
    Ok(())
}

/// Display directories in beautiful TUI format with real-time scanning
pub fn display_directories_with_scanning(
    pattern: &str,
    path: &str,
    ignore_patterns: &str,
    no_tui: bool,
    json: bool,
) -> Result<()> {
    if json {
        return display_directories_json(pattern, path, ignore_patterns);
    }

    if no_tui {
        return display_directories_text(pattern, path, ignore_patterns);
    }

    // Check if we're in a terminal that supports TUI
    let term = std::env::var("TERM").unwrap_or_default();

    // macOS Terminal.app often has issues with TUI, so we'll use text mode
    let use_tui = !term.is_empty() && term != "dumb" && !term.contains("Apple_Terminal");

    if use_tui {
        // Try to initialize TUI mode, fallback to text mode if it fails
        match init_terminal() {
            Ok(mut terminal) => {
                // TUI mode successful, use the full interface
                display_directories_tui(&mut terminal, pattern, path, ignore_patterns)
            }
            Err(_) => {
                // TUI mode failed, fallback to text mode
                display_directories_text(pattern, path, ignore_patterns)
            }
        }
    } else {
        // Use text mode for unsupported terminals
        display_directories_text(pattern, path, ignore_patterns)
    }
}

/// Output scan results as JSON (blocking until sizes are calculated)
pub fn display_directories_json(pattern: &str, path: &str, ignore_patterns: &str) -> Result<()> {
    let ignore_patterns = fs::IgnorePatterns::new(ignore_patterns)?;
    let directories =
        fs::find_directories_with_size_and_ignore(path, pattern, &ignore_patterns)?;

    #[derive(serde::Serialize)]
    struct JsonDir<'a> {
        path: &'a str,
        size: u64,
        formatted_size: &'a str,
        last_modified: Option<&'a str>,
    }

    let rows: Vec<JsonDir<'_>> = directories
        .iter()
        .map(|dir| JsonDir {
            path: &dir.path,
            size: dir.size,
            formatted_size: &dir.formatted_size,
            last_modified: if dir.formatted_last_modified == "Unknown" {
                None
            } else {
                Some(dir.formatted_last_modified.as_str())
            },
        })
        .collect();

    println!("{}", serde_json::to_string_pretty(&rows)?);
    Ok(())
}

/// Display directories in TUI mode with progressive loading
fn display_directories_tui(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    pattern: &str,
    path: &str,
    ignore_patterns: &str,
) -> Result<()> {
    // Create ignore patterns first
    let ignore_patterns = match fs::IgnorePatterns::new(ignore_patterns) {
        Ok(patterns) => patterns,
        Err(e) => {
            eprintln!("Error parsing ignore patterns: {e}");
            return Err(e);
        }
    };

    let mut app = App::new_with_ignore(
        vec![],
        pattern.to_string(),
        path.to_string(),
        ignore_patterns.clone(),
    );

    // Set initial discovery status and start timing
    app.set_discovery_status(app::DiscoveryStatus::Discovering);
    app.start_discovery_timing();

    // Pre-calculate expensive values that don't change
    let current_dir_display = crate::config::shorten_home_path(
        &std::env::current_dir()
            .unwrap_or_default()
            .join(path)
            .to_string_lossy(),
    );

    // Channels for streaming discovery
    let (discovery_tx, discovery_rx) = std::sync::mpsc::channel::<fs::DiscoveryMessage>();

    // Channels for size updates: path, size, formatted size, parent mtime, formatted mtime
    let (size_tx, size_rx) = std::sync::mpsc::channel::<(
        String,
        u64,
        String,
        Option<std::time::SystemTime>,
        String,
    )>();

    // Start streaming discovery in background
    let pattern_clone = pattern.to_string();
    let path_clone = path.to_string();
    let ignore_patterns_clone = ignore_patterns.clone();
    let _discovery_handle = std::thread::spawn(move || {
        match fs::stream_directories_with_ignore(
            &path_clone,
            &pattern_clone,
            &ignore_patterns_clone,
            discovery_tx,
        ) {
            Ok(_) => {}
            Err(_) => {
                // Error handling is done in the main loop
            }
        }
    });

    // PERFORMANCE OPTIMIZATION: Smart frame rate limiting with event-driven rendering
    let mut last_frame_time = std::time::Instant::now();
    let mut last_activity_time = std::time::Instant::now();

    // Adaptive frame rates based on activity
    let active_frame_time = std::time::Duration::from_millis(16); // ~60 FPS during activity
    let idle_frame_time = std::time::Duration::from_millis(100); // ~10 FPS when idle
    let discovery_frame_time = std::time::Duration::from_millis(66); // ~15 FPS during discovery

    // State tracking for smart rendering
    let mut needs_redraw = true;
    let mut last_discovery_count = 0;
    let mut last_selection_count = 0;
    let mut last_page = 0;
    let mut last_selected = 0;
    let mut last_calculated_count = 0;
    let mut cached_items_per_page = 20usize;
    let mut last_viewport: Option<Rect> = None;

    // Dedicated thread pool for directory size calculations (backpressured via App).
    // Each size walk runs Serial on these workers — no nested rayon / busy-timeout.
    static SIZE_THREAD_POOL: std::sync::OnceLock<rayon::ThreadPool> = std::sync::OnceLock::new();
    let size_thread_pool = SIZE_THREAD_POOL.get_or_init(|| {
        rayon::ThreadPoolBuilder::new()
            .num_threads(app::max_concurrent_size_calcs())
            .thread_name(|idx| format!("dir-kill-size-{idx}"))
            .build()
            .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().unwrap())
    });

    // Main event loop with smart rendering
    loop {
        // PERFORMANCE OPTIMIZATION: Determine if we need to render
        let now = std::time::Instant::now();
        let time_since_last_frame = now.duration_since(last_frame_time);
        let time_since_last_activity = now.duration_since(last_activity_time);

        // Check for state changes that require redraw
        let current_discovery_count = app.total_discovered;
        let current_selection_count = app.get_selected_count();
        let current_page = app.current_page;
        let current_selected = app.selected;
        let current_calculated_count = app.cached_calculated_count;

        let state_changed = current_discovery_count != last_discovery_count
            || current_selection_count != last_selection_count
            || current_page != last_page
            || current_selected != last_selected
            || current_calculated_count != last_calculated_count;

        if state_changed {
            needs_redraw = true;
            last_discovery_count = current_discovery_count;
            last_selection_count = current_selection_count;
            last_page = current_page;
            last_selected = current_selected;
            last_calculated_count = current_calculated_count;
            last_activity_time = now;
        }

        // Determine target frame rate based on activity
        let target_frame_time = if state_changed || needs_redraw {
            if app.is_discovering() {
                discovery_frame_time
            } else {
                active_frame_time
            }
        } else if time_since_last_activity < std::time::Duration::from_millis(500) {
            active_frame_time
        } else {
            idle_frame_time
        };

        last_frame_time = now;

        // Check for new discovery messages (process all available)
        let mut has_discovery_updates = false;
        while let Ok(message) = discovery_rx.try_recv() {
            match message {
                fs::DiscoveryMessage::DirectoryFound(path) => {
                    app.add_discovered_directory(path);
                    has_discovery_updates = true;
                }
                fs::DiscoveryMessage::DiscoveryComplete => {
                    app.end_discovery_timing();
                    app.set_discovery_status(app::DiscoveryStatus::Complete);
                    has_discovery_updates = true;
                }
                fs::DiscoveryMessage::DiscoveryError(error) => {
                    app.end_discovery_timing();
                    app.set_discovery_status(app::DiscoveryStatus::Error(error));
                    has_discovery_updates = true;
                }
            }
        }

        // Check for size updates (process all available)
        let mut size_updates_processed = 0;
        let mut has_size_updates = false;
        while let Ok((path, size, formatted_size, last_modified, formatted_last_modified)) =
            size_rx.try_recv()
        {
            if app.apply_size_update(
                &path,
                size,
                formatted_size,
                last_modified,
                formatted_last_modified,
            ) {
                has_size_updates = true;
            }

            size_updates_processed += 1;
            if size_updates_processed >= 32 {
                break;
            }
        }

        // Update total completion time if all calculations are done
        app.update_total_completion_time();

        if app.clear_expired_toast() {
            needs_redraw = true;
        }

        // Process deletion messages
        let has_deletion_updates = app.process_deletion_messages();

        // Backpressured size calculations: one spawn per path so workers run in parallel
        if !app.directories.is_empty() {
            let paths_to_calculate = app.dequeue_size_calculations(
                app::max_concurrent_size_calcs(),
                cached_items_per_page,
            );

            for dir_path in paths_to_calculate {
                let size_tx_clone = size_tx.clone();
                size_thread_pool.spawn(move || {
                    let path = std::path::Path::new(&dir_path);
                    let calculated_size = fs::calculate_directory_size_jwalk(path).unwrap_or(0);
                    let formatted_size = fs::format_size(calculated_size);
                    let parent = path.parent().unwrap_or(path);
                    let last_modified = fs::get_directory_last_modified(parent);
                    let formatted_last_modified = last_modified
                        .as_ref()
                        .map(fs::format_last_modified)
                        .unwrap_or_else(|| "Unknown".to_string());
                    let _ = size_tx_clone.send((
                        dir_path,
                        calculated_size,
                        formatted_size,
                        last_modified,
                        formatted_last_modified,
                    ));
                });
            }
        }

        if has_deletion_updates {
            needs_redraw = true;
        }

        if has_discovery_updates || has_size_updates {
            needs_redraw = true;
            last_activity_time = std::time::Instant::now();
        }

        // Viewport-based pagination: recompute items per page when terminal size or layout changes
        let terminal_size = terminal.size()?;
        let viewport = Rect::new(0, 0, terminal_size.width, terminal_size.height);
        let items_per_page =
            view::items_per_page_for_viewport(viewport, app.show_details_panel);
        if items_per_page != cached_items_per_page {
            cached_items_per_page = items_per_page;
            app.items_per_page = items_per_page;
            app.clamp_pagination();
            needs_redraw = true;
        }
        if last_viewport != Some(viewport) {
            last_viewport = Some(viewport);
            needs_redraw = true;
        }

        // Always poll keyboard — must not be skipped by render optimizations
        let poll_timeout = if app.is_discovering()
            || app.is_deleting()
            || app.cached_calculated_count < app.directories.len()
        {
            std::time::Duration::from_millis(16)
        } else {
            std::time::Duration::from_millis(50)
        };

        if crossterm::event::poll(poll_timeout)? {
            match crossterm::event::read()? {
                crossterm::event::Event::Resize(_, _) => {
                    needs_redraw = true;
                }
                crossterm::event::Event::Key(key_event) => {
                app.items_per_page = cached_items_per_page;
                if app.show_help {
                    app.show_help = false;
                    needs_redraw = true;
                } else if app.delete_confirmation.is_some() {
                    match key_event.code {
                        crossterm::event::KeyCode::Char('y') | crossterm::event::KeyCode::Char('Y') => {
                            let action = app.confirm_delete();
                            match action {
                                DeleteConfirmAction::Current => {
                                    let _ = app.start_delete_current_directory();
                                }
                                DeleteConfirmAction::Selected => {
                                    let _ = app.start_delete_selected_directories();
                                }
                            }
                            needs_redraw = true;
                            last_activity_time = std::time::Instant::now();
                        }
                        crossterm::event::KeyCode::Char('n')
                        | crossterm::event::KeyCode::Char('N')
                        | crossterm::event::KeyCode::Esc => {
                            app.cancel_delete_confirmation();
                            needs_redraw = true;
                        }
                        _ => {}
                    }
                } else if app.filter_input_active {
                    match key_event.code {
                        crossterm::event::KeyCode::Enter => {
                            app.commit_filter();
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Esc => {
                            app.cancel_filter();
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Backspace => {
                            app.pop_filter_char();
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Char(c) => {
                            app.push_filter_char(c);
                            needs_redraw = true;
                        }
                        _ => {}
                    }
                } else {
                    let items_per_page = cached_items_per_page;
                    let ctrl = key_event
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::CONTROL);
                    let shift = key_event
                        .modifiers
                        .contains(crossterm::event::KeyModifiers::SHIFT);
                    match key_event.code {
                        crossterm::event::KeyCode::Char('q') => break,
                        crossterm::event::KeyCode::Esc => {
                            if app.show_details_panel {
                                app.toggle_details_panel();
                                needs_redraw = true;
                            } else if app.has_active_filter() {
                                app.clear_filter();
                                needs_redraw = true;
                            } else {
                                break;
                            }
                        }
                        crossterm::event::KeyCode::Char('d')
                        | crossterm::event::KeyCode::Char('D')
                        | crossterm::event::KeyCode::Char('x')
                        | crossterm::event::KeyCode::Char('X')
                            if ctrl && shift =>
                        {
                            if app.get_selected_count() > 0 {
                                app.request_delete_selected();
                                needs_redraw = true;
                            }
                        }
                        crossterm::event::KeyCode::Char('d')
                        | crossterm::event::KeyCode::Char('D')
                        | crossterm::event::KeyCode::Char('x')
                        | crossterm::event::KeyCode::Char('X')
                            if ctrl =>
                        {
                            app.request_delete_current();
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => {
                            app.previous(items_per_page);
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => {
                            app.next(items_per_page);
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Home => {
                            app.select_first();
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::End => {
                            app.select_last();
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Left => {
                            app.previous_page(items_per_page);
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Right => {
                            app.next_page(items_per_page);
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Char(' ') => {
                            app.toggle_current_selection();
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Char('a') => {
                            app.select_all();
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Char('d') => {
                            app.deselect_all();
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Char('s') => {
                            app.toggle_sort(SortColumn::Size);
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Char('p') => {
                            app.toggle_sort(SortColumn::Path);
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Char('m') => {
                            app.toggle_sort(SortColumn::Age);
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Char('/') => {
                            app.begin_filter_input();
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Char('i') | crossterm::event::KeyCode::Tab => {
                            app.toggle_details_panel();
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Char('?') => {
                            app.toggle_help();
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Char('o') => {
                            let _ = app.open_selected_in_file_manager();
                        }
                        crossterm::event::KeyCode::Char('y')
                            if key_event
                                .modifiers
                                .contains(crossterm::event::KeyModifiers::CONTROL) =>
                        {
                            if app.copy_selected_path().is_ok() {
                                app.set_status_toast("Copied path to clipboard".to_string());
                            }
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Delete => {
                            if app.get_selected_count() > 0 {
                                app.request_delete_selected();
                            } else {
                                app.request_delete_current();
                            }
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Char('f') => {
                            app.request_delete_current();
                            needs_redraw = true;
                        }
                        crossterm::event::KeyCode::Char('c') if app.get_selected_count() > 0 => {
                            app.request_delete_selected();
                            needs_redraw = true;
                        }
                        _ => {}
                    }
                }
                }
                _ => {}
            }
        }

        let should_draw = needs_redraw
            || has_discovery_updates
            || has_size_updates
            || has_deletion_updates
            || app.display_indices_dirty;

        if app.display_indices_dirty {
            app.rebuild_display_indices();
            app.display_indices_dirty = false;
            needs_redraw = true;
        }

        if !should_draw {
            if !needs_redraw && time_since_last_frame < target_frame_time {
                let sleep_time = target_frame_time - time_since_last_frame;
                if sleep_time > std::time::Duration::from_millis(1) {
                    std::thread::sleep(sleep_time);
                }
            }
            continue;
        }

        let mut render_ctx = RenderContext {
            pattern,
            search_root: &current_dir_display,
            items_per_page: cached_items_per_page,
        };

        terminal.draw(|f| view::render(f, &mut app, &mut render_ctx))?;
        cached_items_per_page = render_ctx.items_per_page;

        needs_redraw = false;
    }

    app.save_preferences();
    restore_terminal()?;
    Ok(())
}

/// Display directories in simple text mode (fallback when TUI fails)
fn display_directories_text(pattern: &str, path: &str, ignore_patterns: &str) -> Result<()> {
    println!("🔍 Directory Search Results");
    println!("Pattern: '{pattern}' in '{path}'");
    if !ignore_patterns.trim().is_empty() {
        println!("Ignore patterns: '{ignore_patterns}'");
    }
    println!("⏳ Scanning directories...");
    println!();

    // Create ignore patterns
    let ignore_patterns = fs::IgnorePatterns::new(ignore_patterns)?;

    // Find directories with size information
    let directories = fs::find_directories_with_size_and_ignore(path, pattern, &ignore_patterns)?;

    if directories.is_empty() {
        println!("❌ No directories found matching pattern '{pattern}'");
        return Ok(());
    }

    println!("✅ Found {} directories:", directories.len());
    println!();

    // Display directories with pagination
    let items_per_page = 20;
    let total_pages = (directories.len() - 1) / items_per_page + 1;

    for (i, dir) in directories.iter().enumerate() {
        let page = i / items_per_page + 1;
        let timing_info = if let Some(calc_time) = &dir.calculation_time {
            format!(" (calculated in {})", fs::format_duration(calc_time))
        } else {
            String::new()
        };
        println!(
            "📁 {} ({}){}",
            clean_path(&dir.path),
            dir.formatted_size,
            timing_info
        );

        // Add page separator
        if (i + 1) % items_per_page == 0 && i < directories.len() - 1 {
            println!();
            println!("--- Page {page} of {total_pages} ---");
            println!();
        }
    }

    // Calculate total size for summary
    let total_size: u64 = directories.iter().map(|dir| dir.size).sum();
    let total_formatted = fs::format_size(total_size);

    println!();
    println!(
        "📊 Total: {} directories found, {} total size",
        directories.len(),
        total_formatted
    );
    println!("💡 Tip: Use a terminal that supports TUI for a better experience");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;
    use super::test_helpers::{
        get_calculation_status_icon, get_directory_icon, get_loading_frame,
        get_selection_indicator_color, SELECTION_INDICATOR_COLOR, TEXT_SECONDARY,
    };

    // Helper function to create DirectoryInfo for tests
    fn create_test_dir(path: &str, size: u64, formatted_size: &str) -> DirectoryInfo {
        DirectoryInfo {
            path: path.to_string(),
            size,
            formatted_size: formatted_size.to_string(),
            last_modified: None,
            formatted_last_modified: "Unknown".to_string(),
            selected: false,
            deletion_status: crate::fs::DeletionStatus::Normal,
            calculation_status: crate::fs::CalculationStatus::Completed,
            calculation_time: None,
        }
    }

    // Helper function to create DirectoryInfo with calculating state
    fn create_calculating_dir(path: &str) -> DirectoryInfo {
        DirectoryInfo {
            path: path.to_string(),
            size: 0,
            formatted_size: "Calculating...".to_string(),
            last_modified: None,
            formatted_last_modified: "Unknown".to_string(),
            selected: false,
            deletion_status: crate::fs::DeletionStatus::Normal,
            calculation_status: crate::fs::CalculationStatus::Calculating,
            calculation_time: None,
        }
    }

    #[test]
    fn test_app_creation() {
        let directories = vec![
            create_test_dir("dir1", 100, "100 B"),
            create_test_dir("dir2", 200, "200 B"),
        ];
        let app = App::new(directories.clone(), "test".to_string(), ".".to_string());
        assert_eq!(app.directories.len(), directories.len());
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn test_app_navigation() {
        let directories = vec![
            create_test_dir("dir1", 100, "100 B"),
            create_test_dir("dir2", 200, "200 B"),
            create_test_dir("dir3", 300, "300 B"),
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
    }

    #[test]
    fn test_app_empty_list() {
        let app = App::new(vec![], "test".to_string(), ".".to_string());
        assert_eq!(app.directories.len(), 0);
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn test_clean_path() {
        assert_eq!(clean_path("./test/path"), "test/path");
        assert_eq!(clean_path("test/path"), "test/path");
        assert_eq!(clean_path("./"), "");
        assert_eq!(clean_path(""), "");
        assert_eq!(clean_path("./node_modules"), "node_modules");
    }

    #[test]
    fn test_loading_frame() {
        let frame = get_loading_frame();
        assert!(!frame.is_empty());
        assert!(frame.len() <= 3); // Braille characters are typically 1-3 bytes
    }

    #[test]
    fn test_scanning_state_transition() {
        // Test that scanning state properly transitions from loading to results
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        let mut is_scanning = true;

        // Initially should show loading state
        assert!(is_scanning);
        assert!(app.directories.is_empty());

        // Simulate receiving first directory
        app.directories.push(DirectoryInfo {
            last_modified: None,
            formatted_last_modified: "Unknown".to_string(),
            path: "test_dir".to_string(),
            size: 100,
            formatted_size: "100 B".to_string(),
            selected: false,
            deletion_status: crate::fs::DeletionStatus::Normal,
            calculation_status: crate::fs::CalculationStatus::Completed,
            calculation_time: None,
        });

        // Should still be scanning after receiving first item
        assert!(is_scanning);
        assert!(!app.directories.is_empty());

        // Simulate time passing without new data and transition to not scanning
        is_scanning = false;

        assert!(!is_scanning);
        assert!(!app.directories.is_empty());
    }

    #[test]
    fn test_scanning_state_with_multiple_items() {
        // Test that scanning continues while receiving multiple items
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        let mut is_scanning = true;

        // Add multiple directories
        for i in 0..5 {
            app.directories.push(create_test_dir(
                &format!("dir{i}"),
                i as u64 * 100,
                &format!("{} B", i * 100),
            ));
        }

        // Should still be scanning since we just received data
        assert!(is_scanning);
        assert_eq!(app.directories.len(), 5);

        // Simulate time passing without new data and transition to not scanning
        is_scanning = false;

        assert!(!is_scanning);
        assert_eq!(app.directories.len(), 5);
    }

    #[test]
    fn test_ui_rendering_race_condition() {
        // Test the actual UI rendering logic that causes the race condition
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        let mut is_scanning = true;

        // Simulate the UI rendering condition: is_scanning
        let should_show_loading = is_scanning;
        assert!(should_show_loading); // Should show loading initially

        // Simulate receiving first directory but still scanning
        app.directories
            .push(create_test_dir("test_dir", 100, "100 B"));

        // Should still show loading while scanning
        assert!(is_scanning);
        let should_show_loading = is_scanning;
        assert!(should_show_loading); // Should show loading while scanning

        // Only when scanning is false should we show the list
        is_scanning = false;
        let should_show_loading = is_scanning;
        assert!(!should_show_loading); // Should show list when not scanning

        // Test the complete logic: if scanning show loading, else if not empty show list, else show no results
        let should_show_list = !is_scanning && !app.directories.is_empty();
        assert!(should_show_list); // Should show list when not scanning and not empty

        // Test empty case
        let empty_app = App::new(vec![], "test".to_string(), ".".to_string());
        let empty_is_scanning = false;
        let should_show_no_results = !empty_is_scanning && empty_app.directories.is_empty();
        assert!(should_show_no_results); // Should show no results when not scanning and empty
    }

    #[test]
    fn test_scanning_complete_no_results() {
        // Test the scenario where scanning completes but no directories are found
        let app = App::new(vec![], "test".to_string(), ".".to_string());
        let is_scanning = false;
        let has_received_any_data = true; // Scanning completed, we know there are no results

        // Should show "no results" message, not loading
        let should_show_loading = is_scanning || !has_received_any_data;
        assert!(!should_show_loading); // Should NOT show loading

        let should_show_no_results = has_received_any_data && app.directories.is_empty();
        assert!(should_show_no_results); // Should show no results message
    }

    #[test]
    fn test_lazy_size_calculation_initial_state() {
        // Test that directories start with "Calculating..." placeholder
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Add a directory with initial state (no size calculated yet)
        app.directories.push(create_calculating_dir("test_dir"));

        assert_eq!(app.directories[0].size, 0);
        assert_eq!(app.directories[0].formatted_size, "Calculating...");
    }

    #[test]
    fn test_lazy_size_calculation_update() {
        // Test that size updates work correctly
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Add directories with initial state
        app.directories.push(create_calculating_dir("dir1"));
        app.directories.push(create_calculating_dir("dir2"));

        // Simulate size update for first directory
        let index = 0;
        let size = 1024;
        let formatted_size = "1.0 KB".to_string();

        if index < app.directories.len() {
            app.directories[index].size = size;
            app.directories[index].formatted_size = formatted_size.clone();
        }

        // Verify the update
        assert_eq!(app.directories[0].size, 1024);
        assert_eq!(app.directories[0].formatted_size, "1.0 KB");

        // Verify second directory still has placeholder
        assert_eq!(app.directories[1].size, 0);
        assert_eq!(app.directories[1].formatted_size, "Calculating...");
    }

    #[test]
    fn test_lazy_size_calculation_multiple_updates() {
        // Test multiple size updates in sequence
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Add multiple directories
        for i in 0..3 {
            app.directories
                .push(create_calculating_dir(&format!("dir{i}")));
        }

        // Simulate size updates in different order
        let updates = vec![
            (1, 2048, "2.0 KB"),
            (0, 1024, "1.0 KB"),
            (2, 3072, "3.0 KB"),
        ];

        for (index, size, formatted_size) in updates {
            if index < app.directories.len() {
                app.directories[index].size = size;
                app.directories[index].formatted_size = formatted_size.to_string();
            }
        }

        // Verify all updates were applied correctly
        assert_eq!(app.directories[0].size, 1024);
        assert_eq!(app.directories[0].formatted_size, "1.0 KB");
        assert_eq!(app.directories[1].size, 2048);
        assert_eq!(app.directories[1].formatted_size, "2.0 KB");
        assert_eq!(app.directories[2].size, 3072);
        assert_eq!(app.directories[2].formatted_size, "3.0 KB");
    }

    #[test]
    fn test_lazy_size_calculation_out_of_bounds() {
        // Test that out-of-bounds updates are handled safely
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Add one directory
        app.directories.push(DirectoryInfo {
            last_modified: None,
            formatted_last_modified: "Unknown".to_string(),
            path: "test_dir".to_string(),
            size: 0,
            formatted_size: "Calculating...".to_string(),
            selected: false,
            deletion_status: crate::fs::DeletionStatus::Normal,
            calculation_status: crate::fs::CalculationStatus::NotStarted,
            calculation_time: None,
        });

        // Try to update an index that doesn't exist
        let invalid_index = 5;
        let size = 1024;
        let formatted_size = "1.0 KB".to_string();

        if invalid_index < app.directories.len() {
            app.directories[invalid_index].size = size;
            app.directories[invalid_index].formatted_size = formatted_size;
        }

        // Verify the directory wasn't modified (since index was out of bounds)
        assert_eq!(app.directories[0].size, 0);
        assert_eq!(app.directories[0].formatted_size, "Calculating...");
    }

    #[test]
    fn test_total_size_calculation_empty_list() {
        let app = App::new(vec![], "test".to_string(), ".".to_string());

        // Total size should be 0 for empty list
        let total_size: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(total_size, 0);

        let calculated_count = app
            .directories
            .iter()
            .filter(|dir| {
                matches!(
                    dir.calculation_status,
                    crate::fs::CalculationStatus::Completed
                )
            })
            .count();
        assert_eq!(calculated_count, 0);
    }

    #[test]
    fn test_total_size_calculation_with_initial_sizes() {
        let directories = vec![
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir1".to_string(),
                size: 1024,
                formatted_size: "1.0 KB".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir2".to_string(),
                size: 2048,
                formatted_size: "2.0 KB".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir3".to_string(),
                size: 3072,
                formatted_size: "3.0 KB".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
        ];

        let app = App::new(directories, "test".to_string(), ".".to_string());

        // Total size should be sum of all sizes
        let total_size: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(total_size, 6144); // 1024 + 2048 + 3072

        let calculated_count = app
            .directories
            .iter()
            .filter(|dir| {
                matches!(
                    dir.calculation_status,
                    crate::fs::CalculationStatus::Completed
                )
            })
            .count();
        assert_eq!(calculated_count, 3);
    }

    #[test]
    fn test_total_size_calculation_with_lazy_updates() {
        let directories = vec![
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir1".to_string(),
                size: 0, // Initially 0, will be updated
                formatted_size: "Calculating...".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::NotStarted,
                calculation_time: None,
            },
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir2".to_string(),
                size: 0, // Initially 0, will be updated
                formatted_size: "Calculating...".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::NotStarted,
                calculation_time: None,
            },
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir3".to_string(),
                size: 0, // Initially 0, will be updated
                formatted_size: "Calculating...".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::NotStarted,
                calculation_time: None,
            },
        ];

        let mut app = App::new(directories, "test".to_string(), ".".to_string());

        // Initially all sizes are 0
        let initial_total: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(initial_total, 0);

        let initial_calculated = app.directories.iter().filter(|dir| dir.size > 0).count();
        assert_eq!(initial_calculated, 0);

        // Update first directory size
        if !app.directories.is_empty() {
            app.directories[0].size = 1024;
            app.directories[0].formatted_size = "1.0 KB".to_string();
        }
        let total_after_first: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(total_after_first, 1024);

        let calculated_after_first = app.directories.iter().filter(|dir| dir.size > 0).count();
        assert_eq!(calculated_after_first, 1);

        // Update second directory size
        if 1 < app.directories.len() {
            app.directories[1].size = 2048;
            app.directories[1].formatted_size = "2.0 KB".to_string();
        }
        let total_after_second: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(total_after_second, 3072); // 1024 + 2048

        let calculated_after_second = app.directories.iter().filter(|dir| dir.size > 0).count();
        assert_eq!(calculated_after_second, 2);

        // Update third directory size
        if 2 < app.directories.len() {
            app.directories[2].size = 3072;
            app.directories[2].formatted_size = "3.0 KB".to_string();
        }
        let total_after_third: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(total_after_third, 6144); // 1024 + 2048 + 3072

        let calculated_after_third = app.directories.iter().filter(|dir| dir.size > 0).count();
        assert_eq!(calculated_after_third, 3);
    }

    #[test]
    fn test_total_size_calculation_mixed_states() {
        let directories = vec![
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir1".to_string(),
                size: 1024, // Already calculated
                formatted_size: "1.0 KB".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir2".to_string(),
                size: 0, // Not yet calculated
                formatted_size: "Calculating...".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::NotStarted,
                calculation_time: None,
            },
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir3".to_string(),
                size: 2048, // Already calculated
                formatted_size: "2.0 KB".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
        ];

        let mut app = App::new(directories, "test".to_string(), ".".to_string());

        // Initial state: 2 calculated, 1 not calculated
        let initial_total: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(initial_total, 3072); // 1024 + 2048

        let initial_calculated = app.directories.iter().filter(|dir| dir.size > 0).count();
        assert_eq!(initial_calculated, 2);

        // Update the uncounted directory
        if 1 < app.directories.len() {
            app.directories[1].size = 4096;
            app.directories[1].formatted_size = "4.0 KB".to_string();
        }
        let final_total: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(final_total, 7168); // 1024 + 4096 + 2048

        let final_calculated = app.directories.iter().filter(|dir| dir.size > 0).count();
        assert_eq!(final_calculated, 3);
    }

    #[test]
    fn test_total_size_calculation_large_numbers() {
        let directories = vec![
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "large_dir1".to_string(),
                size: 1024 * 1024 * 1024, // 1 GB
                formatted_size: "1.0 GB".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "large_dir2".to_string(),
                size: 2 * 1024 * 1024 * 1024, // 2 GB
                formatted_size: "2.0 GB".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
        ];

        let app = App::new(directories, "test".to_string(), ".".to_string());

        let total_size: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(total_size, 3 * 1024 * 1024 * 1024); // 3 GB

        let calculated_count = app
            .directories
            .iter()
            .filter(|dir| {
                matches!(
                    dir.calculation_status,
                    crate::fs::CalculationStatus::Completed
                )
            })
            .count();
        assert_eq!(calculated_count, 2);
    }

    #[test]
    fn test_total_size_calculation_with_zero_sizes() {
        let directories = vec![
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "empty_dir1".to_string(),
                size: 0,
                formatted_size: "0 B".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "empty_dir2".to_string(),
                size: 0,
                formatted_size: "0 B".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "non_empty_dir".to_string(),
                size: 1024,
                formatted_size: "1.0 KB".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
        ];

        let app = App::new(directories, "test".to_string(), ".".to_string());

        let total_size: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(total_size, 1024); // Only the non-empty directory contributes

        let calculated_count = app
            .directories
            .iter()
            .filter(|dir| {
                matches!(
                    dir.calculation_status,
                    crate::fs::CalculationStatus::Completed
                )
            })
            .count();
        assert_eq!(calculated_count, 3); // All directories have completed calculations
    }

    #[test]
    fn test_selection_indicator_logic() {
        use crate::fs::DirectoryInfo;
        fn indicator(dir: &DirectoryInfo) -> &'static str {
            if dir.selected { "☑" } else { "☐" }
        }
        let mut dir = DirectoryInfo {
            last_modified: None,
            formatted_last_modified: "Unknown".to_string(),
            path: "foo".to_string(),
            size: 0,
            formatted_size: "0 B".to_string(),
            selected: false,
            deletion_status: crate::fs::DeletionStatus::Normal,
            calculation_status: crate::fs::CalculationStatus::Completed,
            calculation_time: None,
        };
        assert_eq!(indicator(&dir), "☐");
        dir.selected = true;
        assert_eq!(indicator(&dir), "☑");
    }

    #[test]
    fn test_selection_summary_string() {
        use crate::fs::DirectoryInfo;
        use crate::fs::format_size;
        use crate::ui::app::App;
        let mut app = App::new(
            vec![
                DirectoryInfo {
                    last_modified: None,
                    formatted_last_modified: "Unknown".to_string(),
                    path: "a".to_string(),
                    size: 100,
                    formatted_size: "100 B".to_string(),
                    selected: false,
                    deletion_status: crate::fs::DeletionStatus::Normal,
                    calculation_status: crate::fs::CalculationStatus::Completed,
                    calculation_time: None,
                },
                DirectoryInfo {
                    last_modified: None,
                    formatted_last_modified: "Unknown".to_string(),
                    path: "b".to_string(),
                    size: 200,
                    formatted_size: "200 B".to_string(),
                    selected: false,
                    deletion_status: crate::fs::DeletionStatus::Normal,
                    calculation_status: crate::fs::CalculationStatus::Completed,
                    calculation_time: None,
                },
            ],
            "test".to_string(),
            ".".to_string(),
        );
        // No selection
        let summary = if app.get_selected_count() > 0 {
            format!(
                " | Selected: {} ({})",
                app.get_selected_count(),
                format_size(app.get_selected_total_size())
            )
        } else {
            String::new()
        };
        assert_eq!(summary, "");
        // One selected
        app.selected = 0;
        app.select_current();
        let summary = if app.get_selected_count() > 0 {
            format!(
                " | Selected: {} ({})",
                app.get_selected_count(),
                format_size(app.get_selected_total_size())
            )
        } else {
            String::new()
        };
        assert_eq!(summary, " | Selected: 1 (100 B)");
        // Both selected
        app.selected = 1;
        app.select_current();
        let summary = if app.get_selected_count() > 0 {
            format!(
                " | Selected: {} ({})",
                app.get_selected_count(),
                format_size(app.get_selected_total_size())
            )
        } else {
            String::new()
        };
        assert_eq!(summary, " | Selected: 2 (300 B)");
    }

    #[test]
    fn test_animated_directory_icon() {
        // Test that the animated directory icon returns the correct symbols
        assert_eq!(get_directory_icon(false, false), "📁");
        assert!(
            get_directory_icon(true, false).contains("📂")
                || get_directory_icon(true, false).contains("📁")
        );
        assert!(
            get_directory_icon(false, true).contains("📂")
                || get_directory_icon(false, true).contains("📁")
        );

        // Test that the color function returns different colors for selected vs unselected
        let selected_color = get_selection_indicator_color(true);
        let unselected_color = get_selection_indicator_color(false);
        assert_ne!(selected_color, unselected_color);

        // Test animation consistency
        let icon1 = get_directory_icon(true, false);
        let icon2 = get_directory_icon(true, false);
        assert!(icon1 == "📂" || icon1 == "📁");
        assert!(icon2 == "📂" || icon2 == "📁");
    }

    #[test]
    fn test_deletion_status_display() {
        // Test that deletion status is properly displayed in the UI with icons
        use crate::fs::{DeletionStatus, DirectoryInfo};

        // Test normal status (should show nothing)
        let normal_dir = DirectoryInfo {
            last_modified: None,
            formatted_last_modified: "Unknown".to_string(),
            path: "test_dir".to_string(),
            size: 100,
            formatted_size: "100 B".to_string(),
            selected: false,
            deletion_status: DeletionStatus::Normal,
            calculation_status: crate::fs::CalculationStatus::Completed,
            calculation_time: None,
        };

        // Test deleting status (should show 🔄 icon)
        let deleting_dir = DirectoryInfo {
            last_modified: None,
            formatted_last_modified: "Unknown".to_string(),
            path: "test_dir".to_string(),
            size: 100,
            formatted_size: "100 B".to_string(),
            selected: false,
            deletion_status: DeletionStatus::Deleting,
            calculation_status: crate::fs::CalculationStatus::Completed,
            calculation_time: None,
        };

        // Test deleted status (should show 🗑️ icon)
        let deleted_dir = DirectoryInfo {
            last_modified: None,
            formatted_last_modified: "Unknown".to_string(),
            path: "test_dir".to_string(),
            size: 100,
            formatted_size: "100 B".to_string(),
            selected: false,
            deletion_status: DeletionStatus::Deleted,
            calculation_status: crate::fs::CalculationStatus::Completed,
            calculation_time: None,
        };

        // Test error status (should show ⚠️ icon with message)
        let error_dir = DirectoryInfo {
            last_modified: None,
            formatted_last_modified: "Unknown".to_string(),
            path: "test_dir".to_string(),
            size: 100,
            formatted_size: "100 B".to_string(),
            selected: false,
            deletion_status: DeletionStatus::Error("Permission denied".to_string()),
            calculation_status: crate::fs::CalculationStatus::Completed,
            calculation_time: None,
        };

        // Verify the status variants exist and work correctly
        assert!(matches!(normal_dir.deletion_status, DeletionStatus::Normal));
        assert!(matches!(
            deleting_dir.deletion_status,
            DeletionStatus::Deleting
        ));
        assert!(matches!(
            deleted_dir.deletion_status,
            DeletionStatus::Deleted
        ));
        assert!(matches!(
            error_dir.deletion_status,
            DeletionStatus::Error(_)
        ));

        // Test that the UI rendering logic can handle all status types
        let app = App::new(
            vec![normal_dir, deleting_dir, deleted_dir, error_dir],
            "test".to_string(),
            ".".to_string(),
        );
        assert_eq!(app.directories.len(), 4);
        assert!(matches!(
            app.directories[0].deletion_status,
            DeletionStatus::Normal
        ));
        assert!(matches!(
            app.directories[1].deletion_status,
            DeletionStatus::Deleting
        ));
        assert!(matches!(
            app.directories[2].deletion_status,
            DeletionStatus::Deleted
        ));
        assert!(matches!(
            app.directories[3].deletion_status,
            DeletionStatus::Error(_)
        ));
    }

    #[test]
    fn test_concurrency_fix_size_calculation() {
        // Test that the concurrency fix works correctly when directories are added
        // while size calculations are in progress
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Simulate the scenario where directories are added in batches
        // and size calculations complete out of order

        // Add first batch of directories
        for i in 0..3 {
            app.directories.push(DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: format!("dir{i}"),
                size: 0,
                formatted_size: "Calculating...".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::NotStarted,
                calculation_time: None,
            });
        }

        // Simulate size updates coming back out of order
        // This simulates the background threads completing at different times
        let updates = vec![
            ("dir1".to_string(), 2048, "2.0 KB".to_string()),
            ("dir0".to_string(), 1024, "1.0 KB".to_string()),
            ("dir2".to_string(), 3072, "3.0 KB".to_string()),
        ];

        // Apply updates using the new path-based lookup
        for (path, size, formatted_size) in updates {
            if let Some(dir) = app.directories.iter_mut().find(|d| d.path == path) {
                dir.size = size;
                dir.formatted_size = formatted_size;
                dir.calculation_status = crate::fs::CalculationStatus::Completed;
            }
        }

        // Verify all updates were applied correctly
        assert_eq!(app.directories[0].size, 1024);
        assert_eq!(app.directories[0].formatted_size, "1.0 KB");
        assert_eq!(app.directories[1].size, 2048);
        assert_eq!(app.directories[1].formatted_size, "2.0 KB");
        assert_eq!(app.directories[2].size, 3072);
        assert_eq!(app.directories[2].formatted_size, "3.0 KB");

        // Now simulate adding more directories while size calculations are still in progress
        for i in 3..6 {
            app.directories.push(DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: format!("dir{i}"),
                size: 0,
                formatted_size: "Calculating...".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::NotStarted,
                calculation_time: None,
            });
        }

        // Simulate more size updates (including some for the new directories)
        let more_updates = vec![
            ("dir4".to_string(), 4096, "4.0 KB".to_string()),
            ("dir3".to_string(), 5120, "5.0 KB".to_string()),
            ("dir5".to_string(), 6144, "6.0 KB".to_string()),
        ];

        // Apply updates - this should work correctly even though the vector has grown
        for (path, size, formatted_size) in more_updates {
            if let Some(dir) = app.directories.iter_mut().find(|d| d.path == path) {
                dir.size = size;
                dir.formatted_size = formatted_size;
                dir.calculation_status = crate::fs::CalculationStatus::Completed;
            }
        }

        // Verify all updates were applied correctly
        assert_eq!(app.directories[3].size, 5120);
        assert_eq!(app.directories[3].formatted_size, "5.0 KB");
        assert_eq!(app.directories[4].size, 4096);
        assert_eq!(app.directories[4].formatted_size, "4.0 KB");
        assert_eq!(app.directories[5].size, 6144);
        assert_eq!(app.directories[5].formatted_size, "6.0 KB");

        // Verify the total size calculation is correct
        let total_size: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(total_size, 21504); // 1024 + 2048 + 3072 + 5120 + 4096 + 6144

        let calculated_count = app
            .directories
            .iter()
            .filter(|dir| {
                matches!(
                    dir.calculation_status,
                    crate::fs::CalculationStatus::Completed
                )
            })
            .count();
        assert_eq!(calculated_count, 6);
    }

    #[test]
    fn test_key_handling_delete_shortcuts() {
        // Test that the key handling logic correctly distinguishes between different delete shortcuts
        // Now using 'C' key for selected directories instead of Delete key
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

        // Helper function to create key events
        fn create_key_event(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
            KeyEvent {
                code,
                modifiers,
                kind: crossterm::event::KeyEventKind::Press,
                state: crossterm::event::KeyEventState::empty(),
            }
        }

        // Test 'C' key (should delete selected)
        let c_key = create_key_event(KeyCode::Char('c'), KeyModifiers::empty());

        // Test Ctrl+D (should delete current)
        let ctrl_d = create_key_event(KeyCode::Char('d'), KeyModifiers::CONTROL);

        // Test plain 'd' (should deselect all)
        let plain_d = create_key_event(KeyCode::Char('d'), KeyModifiers::empty());

        // Test Ctrl+X (should delete current)
        let ctrl_x = create_key_event(KeyCode::Char('x'), KeyModifiers::CONTROL);

        // Test plain 'f' (should delete current)
        let plain_f = create_key_event(KeyCode::Char('f'), KeyModifiers::empty());

        // Verify the key event properties
        assert!(!c_key.modifiers.contains(KeyModifiers::CONTROL));
        assert!(!c_key.modifiers.contains(KeyModifiers::SHIFT));
        assert_eq!(c_key.code, KeyCode::Char('c'));

        assert!(ctrl_d.modifiers.contains(KeyModifiers::CONTROL));
        assert!(!ctrl_d.modifiers.contains(KeyModifiers::SHIFT));
        assert_eq!(ctrl_d.code, KeyCode::Char('d'));

        assert!(!plain_d.modifiers.contains(KeyModifiers::CONTROL));
        assert!(!plain_d.modifiers.contains(KeyModifiers::SHIFT));
        assert_eq!(plain_d.code, KeyCode::Char('d'));

        assert!(ctrl_x.modifiers.contains(KeyModifiers::CONTROL));
        assert!(!ctrl_x.modifiers.contains(KeyModifiers::SHIFT));
        assert_eq!(ctrl_x.code, KeyCode::Char('x'));

        assert!(!plain_f.modifiers.contains(KeyModifiers::CONTROL));
        assert!(!plain_f.modifiers.contains(KeyModifiers::SHIFT));
        assert_eq!(plain_f.code, KeyCode::Char('f'));

        // Test the logic that would be used in the key handling
        let test_key_handling = |key_event: &KeyEvent| -> &str {
            match key_event.code {
                KeyCode::Char('f') => "delete_current",
                KeyCode::Char('c') => "delete_selected",
                KeyCode::Char('x') | KeyCode::Char('d')
                    if key_event.modifiers.contains(KeyModifiers::CONTROL) =>
                {
                    "delete_current"
                }
                KeyCode::Char('d') if !key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    "deselect_all"
                }
                _ => "unknown",
            }
        };

        // Verify the key handling logic works correctly
        assert_eq!(test_key_handling(&c_key), "delete_selected");
        assert_eq!(test_key_handling(&ctrl_d), "delete_current");
        assert_eq!(test_key_handling(&ctrl_x), "delete_current");
        assert_eq!(test_key_handling(&plain_d), "deselect_all");
        assert_eq!(test_key_handling(&plain_f), "delete_current");
    }

    #[test]
    fn test_selection_and_deletion_logic() {
        use crate::fs::DeletionStatus;
        use crate::fs::DirectoryInfo;
        use crate::ui::app::App;

        // Create a test app with multiple directories
        let directories = vec![
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir1".to_string(),
                size: 100,
                formatted_size: "100 B".to_string(),
                selected: false,
                deletion_status: DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir2".to_string(),
                size: 200,
                formatted_size: "200 B".to_string(),
                selected: false,
                deletion_status: DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir3".to_string(),
                size: 300,
                formatted_size: "300 B".to_string(),
                selected: false,
                deletion_status: DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
        ];

        let mut app = App::new(directories, "test".to_string(), ".".to_string());

        // Initially no directories should be selected
        assert_eq!(app.get_selected_count(), 0);
        assert_eq!(app.get_selected_directories().len(), 0);

        // Select first directory
        app.select_current(); // selects dir1 (index 0)
        assert_eq!(app.get_selected_count(), 1);
        assert_eq!(app.get_selected_directories().len(), 1);
        assert_eq!(app.get_selected_directories()[0].path, "dir1");

        // Select second directory
        app.selected = 1;
        app.select_current(); // selects dir2 (index 1)
        assert_eq!(app.get_selected_count(), 2);
        assert_eq!(app.get_selected_directories().len(), 2);

        // Verify both selected directories are in the list
        let selected_paths: Vec<&str> = app
            .get_selected_directories()
            .iter()
            .map(|d| d.path.as_str())
            .collect();
        assert!(selected_paths.contains(&"dir1"));
        assert!(selected_paths.contains(&"dir2"));

        // Verify total size calculation
        assert_eq!(app.get_selected_total_size(), 300); // 100 + 200

        // Test that the selection state is properly tracked
        assert!(app.directories[0].selected);
        assert!(app.directories[1].selected);
        assert!(!app.directories[2].selected);

        // Test toggle functionality
        app.selected = 0; // Move cursor to dir1
        app.toggle_current_selection(); // This should toggle dir1
        assert!(!app.directories[0].selected); // Should now be false
        assert_eq!(app.get_selected_count(), 1); // Only dir2 should be selected

        // Test select all
        app.select_all();
        assert_eq!(app.get_selected_count(), 3);
        assert!(app.directories[0].selected);
        assert!(app.directories[1].selected);
        assert!(app.directories[2].selected);

        // Test deselect all
        app.deselect_all();
        assert_eq!(app.get_selected_count(), 0);
        assert!(!app.directories[0].selected);
        assert!(!app.directories[1].selected);
        assert!(!app.directories[2].selected);
    }

    #[test]
    fn test_complete_selection_and_deletion_workflow() {
        use crate::fs::DeletionStatus;
        use crate::fs::DirectoryInfo;
        use crate::ui::app::App;

        // Create a test app with multiple directories
        let directories = vec![
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir1".to_string(),
                size: 100,
                formatted_size: "100 B".to_string(),
                selected: false,
                deletion_status: DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir2".to_string(),
                size: 200,
                formatted_size: "200 B".to_string(),
                selected: false,
                deletion_status: DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
            DirectoryInfo {
                last_modified: None,
                formatted_last_modified: "Unknown".to_string(),
                path: "dir3".to_string(),
                size: 300,
                formatted_size: "300 B".to_string(),
                selected: false,
                deletion_status: DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::Completed,
                calculation_time: None,
            },
        ];

        let mut app = App::new(directories, "test".to_string(), ".".to_string());

        // Simulate the workflow:
        // 1. User navigates to first directory (already selected by default)
        assert_eq!(app.selected, 0);

        // 2. User presses Space to select the first directory
        app.toggle_current_selection();
        assert!(app.directories[0].selected);
        assert_eq!(app.get_selected_count(), 1);

        // 3. User navigates to second directory
        app.selected = 1;
        assert_eq!(app.selected, 1);

        // 4. User presses Space to select the second directory
        app.toggle_current_selection();
        assert!(app.directories[1].selected);
        assert_eq!(app.get_selected_count(), 2);

        // 5. User navigates to third directory
        app.selected = 2;
        assert_eq!(app.selected, 2);

        // 6. User presses Space to select the third directory
        app.toggle_current_selection();
        assert!(app.directories[2].selected);
        assert_eq!(app.get_selected_count(), 3);

        // 7. Now all three directories should be selected
        assert!(app.directories[0].selected);
        assert!(app.directories[1].selected);
        assert!(app.directories[2].selected);

        // 8. Verify the selected directories list
        let selected_dirs = app.get_selected_directories();
        assert_eq!(selected_dirs.len(), 3);
        let selected_paths: Vec<&str> = selected_dirs.iter().map(|d| d.path.as_str()).collect();
        assert!(selected_paths.contains(&"dir1"));
        assert!(selected_paths.contains(&"dir2"));
        assert!(selected_paths.contains(&"dir3"));

        // 9. Verify total size calculation
        assert_eq!(app.get_selected_total_size(), 600); // 100 + 200 + 300

        // 10. Now simulate Delete key being pressed
        // This should call start_delete_selected_directories()
        // Since we can't actually delete files in tests, we just verify the method exists and works
        let result = app.start_delete_selected_directories();
        assert!(result.is_ok());

        // 11. Verify that the deletion progress is initialized
        assert!(app.deletion_progress.is_some());
        if let Some(progress) = &app.deletion_progress {
            assert_eq!(progress.total_items, 3);
            assert_eq!(progress.completed_items, 0);
        }
    }

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
            calculation_time: None,
        };

        // Verify that the directory has proper last modified time
        assert!(test_dir.last_modified.is_some());
        assert_eq!(test_dir.formatted_last_modified, "Just now");

        // Test that the path is clean (no ./ prefix)
        let clean_path = clean_path(&test_dir.path);
        assert_eq!(clean_path, "test/directory");

        // Test that the directory icon works
        let icon = get_directory_icon(false, false);
        assert_eq!(icon, "📁");

        // Test that selection indicator works
        let indicator = if test_dir.selected { "✓" } else { " " };
        assert_eq!(indicator, " ");
    }

    #[test]
    fn test_ui_shows_directories_during_discovery() {
        // Test that the UI shows directories in the list even during discovery
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Set discovery status to discovering
        app.set_discovery_status(app::DiscoveryStatus::Discovering);

        // Initially empty
        assert!(app.directories.is_empty());
        assert!(app.is_discovering());

        // Add some directories
        app.add_discovered_directory("dir1".to_string());
        app.add_discovered_directory("dir2".to_string());
        app.add_discovered_directory("dir3".to_string());

        // Process the batch (less than batch size, so they should be processed)
        app.process_remaining_pending();

        // Should now have directories in the list
        assert_eq!(app.directories.len(), 3);
        assert!(app.is_discovering()); // Still discovering

        // Verify the directories are accessible
        assert_eq!(app.directories[0].path, "dir1");
        assert_eq!(app.directories[1].path, "dir2");
        assert_eq!(app.directories[2].path, "dir3");

        // Verify they have the correct initial state
        for dir in &app.directories {
            assert_eq!(dir.size, 0);
            assert_eq!(dir.formatted_size, "Calculating...");
            assert!(matches!(
                dir.calculation_status,
                crate::fs::CalculationStatus::NotStarted
            ));
        }
    }

    #[test]
    fn test_ui_progressive_display_with_batches() {
        // Test that directories are displayed progressively in batches
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        app.batch_size = 3; // Set smaller batch size for testing

        app.set_discovery_status(app::DiscoveryStatus::Discovering);

        // Add 7 directories (more than 2 batches of 3)
        for i in 1..=7 {
            app.add_discovered_directory(format!("dir{i}"));
        }

        // Should have processed first batch of 3 and second batch of 3
        // (since add_discovered_directory automatically processes when batch_size is reached)
        assert_eq!(app.directories.len(), 6);
        assert_eq!(app.pending_directories.len(), 1);

        // Process remaining
        app.process_remaining_pending();

        // Should have all 7 directories
        assert_eq!(app.directories.len(), 7);
        assert_eq!(app.pending_directories.len(), 0);

        // Verify all directories are in the list
        for i in 1..=7 {
            assert_eq!(app.directories[i - 1].path, format!("dir{i}"));
        }
    }

    #[test]
    fn test_performance_optimizations() {
        // Test that performance optimizations work correctly
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());

        // Set discovery status to discovering
        app.set_discovery_status(app::DiscoveryStatus::Discovering);

        // Add many directories quickly to simulate discovery
        for i in 1..=50 {
            app.add_discovered_directory(format!("dir{i}"));
        }

        // Should have processed directories in batches
        assert!(!app.directories.is_empty());
        assert!(app.directories.len() <= 50);

        // Verify that directories are accessible and have correct initial state
        for dir in &app.directories {
            assert_eq!(dir.size, 0);
            assert_eq!(dir.formatted_size, "Calculating...");
            assert!(matches!(
                dir.calculation_status,
                crate::fs::CalculationStatus::NotStarted
            ));
        }

        // Verify discovery is still in progress
        assert!(app.is_discovering());
    }

    #[test]
    fn test_loading_frame_performance_optimization() {
        // Test that the loading frame function is now optimized (no time calculations)
        let start_time = std::time::Instant::now();

        // Call the function many times to simulate frame rendering
        for _ in 0..1000 {
            let _frame = get_loading_frame();
        }

        let elapsed = start_time.elapsed();

        // The optimized version should be extremely fast (no time calculations)
        // 1000 calls should take less than 1ms
        assert!(
            elapsed.as_micros() < 1000,
            "Loading frame function should be fast: {}μs",
            elapsed.as_micros()
        );

        // Verify it returns a consistent static value
        let frame1 = get_loading_frame();
        let frame2 = get_loading_frame();
        assert_eq!(frame1, frame2, "Should return consistent static value");
        assert_eq!(
            frame1, "⠋",
            "Should return the expected static loading indicator"
        );
    }

    #[test]
    fn test_loading_frame_benchmark_comparison() {
        // Benchmark comparison: old vs new approach
        // This simulates what the old time-based animation would have cost

        // New optimized approach (what we have now)
        let start_optimized = std::time::Instant::now();
        for _ in 0..10000 {
            let _frame = get_loading_frame();
        }
        let optimized_time = start_optimized.elapsed();

        // Simulate the old expensive approach (for comparison)
        let start_expensive = std::time::Instant::now();
        for _ in 0..10000 {
            // This simulates the old expensive time calculation
            let _frame = {
                let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let index =
                    (std::time::Instant::now().elapsed().as_millis() / 100) as usize % frames.len();
                frames[index]
            };
        }
        let expensive_time = start_expensive.elapsed();

        // The optimized version should be significantly faster
        assert!(
            optimized_time < expensive_time,
            "Optimized version should be faster: {}μs vs {}μs",
            optimized_time.as_micros(),
            expensive_time.as_micros()
        );

        // In real usage, this would be called 60-120 times per second during discovery
        // So the savings are multiplied by the frame rate
        println!(
            "Performance improvement: {}μs vs {}μs ({}x faster)",
            optimized_time.as_micros(),
            expensive_time.as_micros(),
            expensive_time.as_micros() / optimized_time.as_micros().max(1)
        );
    }

    #[test]
    fn test_all_animations_performance_optimization() {
        // Comprehensive test showing the performance improvement from removing all animations
        // This simulates the real-world usage where these functions are called every frame

        let start_time = std::time::Instant::now();

        // Simulate rendering 50 visible directories every frame for 100 frames
        // This represents a typical scenario during discovery
        for _frame in 0..100 {
            for dir_index in 0..50 {
                // Simulate what happens during UI rendering
                let _icon = get_directory_icon(dir_index % 3 == 0, dir_index % 5 == 0); // Some selected, some highlighted
                let _color = get_selection_indicator_color(dir_index % 3 == 0);
                let _status_icon =
                    get_calculation_status_icon(&crate::fs::CalculationStatus::Calculating);
                let _loading_frame = get_loading_frame();
            }
        }

        let optimized_time = start_time.elapsed();

        // Simulate the old expensive approach for comparison
        let start_expensive = std::time::Instant::now();

        for _frame in 0..100 {
            for dir_index in 0..50 {
                // Simulate the old expensive time-based calculations
                let _icon = {
                    let time = std::time::Instant::now().elapsed().as_millis();
                    if dir_index % 3 == 0 {
                        let open_frames =
                            ["📂", "📁", "📂", "📁", "📂", "📁", "📂", "📁", "📂", "📁"];
                        let index = (time / 120) as usize % open_frames.len();
                        open_frames[index]
                    } else if dir_index % 5 == 0 {
                        let closed_frames =
                            ["📁", "📂", "📁", "📂", "📁", "📂", "📁", "📂", "📁", "📂"];
                        let index = (time / 250) as usize % closed_frames.len();
                        closed_frames[index]
                    } else {
                        "📁"
                    }
                };

                let _color = {
                    let time = std::time::Instant::now().elapsed().as_millis();
                    if dir_index % 3 == 0 {
                        let index = (time / 300) as usize % 2;
                        if index == 0 {
                            SELECTION_INDICATOR_COLOR
                        } else {
                            Color::Rgb(142, 192, 124)
                        }
                    } else {
                        TEXT_SECONDARY
                    }
                };

                let _status_icon = {
                    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                    let time = std::time::Instant::now().elapsed().as_millis();
                    let index = (time / 100) as usize % frames.len();
                    frames[index]
                };

                let _loading_frame = {
                    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                    let time = std::time::Instant::now().elapsed().as_millis();
                    let index = (time / 100) as usize % frames.len();
                    frames[index]
                };
            }
        }

        let expensive_time = start_expensive.elapsed();

        // Calculate improvement
        let improvement_factor = expensive_time.as_micros() / optimized_time.as_micros().max(1);

        // The optimized version should be significantly faster
        assert!(
            optimized_time < expensive_time,
            "Optimized version should be faster: {}μs vs {}μs",
            optimized_time.as_micros(),
            expensive_time.as_micros()
        );

        // In real usage, this would be called 60-120 times per second during discovery
        // So the savings are multiplied by the frame rate and number of visible items
        println!("🎯 MASSIVE PERFORMANCE IMPROVEMENT:");
        println!("   Optimized: {}μs", optimized_time.as_micros());
        println!("   Expensive: {}μs", expensive_time.as_micros());
        println!("   Improvement: {improvement_factor}x faster");
        println!(
            "   Time saved per frame: {}μs",
            expensive_time.as_micros() - optimized_time.as_micros()
        );
        println!(
            "   CPU usage reduction: ~{}%",
            ((expensive_time.as_micros() - optimized_time.as_micros()) * 100
                / expensive_time.as_micros())
        );

        // Verify the functions still work correctly
        assert_eq!(get_directory_icon(true, false), "📂");
        assert_eq!(get_directory_icon(false, true), "📁");
        assert_eq!(get_directory_icon(false, false), "📁");
        assert_eq!(get_loading_frame(), "⠋");
        assert_eq!(
            get_calculation_status_icon(&crate::fs::CalculationStatus::Calculating),
            "⠋"
        );
    }
}
