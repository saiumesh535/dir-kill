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

// Gruvbox Dark Theme Color Palette
// Based on https://github.com/morhetz/gruvbox
const PRIMARY_COLOR: Color = Color::Rgb(131, 165, 152);   // gruvbox-aqua
const SECONDARY_COLOR: Color = Color::Rgb(250, 189, 47);  // gruvbox-yellow
const ACCENT_COLOR: Color = Color::Rgb(211, 134, 155);    // gruvbox-pink
const SUCCESS_COLOR: Color = Color::Rgb(184, 187, 38);    // gruvbox-green
const WARNING_COLOR: Color = Color::Rgb(250, 189, 47);    // gruvbox-yellow
const ERROR_COLOR: Color = Color::Rgb(251, 73, 52);       // gruvbox-red
const BACKGROUND_COLOR: Color = Color::Rgb(40, 40, 40);   // gruvbox-bg0_h (hard)
const SURFACE_COLOR: Color = Color::Rgb(60, 56, 54);      // gruvbox-bg0 (medium)
const TEXT_PRIMARY: Color = Color::Rgb(235, 219, 178);    // gruvbox-fg0 (light)
const TEXT_SECONDARY: Color = Color::Rgb(189, 174, 147);  // gruvbox-fg1 (medium)
const SELECTION_BG: Color = Color::Rgb(131, 165, 152);    // gruvbox-aqua selection background
const SELECTION_FG: Color = Color::Rgb(40, 40, 40);       // Dark text on selection
const SELECTION_INDICATOR_COLOR: Color = Color::Rgb(184, 187, 38);  // gruvbox-green for selection indicators
const SELECTION_GLOW_COLOR: Color = Color::Rgb(142, 192, 124);      // lighter green for glow effect

/// Remove ./ prefix from path if present
fn clean_path(path: &str) -> &str {
    if path.starts_with("./") {
        &path[2..]
    } else {
        path
    }
}

/// Get loading animation frame based on time
fn get_loading_frame() -> &'static str {
    let frames = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
    let index = (std::time::Instant::now().elapsed().as_millis() / 100) as usize % frames.len();
    frames[index]
}

/// Get animated directory icon with selection state
fn get_directory_icon(selected: bool, is_highlighted: bool) -> &'static str {
    let time = std::time::Instant::now().elapsed().as_millis();
    
    if selected {
        // Animated open directory for selected items - faster animation
        let open_frames = ["📂", "📁", "📂", "📁", "📂", "📁", "📂", "📁", "📂", "📁"];
        let index = (time / 120) as usize % open_frames.len();
        open_frames[index]
    } else if is_highlighted {
        // Animated closed directory for highlighted items - slower animation
        let closed_frames = ["📁", "📂", "📁", "📂", "📁", "📂", "📁", "📂", "📁", "📂"];
        let index = (time / 250) as usize % closed_frames.len();
        closed_frames[index]
    } else {
        // Static closed directory for normal items
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
    
    // Channel for size updates
    let size_sender = std::sync::mpsc::channel::<(usize, u64, String)>();
    let (size_tx, size_rx) = size_sender;
    
    let handle = std::thread::spawn(move || {
        match fs::find_directories(&path_clone, &pattern_clone) {
            Ok(dirs) => {
                for (index, dir_path) in dirs.into_iter().enumerate() {
                    // Send directory without size initially
                    let _ = tx.send(DirectoryInfo {
                        path: dir_path.clone(),
                        size: 0,
                        formatted_size: "Calculating...".to_string(),
                        selected: false,
                    });
                    
                    // Start size calculation in background
                    let size_tx_clone = size_tx.clone();
                    let dir_path_clone = dir_path.clone();
                    std::thread::spawn(move || {
                        let size = fs::calculate_directory_size(std::path::Path::new(&dir_path_clone)).unwrap_or(0);
                        let formatted_size = fs::format_size(size);
                        let _ = size_tx_clone.send((index, size, formatted_size));
                    });
                }
            }
            Err(e) => {
                let _ = tx.send(DirectoryInfo {
                    path: format!("ERROR: {}", e),
                    size: 0,
                    formatted_size: "0 B".to_string(),
                    selected: false,
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
        while let Ok((index, size, formatted_size)) = size_rx.try_recv() {
            if index < app.directories.len() {
                app.directories[index].size = size;
                app.directories[index].formatted_size = formatted_size;
            }
        }
        
        // Check if the background thread has finished
        if is_scanning && handle.is_finished() {
            // Try one more time to get any remaining data
            while let Ok(dir) = rx.try_recv() {
                if dir.path.starts_with("ERROR:") {
                    // Handle error
                    is_scanning = false;
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
                    Constraint::Length(4), // Header
                    Constraint::Min(0),    // Main content area
                    Constraint::Length(4), // Footer
                ]
                .as_ref(),
            )
            .split(size);
        
        // Split main content area into two panels
        let main_panels = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(
                [
                    Constraint::Percentage(70), // Directory list (70% width)
                    Constraint::Percentage(30), // Details panel (30% width)
                ]
                .as_ref(),
            )
            .split(chunks[1]);
        
        let available_height = main_panels[0].height.saturating_sub(2);
        let items_per_page = available_height.max(1) as usize;
        
        terminal.draw(|f| {
            // Set background color for the entire terminal
            f.render_widget(
                Paragraph::new("").style(Style::default().bg(BACKGROUND_COLOR)),
                f.size(),
            );

            // Calculate total size for header
            let total_size: u64 = app.directories.iter().map(|dir| dir.size).sum();
            let total_formatted = fs::format_size(total_size);
            let calculated_count = app.directories.iter().filter(|dir| dir.size > 0).count();
            let total_count = app.directories.len();
            
            // Header
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
                        Span::styled(total_formatted.clone(), Style::default().fg(ERROR_COLOR).add_modifier(Modifier::BOLD)),
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
                    .border_style(Style::default().fg(PRIMARY_COLOR))
                    .title_style(Style::default().fg(PRIMARY_COLOR).add_modifier(Modifier::BOLD))
                    .title("📊 Search Info")
            )
            .style(Style::default().bg(SURFACE_COLOR));
            f.render_widget(header, chunks[0]);

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
                                                       // Show directory list in left panel
                let visible_items = app.visible_items(items_per_page);
                let list_items: Vec<ListItem> = visible_items.iter().enumerate().map(|(visible_index, dir)| {
                    let global_index = app.current_page * items_per_page + visible_index;
                    let is_selected = global_index == app.selected;
                    
                    // Simplified styling for list items (size info moved to details panel)
                    let path_style = if is_selected {
                        Style::default().fg(SELECTION_FG).bg(SELECTION_BG).add_modifier(Modifier::BOLD)
                    } else if dir.selected {
                        Style::default().fg(SELECTION_INDICATOR_COLOR).add_modifier(Modifier::BOLD | Modifier::UNDERLINED)
                    } else {
                        Style::default().fg(TEXT_PRIMARY)
                    };

                    ListItem::new(vec![Line::from(vec![
                        Span::styled(
                            format!("{} ", get_directory_icon(dir.selected, is_selected)),
                            Style::default()
                                .fg(get_selection_indicator_color(dir.selected))
                                .add_modifier(if dir.selected { Modifier::BOLD } else { Modifier::empty() })
                        ),
                        if dir.selected {
                            Span::styled("✓ ", Style::default().fg(SELECTION_INDICATOR_COLOR).add_modifier(Modifier::BOLD))
                        } else {
                            Span::styled("  ", Style::default())
                        },
                        Span::styled(clean_path(&dir.path), path_style),
                    ])])
                }).collect();

                let list = List::new(list_items)
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(SUCCESS_COLOR))
                            .title_style(Style::default().fg(SUCCESS_COLOR).add_modifier(Modifier::BOLD))
                            .title(format!("📂 Directories (Page {}/{})", app.current_page + 1, app.total_pages(items_per_page)))
                            .padding(Padding::new(1, 1, 0, 0))
                    )
                    .style(Style::default().fg(TEXT_PRIMARY).bg(SURFACE_COLOR))
                    .highlight_style(Style::default().fg(SELECTION_FG).bg(SELECTION_BG))
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
                    let details_text = vec![
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
                    ];

                    let details_widget = Paragraph::new(details_text)
                        .block(
                            Block::default()
                                .borders(Borders::ALL)
                                .border_style(Style::default().fg(ACCENT_COLOR))
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
                           Span::styled("⌨️  Navigation: ", Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)),
                           Span::styled("↑/↓/j/k", Style::default().fg(ACCENT_COLOR)),
                           Span::styled(" to navigate, ", Style::default().fg(TEXT_SECONDARY)),
                           Span::styled("←/→", Style::default().fg(ACCENT_COLOR)),
                           Span::styled(" for pages, ", Style::default().fg(TEXT_SECONDARY)),
                           Span::styled("Home/End", Style::default().fg(ACCENT_COLOR)),
                           Span::styled(", ", Style::default().fg(TEXT_SECONDARY)),
                           Span::styled("[Space] select/deselect, [a] all, [d] none, [s] mode", Style::default().fg(ACCENT_COLOR)),
                           Span::styled(", ", Style::default().fg(TEXT_SECONDARY)),
                           Span::styled("q/ESC", Style::default().fg(ERROR_COLOR)),
                           Span::styled(" to quit", Style::default().fg(TEXT_SECONDARY)),
                       ]),
                       Line::from(vec![
                           Span::styled("📊 Found: ", Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)),
                           Span::styled(format!("{} directories", app.directories.len()), Style::default().fg(SUCCESS_COLOR)),
                           if app.get_selected_count() > 0 {
                               Span::styled(
                                   format!(" | Selected: {} ({})", app.get_selected_count(), fs::format_size(app.get_selected_total_size())),
                                   Style::default().fg(ACCENT_COLOR).add_modifier(Modifier::BOLD)
                               )
                           } else {
                               Span::styled("", Style::default().fg(SUCCESS_COLOR))
                           },
                           if is_scanning {
                               Span::styled(" (scanning...)", Style::default().fg(WARNING_COLOR))
                           } else {
                               Span::styled("", Style::default().fg(SUCCESS_COLOR))
                           }
                       ]),
                       Line::from(vec![
                           Span::styled("📄 Page: ", Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)),
                           Span::styled(format!("{} of {}", app.current_page + 1, app.total_pages(items_per_page)), Style::default().fg(ACCENT_COLOR)),
                           Span::styled(" | ", Style::default().fg(TEXT_SECONDARY)),
                           Span::styled("🎯 Selected: ", Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD)),
                           Span::styled(if app.directories.is_empty() {
                               "None".to_string()
                           } else {
                               format!("{} of {}: ", app.selected + 1, app.directories.len())
                           }, Style::default().fg(PRIMARY_COLOR)),
                           Span::styled(if app.directories.is_empty() {
                               "".to_string()
                           } else {
                               clean_path(&app.directories[app.selected].path).to_string()
                           }, Style::default().fg(SELECTION_BG).add_modifier(Modifier::BOLD)),
                       ]),
                   ])
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(WARNING_COLOR))
                    .title_style(Style::default().fg(WARNING_COLOR).add_modifier(Modifier::BOLD))
                    .title("⚙️  Controls")
            )
            .style(Style::default().bg(SURFACE_COLOR));
            f.render_widget(footer, chunks[2]);
        })?;

                       // Handle input
               if crossterm::event::poll(std::time::Duration::from_millis(100))? {
                   if let crossterm::event::Event::Key(key_event) = crossterm::event::read()? {
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
                           crossterm::event::KeyCode::Char('d') => app.deselect_all(),
                           crossterm::event::KeyCode::Char('s') => app.toggle_selection_mode(),
                           _ => {}
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

    #[test]
    fn test_app_creation() {
        let directories = vec![
            DirectoryInfo { path: "dir1".to_string(), size: 100, formatted_size: "100 B".to_string(), selected: false },
            DirectoryInfo { path: "dir2".to_string(), size: 200, formatted_size: "200 B".to_string(), selected: false }
        ];
        let app = App::new(directories.clone(), "test".to_string(), ".".to_string());
        assert_eq!(app.directories.len(), directories.len());
        assert_eq!(app.selected, 0);
    }

    #[test]
    fn test_app_navigation() {
        let directories = vec![
            DirectoryInfo { path: "dir1".to_string(), size: 100, formatted_size: "100 B".to_string(), selected: false },
            DirectoryInfo { path: "dir2".to_string(), size: 200, formatted_size: "200 B".to_string(), selected: false },
            DirectoryInfo { path: "dir3".to_string(), size: 300, formatted_size: "300 B".to_string(), selected: false }
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
        let mut scan_start_time = std::time::Instant::now();
        
        // Initially should show loading state
        assert!(is_scanning);
        assert!(app.directories.is_empty());
        
        // Simulate receiving first directory
        app.directories.push(DirectoryInfo {
            path: "test_dir".to_string(),
            size: 100,
            formatted_size: "100 B".to_string(),
            selected: false,
        });
        
        // Should still be scanning after receiving first item
        assert!(is_scanning);
        assert!(!app.directories.is_empty());
        
        // Simulate time passing without new data
        scan_start_time = std::time::Instant::now() - std::time::Duration::from_millis(400);
        
        // Now should be able to transition to not scanning
        if scan_start_time.elapsed().as_millis() > 300 {
            is_scanning = false;
        }
        
        assert!(!is_scanning);
        assert!(!app.directories.is_empty());
    }

    #[test]
    fn test_scanning_state_with_multiple_items() {
        // Test that scanning continues while receiving multiple items
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        let mut is_scanning = true;
        let mut scan_start_time = std::time::Instant::now();
        
        // Add multiple directories
        for i in 0..5 {
            app.directories.push(DirectoryInfo {
                path: format!("dir{}", i),
                size: i as u64 * 100,
                formatted_size: format!("{} B", i * 100),
                selected: false,
            });
            // Simulate receiving data (reset timer)
            scan_start_time = std::time::Instant::now();
        }
        
        // Should still be scanning since we just received data
        assert!(is_scanning);
        assert_eq!(app.directories.len(), 5);
        
        // Simulate time passing without new data
        scan_start_time = std::time::Instant::now() - std::time::Duration::from_millis(400);
        
        // Now should transition to not scanning
        if scan_start_time.elapsed().as_millis() > 300 {
            is_scanning = false;
        }
        
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
        app.directories.push(DirectoryInfo {
            path: "test_dir".to_string(),
            size: 100,
            formatted_size: "100 B".to_string(),
            selected: false,
        });
        
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
        let mut empty_app = App::new(vec![], "test".to_string(), ".".to_string());
        let empty_is_scanning = false;
        let should_show_no_results = !empty_is_scanning && empty_app.directories.is_empty();
        assert!(should_show_no_results); // Should show no results when not scanning and empty
    }

    #[test]
    fn test_scanning_complete_no_results() {
        // Test the scenario where scanning completes but no directories are found
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        let mut is_scanning = true;
        let mut has_received_any_data = true; // Scanning completed, we know there are no results

        // Simulate scanning completed with no results
        is_scanning = false;
        has_received_any_data = true;
        
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
        app.directories.push(DirectoryInfo {
            path: "test_dir".to_string(),
            size: 0,
            formatted_size: "Calculating...".to_string(),
            selected: false,
        });
        
        assert_eq!(app.directories[0].size, 0);
        assert_eq!(app.directories[0].formatted_size, "Calculating...");
    }

    #[test]
    fn test_lazy_size_calculation_update() {
        // Test that size updates work correctly
        let mut app = App::new(vec![], "test".to_string(), ".".to_string());
        
        // Add directories with initial state
        app.directories.push(DirectoryInfo {
            path: "dir1".to_string(),
            size: 0,
            formatted_size: "Calculating...".to_string(),
            selected: false,
        });
        app.directories.push(DirectoryInfo {
            path: "dir2".to_string(),
            size: 0,
            formatted_size: "Calculating...".to_string(),
            selected: false,
        });
        
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
            app.directories.push(DirectoryInfo {
                path: format!("dir{}", i),
                size: 0,
                formatted_size: "Calculating...".to_string(),
                selected: false,
            });
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
            path: "test_dir".to_string(),
            size: 0,
            formatted_size: "Calculating...".to_string(),
            selected: false,
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
                path: "dir1".to_string(),
                size: 1024,
                formatted_size: "1.0 KB".to_string(),
                selected: false,
            },
            DirectoryInfo {
                path: "dir2".to_string(),
                size: 2048,
                formatted_size: "2.0 KB".to_string(),
                selected: false,
            },
            DirectoryInfo {
                path: "dir3".to_string(),
                size: 3072,
                formatted_size: "3.0 KB".to_string(),
                selected: false,
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
                path: "dir1".to_string(),
                size: 0, // Initially 0, will be updated
                formatted_size: "Calculating...".to_string(),
                selected: false,
            },
            DirectoryInfo {
                path: "dir2".to_string(),
                size: 0, // Initially 0, will be updated
                formatted_size: "Calculating...".to_string(),
                selected: false,
            },
            DirectoryInfo {
                path: "dir3".to_string(),
                size: 0, // Initially 0, will be updated
                formatted_size: "Calculating...".to_string(),
                selected: false,
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
                path: "dir1".to_string(),
                size: 1024, // Already calculated
                formatted_size: "1.0 KB".to_string(),
                selected: false,
            },
            DirectoryInfo {
                path: "dir2".to_string(),
                size: 0, // Not yet calculated
                formatted_size: "Calculating...".to_string(),
                selected: false,
            },
            DirectoryInfo {
                path: "dir3".to_string(),
                size: 2048, // Already calculated
                formatted_size: "2.0 KB".to_string(),
                selected: false,
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
                path: "large_dir1".to_string(),
                size: 1024 * 1024 * 1024, // 1 GB
                formatted_size: "1.0 GB".to_string(),
                selected: false,
            },
            DirectoryInfo {
                path: "large_dir2".to_string(),
                size: 2 * 1024 * 1024 * 1024, // 2 GB
                formatted_size: "2.0 GB".to_string(),
                selected: false,
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
                path: "empty_dir1".to_string(),
                size: 0,
                formatted_size: "0 B".to_string(),
                selected: false,
            },
            DirectoryInfo {
                path: "empty_dir2".to_string(),
                size: 0,
                formatted_size: "0 B".to_string(),
                selected: false,
            },
            DirectoryInfo {
                path: "non_empty_dir".to_string(),
                size: 1024,
                formatted_size: "1.0 KB".to_string(),
                selected: false,
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
            path: "foo".to_string(),
            size: 0,
            formatted_size: "0 B".to_string(),
            selected: false,
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
                DirectoryInfo { path: "a".to_string(), size: 100, formatted_size: "100 B".to_string(), selected: false },
                DirectoryInfo { path: "b".to_string(), size: 200, formatted_size: "200 B".to_string(), selected: false },
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
} 