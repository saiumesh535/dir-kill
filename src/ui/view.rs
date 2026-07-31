use crate::fs::DirectoryInfo;
use crate::ui::app::{App, SortColumn, SortDirection};
use std::borrow::Cow;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Cell, Clear, Padding, Paragraph, Row, Table, Wrap},
    Frame,
};

// Semantic roles (+ neutrals). Max four accents.
const FOCUS: Color = Color::Rgb(131, 165, 152); // aqua — brand + focus glyph
const VALUE: Color = Color::Rgb(250, 189, 47); // warm — sizes + hero bytes
const MARK: Color = Color::Rgb(184, 187, 38); // green — checked / selection CTA
const DANGER: Color = Color::Rgb(251, 73, 52); // red — delete only
const BG: Color = Color::Rgb(29, 32, 33);
const PANEL: Color = Color::Rgb(40, 40, 40);
const TEXT: Color = Color::Rgb(235, 219, 178);
const MUTED: Color = Color::Rgb(146, 131, 116);
const BORDER: Color = Color::Rgb(80, 73, 69);
/// Soft desaturated wash — not a mid-teal brick
const SEL_BG: Color = Color::Rgb(50, 58, 56);
const VALUE_DIM: Color = Color::Rgb(168, 153, 80); // quieter Value for lower ranks

const HEADER_HEIGHT: u16 = 2;
const FOOTER_HEIGHT: u16 = 1;

pub struct RenderContext<'a> {
    pub pattern: &'a str,
    pub search_root: &'a str,
    pub items_per_page: usize,
}

fn root_chunks(viewport: Rect) -> Vec<Rect> {
    Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(HEADER_HEIGHT),
            Constraint::Min(4),
            Constraint::Length(FOOTER_HEIGHT),
        ])
        .split(viewport)
        .to_vec()
}

pub fn table_area_for_viewport(viewport: Rect, show_details_panel: bool) -> Rect {
    let root = root_chunks(viewport);
    if show_details_panel {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
            .split(root[1])[0]
    } else {
        root[1]
    }
}

pub fn items_per_page_for_viewport(viewport: Rect, show_details_panel: bool) -> usize {
    items_per_page_for_area(table_area_for_viewport(viewport, show_details_panel))
}

/// Column header (1) + top hairline (1).
pub fn items_per_page_for_area(table_area: Rect) -> usize {
    table_area.height.saturating_sub(2).max(1) as usize
}

pub fn render(f: &mut Frame, app: &mut App, ctx: &mut RenderContext) {
    f.render_widget(Paragraph::new("").style(Style::default().bg(BG)), f.area());

    let root = root_chunks(f.area());

    if app.show_details_panel {
        let cols = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(72), Constraint::Percentage(28)])
            .split(root[1]);
        ctx.items_per_page = items_per_page_for_area(cols[0]);
        app.items_per_page = ctx.items_per_page;
        app.clamp_pagination();
        render_header(f, app, ctx, root[0]);
        render_table(f, app, ctx, cols[0]);
        render_details_panel(f, app, cols[1]);
    } else {
        ctx.items_per_page = items_per_page_for_area(root[1]);
        app.items_per_page = ctx.items_per_page;
        app.clamp_pagination();
        render_header(f, app, ctx, root[0]);
        render_table(f, app, ctx, root[1]);
    }

    render_footer(f, app, ctx, root[2]);

    if app.show_help {
        render_help_overlay(f);
    }
    if app.delete_confirmation.is_some() {
        render_delete_confirm(f, app);
    }
    if app.filter_input_active {
        render_filter_overlay(f, app);
    }
}

fn render_header(f: &mut Frame, app: &App, ctx: &RenderContext, area: Rect) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Length(1)])
        .split(area);

    // L1: brand (Focus) + muted pattern + muted root
    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " dir-kill ",
                Style::default().fg(FOCUS).add_modifier(Modifier::BOLD),
            ),
            Span::styled(ctx.pattern.to_string(), Style::default().fg(MUTED)),
            Span::raw("  "),
            Span::styled(ctx.search_root.to_string(), Style::default().fg(MUTED)),
        ]))
        .style(Style::default().bg(BG)),
        rows[0],
    );

    // L2: context left · hero bytes right (never hidden when sizes exist)
    let (left, hero) = header_metrics(app);
    let left_span = if let Some(toast) = app.active_toast_message() {
        Span::styled(
            format!(" {toast}"),
            Style::default().fg(MARK).add_modifier(Modifier::BOLD),
        )
    } else if let Some(label) = app.deletion_status_label() {
        Span::styled(
            format!(" {label}"),
            Style::default().fg(VALUE).add_modifier(Modifier::BOLD),
        )
    } else if let Some(filter) = app.filter_status_label() {
        Span::styled(format!(" {filter}"), Style::default().fg(MUTED))
    } else if app.get_total_freed_space() > 0 {
        Span::styled(
            format!(
                " freed {}",
                crate::fs::format_size(app.get_total_freed_space())
            ),
            Style::default().fg(MARK).add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(format!(" {left}"), Style::default().fg(TEXT))
    };

    let mut spans = vec![left_span];
    if !hero.is_empty() {
        let left_w: usize = spans.iter().map(|s| s.content.chars().count()).sum();
        let pad = (area.width as usize)
            .saturating_sub(left_w)
            .saturating_sub(hero.chars().count())
            .saturating_sub(1);
        spans.push(Span::raw(" ".repeat(pad.max(2))));
        spans.push(Span::styled(
            format!("{hero} "),
            Style::default().fg(VALUE).add_modifier(Modifier::BOLD),
        ));
    }

    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(BG)),
        rows[1],
    );
}

/// Left = dirs/timing context; right = hero bytes (selected sum or releasable).
fn header_metrics(app: &App) -> (String, String) {
    let total = app.directories.len();
    let calculated = app.cached_calculated_count;
    let timing = app.format_status_timing_label();
    let selected = app.get_selected_count();

    let left = match &app.discovery_status {
        crate::ui::app::DiscoveryStatus::Discovering if app.total_discovered == 0 => {
            format!("scanning… · {timing}")
        }
        crate::ui::app::DiscoveryStatus::Discovering => {
            format!("{} dirs · {timing}", app.total_discovered)
        }
        crate::ui::app::DiscoveryStatus::Complete if total == 0 => {
            format!("0 dirs · {timing}")
        }
        crate::ui::app::DiscoveryStatus::Complete => format!("{total} dirs · {timing}"),
        crate::ui::app::DiscoveryStatus::Error(err) => format!("error: {err}"),
        crate::ui::app::DiscoveryStatus::NotStarted => "ready".to_string(),
    };

    let hero = if selected > 0 {
        format!(
            "{} sel · {}",
            selected,
            crate::fs::format_size(app.get_selected_total_size())
        )
    } else if total == 0 {
        String::new()
    } else if calculated < total {
        format!(
            "~{} · sizing {calculated}/{total}",
            app.cached_total_formatted
        )
    } else {
        format!("~{} releasable", app.cached_total_formatted)
    };

    (left, hero)
}

fn render_footer(f: &mut Frame, app: &App, ctx: &RenderContext, area: Rect) {
    let total_pages = app.total_pages(ctx.items_per_page).max(1);
    let selected = app.get_selected_count();

    let mut spans = vec![
        Span::styled(" ↑↓", Style::default().fg(FOCUS).add_modifier(Modifier::BOLD)),
        Span::styled("  ", Style::default()),
        Span::styled("space", Style::default().fg(FOCUS).add_modifier(Modifier::BOLD)),
        Span::styled("  ", Style::default()),
    ];

    if selected > 0 {
        spans.extend([
            Span::styled("f", Style::default().fg(DANGER).add_modifier(Modifier::BOLD)),
            Span::styled("/", Style::default().fg(MUTED)),
            Span::styled("c", Style::default().fg(DANGER).add_modifier(Modifier::BOLD)),
            Span::styled(" delete  ", Style::default().fg(MUTED)),
        ]);
    } else {
        spans.extend([
            Span::styled("f", Style::default().fg(DANGER).add_modifier(Modifier::BOLD)),
            Span::styled(" delete  ", Style::default().fg(MUTED)),
        ]);
    }

    spans.extend([
        Span::styled("?", Style::default().fg(FOCUS).add_modifier(Modifier::BOLD)),
        Span::styled("  ", Style::default()),
        Span::styled("q", Style::default().fg(MUTED).add_modifier(Modifier::BOLD)),
        Span::styled(" quit", Style::default().fg(MUTED)),
    ]);

    // Right: sort · page · selection CTA
    let mut right = app.sort_label();
    if total_pages > 1 {
        right = format!(
            "{right} · {}/{}",
            app.current_page + 1,
            total_pages
        );
    }
    if selected > 0 {
        right = format!(
            "{right} · {} sel · {}",
            selected,
            crate::fs::format_size(app.get_selected_total_size())
        );
    }

    let left_w: usize = spans.iter().map(|s| s.content.chars().count()).sum();
    let pad = (area.width as usize)
        .saturating_sub(left_w)
        .saturating_sub(right.chars().count())
        .saturating_sub(1);
    spans.push(Span::raw(" ".repeat(pad.max(2))));
    spans.push(Span::styled(
        format!("{right} "),
        if selected > 0 {
            Style::default().fg(MARK).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(MUTED)
        },
    ));

    f.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(BG)),
        area,
    );
}

fn render_table(f: &mut Frame, app: &App, ctx: &RenderContext, area: Rect) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(BG));

    if app.is_discovering() && app.view_len() == 0 {
        f.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled(
                    "Scanning…",
                    Style::default().fg(VALUE).add_modifier(Modifier::BOLD),
                )),
                Line::from(Span::styled(
                    format!("Searching for “{}”", ctx.pattern),
                    Style::default().fg(MUTED),
                )),
            ])
            .alignment(Alignment::Center)
            .block(block),
            area,
        );
        return;
    }

    if !app.is_discovering() && app.view_len() == 0 {
        let empty_owned;
        let msg = if app.is_filtering() {
            "No filter matches — / to edit · Esc to clear"
        } else {
            empty_owned = format!("No “{}” directories found", ctx.pattern);
            empty_owned.as_str()
        };
        f.render_widget(
            Paragraph::new(Span::styled(msg, Style::default().fg(MUTED)))
                .alignment(Alignment::Center)
                .block(block),
            area,
        );
        return;
    }

    let page_start = app.current_page * ctx.items_per_page;
    let page_end = (page_start + ctx.items_per_page).min(app.display_indices.len());
    let view_len = app.view_len().max(1);

    let header = Row::new(vec![
        Cell::from(" "),
        Cell::from(" "),
        Cell::from(sort_label("SIZE", SortColumn::Size, app)),
        Cell::from(sort_label("PATH", SortColumn::Path, app)),
    ])
    .style(Style::default().fg(MUTED));

    let rows: Vec<Row> = app.display_indices[page_start..page_end]
        .iter()
        .enumerate()
        .filter_map(|(i, &idx)| {
            let dir = app.directories.get(idx)?;
            let focused = page_start + i == app.selected;
            let rank = page_start + i;
            let quiet = rank * 2 >= view_len;
            Some(row_for_directory(dir, ctx.pattern, focused, quiet))
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(10),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .block(block)
    .column_spacing(1)
    .row_highlight_style(Style::default().bg(SEL_BG))
    .highlight_symbol("");

    let mut state = ratatui::widgets::TableState::default();
    state.select(Some(app.visible_selected_index(ctx.items_per_page)));
    f.render_stateful_widget(table, area, &mut state);
}

fn sort_label(label: &str, column: SortColumn, app: &App) -> Span<'static> {
    let active = app.sort_column == column;
    let text = if active {
        match app.sort_direction {
            SortDirection::Asc => format!("{label} ↑"),
            SortDirection::Desc => format!("{label} ↓"),
        }
    } else {
        label.to_string()
    };
    if active {
        Span::styled(text, Style::default().fg(FOCUS).add_modifier(Modifier::BOLD))
    } else {
        Span::styled(text, Style::default().fg(MUTED))
    }
}

fn row_for_directory(
    dir: &DirectoryInfo,
    pattern: &str,
    focused: bool,
    quiet: bool,
) -> Row<'static> {
    let glyph = if focused {
        Span::styled("❯", Style::default().fg(FOCUS).add_modifier(Modifier::BOLD))
    } else {
        Span::raw(" ")
    };

    let (check, check_style) = match &dir.deletion_status {
        crate::fs::DeletionStatus::Deleting => ("…", Style::default().fg(VALUE)),
        crate::fs::DeletionStatus::Deleted => (
            "×",
            Style::default().fg(MUTED).add_modifier(Modifier::CROSSED_OUT),
        ),
        crate::fs::DeletionStatus::Error(_) => ("!", Style::default().fg(DANGER)),
        crate::fs::DeletionStatus::Normal if dir.selected => (
            "[x]",
            Style::default().fg(MARK).add_modifier(Modifier::BOLD),
        ),
        crate::fs::DeletionStatus::Normal => ("[ ]", Style::default().fg(MUTED)),
    };

    let (size_text, size_style) = size_cell(dir, quiet);
    let path_fg = if focused { TEXT } else { TEXT };
    let path_dim = MUTED;

    let mut row = Row::new(vec![
        Cell::from(glyph),
        Cell::from(check).style(check_style),
        Cell::from(size_text).style(size_style),
        Cell::from(path_line(dir, pattern, path_fg, path_dim)),
    ]);

    if focused {
        row = row.style(Style::default().bg(SEL_BG));
    }

    row
}

fn size_cell(dir: &DirectoryInfo, quiet: bool) -> (Cow<'static, str>, Style) {
    if matches!(dir.deletion_status, crate::fs::DeletionStatus::Deleting) {
        return (Cow::Borrowed("del…"), Style::default().fg(VALUE));
    }
    if matches!(dir.deletion_status, crate::fs::DeletionStatus::Deleted) {
        return (Cow::Borrowed("—"), Style::default().fg(MUTED));
    }
    if matches!(dir.deletion_status, crate::fs::DeletionStatus::Error(_)) {
        return (Cow::Borrowed("err"), Style::default().fg(DANGER));
    }

    match dir.calculation_status {
        crate::fs::CalculationStatus::NotStarted | crate::fs::CalculationStatus::Calculating => {
            (Cow::Borrowed("…"), Style::default().fg(MUTED))
        }
        crate::fs::CalculationStatus::Error(_) => {
            (Cow::Borrowed("err"), Style::default().fg(DANGER))
        }
        crate::fs::CalculationStatus::Completed => {
            (Cow::Owned(dir.formatted_size.clone()), size_value_style(&dir.formatted_size, quiet))
        }
    }
}

/// Single Value hue; bold for GB/TB; dimmer for lower ranks.
fn size_value_style(formatted: &str, quiet: bool) -> Style {
    let upper = formatted.to_ascii_uppercase();
    let fg = if quiet { VALUE_DIM } else { VALUE };
    if upper.contains("GB") || upper.contains("TB") {
        Style::default().fg(fg).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(fg)
    }
}

fn path_line(dir: &DirectoryInfo, pattern: &str, main: Color, dim: Color) -> Line<'static> {
    let path = dir.path.strip_prefix("./").unwrap_or(&dir.path);

    if !pattern.is_empty() && path.len() > pattern.len() + 1 {
        let suffix_start = path.len() - pattern.len() - 1;
        if path.as_bytes().get(suffix_start) == Some(&b'/')
            && &path[suffix_start + 1..] == pattern
        {
            return Line::from(vec![
                Span::styled(
                    path[..suffix_start].to_string(),
                    Style::default().fg(main).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("/{pattern}"), Style::default().fg(dim)),
            ]);
        }
    }

    Line::from(Span::styled(path.to_string(), Style::default().fg(main)))
}

fn render_details_panel(f: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .borders(Borders::LEFT | Borders::TOP)
        .border_style(Style::default().fg(BORDER))
        .title(Span::styled(" detail ", Style::default().fg(MUTED)))
        .style(Style::default().bg(PANEL))
        .padding(Padding::horizontal(1));

    let Some(dir) = app.get_selected_directory() else {
        f.render_widget(
            Paragraph::new(Span::styled("No selection", Style::default().fg(MUTED))).block(block),
            area,
        );
        return;
    };

    let size_line = match &dir.deletion_status {
        crate::fs::DeletionStatus::Deleting => "Deleting…".to_string(),
        crate::fs::DeletionStatus::Deleted => "Deleted".to_string(),
        crate::fs::DeletionStatus::Error(err) => format!("Error: {err}"),
        crate::fs::DeletionStatus::Normal => match dir.calculation_status {
            crate::fs::CalculationStatus::Completed => {
                if let Some(pct) = app.selected_size_percent() {
                    format!("{} · {:.1}% of total", dir.formatted_size, pct)
                } else {
                    dir.formatted_size.clone()
                }
            }
            crate::fs::CalculationStatus::Calculating => "Calculating…".to_string(),
            _ => "Pending…".to_string(),
        },
    };

    let path = dir.path.strip_prefix("./").unwrap_or(&dir.path);
    let text = vec![
        Line::from(Span::styled("PATH", Style::default().fg(MUTED))),
        Line::from(Span::styled(path.to_string(), Style::default().fg(TEXT))),
        Line::from(""),
        Line::from(Span::styled("SIZE", Style::default().fg(MUTED))),
        Line::from(Span::styled(
            size_line,
            Style::default().fg(VALUE).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled("AGE", Style::default().fg(MUTED))),
        Line::from(Span::styled(
            dir.formatted_last_modified.clone(),
            Style::default().fg(TEXT),
        )),
    ];

    f.render_widget(
        Paragraph::new(text)
            .block(block)
            .wrap(Wrap { trim: true }),
        area,
    );
}

fn render_help_overlay(f: &mut Frame) {
    let area = centered_rect(64, 72, f.area());
    f.render_widget(Clear, area);

    let mut lines = vec![
        Line::from(Span::styled(
            " Shortcuts ",
            Style::default().fg(BG).bg(FOCUS).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    for (section, rows) in [
        (
            "Navigate",
            &[
                ("↑ ↓  j k", "move"),
                ("Home End", "first / last"),
                ("← →", "page"),
            ][..],
        ),
        (
            "Select",
            &[("space", "toggle"), ("a", "all"), ("d", "none")][..],
        ),
        (
            "Sort / filter",
            &[
                ("s / p / m", "size / path / age"),
                ("/", "filter"),
                ("Esc", "clear filter"),
            ][..],
        ),
        (
            "Delete",
            &[
                ("f  Del  ^D", "current"),
                ("c", "selected"),
                ("y / n", "confirm / cancel"),
            ][..],
        ),
        (
            "Other",
            &[
                ("i", "details"),
                ("o", "open"),
                ("^Y", "copy path"),
                ("q", "quit"),
            ][..],
        ),
    ] {
        lines.push(Line::from(Span::styled(
            format!(" {section}"),
            Style::default().fg(VALUE).add_modifier(Modifier::BOLD),
        )));
        for &(k, d) in rows {
            lines.push(Line::from(vec![
                Span::styled(format!("  {k:<14}"), Style::default().fg(FOCUS)),
                Span::styled(d.to_string(), Style::default().fg(TEXT)),
            ]));
        }
        lines.push(Line::from(""));
    }
    lines.push(Line::from(Span::styled(
        " any key closes ",
        Style::default().fg(MUTED),
    )));

    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(FOCUS))
                .style(Style::default().bg(PANEL)),
        ),
        area,
    );
}

fn render_delete_confirm(f: &mut Frame, app: &App) {
    let Some(confirm) = &app.delete_confirmation else {
        return;
    };
    let area = centered_rect(56, 44, f.area());
    f.render_widget(Clear, area);

    let size = confirm.total_size;
    let count = confirm.count;
    let unsized_count = confirm
        .preview_paths
        .iter()
        .filter(|path| {
            app.directories.iter().any(|d| {
                &d.path == *path
                    && !matches!(
                        d.calculation_status,
                        crate::fs::CalculationStatus::Completed
                    )
            })
        })
        .count();

    let mut lines = vec![
        Line::from(Span::styled(
            format!("Delete ~{}?", crate::fs::format_size(size)),
            Style::default().fg(DANGER).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            format!("{count} director{}", if count == 1 { "y" } else { "ies" }),
            Style::default().fg(TEXT),
        )),
        Line::from(""),
    ];

    if unsized_count > 0 {
        lines.push(Line::from(Span::styled(
            format!("{unsized_count} still sizing — total may be higher"),
            Style::default().fg(VALUE),
        )));
        lines.push(Line::from(""));
    }

    for path in confirm.preview_paths.iter().take(6) {
        lines.push(Line::from(Span::styled(
            format!("  {}", preview_path(path)),
            Style::default().fg(TEXT),
        )));
    }
    if confirm.preview_paths.len() > 6 {
        lines.push(Line::from(Span::styled(
            format!("  … +{} more", confirm.preview_paths.len() - 6),
            Style::default().fg(MUTED),
        )));
    }
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled(
            " y ",
            Style::default().fg(BG).bg(DANGER).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" confirm   ", Style::default().fg(MUTED)),
        Span::styled(
            " n ",
            Style::default().fg(BG).bg(MUTED).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" cancel", Style::default().fg(MUTED)),
    ]));

    f.render_widget(
        Paragraph::new(lines).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(DANGER))
                .title(Span::styled(" confirm ", Style::default().fg(DANGER)))
                .style(Style::default().bg(PANEL))
                .padding(Padding::horizontal(1)),
        ),
        area,
    );
}

/// Parent/leaf preview: `my-app/node_modules` instead of a long absolute path.
fn preview_path(path: &str) -> String {
    let path = path.strip_prefix("./").unwrap_or(path);
    let parts: Vec<&str> = path.split('/').filter(|p| !p.is_empty()).collect();
    match parts.as_slice() {
        [] => path.to_string(),
        [leaf] => (*leaf).to_string(),
        [.., parent, leaf] => format!("{parent}/{leaf}"),
    }
}

fn render_filter_overlay(f: &mut Frame, app: &App) {
    let area = Rect {
        x: f.area().x + 1,
        y: f.area().y,
        width: f.area().width.saturating_sub(2),
        height: 3,
    };
    f.render_widget(Clear, area);

    let query = if app.filter_query.is_empty() {
        "▌".to_string()
    } else {
        format!("{}▌", app.filter_query)
    };

    f.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " filter ",
                Style::default().fg(BG).bg(FOCUS).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(query, Style::default().fg(TEXT).add_modifier(Modifier::BOLD)),
            Span::styled("   Enter · Esc", Style::default().fg(MUTED)),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(FOCUS))
                .style(Style::default().bg(PANEL)),
        ),
        area,
    );
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::layout::Rect;

    #[test]
    fn items_per_page_matches_visible_table_rows() {
        // height 15 → minus hairline+header (2) = 13
        assert_eq!(items_per_page_for_area(Rect::new(0, 0, 80, 15)), 13);
        assert_eq!(items_per_page_for_area(Rect::new(0, 0, 80, 4)), 2);
        assert_eq!(items_per_page_for_area(Rect::new(0, 0, 80, 0)), 1);
    }

    #[test]
    fn items_per_page_scales_with_terminal_height() {
        let short = items_per_page_for_viewport(Rect::new(0, 0, 80, 20), false);
        let tall = items_per_page_for_viewport(Rect::new(0, 0, 80, 40), false);
        assert!(tall > short);

        let with_details = items_per_page_for_viewport(Rect::new(0, 0, 80, 30), true);
        let without_details = items_per_page_for_viewport(Rect::new(0, 0, 80, 30), false);
        assert!(without_details >= with_details);
    }

    #[test]
    fn size_value_style_bold_only_for_large_units() {
        assert_eq!(
            size_value_style("2.6 GB", false),
            Style::default().fg(VALUE).add_modifier(Modifier::BOLD)
        );
        assert_eq!(
            size_value_style("214.9 MB", false),
            Style::default().fg(VALUE)
        );
        assert_eq!(
            size_value_style("12.0 KB", true),
            Style::default().fg(VALUE_DIM)
        );
    }

    #[test]
    fn focused_path_uses_explicit_text_color() {
        let dir = DirectoryInfo {
            path: "proj/node_modules".to_string(),
            size: 1,
            formatted_size: "1 MB".to_string(),
            last_modified: None,
            formatted_last_modified: "1 day ago".to_string(),
            selected: false,
            deletion_status: crate::fs::DeletionStatus::Normal,
            calculation_status: crate::fs::CalculationStatus::Completed,
            calculation_time: None,
        };
        let line = path_line(&dir, "node_modules", TEXT, MUTED);
        for span in line.spans {
            assert!(span.style.fg.is_some(), "path spans must set explicit fg");
        }
    }

    #[test]
    fn preview_path_shows_parent_and_leaf() {
        assert_eq!(
            preview_path("/Users/x/Developer/my-app/node_modules"),
            "my-app/node_modules"
        );
        assert_eq!(preview_path("node_modules"), "node_modules");
    }

    #[test]
    fn header_hero_prefers_selection_over_releasable() {
        let mut app = App::new(vec![], "node_modules".to_string(), ".".to_string());
        app.directories.push(DirectoryInfo {
            path: "a/node_modules".to_string(),
            size: 1_000_000,
            formatted_size: "1.0 MB".to_string(),
            last_modified: None,
            formatted_last_modified: "1d".to_string(),
            selected: true,
            deletion_status: crate::fs::DeletionStatus::Normal,
            calculation_status: crate::fs::CalculationStatus::Completed,
            calculation_time: None,
        });
        app.directories.push(DirectoryInfo {
            path: "b/node_modules".to_string(),
            size: 2_000_000,
            formatted_size: "2.0 MB".to_string(),
            last_modified: None,
            formatted_last_modified: "1d".to_string(),
            selected: false,
            deletion_status: crate::fs::DeletionStatus::Normal,
            calculation_status: crate::fs::CalculationStatus::Completed,
            calculation_time: None,
        });
        app.rebuild_aggregates_from_directories();
        app.set_discovery_status(crate::ui::app::DiscoveryStatus::Complete);

        let (_left, hero) = header_metrics(&app);
        assert!(hero.contains("sel"), "hero should show selection CTA: {hero}");
        assert!(!hero.contains("releasable"));
    }
}
