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
    Block, BorderType, Borders, Clear, Gauge, HighlightSpacing, Paragraph, Row, Table, TableState,
    Wrap,
};

use crate::core::cleaner::Cleaner;
use crate::core::engine::ScanStats;
use crate::core::size::format_bytes;
use crate::core::traits::{DiscoveredProject, GitInfo, GitStatus, ProjectType};

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

        let mut table_state = TableState::default();
        if !projects.is_empty() {
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
        }
    }

    pub fn move_up(&mut self) {
        if self.projects.is_empty() {
            return;
        }
        if self.cursor_index > 0 {
            self.cursor_index -= 1;
        } else {
            self.cursor_index = self.projects.len() - 1;
        }
        self.table_state.select(Some(self.cursor_index));
    }

    pub fn move_down(&mut self) {
        if self.projects.is_empty() {
            return;
        }
        if self.cursor_index + 1 < self.projects.len() {
            self.cursor_index += 1;
        } else {
            self.cursor_index = 0;
        }
        self.table_state.select(Some(self.cursor_index));
    }

    pub fn toggle_selection(&mut self) {
        if self.projects.is_empty() {
            return;
        }
        if self.selected_indices.contains(&self.cursor_index) {
            self.selected_indices.remove(&self.cursor_index);
        } else {
            self.selected_indices.insert(self.cursor_index);
        }
    }

    pub fn toggle_all(&mut self) {
        if self.selected_indices.len() == self.projects.len() {
            self.selected_indices.clear();
        } else {
            self.selected_indices = (0..self.projects.len()).collect();
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
                    .fg(Color::Cyan)
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
                                Style::default().fg(Color::Red),
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
            self.cursor_index = 0;
            if !self.projects.is_empty() {
                self.table_state.select(Some(0));
            } else {
                self.table_state.select(None);
            }

            self.notification_message = Some((
                format!(
                    "✔ Cleaned {} artifact(s) successfully! Reclaimed {}",
                    report.items_cleaned,
                    format_bytes(report.total_bytes_reclaimed)
                ),
                Style::default()
                    .fg(Color::Green)
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
            } else {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        app.should_quit = true;
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

    // Main layout: Header, Central Body, Footer
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(3),
        ])
        .split(size);

    // 1. Header
    let header_text = vec![Line::from(vec![
        Span::styled(
            " SWEEP-RS ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            " High-Performance Git-Aware Workspace Cleaner",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  │  Inspected ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{} dirs", app.stats.dirs_inspected),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" in ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{:.2}s", app.stats.duration.as_secs_f64()),
            Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  │  ", Style::default().fg(Color::Gray)),
        Span::styled(
            format!("{} projects found", app.projects.len()),
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        ),
    ])];
    let header_block = Paragraph::new(header_text).block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Color::Cyan)),
    );
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
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
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
    let rows: Vec<Row> = app
        .projects
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let is_selected = app.selected_indices.contains(&i);
            let check_mark = if is_selected {
                Span::styled(
                    "[✓]",
                    Style::default()
                        .fg(Color::Rgb(80, 245, 140))
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled("[ ]", Style::default().fg(Color::Rgb(100, 115, 145)))
            };

            let path_str = p.root.display().to_string();
            let display_path = if path_str.len() > 28 {
                format!("...{}", &path_str[path_str.len() - 25..])
            } else {
                path_str
            };

            let type_style = match p.project_type {
                ProjectType::Rust => Style::default()
                    .fg(Color::Rgb(255, 145, 75))
                    .add_modifier(Modifier::BOLD),
                ProjectType::Node => Style::default()
                    .fg(Color::Rgb(95, 230, 110))
                    .add_modifier(Modifier::BOLD),
                ProjectType::Python => Style::default()
                    .fg(Color::Rgb(70, 185, 255))
                    .add_modifier(Modifier::BOLD),
                ProjectType::Dotnet => Style::default()
                    .fg(Color::Rgb(190, 125, 255))
                    .add_modifier(Modifier::BOLD),
                _ => Style::default()
                    .fg(Color::Rgb(255, 215, 80))
                    .add_modifier(Modifier::BOLD),
            };

            let (status_text, status_style) = match &p.git_info {
                Some(info) => {
                    if info.is_dirty {
                        (
                            format!("✖ {} (Dirty)", info.status),
                            Style::default()
                                .fg(Color::Rgb(255, 85, 85))
                                .add_modifier(Modifier::BOLD),
                        )
                    } else {
                        match info.status {
                            GitStatus::Stale => (
                                "● Stale".to_string(),
                                Style::default()
                                    .fg(Color::Rgb(75, 235, 130))
                                    .add_modifier(Modifier::BOLD),
                            ),
                            GitStatus::Active => (
                                "▲ Active".to_string(),
                                Style::default().fg(Color::Rgb(255, 205, 60)),
                            ),
                            GitStatus::Moderate => (
                                "◆ Moderate".to_string(),
                                Style::default().fg(Color::Rgb(85, 195, 255)),
                            ),
                            GitStatus::Unknown => (
                                "? Unknown".to_string(),
                                Style::default().fg(Color::Rgb(130, 140, 165)),
                            ),
                        }
                    }
                }
                None => (
                    "- No Git".to_string(),
                    Style::default().fg(Color::Rgb(120, 130, 150)),
                ),
            };

            let reclaimable_bytes = p.total_reclaimable_bytes();
            let size_style = if reclaimable_bytes >= 1024 * 1024 * 1024 {
                Style::default()
                    .fg(Color::Rgb(255, 95, 135))
                    .add_modifier(Modifier::BOLD)
            } else if reclaimable_bytes >= 100 * 1024 * 1024 {
                Style::default()
                    .fg(Color::Rgb(255, 210, 65))
                    .add_modifier(Modifier::BOLD)
            } else if reclaimable_bytes >= 10 * 1024 * 1024 {
                Style::default().fg(Color::Rgb(85, 225, 255))
            } else {
                Style::default().fg(Color::Rgb(185, 200, 220))
            };

            Row::new(vec![
                check_mark,
                Span::styled(display_path, Style::default().fg(Color::White)),
                Span::styled(p.project_type.to_string(), type_style),
                Span::styled(status_text, status_style),
                Span::styled(format_bytes(reclaimable_bytes), size_style),
            ])
        })
        .collect();

    let header = Row::new(vec![
        Span::styled(
            "SEL",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "PROJECT PATH",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "TYPE",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "GIT STATUS",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            "RECLAIMABLE",
            Style::default()
                .fg(Color::Cyan)
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

    let table = Table::new(rows, widths)
        .header(header)
        .block(
            Block::default()
                .title(Span::styled(
                    " Discovered Cleanable Projects ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Rgb(70, 110, 170))),
        )
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(Color::White),
        )
        .highlight_symbol("▶ ")
        .highlight_spacing(HighlightSpacing::Always);

    f.render_stateful_widget(table, area, &mut app.table_state);
}

fn draw_inspector(f: &mut Frame, app: &TuiApp, area: Rect) {
    let content = if let Some(p) = app.projects.get(app.cursor_index) {
        let mut lines = Vec::new();
        lines.push(Line::from(vec![
            Span::styled(
                "Project Root: ",
                Style::default()
                    .fg(Color::Rgb(140, 185, 255))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                p.root.display().to_string(),
                Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            ),
        ]));

        let type_style = match p.project_type {
            ProjectType::Rust => Style::default()
                .fg(Color::Rgb(255, 145, 75))
                .add_modifier(Modifier::BOLD),
            ProjectType::Node => Style::default()
                .fg(Color::Rgb(95, 230, 110))
                .add_modifier(Modifier::BOLD),
            ProjectType::Python => Style::default()
                .fg(Color::Rgb(70, 185, 255))
                .add_modifier(Modifier::BOLD),
            ProjectType::Dotnet => Style::default()
                .fg(Color::Rgb(190, 125, 255))
                .add_modifier(Modifier::BOLD),
            _ => Style::default()
                .fg(Color::Rgb(255, 215, 80))
                .add_modifier(Modifier::BOLD),
        };

        lines.push(Line::from(vec![
            Span::styled(
                "Ecosystem:    ",
                Style::default()
                    .fg(Color::Rgb(140, 185, 255))
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
                        .fg(Color::Rgb(140, 185, 255))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    commit_age,
                    Style::default()
                        .fg(Color::Rgb(255, 215, 75))
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
            lines.push(Line::from(vec![
                Span::styled(
                    "Worktree:     ",
                    Style::default()
                        .fg(Color::Rgb(140, 185, 255))
                        .add_modifier(Modifier::BOLD),
                ),
                if git.is_dirty {
                    Span::styled(
                        "✖ Uncommitted changes present (Dirty)",
                        Style::default()
                            .fg(Color::Rgb(255, 85, 85))
                            .add_modifier(Modifier::BOLD),
                    )
                } else {
                    Span::styled(
                        "✓ Clean",
                        Style::default()
                            .fg(Color::Rgb(75, 235, 130))
                            .add_modifier(Modifier::BOLD),
                    )
                },
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::styled(
                    "Git:          ",
                    Style::default()
                        .fg(Color::Rgb(140, 185, 255))
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    "Not a Git repository",
                    Style::default().fg(Color::Rgb(150, 160, 180)),
                ),
            ]));
        }

        lines.push(Line::raw(""));
        lines.push(Line::from(Span::styled(
            "Cleanable Artifact Targets:",
            Style::default()
                .fg(Color::Rgb(255, 205, 60))
                .add_modifier(Modifier::BOLD),
        )));

        for artifact in &p.artifacts {
            lines.push(Line::from(vec![
                Span::styled("  ✦ ", Style::default().fg(Color::Rgb(0, 230, 255))),
                Span::styled(
                    artifact.target.name,
                    Style::default()
                        .fg(Color::White)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    format!(" ({})", format_bytes(artifact.size_bytes)),
                    Style::default()
                        .fg(Color::Rgb(255, 215, 75))
                        .add_modifier(Modifier::BOLD),
                ),
            ]));
        }

        lines
    } else {
        vec![Line::from(Span::styled(
            "No project selected",
            Style::default().fg(Color::Rgb(130, 140, 160)),
        ))]
    };

    let p = Paragraph::new(content)
        .block(
            Block::default()
                .title(Span::styled(
                    " Project Inspector ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Rgb(70, 110, 170))),
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

    let (gauge_fg, gauge_bg) = if ratio == 0.0 {
        (Color::Rgb(85, 105, 140), Color::Rgb(25, 32, 45))
    } else if ratio < 0.35 {
        (Color::Rgb(0, 215, 185), Color::Rgb(18, 48, 48))
    } else if ratio < 0.75 {
        (Color::Rgb(55, 220, 115), Color::Rgb(20, 50, 30))
    } else {
        (Color::Rgb(255, 175, 45), Color::Rgb(55, 38, 15))
    };

    let label = format!(
        "{} / {} ({:.0}%)",
        format_bytes(selected),
        format_bytes(total),
        ratio * 100.0
    );

    let gauge = Gauge::default()
        .block(
            Block::default()
                .title(Span::styled(
                    " Selected Reclaimable Space ",
                    Style::default()
                        .fg(Color::Cyan)
                        .add_modifier(Modifier::BOLD),
                ))
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Color::Rgb(70, 110, 170))),
        )
        .gauge_style(
            Style::default()
                .fg(gauge_fg)
                .bg(gauge_bg)
                .add_modifier(Modifier::BOLD),
        )
        .ratio(ratio)
        .label(Span::styled(
            label,
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        ));

    f.render_widget(gauge, area);
}

fn draw_footer(f: &mut Frame, app: &TuiApp, area: Rect) {
    let (footer_line, footer_border_style) =
        if let Some((ref msg, style)) = app.notification_message {
            (
                Line::from(vec![Span::styled(msg.clone(), style)]),
                Style::default().fg(Color::Yellow),
            )
        } else {
            (
                Line::from(vec![
                    Span::styled(
                        " ↑/k ",
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Rgb(45, 75, 150))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " Up ",
                        Style::default()
                            .fg(Color::Rgb(180, 215, 255))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " ↓/j ",
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Rgb(45, 75, 150))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " Down ",
                        Style::default()
                            .fg(Color::Rgb(180, 215, 255))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(65, 80, 115))),
                    Span::styled(
                        " Space ",
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(245, 175, 45))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " Toggle ",
                        Style::default()
                            .fg(Color::Rgb(255, 225, 130))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(65, 80, 115))),
                    Span::styled(
                        " a ",
                        Style::default()
                            .fg(Color::Black)
                            .bg(Color::Rgb(30, 190, 165))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " Select All ",
                        Style::default()
                            .fg(Color::Rgb(120, 250, 230))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(65, 80, 115))),
                    Span::styled(
                        " d ",
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Rgb(220, 45, 75))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " Clean Selected ",
                        Style::default()
                            .fg(Color::Rgb(255, 130, 155))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(" │ ", Style::default().fg(Color::Rgb(65, 80, 115))),
                    Span::styled(
                        " q/Esc ",
                        Style::default()
                            .fg(Color::White)
                            .bg(Color::Rgb(135, 65, 185))
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        " Exit ",
                        Style::default()
                            .fg(Color::Rgb(225, 185, 255))
                            .add_modifier(Modifier::BOLD),
                    ),
                ]),
                Style::default().fg(Color::Rgb(85, 115, 165)),
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
    let popup_area = centered_rect(50, 25, area);

    f.render_widget(Clear, popup_area);

    let text = vec![
        Line::raw(""),
        Line::from(vec![
            Span::raw("Are you sure you want to clean "),
            Span::styled(
                format!("{} artifact(s)", app.selected_artifacts_count()),
                Style::default()
                    .fg(Color::Rgb(255, 215, 75))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("?"),
        ]),
        Line::from(vec![
            Span::raw("Reclaiming "),
            Span::styled(
                format_bytes(app.selected_reclaimable_bytes()),
                Style::default()
                    .fg(Color::Rgb(75, 240, 135))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" of disk space."),
        ]),
        Line::raw(""),
        Line::from(vec![
            Span::styled(
                " [y / Enter] Confirm ",
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Rgb(50, 220, 110))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw("     "),
            Span::styled(
                " [n / Esc] Cancel ",
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Rgb(215, 45, 70))
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
    ];

    let popup = Paragraph::new(text).alignment(Alignment::Center).block(
        Block::default()
            .title(Span::styled(
                " Confirm Deletion ",
                Style::default()
                    .fg(Color::Rgb(255, 80, 80))
                    .add_modifier(Modifier::BOLD),
            ))
            .borders(Borders::ALL)
            .border_type(BorderType::Double)
            .border_style(Style::default().fg(Color::Rgb(255, 80, 80))),
    );

    f.render_widget(popup, popup_area);
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
    use crate::core::traits::{ArtifactTarget, DiscoveredArtifact, ProjectType};

    #[test]
    fn test_tui_app_state_navigation() {
        let projects = vec![
            DiscoveredProject {
                root: PathBuf::from("/mock/app1"),
                project_type: ProjectType::Rust,
                artifacts: vec![DiscoveredArtifact {
                    target: ArtifactTarget {
                        name: "target",
                        rel_path: PathBuf::from("target"),
                        is_reconstructible: true,
                    },
                    abs_path: PathBuf::from("/mock/app1/target"),
                    size_bytes: 1024,
                }],
                git_info: None,
            },
            DiscoveredProject {
                root: PathBuf::from("/mock/app2"),
                project_type: ProjectType::Node,
                artifacts: vec![DiscoveredArtifact {
                    target: ArtifactTarget {
                        name: "node_modules",
                        rel_path: PathBuf::from("node_modules"),
                        is_reconstructible: true,
                    },
                    abs_path: PathBuf::from("/mock/app2/node_modules"),
                    size_bytes: 2048,
                }],
                git_info: None,
            },
        ];

        let stats = ScanStats {
            dirs_inspected: 10,
            duration: Duration::from_millis(50),
        };

        let mut app = TuiApp::new(projects, stats, false);
        assert_eq!(app.cursor_index, 0);

        app.move_down();
        assert_eq!(app.cursor_index, 1);

        app.move_down();
        assert_eq!(app.cursor_index, 0); // Loops back to start

        app.move_up();
        assert_eq!(app.cursor_index, 1); // Loops to end

        // Toggle selection
        app.toggle_selection();
        assert!(app.selected_indices.contains(&1));
        assert_eq!(app.selected_reclaimable_bytes(), 2048);

        // Toggle all
        app.toggle_all();
        assert_eq!(app.selected_indices.len(), 2);
        assert_eq!(app.selected_reclaimable_bytes(), 3072);
    }
}
