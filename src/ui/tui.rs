use std::collections::HashSet;
use std::io::stdout;
use std::path::PathBuf;
use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use inquire::MultiSelect;
use ratatui::Frame;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, BorderType, Borders, Clear, HighlightSpacing, Paragraph, Row, Table, TableState, Wrap,
};

use crate::core::cleaner::Cleaner;
use crate::core::engine::ScanStats;
use crate::core::size::format_bytes;
use crate::core::traits::{DiscoveredProject, GitInfo, GitStatus, ProjectType};

// =========================================================================
// Unified 24-bit TrueColor Palette (Vibrant Electric Neon Theme)
// Guarantees identical, rich visual appearance across all terminals & OSes
// =========================================================================
pub struct Palette;

impl Palette {
    // Primary & Accent Colors
    pub const CYAN: Color = Color::Rgb(0, 215, 255); // Electric Cyan
    pub const BLUE: Color = Color::Rgb(85, 170, 255); // Electric Royal Blue
    pub const PURPLE: Color = Color::Rgb(190, 125, 255); // Vibrant Purple
    pub const PINK: Color = Color::Rgb(255, 140, 200); // Neon Pink
    pub const TEAL: Color = Color::Rgb(80, 235, 210); // Neon Teal

    // Status Colors
    pub const SUCCESS_GREEN: Color = Color::Rgb(75, 235, 130); // Neon Green
    pub const WARNING_YELLOW: Color = Color::Rgb(255, 215, 75); // Bright Amber
    pub const ORANGE: Color = Color::Rgb(255, 145, 75); // Neon Peach/Orange
    pub const DANGER_RED: Color = Color::Rgb(255, 85, 95); // Crimson Red

    // Neutral & Text Colors
    pub const TEXT_MAIN: Color = Color::Rgb(255, 255, 255); // Crisp Pure White
    pub const TEXT_MUTED: Color = Color::Rgb(170, 185, 215); // Readable Light Gray/Blue
    pub const TEXT_DIM: Color = Color::Rgb(110, 125, 155); // Muted Slate
    pub const TEXT_DARK: Color = Color::Rgb(10, 15, 25); // Dark badge text

    // Borders & Background Highlights
    pub const BORDER_CYAN: Color = Color::Rgb(0, 215, 255); // Electric Cyan Border
    pub const BORDER_PRIMARY: Color = Color::Rgb(70, 130, 220); // Vibrant Electric Blue Border (Glow frame)
    pub const BORDER_ALERT: Color = Color::Rgb(255, 85, 95); // Red Alert Border
    pub const BG_HIGHLIGHT: Color = Color::Rgb(25, 45, 75); // Subtle Dark Indigo Row Highlight
    pub const BG_DARK: Color = Color::Rgb(17, 17, 27); // Dark badge background
}

// =========================================================================
// 1. Inquire Selection Prompt (Lightweight CLI Checkbox Mode)
// =========================================================================

#[derive(Debug, Clone)]
pub struct SelectableArtifact {
    pub project_root: PathBuf,
    pub project_type: ProjectType,
    pub artifact_path: PathBuf,
    pub artifact_name: &'static str,
    pub size_bytes: u64,
    pub git_info: Option<GitInfo>,
}

impl std::fmt::Display for SelectableArtifact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let path_str = self.project_root.display().to_string();
        let display_path = if path_str.len() > 30 {
            format!("...{}", &path_str[path_str.len() - 27..])
        } else {
            path_str
        };

        let status_str = match &self.git_info {
            Some(info) => {
                if info.is_dirty {
                    format!("{} (Dirty)", info.status)
                } else {
                    info.status.to_string()
                }
            }
            None => "No Git".to_string(),
        };

        write!(
            f,
            "{} ({}: {}) - {} [{}]",
            display_path,
            self.project_type,
            self.artifact_name,
            format_bytes(self.size_bytes),
            status_str
        )
    }
}

pub fn prompt_selection(projects: &[DiscoveredProject]) -> Result<Vec<SelectableArtifact>> {
    let mut items = Vec::new();
    let mut default_indices = Vec::new();

    for project in projects {
        for artifact in &project.artifacts {
            let is_stale = project
                .git_info
                .as_ref()
                .map(|info| info.status == GitStatus::Stale && !info.is_dirty)
                .unwrap_or(false);

            if is_stale {
                default_indices.push(items.len());
            }

            items.push(SelectableArtifact {
                project_root: project.root.clone(),
                project_type: project.project_type.clone(),
                artifact_path: artifact.abs_path.clone(),
                artifact_name: artifact.target.name,
                size_bytes: artifact.size_bytes,
                git_info: project.git_info.clone(),
            });
        }
    }

    if items.is_empty() {
        return Ok(Vec::new());
    }

    let prompt = MultiSelect::new("Select the build artifacts to clean:", items)
        .with_default(&default_indices)
        .with_help_message(
            "Space: toggle item, 'a': toggle all, Enter: confirm selection, Esc: cancel",
        );

    match prompt.prompt() {
        Ok(selected) => Ok(selected),
        Err(inquire::InquireError::OperationCanceled) => Ok(Vec::new()),
        Err(e) => Err(anyhow::anyhow!("Interactive selection failed: {}", e)),
    }
}

// =========================================================================
// 2. Ratatui Fullscreen TUI Dashboard Mode
// =========================================================================

pub struct TuiApp {
    pub projects: Vec<DiscoveredProject>,
    pub selected_indices: HashSet<usize>,
    pub cursor_index: usize,
    pub show_confirm_dialog: bool,
    pub notification_message: Option<(String, Style)>,
    pub stats: ScanStats,
    pub dry_run: bool,
    pub should_quit: bool,
    pub table_state: TableState,
    pub filter_query: String,
    pub is_filtering: bool,
    pub filtered_indices: Vec<usize>,
}

impl TuiApp {
    pub fn new(projects: Vec<DiscoveredProject>, stats: ScanStats, dry_run: bool) -> Self {
        let mut selected = HashSet::new();
        for (i, p) in projects.iter().enumerate() {
            if let Some(ref git) = p.git_info
                && git.status == GitStatus::Stale
                && !git.is_dirty
            {
                selected.insert(i);
            }
        }

        let filtered_indices: Vec<usize> = (0..projects.len()).collect();

        let mut table_state = TableState::default();
        if !filtered_indices.is_empty() {
            table_state.select(Some(0));
        }

        Self {
            projects,
            selected_indices: selected,
            cursor_index: 0,
            show_confirm_dialog: false,
            notification_message: None,
            stats,
            dry_run,
            should_quit: false,
            table_state,
            filter_query: String::new(),
            is_filtering: false,
            filtered_indices,
        }
    }

    pub fn apply_filter(&mut self) {
        let q = self.filter_query.trim().to_lowercase();
        if q.is_empty() {
            self.filtered_indices = (0..self.projects.len()).collect();
        } else {
            self.filtered_indices = self
                .projects
                .iter()
                .enumerate()
                .filter(|(_, p)| {
                    let path = p.root.display().to_string().to_lowercase();
                    let eco = p.project_type.to_string().to_lowercase();
                    let git_status = match &p.git_info {
                        Some(info) => {
                            if info.is_dirty {
                                format!("{} dirty", info.status.to_string().to_lowercase())
                            } else {
                                info.status.to_string().to_lowercase()
                            }
                        }
                        None => "no git".to_string(),
                    };
                    path.contains(&q) || eco.contains(&q) || git_status.contains(&q)
                })
                .map(|(i, _)| i)
                .collect();
        }

        if self.filtered_indices.is_empty() {
            self.cursor_index = 0;
            self.table_state.select(None);
        } else {
            if self.cursor_index >= self.filtered_indices.len() {
                self.cursor_index = self.filtered_indices.len() - 1;
            }
            self.table_state.select(Some(self.cursor_index));
        }
    }

    pub fn current_selected_project_index(&self) -> Option<usize> {
        self.filtered_indices.get(self.cursor_index).copied()
    }

    pub fn move_up(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        if self.cursor_index > 0 {
            self.cursor_index -= 1;
        } else {
            self.cursor_index = self.filtered_indices.len() - 1;
        }
        self.table_state.select(Some(self.cursor_index));
    }

    pub fn move_down(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        if self.cursor_index + 1 < self.filtered_indices.len() {
            self.cursor_index += 1;
        } else {
            self.cursor_index = 0;
        }
        self.table_state.select(Some(self.cursor_index));
    }

    pub fn toggle_selection(&mut self) {
        if let Some(&actual_idx) = self.filtered_indices.get(self.cursor_index) {
            if self.selected_indices.contains(&actual_idx) {
                self.selected_indices.remove(&actual_idx);
            } else {
                self.selected_indices.insert(actual_idx);
            }
        }
    }

    pub fn toggle_all(&mut self) {
        if self.filtered_indices.is_empty() {
            return;
        }
        let all_filtered_selected = self
            .filtered_indices
            .iter()
            .all(|idx| self.selected_indices.contains(idx));

        if all_filtered_selected {
            for &idx in &self.filtered_indices {
                self.selected_indices.remove(&idx);
            }
        } else {
            for &idx in &self.filtered_indices {
                self.selected_indices.insert(idx);
            }
        }
    }

    pub fn unselect_all(&mut self) {
        if self.filter_query.is_empty() {
            self.selected_indices.clear();
        } else {
            for &idx in &self.filtered_indices {
                self.selected_indices.remove(&idx);
            }
        }
    }

    pub fn total_reclaimable_bytes(&self) -> u64 {
        self.projects
            .iter()
            .map(|p| p.total_reclaimable_bytes())
            .sum()
    }

    pub fn selected_reclaimable_bytes(&self) -> u64 {
        self.selected_indices
            .iter()
            .filter_map(|&idx| self.projects.get(idx))
            .map(|p| p.total_reclaimable_bytes())
            .sum()
    }

    pub fn selected_artifacts_count(&self) -> usize {
        self.selected_indices
            .iter()
            .filter_map(|&idx| self.projects.get(idx))
            .map(|p| p.artifacts.len())
            .sum()
    }

    pub fn perform_clean(&mut self) {
        if self.dry_run {
            self.notification_message = Some((
                "[DRY-RUN] Clean simulated: No files were touched.".to_string(),
                Style::default()
                    .fg(Palette::CYAN)
                    .add_modifier(Modifier::BOLD),
            ));
            return;
        }

        let cleaner = Cleaner::new();
        let mut renamed = Vec::new();

        let selected_sorted: Vec<usize> = {
            let mut s: Vec<usize> = self.selected_indices.iter().copied().collect();
            s.sort_unstable();
            s
        };

        for &idx in &selected_sorted {
            if let Some(project) = self.projects.get(idx) {
                for artifact in &project.artifacts {
                    match cleaner.rename_to_trash(&artifact.abs_path, artifact.size_bytes) {
                        Ok(item) => {
                            renamed.push(item);
                        }
                        Err(e) => {
                            self.notification_message = Some((
                                format!("Error cleaning {}: {}", artifact.target.name, e),
                                Style::default().fg(Palette::DANGER_RED),
                            ));
                        }
                    }
                }
            }
        }

        if !renamed.is_empty() {
            let handle = cleaner.purge_items_in_background(renamed);
            let report = handle.join().unwrap_or_default();

            // Remove cleaned projects
            let mut remaining = Vec::new();
            for (i, proj) in self.projects.drain(..).enumerate() {
                if !self.selected_indices.contains(&i) {
                    remaining.push(proj);
                }
            }
            self.projects = remaining;
            self.selected_indices.clear();
            self.apply_filter();

            self.notification_message = Some((
                format!(
                    "✔ Cleaned {} artifact(s) successfully! Reclaimed {}",
                    report.items_cleaned,
                    format_bytes(report.total_bytes_reclaimed)
                ),
                Style::default()
                    .fg(Palette::SUCCESS_GREEN)
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }
}

pub fn run_tui_app(
    projects: Vec<DiscoveredProject>,
    stats: ScanStats,
    dry_run: bool,
) -> Result<()> {
    enable_raw_mode()?;
    let mut stdout_handle = stdout();
    execute!(stdout_handle, EnterAlternateScreen, crossterm::cursor::Hide)?;
    let backend = CrosstermBackend::new(stdout_handle);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut app = TuiApp::new(projects, stats, dry_run);

    let res = run_app_loop(&mut terminal, &mut app);

    // Teardown terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::cursor::Show
    )?;

    res
}

fn run_app_loop(
    terminal: &mut ratatui::Terminal<CrosstermBackend<std::io::Stdout>>,
    app: &mut TuiApp,
) -> Result<()> {
    loop {
        terminal.draw(|f| draw_ui(f, app))?;

        if app.should_quit {
            break;
        }

        if event::poll(Duration::from_millis(50))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            if app.show_confirm_dialog {
                match key.code {
                    KeyCode::Char('y') | KeyCode::Enter => {
                        app.show_confirm_dialog = false;
                        app.perform_clean();
                    }
                    KeyCode::Char('n') | KeyCode::Esc => {
                        app.show_confirm_dialog = false;
                    }
                    _ => {}
                }
            } else if app.is_filtering {
                match key.code {
                    KeyCode::Enter => {
                        app.is_filtering = false;
                    }
                    KeyCode::Esc => {
                        app.filter_query.clear();
                        app.apply_filter();
                        app.is_filtering = false;
                    }
                    KeyCode::Backspace => {
                        app.filter_query.pop();
                        app.apply_filter();
                    }
                    KeyCode::Char(c) => {
                        app.filter_query.push(c);
                        app.apply_filter();
                    }
                    _ => {}
                }
            } else {
                match key.code {
                    KeyCode::Char('q') => {
                        app.should_quit = true;
                    }
                    KeyCode::Esc => {
                        if !app.filter_query.is_empty() {
                            app.filter_query.clear();
                            app.apply_filter();
                        } else {
                            app.should_quit = true;
                        }
                    }
                    KeyCode::Char('/') | KeyCode::Char('f') => {
                        app.is_filtering = true;
                    }
                    KeyCode::Char('c') if !app.filter_query.is_empty() => {
                        app.filter_query.clear();
                        app.apply_filter();
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        app.move_up();
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        app.move_down();
                    }
                    KeyCode::Char(' ') => {
                        app.toggle_selection();
                    }
                    KeyCode::Char('a') => {
                        app.toggle_all();
                    }
                    KeyCode::Char('u') => {
                        app.unselect_all();
                    }
                    KeyCode::Char('d') if !app.selected_indices.is_empty() => {
                        app.show_confirm_dialog = true;
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(())
}

fn draw_ui(f: &mut Frame, app: &mut TuiApp) {
    let size = f.area();

    let show_big_banner = size.height >= 26 && size.width >= 68;
    let header_height = if show_big_banner { 9 } else { 3 };

    // Main layout: Header, Central Body, Footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(header_height),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(size);

    // 1. Header
    let header_block = if show_big_banner {
        let ascii_lines = [
            "███████╗██╗    ██╗███████╗███████╗██████╗       ██████╗ ███████╗",
            "██╔════╝██║    ██║██╔════╝██╔════╝██╔══██╗      ██╔══██╗██╔════╝",
            "███████╗██║ █╗ ██║█████╗  █████╗  ██████╔╝█████╗██████╔╝███████╗",
            "╚════██║██║███╗██║██╔══╝  ██╔══╝  ██╔═══╝ ╚════╝██╔══██╗╚════██║",
            "███████║╚███╔███╔╝███████╗███████╗██║           ██║  ██║███████║",
            "╚══════╝ ╚══╝╚══╝ ╚══════╝╚══════╝╚═╝           ╚═╝  ╚═╝╚══════╝",
        ];

        let mut text = Vec::new();
        for line in ascii_lines {
            text.push(Line::from(Span::styled(
                line,
                Style::default()
                    .fg(Palette::CYAN)
                    .add_modifier(Modifier::BOLD),
            )));
        }
        text.push(Line::from(vec![
            Span::styled(
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
                Style::default()
                    .fg(Palette::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  │  Inspected ", Style::default().fg(Palette::TEXT_DIM)),
            Span::styled(
                format!("{} dirs", app.stats.dirs_inspected),
                Style::default()
                    .fg(Palette::CYAN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" in ", Style::default().fg(Palette::TEXT_DIM)),
            Span::styled(
                format!("{:.2}s", app.stats.duration.as_secs_f64()),
                Style::default()
                    .fg(Palette::WARNING_YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  │  ", Style::default().fg(Palette::TEXT_DIM)),
            Span::styled(
                format!("{} projects found", app.projects.len()),
                Style::default()
                    .fg(Palette::SUCCESS_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        Paragraph::new(text).alignment(Alignment::Center).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Palette::BORDER_CYAN)),
        )
    } else {
        let header_text = vec![Line::from(vec![
            Span::styled(
                " SWEEP-RS ",
                Style::default()
                    .fg(Palette::TEXT_DARK)
                    .bg(Palette::CYAN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S")),
                Style::default()
                    .fg(Palette::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  │  Inspected ", Style::default().fg(Palette::TEXT_DIM)),
            Span::styled(
                format!("{} dirs", app.stats.dirs_inspected),
                Style::default()
                    .fg(Palette::CYAN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" in ", Style::default().fg(Palette::TEXT_DIM)),
            Span::styled(
                format!("{:.2}s", app.stats.duration.as_secs_f64()),
                Style::default()
                    .fg(Palette::WARNING_YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled("  │  ", Style::default().fg(Palette::TEXT_DIM)),
            Span::styled(
                format!("{} projects found", app.projects.len()),
                Style::default()
                    .fg(Palette::SUCCESS_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
        ])];
        Paragraph::new(header_text).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Palette::BORDER_CYAN)),
        )
    };
    f.render_widget(header_block, chunks[0]);

    // 2. Central Body: Split into Left (Table 60%) and Right (Details + Gauge 40%)
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(chunks[1]);

    // Left: Project Table
    draw_table(f, app, body_chunks[0]);

    // Right: Split into Inspector (Top) and Storage Gauge (Bottom)
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
        .split(body_chunks[1]);

    draw_inspector(f, app, right_chunks[0]);
    draw_gauge(f, app, right_chunks[1]);

    // 3. Footer / Keybindings & Notifications
    draw_footer(f, app, chunks[2]);

    // 4. Modal Confirmation Popup
    if app.show_confirm_dialog {
        draw_confirm_dialog(f, app, size);
    }
}

fn draw_table(f: &mut Frame, app: &mut TuiApp, area: Rect) {
    let rows: Vec<Row> = if app.filtered_indices.is_empty() {
        let msg = if !app.filter_query.is_empty() {
            format!(
                "No projects match filter \"{}\". Press [c] or [Esc] to clear.",
                app.filter_query
            )
        } else {
            "No cleanable projects discovered.".to_string()
        };
        vec![Row::new(vec![
            Span::raw(""),
            Span::styled(
                msg,
                Style::default()
                    .fg(Palette::TEXT_MUTED)
                    .add_modifier(Modifier::ITALIC),
            ),
            Span::raw(""),
            Span::raw(""),
            Span::raw(""),
        ])]
    } else {
        app.filtered_indices
            .iter()
            .map(|&idx| {
                let p = &app.projects[idx];
                let is_selected = app.selected_indices.contains(&idx);
                let check_mark = if is_selected {
                    Span::styled(
                        "[✓]",
                        Style::default()
                            .fg(Palette::SUCCESS_GREEN)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled("[ ]", Style::default().fg(Palette::TEXT_DIM))
                };

                let path_str = p.root.display().to_string();
                let display_path = if path_str.len() > 28 {
                    format!("...{}", &path_str[path_str.len() - 25..])
                } else {
                    path_str
                };

                let type_style = match p.project_type {
                    ProjectType::Rust => Style::default()
                        .fg(Palette::ORANGE)
                        .add_modifier(Modifier::BOLD),
                    ProjectType::Node => Style::default()
                        .fg(Palette::SUCCESS_GREEN)
                        .add_modifier(Modifier::BOLD),
                    ProjectType::Python => Style::default()
                        .fg(Palette::CYAN)
                        .add_modifier(Modifier::BOLD),
                    ProjectType::Dotnet => Style::default()
                        .fg(Palette::PURPLE)
                        .add_modifier(Modifier::BOLD),
                    ProjectType::Java => Style::default()
                        .fg(Palette::WARNING_YELLOW)
                        .add_modifier(Modifier::BOLD),
                    _ => Style::default()
                        .fg(Palette::WARNING_YELLOW)
                        .add_modifier(Modifier::BOLD),
                };

                let (status_text, status_style) = match &p.git_info {
                    Some(info) => {
                        if info.is_dirty {
                            (
                                format!("✖ {} (Dirty)", info.status),
                                Style::default()
                                    .fg(Palette::DANGER_RED)
                                    .add_modifier(Modifier::BOLD),
                            )
                        } else {
                            match info.status {
                                GitStatus::Stale => (
                                    "● Stale".to_string(),
                                    Style::default()
                                        .fg(Palette::SUCCESS_GREEN)
                                        .add_modifier(Modifier::BOLD),
                                ),
                                GitStatus::Active => (
                                    "▲ Active".to_string(),
                                    Style::default().fg(Palette::WARNING_YELLOW),
                                ),
                                GitStatus::Moderate => {
                                    ("◆ Moderate".to_string(), Style::default().fg(Palette::CYAN))
                                }
                                GitStatus::Unknown => (
                                    "? Unknown".to_string(),
                                    Style::default().fg(Palette::TEXT_DIM),
                                ),
                            }
                        }
                    }
                    None => (
                        "- No Git".to_string(),
                        Style::default().fg(Palette::TEXT_DIM),
                    ),
                };

                let reclaimable_bytes = p.total_reclaimable_bytes();
                let size_style = if reclaimable_bytes >= 1024 * 1024 * 1024 {
                    Style::default()
                        .fg(Palette::DANGER_RED)
                        .add_modifier(Modifier::BOLD)
                } else if reclaimable_bytes >= 100 * 1024 * 1024 {
                    Style::default()
                        .fg(Palette::ORANGE)
                        .add_modifier(Modifier::BOLD)
                } else if reclaimable_bytes >= 10 * 1024 * 1024 {
                    Style::default().fg(Palette::CYAN)
                } else {
                    Style::default().fg(Palette::TEXT_MUTED)
                };

                Row::new(vec![
                    check_mark,
                    Span::styled(display_path, Style::default().fg(Palette::TEXT_MAIN)),
                    Span::styled(p.project_type.to_string(), type_style),
                    Span::styled(status_text, status_style),
                    Span::styled(format_bytes(reclaimable_bytes), size_style),
                ])
            })
            .collect()
    };

    let header = Row::new(vec![
        Span::styled(
            "SEL",
            Style::default()
                .fg(Palette::CYAN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "PROJECT PATH",
            Style::default()
                .fg(Palette::CYAN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "TYPE",
            Style::default()
                .fg(Palette::CYAN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "GIT STATUS",
            Style::default()
                .fg(Palette::CYAN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "RECLAIMABLE",
            Style::default()
                .fg(Palette::CYAN)
                .add_modifier(Modifier::BOLD),
        ),
    ])
    .bottom_margin(1);

    let widths = [
        Constraint::Length(5),
        Constraint::Percentage(45),
        Constraint::Length(10),
        Constraint::Length(17),
        Constraint::Length(12),
    ];

    let table_title = if !app.filter_query.is_empty() {
        format!(
            " Discovered Cleanable Projects [Filter: \"{}\" ({}/{})] ",
            app.filter_query,
            app.filtered_indices.len(),
            app.projects.len()
        )
    } else {
        format!(" Discovered Cleanable Projects ({}) ", app.projects.len())
    };

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(Span::styled(
                    table_title,
                    Style::default()
                        .fg(Palette::CYAN)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Palette::BORDER_PRIMARY)),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Palette::TEXT_MAIN)
                .bg(Palette::BG_HIGHLIGHT),
        )
        .highlight_symbol("▶ ")
        .highlight_spacing(HighlightSpacing::Always);

    f.render_stateful_widget(table, area, &mut app.table_state);
}

fn draw_inspector(f: &mut Frame, app: &TuiApp, area: Rect) {
    let content = if let Some(proj_idx) = app.current_selected_project_index()
        && let Some(p) = app.projects.get(proj_idx)
    {
        let mut lines = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(
                "Project Root: ",
                Style::default()
                    .fg(Palette::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                p.root.display().to_string(),
                Style::default()
                    .fg(Palette::TEXT_MAIN)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        let type_style = match p.project_type {
            ProjectType::Rust => Style::default()
                .fg(Palette::ORANGE)
                .add_modifier(Modifier::BOLD),
            ProjectType::Node => Style::default()
                .fg(Palette::SUCCESS_GREEN)
                .add_modifier(Modifier::BOLD),
            ProjectType::Python => Style::default()
                .fg(Palette::CYAN)
                .add_modifier(Modifier::BOLD),
            ProjectType::Dotnet => Style::default()
                .fg(Palette::PURPLE)
                .add_modifier(Modifier::BOLD),
            ProjectType::Java => Style::default()
                .fg(Palette::WARNING_YELLOW)
                .add_modifier(Modifier::BOLD),
            _ => Style::default()
                .fg(Palette::WARNING_YELLOW)
                .add_modifier(Modifier::BOLD),
        };

        lines.push(Line::from(vec![
            Span::styled(
                "Ecosystem:    ",
                Style::default()
                    .fg(Palette::BLUE)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(format!("● {}", p.project_type), type_style),
        ]));

        if let Some(ref git) = p.git_info {
            let commit_age = match git.last_commit_days {
                Some(0) => "Today".to_string(),
                Some(1) => "1 day ago".to_string(),
                Some(d) => format!("{} days ago", d),
                None => "No commits found".to_string(),
            };
            lines.push(Line::from(vec![
                Span::styled(
                    "Last Commit:  ",
                    Style::default()
                        .fg(Palette::BLUE)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    commit_age,
                    Style::default()
                        .fg(Palette::WARNING_YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled(
                    "Worktree:     ",
                    Style::default()
                        .fg(Palette::BLUE)
                        .add_modifier(Modifier::BOLD),
                ),
                if git.is_dirty {
                    Span::styled(
                        "✖ Uncommitted changes present (Dirty)",
                        Style::default()
                            .fg(Palette::DANGER_RED)
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(
                        "✓ Clean",
                        Style::default()
                            .fg(Palette::SUCCESS_GREEN)
                            .add_modifier(Modifier::BOLD),
                    )
                },
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    "Git:          ",
                    Style::default()
                        .fg(Palette::BLUE)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Not a Git repository",
                    Style::default().fg(Palette::TEXT_MUTED),
                ),
            ]));
        }

        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "Cleanable Artifact Targets:",
            Style::default()
                .fg(Palette::WARNING_YELLOW)
                .add_modifier(Modifier::BOLD),
        )));

        for artifact in &p.artifacts {
            lines.push(Line::from(vec![
                Span::styled("  ✦ ", Style::default().fg(Palette::CYAN)),
                Span::styled(
                    artifact.target.name,
                    Style::default()
                        .fg(Palette::TEXT_MAIN)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({})", format_bytes(artifact.size_bytes)),
                    Style::default()
                        .fg(Palette::WARNING_YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        lines
    } else {
        vec![Line::from(Span::styled(
            "No project selected",
            Style::default().fg(Palette::TEXT_MUTED),
        ))]
    };

    let p = Paragraph::new(content)
        .block(
            Block::default()
                .title(Span::styled(
                    " Project Inspector ",
                    Style::default()
                        .fg(Palette::CYAN)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Palette::BORDER_PRIMARY)),
        )
        .wrap(Wrap { trim: true });

    f.render_widget(p, area);
}

fn draw_gauge(f: &mut Frame, app: &TuiApp, area: Rect) {
    let total = app.total_reclaimable_bytes();
    let selected = app.selected_reclaimable_bytes();
    let ratio = if total > 0 {
        (selected as f64 / total as f64).clamp(0.0, 1.0)
    } else {
        0.0
    };

    // Card container block
    let block = Block::default()
        .title(Span::styled(
            " Selected Reclaimable Space ",
            Style::default()
                .fg(Palette::CYAN)
                .add_modifier(Modifier::BOLD),
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(Palette::BORDER_PRIMARY));

    let inner_area = block.inner(area);
    f.render_widget(block, area);

    if inner_area.height < 3 || inner_area.width < 10 {
        return;
    }

    // Calculate ecosystem breakdown of selected items
    let mut rust_bytes = 0u64;
    let mut rust_count = 0usize;
    let mut node_bytes = 0u64;
    let mut node_count = 0usize;
    let mut dotnet_bytes = 0u64;
    let mut dotnet_count = 0usize;
    let mut python_bytes = 0u64;
    let mut python_count = 0usize;
    let mut java_bytes = 0u64;
    let mut java_count = 0usize;
    let mut other_bytes = 0u64;
    let mut other_count = 0usize;

    let mut stale_bytes = 0u64;
    let mut active_bytes = 0u64;

    for &idx in &app.selected_indices {
        if let Some(p) = app.projects.get(idx) {
            let p_bytes = p.total_reclaimable_bytes();
            match p.project_type {
                ProjectType::Rust => {
                    rust_bytes += p_bytes;
                    rust_count += 1;
                }
                ProjectType::Node => {
                    node_bytes += p_bytes;
                    node_count += 1;
                }
                ProjectType::Dotnet => {
                    dotnet_bytes += p_bytes;
                    dotnet_count += 1;
                }
                ProjectType::Python => {
                    python_bytes += p_bytes;
                    python_count += 1;
                }
                ProjectType::Java => {
                    java_bytes += p_bytes;
                    java_count += 1;
                }
                _ => {
                    other_bytes += p_bytes;
                    other_count += 1;
                }
            }
            if let Some(ref git) = p.git_info {
                if git.status == GitStatus::Stale && !git.is_dirty {
                    stale_bytes += p_bytes;
                } else {
                    active_bytes += p_bytes;
                }
            } else {
                stale_bytes += p_bytes;
            }
        }
    }

    let mut lines = Vec::new();

    // 1. Storage Metric line
    lines.push(Line::from(vec![
        Span::styled(
            "Selected: ",
            Style::default()
                .fg(Palette::BLUE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format_bytes(selected),
            Style::default()
                .fg(Palette::SUCCESS_GREEN)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" / ", Style::default().fg(Palette::TEXT_DIM)),
        Span::styled(
            format_bytes(total),
            Style::default().fg(Palette::TEXT_MUTED),
        ),
        Span::styled(
            format!(" ({:.1}%)", ratio * 100.0),
            Style::default()
                .fg(Palette::WARNING_YELLOW)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // 2. Artifact targets line
    lines.push(Line::from(vec![
        Span::styled(
            "Targets:  ",
            Style::default()
                .fg(Palette::BLUE)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!("{} artifact(s)", app.selected_artifacts_count()),
            Style::default().fg(Palette::TEXT_MAIN),
        ),
        Span::styled(" across ", Style::default().fg(Palette::TEXT_DIM)),
        Span::styled(
            format!("{} project(s)", app.selected_indices.len()),
            Style::default()
                .fg(Palette::CYAN)
                .add_modifier(Modifier::BOLD),
        ),
    ]));

    // 3. Slim, modern 1-line progress bar (Never floods screen with solid bright background)
    let bar_width = (inner_area.width.saturating_sub(2) as usize).max(5);
    let filled_chars = ((ratio * bar_width as f64).round() as usize).min(bar_width);
    let unfilled_chars = bar_width.saturating_sub(filled_chars);

    let filled_str = "█".repeat(filled_chars);
    let unfilled_str = "░".repeat(unfilled_chars);

    let bar_color = if ratio == 0.0 {
        Palette::TEXT_DIM
    } else if ratio < 0.35 {
        Palette::TEAL
    } else if ratio < 0.75 {
        Palette::SUCCESS_GREEN
    } else {
        Palette::ORANGE
    };

    lines.push(Line::from(vec![
        Span::styled(
            filled_str,
            Style::default().fg(bar_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(unfilled_str, Style::default().fg(Palette::BG_HIGHLIGHT)),
    ]));

    // 4. Ecosystem breakdown lines
    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled(
        "Ecosystem Breakdown:",
        Style::default()
            .fg(Palette::WARNING_YELLOW)
            .add_modifier(Modifier::BOLD),
    )));

    if app.selected_indices.is_empty() {
        lines.push(Line::from(Span::styled(
            "  (No projects selected. Press 'Space' or 'a')",
            Style::default().fg(Palette::TEXT_MUTED),
        )));
    } else {
        if rust_count > 0 {
            lines.push(Line::from(vec![
                Span::styled("  🦀 Rust:    ", Style::default().fg(Palette::ORANGE)),
                Span::styled(
                    format_bytes(rust_bytes),
                    Style::default()
                        .fg(Palette::TEXT_MAIN)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({} project(s))", rust_count),
                    Style::default().fg(Palette::TEXT_MUTED),
                ),
            ]));
        }
        if node_count > 0 {
            lines.push(Line::from(vec![
                Span::styled(
                    "  🟢 Node:    ",
                    Style::default().fg(Palette::SUCCESS_GREEN),
                ),
                Span::styled(
                    format_bytes(node_bytes),
                    Style::default()
                        .fg(Palette::TEXT_MAIN)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({} project(s))", node_count),
                    Style::default().fg(Palette::TEXT_MUTED),
                ),
            ]));
        }
        if dotnet_count > 0 {
            lines.push(Line::from(vec![
                Span::styled("  🟣 .NET:    ", Style::default().fg(Palette::PURPLE)),
                Span::styled(
                    format_bytes(dotnet_bytes),
                    Style::default()
                        .fg(Palette::TEXT_MAIN)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({} project(s))", dotnet_count),
                    Style::default().fg(Palette::TEXT_MUTED),
                ),
            ]));
        }
        if python_count > 0 {
            lines.push(Line::from(vec![
                Span::styled("  🐍 Python:  ", Style::default().fg(Palette::CYAN)),
                Span::styled(
                    format_bytes(python_bytes),
                    Style::default()
                        .fg(Palette::TEXT_MAIN)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({} project(s))", python_count),
                    Style::default().fg(Palette::TEXT_MUTED),
                ),
            ]));
        }
        if java_count > 0 {
            lines.push(Line::from(vec![
                Span::styled(
                    "  ☕ Java:    ",
                    Style::default().fg(Palette::WARNING_YELLOW),
                ),
                Span::styled(
                    format_bytes(java_bytes),
                    Style::default()
                        .fg(Palette::TEXT_MAIN)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({} project(s))", java_count),
                    Style::default().fg(Palette::TEXT_MUTED),
                ),
            ]));
        }
        if other_count > 0 {
            lines.push(Line::from(vec![
                Span::styled(
                    "  📦 Other:   ",
                    Style::default().fg(Palette::WARNING_YELLOW),
                ),
                Span::styled(
                    format_bytes(other_bytes),
                    Style::default()
                        .fg(Palette::TEXT_MAIN)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({} project(s))", other_count),
                    Style::default().fg(Palette::TEXT_MUTED),
                ),
            ]));
        }

        // Safety summary
        lines.push(Line::from(vec![
            Span::styled("  Safety:     ", Style::default().fg(Palette::CYAN)),
            Span::styled(
                format!("● Stale: {}", format_bytes(stale_bytes)),
                Style::default()
                    .fg(Palette::SUCCESS_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!("  ▲ Active: {}", format_bytes(active_bytes)),
                if active_bytes > 0 {
                    Style::default()
                        .fg(Palette::WARNING_YELLOW)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Palette::TEXT_MUTED)
                },
            ),
        ]));
    }

    let p = Paragraph::new(lines).wrap(Wrap { trim: true });
    f.render_widget(p, inner_area);
}

fn draw_footer(f: &mut Frame, app: &TuiApp, area: Rect) {
    let (footer_line, footer_border_style) =
        if let Some((ref msg, style)) = app.notification_message {
            (
                Line::from(vec![Span::styled(msg.clone(), style)]),
                Style::default().fg(Palette::WARNING_YELLOW),
            )
        } else if app.is_filtering {
            let prompt = if app.filter_query.is_empty() {
                Span::styled(
                    "Type to filter by name, path, ecosystem, or git status... ",
                    Style::default().fg(Palette::TEXT_MUTED),
                )
            } else {
                Span::styled(
                    format!("{} ", app.filter_query),
                    Style::default()
                        .fg(Palette::TEXT_MAIN)
                        .add_modifier(Modifier::BOLD),
                )
            };

            (
                Line::from(vec![
                    Span::styled(
                        "  Filter: ",
                        Style::default()
                            .fg(Palette::TEXT_DARK)
                            .bg(Palette::WARNING_YELLOW)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    prompt,
                    Span::styled(
                        "█",
                        Style::default()
                            .fg(Palette::WARNING_YELLOW)
                            .add_modifier(Modifier::RAPID_BLINK),
                    ),
                    Span::raw("   "),
                    Span::styled(
                        format!(
                            "({}/{} matched)",
                            app.filtered_indices.len(),
                            app.projects.len()
                        ),
                        Style::default().fg(Palette::CYAN),
                    ),
                    Span::styled(" │ ", Style::default().fg(Palette::TEXT_DIM)),
                    Span::styled(
                        " [Enter] Done ",
                        Style::default()
                            .fg(Palette::TEXT_DARK)
                            .bg(Palette::SUCCESS_GREEN)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                    Span::styled(
                        " [Esc] Cancel ",
                        Style::default()
                            .fg(Palette::TEXT_MAIN)
                            .bg(Palette::DANGER_RED)
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Style::default().fg(Palette::WARNING_YELLOW),
            )
        } else {
            let mut spans = vec![
                Span::styled(
                    " ↑/k ",
                    Style::default()
                        .fg(Palette::TEXT_MAIN)
                        .bg(Color::Rgb(55, 75, 120))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " Up ",
                    Style::default()
                        .fg(Palette::TEXT_MUTED)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " ↓/j ",
                    Style::default()
                        .fg(Palette::TEXT_MAIN)
                        .bg(Color::Rgb(55, 75, 120))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " Down ",
                    Style::default()
                        .fg(Palette::TEXT_MUTED)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" │ ", Style::default().fg(Palette::TEXT_DIM)),
                Span::styled(
                    " Space ",
                    Style::default()
                        .fg(Palette::TEXT_DARK)
                        .bg(Palette::ORANGE)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " Toggle ",
                    Style::default()
                        .fg(Palette::WARNING_YELLOW)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" │ ", Style::default().fg(Palette::TEXT_DIM)),
                Span::styled(
                    " a ",
                    Style::default()
                        .fg(Palette::TEXT_DARK)
                        .bg(Palette::TEAL)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " Select All ",
                    Style::default()
                        .fg(Palette::TEAL)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" │ ", Style::default().fg(Palette::TEXT_DIM)),
                Span::styled(
                    " u ",
                    Style::default()
                        .fg(Palette::TEXT_MAIN)
                        .bg(Palette::PURPLE)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    " Unselect All ",
                    Style::default()
                        .fg(Palette::PINK)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(" │ ", Style::default().fg(Palette::TEXT_DIM)),
            ];

            if !app.filter_query.is_empty() {
                spans.push(Span::styled(
                    " c ",
                    Style::default()
                        .fg(Palette::TEXT_DARK)
                        .bg(Palette::WARNING_YELLOW)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    " Clear Filter ",
                    Style::default()
                        .fg(Palette::WARNING_YELLOW)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(
                    " / ",
                    Style::default()
                        .fg(Palette::TEXT_DARK)
                        .bg(Palette::CYAN)
                        .add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    " Filter ",
                    Style::default()
                        .fg(Palette::CYAN)
                        .add_modifier(Modifier::BOLD),
                ));
            }

            spans.push(Span::styled(" │ ", Style::default().fg(Palette::TEXT_DIM)));
            spans.push(Span::styled(
                " d ",
                Style::default()
                    .fg(Palette::TEXT_MAIN)
                    .bg(Palette::DANGER_RED)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                " Clean Selected ",
                Style::default()
                    .fg(Palette::PINK)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(" │ ", Style::default().fg(Palette::TEXT_DIM)));
            spans.push(Span::styled(
                " q/Esc ",
                Style::default()
                    .fg(Palette::TEXT_MAIN)
                    .bg(Palette::PURPLE)
                    .add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::styled(
                " Exit ",
                Style::default()
                    .fg(Palette::PINK)
                    .add_modifier(Modifier::BOLD),
            ));

            (
                Line::from(spans),
                Style::default().fg(Palette::BORDER_PRIMARY),
            )
        };

    let footer = Paragraph::new(footer_line)
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(footer_border_style),
        );

    f.render_widget(footer, area);
}

fn draw_confirm_dialog(f: &mut Frame, app: &TuiApp, area: Rect) {
    let width = 58.min(area.width.saturating_sub(4));
    let height = 11.min(area.height.saturating_sub(2));
    let popup_area = Rect {
        x: area.x + (area.width.saturating_sub(width)) / 2,
        y: area.y + (area.height.saturating_sub(height)) / 2,
        width,
        height,
    };

    f.render_widget(Clear, popup_area);

    let text = vec![
        Line::raw(""),
        Line::from(vec![Span::styled(
            "  Ready to clean selected build artifacts?",
            Style::default()
                .fg(Palette::TEXT_MAIN)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::raw(""),
        Line::from(vec![
            Span::styled("  Targets:    ", Style::default().fg(Palette::TEXT_MUTED)),
            Span::styled(
                format!("{} artifact(s)", app.selected_artifacts_count()),
                Style::default()
                    .fg(Palette::WARNING_YELLOW)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" across ", Style::default().fg(Palette::TEXT_MUTED)),
            Span::styled(
                format!("{} project(s)", app.selected_indices.len()),
                Style::default()
                    .fg(Palette::CYAN)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Reclaim:    ", Style::default().fg(Palette::TEXT_MUTED)),
            Span::styled(
                format_bytes(app.selected_reclaimable_bytes()),
                Style::default()
                    .fg(Palette::SUCCESS_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(" of disk space", Style::default().fg(Palette::TEXT_MUTED)),
        ]),
        Line::from(vec![
            Span::styled("  Safety:     ", Style::default().fg(Palette::TEXT_MUTED)),
            Span::styled(
                "Atomic rename to .sweep-trash (safe rollback)",
                Style::default().fg(Palette::CYAN),
            ),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::raw("    "),
            Span::styled(
                " [y / Enter] Confirm Clean ",
                Style::default()
                    .fg(Palette::TEXT_DARK)
                    .bg(Palette::SUCCESS_GREEN)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("    "),
            Span::styled(
                " [n / Esc] Cancel ",
                Style::default()
                    .fg(Palette::TEXT_MAIN)
                    .bg(Palette::DANGER_RED)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let popup = Paragraph::new(text).block(
        Block::default()
            .title(Span::styled(
                " 󰈸 Confirm Cleanup ",
                Style::default()
                    .fg(Palette::DANGER_RED)
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Palette::BORDER_ALERT)),
    );

    f.render_widget(popup, popup_area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::traits::{ArtifactTarget, DiscoveredArtifact, ProjectType};

    fn create_mock_projects() -> Vec<DiscoveredProject> {
        vec![
            DiscoveredProject {
                root: PathBuf::from("/mock/hero_backend"),
                project_type: ProjectType::Rust,
                artifacts: vec![DiscoveredArtifact {
                    target: ArtifactTarget {
                        name: "target",
                        rel_path: PathBuf::from("target"),
                        is_reconstructible: true,
                    },
                    abs_path: PathBuf::from("/mock/hero_backend/target"),
                    size_bytes: 1024,
                }],
                git_info: None,
            },
            DiscoveredProject {
                root: PathBuf::from("/mock/hdp_frontend"),
                project_type: ProjectType::Node,
                artifacts: vec![DiscoveredArtifact {
                    target: ArtifactTarget {
                        name: "node_modules",
                        rel_path: PathBuf::from("node_modules"),
                        is_reconstructible: true,
                    },
                    abs_path: PathBuf::from("/mock/hdp_frontend/node_modules"),
                    size_bytes: 2048,
                }],
                git_info: None,
            },
            DiscoveredProject {
                root: PathBuf::from("/mock/order_service"),
                project_type: ProjectType::Dotnet,
                artifacts: vec![DiscoveredArtifact {
                    target: ArtifactTarget {
                        name: "bin",
                        rel_path: PathBuf::from("bin"),
                        is_reconstructible: true,
                    },
                    abs_path: PathBuf::from("/mock/order_service/bin"),
                    size_bytes: 4096,
                }],
                git_info: None,
            },
        ]
    }

    #[test]
    fn test_tui_app_state_navigation() {
        let projects = create_mock_projects();
        let stats = ScanStats {
            dirs_inspected: 10,
            duration: Duration::from_millis(50),
        };

        let mut app = TuiApp::new(projects, stats, false);
        assert_eq!(app.cursor_index, 0);

        app.move_down();
        assert_eq!(app.cursor_index, 1);

        app.move_down();
        assert_eq!(app.cursor_index, 2);

        app.move_down();
        assert_eq!(app.cursor_index, 0); // Loops back to start

        app.move_up();
        assert_eq!(app.cursor_index, 2); // Loops to end

        // Toggle selection
        app.toggle_selection();
        assert!(app.selected_indices.contains(&2));
        assert_eq!(app.selected_reclaimable_bytes(), 4096);

        // Toggle all
        app.toggle_all();
        assert_eq!(app.selected_indices.len(), 3);
        assert_eq!(app.selected_reclaimable_bytes(), 7168);
    }

    #[test]
    fn test_tui_filtering_and_navigation() {
        let projects = create_mock_projects();
        let stats = ScanStats {
            dirs_inspected: 10,
            duration: Duration::from_millis(50),
        };

        let mut app = TuiApp::new(projects, stats, false);
        assert_eq!(app.filtered_indices.len(), 3);

        // Filter by ecosystem "node"
        app.filter_query = "node".to_string();
        app.apply_filter();
        assert_eq!(app.filtered_indices.len(), 1);
        assert_eq!(app.filtered_indices[0], 1); // Index 1 is node
        assert_eq!(app.cursor_index, 0);

        // Filter by path substring "hero"
        app.filter_query = "hero".to_string();
        app.apply_filter();
        assert_eq!(app.filtered_indices.len(), 1);
        assert_eq!(app.filtered_indices[0], 0);

        // Filter non-matching
        app.filter_query = "non_existent".to_string();
        app.apply_filter();
        assert_eq!(app.filtered_indices.len(), 0);

        // Clear filter
        app.filter_query.clear();
        app.apply_filter();
        assert_eq!(app.filtered_indices.len(), 3);
    }

    #[test]
    fn test_tui_unselect_all_and_filtered_toggle() {
        let projects = create_mock_projects();
        let stats = ScanStats {
            dirs_inspected: 10,
            duration: Duration::from_millis(50),
        };

        let mut app = TuiApp::new(projects, stats, false);

        // Select all
        app.toggle_all();
        assert_eq!(app.selected_indices.len(), 3);

        // Unselect all via 'u'
        app.unselect_all();
        assert_eq!(app.selected_indices.len(), 0);

        // Now filter by "node", select all matching
        app.filter_query = "node".to_string();
        app.apply_filter();
        app.toggle_all(); // Toggles only the filtered items!
        assert_eq!(app.selected_indices.len(), 1);
        assert!(app.selected_indices.contains(&1));

        // Clear filter: the node item remains selected
        app.filter_query.clear();
        app.apply_filter();
        assert_eq!(app.selected_indices.len(), 1);
        assert!(app.selected_indices.contains(&1));

        // Unselect all
        app.unselect_all();
        assert_eq!(app.selected_indices.len(), 0);
    }
}
