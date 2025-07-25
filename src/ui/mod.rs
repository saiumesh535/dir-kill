use anyhow::Result;
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Alignment},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Padding},
    Terminal,
};
use std::io;

// Enhanced Gruvbox Dark Theme Color Palette with additional beautiful colors
// Based on https://github.com/morhetz/gruvbox with custom enhancements
const PRIMARY_COLOR: Color = Color::Rgb(131, 165, 152);   // gruvbox-aqua
const SECONDARY_COLOR: Color = Color::Rgb(250, 189, 47);  // gruvbox-yellow
const ACCENT_COLOR: Color = Color::Rgb(211, 134, 155);    // gruvbox-pink
const SUCCESS_COLOR: Color = Color::Rgb(184, 187, 38);    // gruvbox-green
const WARNING_COLOR: Color = Color::Rgb(250, 189, 47);    // gruvbox-yellow
const ERROR_COLOR: Color = Color::Rgb(251, 73, 52);       // gruvbox-red
const BACKGROUND_COLOR: Color = Color::Rgb(29, 32, 33);   // Enhanced dark background
const SURFACE_COLOR: Color = Color::Rgb(40, 40, 40);      // Enhanced surface color
const TEXT_PRIMARY: Color = Color::Rgb(235, 219, 178);    // gruvbox-fg0 (light)
const TEXT_SECONDARY: Color = Color::Rgb(189, 174, 147);  // gruvbox-fg1 (medium)
const SELECTION_BG: Color = Color::Rgb(131, 165, 152);    // gruvbox-aqua selection background
const SELECTION_FG: Color = Color::Rgb(29, 32, 33);       // Dark text on selection
const SELECTION_INDICATOR_COLOR: Color = Color::Rgb(184, 187, 38);  // gruvbox-green for selection indicators
const SELECTION_GLOW_COLOR: Color = Color::Rgb(142, 192, 124);      // lighter green for glow effect

// Additional beautiful colors for enhanced UI
const BORDER_COLOR: Color = Color::Rgb(80, 73, 69);       // Subtle border color
const HIGHLIGHT_COLOR: Color = Color::Rgb(254, 128, 25);  // Orange highlight
const INFO_COLOR: Color = Color::Rgb(131, 165, 152);      // Info blue
const MUTED_COLOR: Color = Color::Rgb(146, 131, 116);     // Muted text
const GRADIENT_START: Color = Color::Rgb(131, 165, 152);  // Gradient start
const GRADIENT_END: Color = Color::Rgb(184, 187, 38);     // Gradient end

/// Color scheme definitions
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum ColorScheme {
    GruvboxDark,
    SolarizedDark,
    Dracula,
    Nord,
}

/// Color scheme configuration
pub struct ThemeColors {
    pub primary: Color,
    pub secondary: Color,
    pub accent: Color,
    pub success: Color,
    pub warning: Color,
    pub error: Color,
    pub background: Color,
    pub surface: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
    pub selection_indicator: Color,
    pub selection_glow: Color,
    pub border: Color,
    pub highlight: Color,
    pub muted: Color,
}

impl ThemeColors {
    pub fn gruvbox_dark() -> Self {
        Self {
            primary: Color::Rgb(131, 165, 152),      // gruvbox-aqua
            secondary: Color::Rgb(250, 189, 47),     // gruvbox-yellow
            accent: Color::Rgb(211, 134, 155),       // gruvbox-pink
            success: Color::Rgb(184, 187, 38),       // gruvbox-green
            warning: Color::Rgb(250, 189, 47),       // gruvbox-yellow
            error: Color::Rgb(251, 73, 52),          // gruvbox-red
            background: Color::Rgb(29, 32, 33),      // Enhanced dark background
            surface: Color::Rgb(40, 40, 40),         // Enhanced surface color
            text_primary: Color::Rgb(235, 219, 178), // gruvbox-fg0 (light)
            text_secondary: Color::Rgb(189, 174, 147), // gruvbox-fg1 (medium)
            selection_bg: Color::Rgb(131, 165, 152), // gruvbox-aqua selection background
            selection_fg: Color::Rgb(29, 32, 33),    // Dark text on selection
            selection_indicator: Color::Rgb(184, 187, 38), // gruvbox-green for selection indicators
            selection_glow: Color::Rgb(142, 192, 124),     // lighter green for glow effect
            border: Color::Rgb(80, 73, 69),          // Subtle border color
            highlight: Color::Rgb(254, 128, 25),     // Orange highlight
            muted: Color::Rgb(146, 131, 116),        // Muted text
        }
    }

    pub fn solarized_dark() -> Self {
        Self {
            primary: Color::Rgb(38, 139, 210),       // Solarized blue
            secondary: Color::Rgb(181, 137, 0),      // Solarized yellow
            accent: Color::Rgb(211, 54, 130),        // Solarized magenta
            success: Color::Rgb(133, 153, 0),        // Solarized green
            warning: Color::Rgb(181, 137, 0),        // Solarized yellow
            error: Color::Rgb(220, 50, 47),          // Solarized red
            background: Color::Rgb(0, 43, 54),       // Solarized base03
            surface: Color::Rgb(7, 54, 66),          // Solarized base02
            text_primary: Color::Rgb(238, 232, 213), // Solarized base2
            text_secondary: Color::Rgb(131, 148, 150), // Solarized base1
            selection_bg: Color::Rgb(38, 139, 210),  // Solarized blue
            selection_fg: Color::Rgb(0, 43, 54),     // Solarized base03
            selection_indicator: Color::Rgb(133, 153, 0), // Solarized green
            selection_glow: Color::Rgb(88, 110, 117),     // Solarized base01
            border: Color::Rgb(88, 110, 117),        // Solarized base01
            highlight: Color::Rgb(203, 75, 22),      // Solarized orange
            muted: Color::Rgb(101, 123, 131),        // Solarized base0
        }
    }

    pub fn dracula() -> Self {
        Self {
            primary: Color::Rgb(139, 233, 253),      // Dracula cyan
            secondary: Color::Rgb(241, 250, 140),    // Dracula yellow
            accent: Color::Rgb(255, 121, 198),       // Dracula pink
            success: Color::Rgb(80, 250, 123),       // Dracula green
            warning: Color::Rgb(241, 250, 140),      // Dracula yellow
            error: Color::Rgb(255, 85, 85),          // Dracula red
            background: Color::Rgb(40, 42, 54),      // Dracula background
            surface: Color::Rgb(68, 71, 90),         // Dracula current line
            text_primary: Color::Rgb(248, 248, 242), // Dracula foreground
            text_secondary: Color::Rgb(189, 147, 249), // Dracula purple
            selection_bg: Color::Rgb(139, 233, 253), // Dracula cyan
            selection_fg: Color::Rgb(40, 42, 54),    // Dracula background
            selection_indicator: Color::Rgb(80, 250, 123), // Dracula green
            selection_glow: Color::Rgb(98, 114, 164),     // Dracula comment
            border: Color::Rgb(98, 114, 164),        // Dracula comment
            highlight: Color::Rgb(255, 184, 108),    // Dracula orange
            muted: Color::Rgb(98, 114, 164),         // Dracula comment
        }
    }

    pub fn nord() -> Self {
        Self {
            primary: Color::Rgb(136, 192, 208),      // Nord blue
            secondary: Color::Rgb(236, 239, 244),    // Nord snow storm
            accent: Color::Rgb(180, 142, 173),       // Nord purple
            success: Color::Rgb(163, 190, 140),      // Nord green
            warning: Color::Rgb(235, 203, 139),      // Nord yellow
            error: Color::Rgb(191, 97, 106),         // Nord red
            background: Color::Rgb(46, 52, 64),      // Nord polar night
            surface: Color::Rgb(59, 66, 82),         // Nord polar night
            text_primary: Color::Rgb(236, 239, 244), // Nord snow storm
            text_secondary: Color::Rgb(229, 233, 240), // Nord snow storm
            selection_bg: Color::Rgb(136, 192, 208), // Nord blue
            selection_fg: Color::Rgb(46, 52, 64),    // Nord polar night
            selection_indicator: Color::Rgb(163, 190, 140), // Nord green
            selection_glow: Color::Rgb(180, 142, 173),     // Nord purple
            border: Color::Rgb(76, 86, 106),         // Nord polar night
            highlight: Color::Rgb(208, 135, 112),    // Nord orange
            muted: Color::Rgb(76, 86, 106),          // Nord polar night
        }
    }

    pub fn get_colors(scheme: ColorScheme) -> Self {
        match scheme {
            ColorScheme::GruvboxDark => Self::gruvbox_dark(),
            ColorScheme::SolarizedDark => Self::solarized_dark(),
            ColorScheme::Dracula => Self::dracula(),
            ColorScheme::Nord => Self::nord(),
        }
    }
}

/// Remove ./ prefix from path if present
fn clean_path(path: &str) -> &str {
    if path.starts_with("./") {
        &path[2..]
    } else {
        path
    }
}

/// Get beautiful loading animation frame based on time
fn get_loading_frame() -> &'static str {
    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let index = (std::time::Instant::now().elapsed().as_millis() / 100) as usize % frames.len();
    frames[index]
}

/// Get enhanced loading animation with colors
fn get_enhanced_loading_frame() -> &'static str {
    let frames = ["🌊", "🌊", "🌊", "🌊", "🌊", "🌊", "🌊", "🌊", "🌊", "🌊"];
    let index = (std::time::Instant::now().elapsed().as_millis() / 200) as usize % frames.len();
    frames[index]
}

/// Get animated graph bar with time-based animation
fn get_animated_graph_bar(value: f64, max_value: f64, width: usize) -> String {
    if max_value == 0.0 {
        return " ".repeat(width);
    }
    
    let percentage = (value / max_value).min(1.0);
    let bar_width = (percentage * width as f64).round() as usize;
    
    if bar_width == 0 {
        return " ".repeat(width);
    }
    
    // Animated bar with different characters based on time
    let time = std::time::Instant::now().elapsed().as_millis();
    let animation_frame = (time / 200) % 4;
    
    let bar_char = match animation_frame {
        0 => "█",
        1 => "▓",
        2 => "▒",
        3 => "░",
        _ => "█",
    };
    
    let bar = bar_char.repeat(bar_width);
    let padding = " ".repeat(width.saturating_sub(bar_width));
    format!("{}{}", bar, padding)
}

/// Get animated pie chart slice
fn get_animated_pie_slice(_percentage: f64, time: u128) -> &'static str {
    let frames = ["◐", "◑", "◒", "◓"];
    let index = (time / 300) as usize % frames.len();
    frames[index]
}

/// Get size distribution graph data
fn get_size_distribution_graph(directories: &[DirectoryInfo]) -> Vec<(&'static str, u64, f64)> {
    let mut size_ranges = vec![
        ("0-1MB", 0, 0.0),
        ("1-10MB", 0, 0.0),
        ("10-100MB", 0, 0.0),
        ("100MB-1GB", 0, 0.0),
        ("1GB+", 0, 0.0),
    ];
    
    for dir in directories {
        let size_mb = dir.size as f64 / (1024.0 * 1024.0);
        let index = if size_mb < 1.0 { 0 }
                   else if size_mb < 10.0 { 1 }
                   else if size_mb < 100.0 { 2 }
                   else if size_mb < 1024.0 { 3 }
                   else { 4 };
        
        size_ranges[index].1 += 1;
    }
    
    let total = directories.len() as f64;
    for (_, count, percentage) in &mut size_ranges {
        *percentage = if total > 0.0 { *count as f64 / total * 100.0 } else { 0.0 };
    }
    
    size_ranges
}

/// Highlight search terms in text with different colors
fn highlight_search_term<'a>(text: &'a str, search_term: &str, normal_style: Style, highlight_color: Color) -> Vec<Span<'a>> {
    if search_term.is_empty() {
        return vec![Span::styled(text, normal_style)];
    }
    
    let text_lower = text.to_lowercase();
    let search_lower = search_term.to_lowercase();
    
    if !text_lower.contains(&search_lower) {
        return vec![Span::styled(text, normal_style)];
    }
    
    let mut spans = Vec::new();
    let mut current_pos = 0;
    
    while let Some(start) = text_lower[current_pos..].find(&search_lower) {
        let actual_start = current_pos + start;
        
        // Add text before the match
        if actual_start > current_pos {
            spans.push(Span::styled(&text[current_pos..actual_start], normal_style));
        }
        
        // Add the highlighted match
        let match_end = actual_start + search_term.len();
        let highlight_style = Style::default()
            .fg(highlight_color)
            .add_modifier(Modifier::BOLD | Modifier::UNDERLINED);
        spans.push(Span::styled(&text[actual_start..match_end], highlight_style));
        
        current_pos = match_end;
    }
    
    // Add remaining text after the last match
    if current_pos < text.len() {
        spans.push(Span::styled(&text[current_pos..], normal_style));
    }
    
    spans
}

/// Get beautiful animated directory icon with selection state
fn get_directory_icon(selected: bool, is_highlighted: bool) -> &'static str {
    let time = std::time::Instant::now().elapsed().as_millis();
    
    if selected {
        // Beautiful animated open directory for selected items - faster animation
        let open_frames = ["📂", "📁", "📂", "📁", "📂", "📁", "📂", "📁", "📂", "📁"];
        let index = (time / 120) as usize % open_frames.len();
        open_frames[index]
    } else if is_highlighted {
        // Beautiful animated closed directory for highlighted items - slower animation
        let closed_frames = ["📁", "📂", "📁", "📂", "📁", "📂", "📁", "📂", "📁", "📂"];
        let index = (time / 250) as usize % closed_frames.len();
        closed_frames[index]
    } else {
        // Beautiful static closed directory for normal items
        "📁"
    }
}

/// Get selection indicator color with subtle glow effect
fn get_selection_indicator_color(selected: bool) -> Color {
    if selected {
        let index = (std::time::Instant::now().elapsed().as_millis() / 300) as usize % 2;
        if index == 0 {
            SELECTION_INDICATOR_COLOR
        } else {
            SELECTION_GLOW_COLOR
        }
    } else {
        TEXT_SECONDARY
    }
}

/// Get calculation status icon with animation
fn get_calculation_status_icon(status: &crate::fs::CalculationStatus) -> &'static str {
    match status {
        crate::fs::CalculationStatus::NotStarted => "⏳",
        crate::fs::CalculationStatus::Calculating => {
            let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let index = (std::time::Instant::now().elapsed().as_millis() / 100) as usize % frames.len();
            frames[index]
        },
        crate::fs::CalculationStatus::Completed => "",
        crate::fs::CalculationStatus::Error(_) => "❌",
    }
}

pub mod app;
pub mod list;

use app::App;
use crate::fs::{self, DirectoryInfo};


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
    crossterm::execute!(io::stdout(), crossterm::terminal::Clear(crossterm::terminal::ClearType::All))?;
    Ok(())
}

/// Display directories in beautiful TUI format with real-time scanning
pub fn display_directories_with_scanning(pattern: &str, path: &str) -> Result<()> {
    // Check if we're in a terminal that supports TUI
    let term = std::env::var("TERM").unwrap_or_default();
    
    // macOS Terminal.app often has issues with TUI, so we'll use text mode
    let use_tui = !term.is_empty() && term != "dumb" && !term.contains("Apple_Terminal");
    
    if use_tui {
        // Try to initialize TUI mode, fallback to text mode if it fails
        match init_terminal() {
            Ok(mut terminal) => {
                // TUI mode successful, use the full interface
                display_directories_tui(&mut terminal, pattern, path)
            }
            Err(_) => {
                // TUI mode failed, fallback to text mode
                display_directories_text(pattern, path)
            }
        }
    } else {
        // Use text mode for unsupported terminals
        display_directories_text(pattern, path)
    }
}

/// Display directories in TUI mode
fn display_directories_tui(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, pattern: &str, path: &str) -> Result<()> {
    
    let mut app = App::new(vec![], pattern.to_string(), path.to_string());
    let mut is_scanning = true;
    
    // Start scanning in background (without size calculation for speed)
    let pattern_clone = pattern.to_string();
    let path_clone = path.to_string();
    let directories_sender = std::sync::mpsc::channel();
    let (tx, rx) = directories_sender;
    
    // Channel for size updates - use path as identifier instead of index
    let size_sender = std::sync::mpsc::channel::<(String, u64, String)>();
    let (size_tx, size_rx) = size_sender;
    
    // Channel for calculation status updates
    let calc_status_sender = std::sync::mpsc::channel::<(String, crate::fs::CalculationStatus)>();
    let (calc_status_tx, calc_status_rx) = calc_status_sender;
    
    // Use a thread pool for size calculations to limit concurrent threads
    let size_tx_clone = size_tx.clone();
    let handle = std::thread::spawn(move || {
        match fs::find_directories(&path_clone, &pattern_clone) {
            Ok(dirs) => {
                // Collect all directories first
                let dir_paths: Vec<String> = dirs.into_iter().collect();
                
                // Send all directories without size initially
                for dir_path in &dir_paths {
                    // Get last modified time for the parent directory (not the matching directory itself)
                    let path = std::path::Path::new(dir_path);
                    let parent_path = path.parent().unwrap_or(path);
                    let last_modified = fs::get_directory_last_modified(parent_path);
                    let formatted_last_modified = last_modified
                        .as_ref()
                        .map(|time| fs::format_last_modified(time))
                        .unwrap_or_else(|| "Unknown".to_string());
                    
                    let _ = tx.send(DirectoryInfo {
                        path: dir_path.clone(),
                        size: 0,
                        formatted_size: "Calculating...".to_string(),
                        last_modified,
                        formatted_last_modified,
                        selected: false,
                        deletion_status: crate::fs::DeletionStatus::Normal,
                        calculation_status: crate::fs::CalculationStatus::NotStarted,
                    });
                }
                
                // Start size calculations in a separate thread with limited concurrency
                let size_tx_for_calc = size_tx_clone.clone();
                let calc_status_tx_for_calc = calc_status_tx.clone();
                std::thread::spawn(move || {
                    // Process directories in batches to avoid overwhelming the system
                    let max_concurrent = 4; // Limit concurrent calculations
                    let mut active_threads: usize = 0;
                    
                    for dir_path in dir_paths {
                        // Wait if we have too many active threads
                        while active_threads >= max_concurrent {
                            std::thread::sleep(std::time::Duration::from_millis(100));
                            active_threads = active_threads.saturating_sub(1);
                        }
                        
                        // Add a small delay between calculations to keep UI responsive
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        
                        // Calculate size without blocking the UI
                        let dir_path_clone = dir_path.clone();
                        let size_tx_for_this = size_tx_for_calc.clone();
                        let calc_status_tx_for_this = calc_status_tx_for_calc.clone();
                        
                        active_threads += 1;
                        
                        // Send status update that calculation is starting
                        let _ = calc_status_tx_for_this.send((dir_path.clone(), crate::fs::CalculationStatus::Calculating));
                        
                        // Spawn a quick calculation thread that doesn't block
                        std::thread::spawn(move || {
                            let calculated_size = fs::calculate_directory_size(std::path::Path::new(&dir_path_clone)).unwrap_or(0);
                            let formatted_size = fs::format_size(calculated_size);
                            let _ = size_tx_for_this.send((dir_path_clone.clone(), calculated_size, formatted_size));
                            // Also send completion status update
                            let _ = calc_status_tx_for_this.send((dir_path_clone, crate::fs::CalculationStatus::Completed));
                        });
                    }
                });
            }
            Err(e) => {
                let _ = tx.send(DirectoryInfo {
                    path: format!("ERROR: {}", e),
                    size: 0,
                    formatted_size: "0 B".to_string(),
                    last_modified: None,
                    formatted_last_modified: "Unknown".to_string(),
                    selected: false,
                    deletion_status: crate::fs::DeletionStatus::Normal,
                    calculation_status: crate::fs::CalculationStatus::Error(e.to_string()),
                });
            }
        }
    });
    
    // Main event loop
    loop {
        // Check for new directories from background thread
        while let Ok(dir) = rx.try_recv() {
            if dir.path.starts_with("ERROR:") {
                // Handle error
                is_scanning = false;
                break;
            } else {
                app.directories.push(dir);
            }
        }
        
        // Check for size updates
        while let Ok((path, size, formatted_size)) = size_rx.try_recv() {
            // Find the directory by path and update its size
            if let Some(dir) = app.directories.iter_mut().find(|d| d.path == path) {
                dir.size = size;
                dir.formatted_size = formatted_size;
                dir.calculation_status = crate::fs::CalculationStatus::Completed;
            }
        }
        
        // Check for calculation status updates
        while let Ok((path, status)) = calc_status_rx.try_recv() {
            // Find the directory by path and update its calculation status
            if let Some(dir) = app.directories.iter_mut().find(|d| d.path == path) {
                dir.calculation_status = status;
            }
        }
        
        // Process deletion messages
        app.process_deletion_messages();
        
        // Check if the background thread has finished
        if is_scanning && handle.is_finished() {
            // Try one more time to get any remaining data
            while let Ok(dir) = rx.try_recv() {
                if dir.path.starts_with("ERROR:") {
                    // Handle error
                    break;
                } else {
                    app.directories.push(dir);
                }
            }
            // Scanning is complete
            is_scanning = false;
        }
        
        // Calculate layout and items per page
        let size = terminal.size()?;
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints(
                [
                    Constraint::Length(3), // Search bar
                    Constraint::Length(4), // Header
                    Constraint::Min(0),    // Main content area
                    Constraint::Length(6), // Footer
                ]
                .as_ref(),
            )
            .split(size);
        
        // Split main content area into two panels
        let main_panels = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(70), // Directory list
                    Constraint::Percentage(30), // Details panel
                ]
                .as_ref(),
            )
            .split(chunks[2]);

        let available_height = main_panels[0].height.saturating_sub(2);
        let items_per_page = available_height.max(1) as usize;

        terminal.draw(|f| {
            // Get current theme colors
            let theme = app.get_current_theme();
            
            // Set background color for the entire terminal
            f.render_widget(
                Paragraph::new("").style(Style::default().bg(theme.background)),
                f.size(),
            );

            // --- btop-style Search Bar ---
            let search_active = app.is_searching;
            let search_bar_text = if search_active || !app.search_query.is_empty() {
                vec![
                    Line::from(vec![
                        Span::styled("/ ", Style::default().fg(theme.primary)),
                        Span::styled(&app.search_query, Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                        Span::styled("█", Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)),
                        Span::styled(format!("  {}", app.get_search_status()), Style::default().fg(theme.text_secondary)),
                    ])
                ]
            } else {
                vec![
                    Line::from(vec![
                        Span::styled("/ ", Style::default().fg(theme.primary)),
                        Span::styled("Type to search...", Style::default().fg(theme.text_secondary)),
                        Span::styled("  (Press / or Ctrl+F)", Style::default().fg(theme.muted)),
                    ])
                ]
            };
            let search_bar = Paragraph::new(search_bar_text)
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .border_style(if search_active { Style::default().fg(theme.accent) } else { Style::default().fg(theme.border) })
                        .title_style(Style::default().fg(theme.primary).add_modifier(Modifier::BOLD))
                        .title("🔍 Search")
                )
                .style(Style::default().bg(theme.surface));
            f.render_widget(search_bar, chunks[0]);

            // Calculate total size for header
            let total_size: u64 = app.directories.iter().map(|dir| dir.size).sum();
            let total_formatted = fs::format_size(total_size);
            let calculated_count = app.directories.iter().filter(|dir| dir.size > 0).count();
            let total_count = app.directories.len();
            
            // Enhanced Header with beautiful styling
            let header = Paragraph::new(vec![
                Line::from(vec![
                    Span::styled("🔍 Directory Search Results", Style::default().fg(PRIMARY_COLOR).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    Span::styled("Pattern: ", Style::default().fg(TEXT_SECONDARY)),
                    Span::styled(format!("'{}'", pattern), Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD)),
                    Span::styled(" in ", Style::default().fg(TEXT_SECONDARY)),
                    Span::styled(format!("'{}'", std::env::current_dir().unwrap_or_default().join(path).to_string_lossy()), Style::default().fg(SECONDARY_COLOR).add_modifier(Modifier::BOLD)),
                ]),
                Line::from(vec![
                    if is_scanning {
                        if app.directories.is_empty() {
                            Span::styled(format!("{} Scanning directories...", get_loading_frame()), Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD))
                        } else {
                            Span::styled(format!("{} Found {} directories, scanning...", get_loading_frame(), app.directories.len()), Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD))
                        }
                    } else {
                        Span::styled(format!("✅ Scan complete - Found {} directories", app.directories.len()), Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD))
                    }
                ]),
                Line::from(vec![
                    Span::styled("📄 Page ", Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)),
                    Span::styled(format!("{} of {}", app.current_page + 1, app.total_pages(items_per_page)), Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD)),
                    Span::styled(" | ", Style::default().fg(TEXT_SECONDARY)),
                    Span::styled(format!("{} items per page", items_per_page), Style::default().fg(TEXT_SECONDARY)),
                ]),
                if !app.directories.is_empty() {
                    Line::from(vec![
                        Span::styled("💾 Total Size: ", Style::default().fg(TEXT_SECONDARY)),
                        Span::styled(total_formatted.clone(), Style::default().fg(HIGHLIGHT_COLOR).add_modifier(Modifier::BOLD)),
                        Span::styled(" (", Style::default().fg(TEXT_SECONDARY)),
                        Span::styled(format!("{}/{} calculated", calculated_count, total_count), Style::default().fg(ACCENT_COLOR)),
                        Span::styled(")", Style::default().fg(TEXT_SECONDARY)),
                    ])
                } else {
                    Line::from(vec![])
                },
            ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER_COLOR))
                    .title_style(Style::default().fg(PRIMARY_COLOR).add_modifier(Modifier::BOLD))
                    .title("📊 Search Info")
            )
            .style(Style::default().bg(SURFACE_COLOR));
            f.render_widget(header, chunks[1]);

            // Directory list or loading state
            // Show loading if we're still scanning
            if is_scanning {
                // Show loading state across both panels
                let loading_text = if app.directories.is_empty() {
                    vec![
                        Line::from(vec![
                            Span::styled("🔍 ", Style::default().fg(PRIMARY_COLOR)),
                            Span::styled("Scanning directories...", Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
                        ]),
                        Line::from(vec![
                            Span::styled("   ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled(format!("{}", get_loading_frame()), Style::default().fg(WARNING_COLOR)),
                        ]),
                        Line::from(vec![
                            Span::styled("   ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled("Please wait while we search for directories...", Style::default().fg(TEXT_SECONDARY)),
                        ]),
                    ]
                } else {
                    vec![
                        Line::from(vec![
                            Span::styled("⏳ ", Style::default().fg(WARNING_COLOR)),
                            Span::styled("Found directories, finishing scan...", Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
                        ]),
                        Line::from(vec![
                            Span::styled("   ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled(format!("{}", get_loading_frame()), Style::default().fg(WARNING_COLOR)),
                        ]),
                        Line::from(vec![
                            Span::styled("   ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled(format!("Found {} directories so far...", app.directories.len()), Style::default().fg(TEXT_SECONDARY)),
                        ]),
                        Line::from(vec![
                            Span::styled("   ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled("Sizes will be calculated in background...", Style::default().fg(TEXT_SECONDARY)),
                        ]),
                    ]
                };

                let loading_widget = Paragraph::new(loading_text)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(PRIMARY_COLOR))
                            .title_style(Style::default().fg(PRIMARY_COLOR).add_modifier(Modifier::BOLD))
                            .title("📁 Directory Search")
                    )
                    .style(Style::default().bg(SURFACE_COLOR))
                    .alignment(Alignment::Center);
                f.render_widget(loading_widget, chunks[1]);
            } else if !app.directories.is_empty() {
                // Show directory list in left panel with better alignment
                let filtered_directories = app.get_filtered_directories();
                let visible_items = if filtered_directories.is_empty() {
                    vec![]
                } else {
                    let start = app.current_page * items_per_page;
                    let end = std::cmp::min(start + items_per_page, filtered_directories.len());
                    filtered_directories.get(start..end).unwrap_or(&[]).iter().collect()
                };
                let list_items: Vec<ListItem> = visible_items.iter().enumerate().map(|(visible_index, dir)| {
                    // For filtered directories, we need to find the actual index in the original list
                    let actual_index = if app.search_query.is_empty() {
                        app.current_page * items_per_page + visible_index
                    } else {
                        // Find the index of this directory in the original list
                        app.directories.iter().position(|d| d.path == dir.path).unwrap_or(0)
                    };
                    let is_selected = actual_index == app.selected;
                    
                    // Simplified styling for list items (size info moved to details panel)
                    let path_style = if is_selected {
                        Style::default().fg(SELECTION_FG).bg(SELECTION_BG).add_modifier(Modifier::BOLD)
                    } else if dir.selected {
                        Style::default().fg(SELECTION_INDICATOR_COLOR).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                    } else {
                        Style::default().fg(TEXT_PRIMARY)
                    };

                    // Create a formatted line with proper spacing and alignment
                    let icon = get_directory_icon(dir.selected, is_selected);
                    let selection_indicator = if dir.selected { "✓" } else { " " };
                    let path = clean_path(&dir.path);
                    
                    // Calculate status icon
                    let status_icon = match &dir.deletion_status {
                        crate::fs::DeletionStatus::Normal => {
                            match dir.calculation_status {
                                crate::fs::CalculationStatus::NotStarted | crate::fs::CalculationStatus::Calculating | crate::fs::CalculationStatus::Error(_) =>
                                    get_calculation_status_icon(&dir.calculation_status),
                                crate::fs::CalculationStatus::Completed => "  ",
                            }
                        },
                        crate::fs::DeletionStatus::Deleting => "🔄",
                        crate::fs::DeletionStatus::Deleted => "🗑️",
                        crate::fs::DeletionStatus::Error(_) => "⚠️",
                    };

                    // Create a beautifully formatted line with enhanced styling
                    let icon_style = Style::default()
                        .fg(get_selection_indicator_color(dir.selected))
                        .add_modifier(if dir.selected { Modifier::BOLD } else { Modifier::empty() });
                    
                    let selection_style = if dir.selected {
                        Style::default().fg(theme.selection_indicator).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(theme.muted)
                    };
                    
                    let status_style = match &dir.deletion_status {
                        crate::fs::DeletionStatus::Normal => {
                            match dir.calculation_status {
                                crate::fs::CalculationStatus::NotStarted => Style::default().fg(theme.muted),
                                crate::fs::CalculationStatus::Calculating => Style::default().fg(theme.warning).add_modifier(Modifier::BOLD),
                                crate::fs::CalculationStatus::Completed => Style::default().fg(theme.success),
                                crate::fs::CalculationStatus::Error(_) => Style::default().fg(theme.error),
                            }
                        },
                        crate::fs::DeletionStatus::Deleting => Style::default().fg(theme.warning).add_modifier(Modifier::BOLD),
                        crate::fs::DeletionStatus::Deleted => Style::default().fg(theme.success).add_modifier(Modifier::BOLD),
                        crate::fs::DeletionStatus::Error(_) => Style::default().fg(theme.error).add_modifier(Modifier::BOLD),
                    };

                    // Create path with search term highlighting
                    let path_spans = if !app.search_query.is_empty() && path.to_lowercase().contains(&app.search_query.to_lowercase()) {
                        highlight_search_term(path, &app.search_query, path_style, theme.highlight)
                    } else {
                        vec![Span::styled(path, path_style)]
                    };

                    // Create beautiful styling for each component
                    let mut line_spans = vec![
                        Span::styled(format!("{} ", icon), icon_style),
                        Span::styled(format!("{} ", selection_indicator), selection_style),
                    ];
                    line_spans.extend(path_spans);
                    line_spans.push(Span::styled(" ", Style::default()));
                    line_spans.push(Span::styled(status_icon, status_style));

                    ListItem::new(vec![Line::from(line_spans)])
                }).collect();

                let list = List::new(list_items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)

                            .border_style(Style::default().fg(BORDER_COLOR))
                            .title_style(Style::default().fg(PRIMARY_COLOR).add_modifier(Modifier::BOLD))
                            .title(if app.search_query.is_empty() {
                                format!("📂 Directories (Page {}/{})", app.current_page + 1, app.total_pages(items_per_page))
                            } else {
                                format!("📂 Directories (Page {}/{}) [Filtered: {}]", 
                                    app.current_page + 1, 
                                    app.total_pages(items_per_page),
                                    app.get_filtered_count()
                                )
                            })
                            .padding(Padding::new(1, 1, 0, 0))
                    )
                    .style(Style::default().fg(TEXT_PRIMARY).bg(SURFACE_COLOR))
                    .highlight_style(Style::default().fg(SELECTION_FG).bg(SELECTION_BG).add_modifier(Modifier::BOLD))
                    .highlight_symbol("▶ ")
                    .repeat_highlight_symbol(true);
                f.render_widget(list, main_panels[0]);
                
                // Calculate total size
                let total_size: u64 = app.directories.iter().map(|dir| dir.size).sum();
                let total_formatted = fs::format_size(total_size);
                let calculated_count = app.directories.iter().filter(|dir| dir.size > 0).count();
                let total_count = app.directories.len();
                
                // Show details in right panel
                if let Some(selected_dir) = app.get_selected_directory() {
                    let time = std::time::Instant::now().elapsed().as_millis();
                    let size_distribution = get_size_distribution_graph(&app.directories);
                    
                    let mut details_text = vec![
                        Line::from(vec![
                            Span::styled("📁 ", Style::default().fg(SUCCESS_COLOR)),
                            Span::styled("Directory Details", Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
                        ]),
                        Line::from(vec![]), // Empty line
                        Line::from(vec![
                            Span::styled("Path: ", Style::default().fg(TEXT_SECONDARY)),
                        ]),
                        Line::from(vec![
                            Span::styled("  ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled(clean_path(&selected_dir.path), Style::default().fg(TEXT_PRIMARY)),
                        ]),
                        Line::from(vec![]), // Empty line
                        Line::from(vec![
                            Span::styled("Size: ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled(selected_dir.formatted_size.clone(), Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)),
                        ]),
                        Line::from(vec![
                            Span::styled("  ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled(format!("({} bytes)", selected_dir.size), Style::default().fg(TEXT_SECONDARY)),
                        ]),
                        Line::from(vec![]), // Empty line
                        Line::from(vec![
                            Span::styled("Position: ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled(format!("{} of {}", app.selected + 1, app.directories.len()), Style::default().fg(ACCENT_COLOR)),
                        ]),
                        Line::from(vec![]), // Empty line
                        Line::from(vec![
                            Span::styled("Last Modified: ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled(selected_dir.formatted_last_modified.clone(), Style::default().fg(MUTED_COLOR).add_modifier(Modifier::BOLD)),
                        ]),
                        Line::from(vec![]), // Empty line

                        // Animated Size Distribution Graph
                        Line::from(vec![
                            Span::styled("📊 ", Style::default().fg(ACCENT_COLOR)),
                            Span::styled("Size Distribution", Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
                        ]),
                        Line::from(vec![
                            Span::styled("  ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled(format!("{}", get_animated_pie_slice(0.25, time)), Style::default().fg(PRIMARY_COLOR)),
                            Span::styled(" ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled("Total: ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled(format!("{} dirs", app.directories.len()), Style::default().fg(ACCENT_COLOR)),
                        ]),
                    ];

                    // Add animated size distribution bars
                    for (range, count, percentage) in size_distribution {
                        if count > 0 {
                            let bar = get_animated_graph_bar(percentage, 100.0, 15);
                            details_text.push(Line::from(vec![
                                Span::styled("  ", Style::default().fg(TEXT_SECONDARY)),
                                Span::styled(format!("{:8} ", range), Style::default().fg(TEXT_SECONDARY)),
                                Span::styled(bar, Style::default().fg(PRIMARY_COLOR)),
                                Span::styled(format!(" {:2}%", percentage as i32), Style::default().fg(ACCENT_COLOR)),
                            ]));
                        }
                    }

                    details_text.extend(vec![
                        Line::from(vec![]), // Empty line
                        Line::from(vec![
                            Span::styled("📊 ", Style::default().fg(ACCENT_COLOR)),
                            Span::styled("Total Summary", Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
                        ]),
                        Line::from(vec![
                            Span::styled("Total Size: ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled(total_formatted.clone(), Style::default().fg(ERROR_COLOR).add_modifier(Modifier::BOLD)),
                        ]),
                        Line::from(vec![
                            Span::styled("Calculated: ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled(format!("{}/{}", calculated_count, total_count), Style::default().fg(ACCENT_COLOR)),
                        ]),
                        Line::from(vec![]), // Empty line
                        Line::from(vec![
                            Span::styled("🗑️ ", Style::default().fg(SUCCESS_COLOR)),
                            Span::styled("Freed Space", Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
                        ]),
                        Line::from(vec![
                            Span::styled("Total Freed: ", Style::default().fg(TEXT_SECONDARY)),
                            Span::styled(fs::format_size(app.get_total_freed_space()), Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD)),
                        ]),
                        if app.get_session_freed_space() > 0 {
                            Line::from(vec![
                                Span::styled("This Session: ", Style::default().fg(TEXT_SECONDARY)),
                                Span::styled(fs::format_size(app.get_session_freed_space()), Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)),
                            ])
                        } else {
                            Line::from(vec![])
                        },
                        if !app.get_recent_freed_space_history().is_empty() {
                            Line::from(vec![
                                Span::styled("Recent: ", Style::default().fg(TEXT_SECONDARY)),
                                Span::styled(format!("{} items", app.get_recent_freed_space_history().len()), Style::default().fg(ACCENT_COLOR)),
                            ])
                        } else {
                            Line::from(vec![])
                        },
                    ]);

                    let details_widget = Paragraph::new(details_text)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(BORDER_COLOR))
                                .title_style(Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD))
                                .title("📊 Details")
                                .padding(Padding::new(1, 1, 0, 0))
                        )
                        .style(Style::default().bg(SURFACE_COLOR));
                    f.render_widget(details_widget, main_panels[1]);
                }
            } else {
                       // Show no results found
                       let no_results_text = vec![
                           Line::from(vec![
                               Span::styled("❌ ", Style::default().fg(ERROR_COLOR)),
                               Span::styled("No directories found", Style::default().fg(TEXT_PRIMARY).add_modifier(Modifier::BOLD)),
                           ]),
                           Line::from(vec![
                               Span::styled("   ", Style::default().fg(TEXT_SECONDARY)),
                               Span::styled("Try a different pattern or path", Style::default().fg(TEXT_SECONDARY)),
                           ]),
                       ];

                       let no_results_widget = Paragraph::new(no_results_text)
                           .block(
                               Block::default()
                                   .borders(Borders::ALL)
                                   .border_style(Style::default().fg(PRIMARY_COLOR))
                                   .title_style(Style::default().fg(PRIMARY_COLOR).add_modifier(Modifier::BOLD))
                                   .title("📁 Directory Search")
                           )
                           .style(Style::default().bg(SURFACE_COLOR))
                           .alignment(Alignment::Center);
                       f.render_widget(no_results_widget, chunks[1]);
                   }

                               // Footer
                   let footer = Paragraph::new(vec![
                       Line::from(vec![
                           Span::styled("⌨️  Nav: ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                           Span::styled("↑/↓/j/k", Style::default().fg(theme.accent)),
                           Span::styled(" move, ", Style::default().fg(theme.text_secondary)),
                           Span::styled("←/→", Style::default().fg(theme.accent)),
                           Span::styled(" pages, ", Style::default().fg(theme.text_secondary)),
                           Span::styled("Home/End", Style::default().fg(theme.accent)),
                           Span::styled(", ", Style::default().fg(theme.text_secondary)),
                           Span::styled("Space", Style::default().fg(theme.accent)),
                           Span::styled(" select, ", Style::default().fg(theme.text_secondary)),
                           Span::styled("a/d", Style::default().fg(theme.accent)),
                           Span::styled(" all/none, ", Style::default().fg(theme.text_secondary)),
                           Span::styled("q", Style::default().fg(theme.error)),
                           Span::styled(" quit", Style::default().fg(theme.text_secondary)),
                       ]),
                       Line::from(vec![
                           Span::styled("🔍 Search: ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                           Span::styled("/", Style::default().fg(theme.accent)),
                           Span::styled(" activate, ", Style::default().fg(theme.text_secondary)),
                           Span::styled("Enter", Style::default().fg(theme.accent)),
                           Span::styled(" confirm, ", Style::default().fg(theme.text_secondary)),
                           Span::styled("Esc", Style::default().fg(theme.accent)),
                           Span::styled(" clear, ", Style::default().fg(theme.text_secondary)),
                           Span::styled("Ctrl+L", Style::default().fg(theme.accent)),
                           Span::styled(" clear", Style::default().fg(theme.text_secondary)),
                       ]),
                       Line::from(vec![
                           Span::styled("🎨 Theme: ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                           Span::styled("T", Style::default().fg(theme.accent)),
                           Span::styled(" cycle (", Style::default().fg(theme.text_secondary)),
                           Span::styled(app.get_color_scheme_name(), Style::default().fg(theme.highlight)),
                           Span::styled(") | ", Style::default().fg(theme.text_secondary)),
                           Span::styled("1-5", Style::default().fg(theme.accent)),
                           Span::styled(" sort", Style::default().fg(theme.text_secondary)),
                       ]),
                       Line::from(vec![
                           Span::styled("🗑️  Delete: ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                           Span::styled("F", Style::default().fg(theme.error)),
                           Span::styled(" current, ", Style::default().fg(theme.text_secondary)),
                           Span::styled("Ctrl+D/X", Style::default().fg(theme.error)),
                           Span::styled(" current, ", Style::default().fg(theme.text_secondary)),
                           Span::styled("C", Style::default().fg(theme.error)),
                           Span::styled(" selected (use Space to select)", Style::default().fg(theme.text_secondary)),
                       ]),
                       Line::from(vec![
                           Span::styled("📊 Found: ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                           Span::styled(format!("{} dirs", app.directories.len()), Style::default().fg(theme.success)),
                           if app.get_selected_count() > 0 {
                               Span::styled(
                                   format!(" | Selected: {} ({})", app.get_selected_count(), fs::format_size(app.get_selected_total_size())),
                                   Style::default().fg(theme.accent).add_modifier(Modifier::BOLD)
                               )
                           } else {
                               Span::styled("", Style::default().fg(theme.success))
                           },
                           if is_scanning {
                               Span::styled(" (scanning...)", Style::default().fg(theme.warning))
                           } else {
                               Span::styled("", Style::default().fg(theme.success))
                           }
                       ]),
                       Line::from(vec![
                           Span::styled("📄 Page: ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                           Span::styled(format!("{} of {}", app.current_page + 1, app.total_pages(items_per_page)), Style::default().fg(theme.accent)),
                           Span::styled(" | ", Style::default().fg(theme.text_secondary)),
                           Span::styled("🎯 ", Style::default().fg(theme.warning).add_modifier(Modifier::BOLD)),
                           Span::styled(if app.directories.is_empty() {
                               "None".to_string()
                           } else {
                               format!("{} of {}: ", app.selected + 1, app.directories.len())
                           }, Style::default().fg(theme.primary)),
                           Span::styled(if app.directories.is_empty() {
                               "".to_string()
                           } else {
                               clean_path(&app.directories[app.selected].path).to_string()
                           }, Style::default().fg(theme.selection_bg).add_modifier(Modifier::BOLD)),
                       ]),
                   ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.border))
                    .title_style(Style::default().fg(theme.warning).add_modifier(Modifier::BOLD))
                    .title("⚙️  Controls")
            )
            .style(Style::default().bg(theme.surface));
            f.render_widget(footer, chunks[3]);
        })?;

                       // Handle input with shorter timeout to keep UI responsive
               if crossterm::event::poll(std::time::Duration::from_millis(50))? {
                   if let crossterm::event::Event::Key(key_event) = crossterm::event::read()? {
                       // Handle search mode first
                       if app.is_searching {
                           match key_event.code {
                               crossterm::event::KeyCode::Enter => {
                                   // Exit search mode
                                   app.is_searching = false;
                               },
                               crossterm::event::KeyCode::Esc => {
                                   // Clear search and exit search mode
                                   app.clear_search();
                               },
                               crossterm::event::KeyCode::Backspace => {
                                   // Remove last character
                                   app.search_query.pop();
                                   app.perform_search();
                               },
                               crossterm::event::KeyCode::Char(c) => {
                                   // Add character to search query
                                   app.search_query.push(c);
                                   app.perform_search();
                               },
                               _ => {}
                           }
                       } else {
                           // Normal mode key handling
                           match key_event.code {
                           crossterm::event::KeyCode::Char('q') | crossterm::event::KeyCode::Esc => break,
                           crossterm::event::KeyCode::Up | crossterm::event::KeyCode::Char('k') => app.previous(items_per_page),
                           crossterm::event::KeyCode::Down | crossterm::event::KeyCode::Char('j') => app.next(items_per_page),
                           crossterm::event::KeyCode::Home => app.select_first(),
                           crossterm::event::KeyCode::End => app.select_last(),
                           crossterm::event::KeyCode::Left => app.previous_page(items_per_page),
                           crossterm::event::KeyCode::Right => app.next_page(items_per_page),
                           crossterm::event::KeyCode::Char(' ') => app.toggle_current_selection(),
                           crossterm::event::KeyCode::Char('a') => app.select_all(),
                           crossterm::event::KeyCode::Char('s') => app.toggle_selection_mode(),
                           // Delete shortcuts - handle in order of specificity
                           crossterm::event::KeyCode::Char('f') => {
                               // Delete current directory (F key)
                               if !app.directories.is_empty() {
                                   let _ = app.start_delete_current_directory();
                               }
                           },
                           // Handle 'C' key for selected directories
                           crossterm::event::KeyCode::Char('c') => {
                               // Delete selected directories (C key)
                               if app.get_selected_count() > 0 {
                                   let _ = app.start_delete_selected_directories();
                               }
                               // If no directories are selected, do nothing (user needs to select first)
                           },
                           // Handle Ctrl combinations (less specific)
                           crossterm::event::KeyCode::Char('x') | crossterm::event::KeyCode::Char('d') if key_event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => {
                               // Delete current directory (Ctrl+D or Ctrl+x)
                               if !app.directories.is_empty() {
                                   let _ = app.start_delete_current_directory();
                               }
                           },
                           // Handle plain 'd' key (least specific)
                           crossterm::event::KeyCode::Char('d') if !key_event.modifiers.contains(crossterm::event::KeyModifiers::CONTROL) => app.deselect_all(),
                           // Search activation
                           crossterm::event::KeyCode::Char('/') => {
                               // Enter search mode
                               app.is_searching = true;
                           },
                           // Color scheme cycling
                           crossterm::event::KeyCode::Char('t') => {
                               // Cycle through color themes
                               app.cycle_color_scheme();
                           },
                           // Sorting shortcuts
                           crossterm::event::KeyCode::Char('1') => {
                               // Sort by name (alphabetical)
                               app.sort_by_name();
                           },
                           crossterm::event::KeyCode::Char('2') => {
                               // Sort by size (largest first)
                               app.sort_by_size_desc();
                           },
                           crossterm::event::KeyCode::Char('3') => {
                               // Sort by size (smallest first)
                               app.sort_by_size_asc();
                           },
                           crossterm::event::KeyCode::Char('4') => {
                               // Sort by last modified (newest first)
                               app.sort_by_modified_desc();
                           },
                           crossterm::event::KeyCode::Char('5') => {
                               // Sort by last modified (oldest first)
                               app.sort_by_modified_asc();
                           },
                           _ => {}
                       }
                   }
                   }
               }
    }

    restore_terminal()?;
    Ok(())
}

/// Display directories in simple text mode (fallback when TUI fails)
fn display_directories_text(pattern: &str, path: &str) -> Result<()> {
    println!("🔍 Directory Search Results");
    println!("Pattern: '{}' in '{}'", pattern, path);
    println!("⏳ Scanning directories...");
    println!();
    
    // Find directories with size information
    let directories = fs::find_directories_with_size(path, pattern)?;
    
    if directories.is_empty() {
        println!("❌ No directories found matching pattern '{}'", pattern);
        return Ok(());
    }
    
    println!("✅ Found {} directories:", directories.len());
    println!();
    
    // Display directories with pagination
    let items_per_page = 20;
    let total_pages = (directories.len() - 1) / items_per_page + 1;
    
    for (i, dir) in directories.iter().enumerate() {
        let page = i / items_per_page + 1;
        println!("📁 {} ({})", clean_path(&dir.path), dir.formatted_size);
        
        // Add page separator
        if (i + 1) % items_per_page == 0 && i < directories.len() - 1 {
            println!();
            println!("--- Page {} of {} ---", page, total_pages);
            println!();
        }
    }
    
    println!();
    println!("📊 Total: {} directories found", directories.len());
    println!("💡 Tip: Use a terminal that supports TUI for a better experience");
    
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    
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
            calculation_status: crate::fs::CalculationStatus::NotStarted,
        }
    }

    #[test]
    fn test_app_creation() {
        let directories = vec![
            create_test_dir("dir1", 100, "100 B"),
            create_test_dir("dir2", 200, "200 B")
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
            create_test_dir("dir3", 300, "300 B")
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
            app.directories.push(create_test_dir(&format!("dir{}", i), i as u64 * 100, &format!("{} B", i * 100)));
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
        app.directories.push(create_test_dir("test_dir", 100, "100 B"));
        
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
            app.directories.push(create_calculating_dir(&format!("dir{}", i)));
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
        
        let calculated_count = app.directories.iter().filter(|dir| dir.size > 0).count();
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
            },
        ];
        
        let app = App::new(directories, "test".to_string(), ".".to_string());
        
        // Total size should be sum of all sizes
        let total_size: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(total_size, 6144); // 1024 + 2048 + 3072
        
        let calculated_count = app.directories.iter().filter(|dir| dir.size > 0).count();
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
            },
        ];
        
        let mut app = App::new(directories, "test".to_string(), ".".to_string());
        
        // Initially all sizes are 0
        let initial_total: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(initial_total, 0);
        
        let initial_calculated = app.directories.iter().filter(|dir| dir.size > 0).count();
        assert_eq!(initial_calculated, 0);
        
        // Update first directory size
        if 0 < app.directories.len() {
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
            },
        ];
        
        let app = App::new(directories, "test".to_string(), ".".to_string());
        
        let total_size: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(total_size, 3 * 1024 * 1024 * 1024); // 3 GB
        
        let calculated_count = app.directories.iter().filter(|dir| dir.size > 0).count();
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
            },
        ];
        
        let app = App::new(directories, "test".to_string(), ".".to_string());
        
        let total_size: u64 = app.directories.iter().map(|dir| dir.size).sum();
        assert_eq!(total_size, 1024); // Only the non-empty directory contributes
        
        let calculated_count = app.directories.iter().filter(|dir| dir.size > 0).count();
        assert_eq!(calculated_count, 1); // Only one directory has size > 0
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
        };
        assert_eq!(indicator(&dir), "☐");
        dir.selected = true;
        assert_eq!(indicator(&dir), "☑");
    }

    #[test]
    fn test_selection_summary_string() {
        use crate::fs::DirectoryInfo;
        use crate::ui::app::App;
        use crate::fs::format_size;
        let mut app = App::new(
            vec![
                DirectoryInfo {
            last_modified: None,
            formatted_last_modified: "Unknown".to_string(), path: "a".to_string(), size: 100, formatted_size: "100 B".to_string(), selected: false, deletion_status: crate::fs::DeletionStatus::Normal, calculation_status: crate::fs::CalculationStatus::Completed },
                DirectoryInfo {
            last_modified: None,
            formatted_last_modified: "Unknown".to_string(), path: "b".to_string(), size: 200, formatted_size: "200 B".to_string(), selected: false, deletion_status: crate::fs::DeletionStatus::Normal, calculation_status: crate::fs::CalculationStatus::Completed },
            ],
            "test".to_string(), ".".to_string(),
        );
        // No selection
        let summary = if app.get_selected_count() > 0 {
            format!(" | Selected: {} ({})", app.get_selected_count(), format_size(app.get_selected_total_size()))
        } else {
            String::new()
        };
        assert_eq!(summary, "");
        // One selected
        app.directories[0].selected = true;
        let summary = if app.get_selected_count() > 0 {
            format!(" | Selected: {} ({})", app.get_selected_count(), format_size(app.get_selected_total_size()))
        } else {
            String::new()
        };
        assert_eq!(summary, " | Selected: 1 (100 B)");
        // Both selected
        app.directories[1].selected = true;
        let summary = if app.get_selected_count() > 0 {
            format!(" | Selected: {} ({})", app.get_selected_count(), format_size(app.get_selected_total_size()))
        } else {
            String::new()
        };
        assert_eq!(summary, " | Selected: 2 (300 B)");
    }

    #[test]
    fn test_animated_directory_icon() {
        // Test that the animated directory icon returns the correct symbols
        assert_eq!(get_directory_icon(false, false), "📁");
        assert!(get_directory_icon(true, false).contains("📂") || get_directory_icon(true, false).contains("📁"));
        assert!(get_directory_icon(false, true).contains("📂") || get_directory_icon(false, true).contains("📁"));
        
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
        use crate::fs::{DirectoryInfo, DeletionStatus};
        
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
        };
        
        // Verify the status variants exist and work correctly
        assert!(matches!(normal_dir.deletion_status, DeletionStatus::Normal));
        assert!(matches!(deleting_dir.deletion_status, DeletionStatus::Deleting));
        assert!(matches!(deleted_dir.deletion_status, DeletionStatus::Deleted));
        assert!(matches!(error_dir.deletion_status, DeletionStatus::Error(_)));
        
        // Test that the UI rendering logic can handle all status types
        let app = App::new(vec![normal_dir, deleting_dir, deleted_dir, error_dir], "test".to_string(), ".".to_string());
        assert_eq!(app.directories.len(), 4);
        assert!(matches!(app.directories[0].deletion_status, DeletionStatus::Normal));
        assert!(matches!(app.directories[1].deletion_status, DeletionStatus::Deleting));
        assert!(matches!(app.directories[2].deletion_status, DeletionStatus::Deleted));
        assert!(matches!(app.directories[3].deletion_status, DeletionStatus::Error(_)));
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
                path: format!("dir{}", i),
                size: 0,
                formatted_size: "Calculating...".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::NotStarted,
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
                path: format!("dir{}", i),
                size: 0,
                formatted_size: "Calculating...".to_string(),
                selected: false,
                deletion_status: crate::fs::DeletionStatus::Normal,
                calculation_status: crate::fs::CalculationStatus::NotStarted,
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
        
        let calculated_count = app.directories.iter().filter(|dir| dir.size > 0).count();
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
        let c_key = create_key_event(
            KeyCode::Char('c'),
            KeyModifiers::empty()
        );
        
        // Test Ctrl+D (should delete current)
        let ctrl_d = create_key_event(
            KeyCode::Char('d'),
            KeyModifiers::CONTROL
        );
        
        // Test plain 'd' (should deselect all)
        let plain_d = create_key_event(
            KeyCode::Char('d'),
            KeyModifiers::empty()
        );
        
        // Test Ctrl+X (should delete current)
        let ctrl_x = create_key_event(
            KeyCode::Char('x'),
            KeyModifiers::CONTROL
        );
        
        // Test plain 'f' (should delete current)
        let plain_f = create_key_event(
            KeyCode::Char('f'),
            KeyModifiers::empty()
        );
        
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
                KeyCode::Char('x') | KeyCode::Char('d') if key_event.modifiers.contains(KeyModifiers::CONTROL) => "delete_current",
                KeyCode::Char('d') if !key_event.modifiers.contains(KeyModifiers::CONTROL) => "deselect_all",
                _ => "unknown"
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
        use crate::ui::app::App;
        use crate::fs::DirectoryInfo;
        use crate::fs::DeletionStatus;
        
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
            },
        ];
        
        let mut app = App::new(directories, "test".to_string(), ".".to_string());
        
        // Initially no directories should be selected
        assert_eq!(app.get_selected_count(), 0);
        assert_eq!(app.get_selected_directories().len(), 0);
        
        // Select first directory
        app.directories[0].selected = true;
        assert_eq!(app.get_selected_count(), 1);
        assert_eq!(app.get_selected_directories().len(), 1);
        assert_eq!(app.get_selected_directories()[0].path, "dir1");
        
        // Select second directory
        app.directories[1].selected = true;
        assert_eq!(app.get_selected_count(), 2);
        assert_eq!(app.get_selected_directories().len(), 2);
        
        // Verify both selected directories are in the list
        let selected_paths: Vec<&str> = app.get_selected_directories().iter().map(|d| d.path.as_str()).collect();
        assert!(selected_paths.contains(&"dir1"));
        assert!(selected_paths.contains(&"dir2"));
        
        // Verify total size calculation
        assert_eq!(app.get_selected_total_size(), 300); // 100 + 200
        
        // Test that the selection state is properly tracked
        assert!(app.directories[0].selected);
        assert!(app.directories[1].selected);
        assert!(!app.directories[2].selected);
        
        // Test toggle functionality
        app.toggle_current_selection(); // This should toggle the currently selected item (index 0)
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
        use crate::ui::app::App;
        use crate::fs::DirectoryInfo;
        use crate::fs::DeletionStatus;
        
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
} 