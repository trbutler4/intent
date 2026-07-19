use std::{io, time::Duration};

use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap},
    Frame, Terminal,
};
use review_tool::engine::{DiffLine, Finding, ReviewAction, ReviewSession};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusPane {
    Files,
    Diff,
    Findings,
}

struct TuiState {
    focus: FocusPane,
    diff_scroll: u16,
}

impl Default for TuiState {
    fn default() -> Self {
        Self {
            focus: FocusPane::Files,
            diff_scroll: 0,
        }
    }
}

fn main() -> io::Result<()> {
    let session = ReviewSession::load_current_worktree().map_err(io::Error::other)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run_app(&mut terminal, session);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut session: ReviewSession) -> io::Result<()> {
    let mut state = TuiState::default();

    loop {
        terminal.draw(|frame| render(frame, &session, &mut state))?;

        if !event::poll(Duration::from_millis(250))? {
            continue;
        }

        let Event::Key(key) = event::read()? else {
            continue;
        };

        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => break,
            KeyCode::Tab => state.focus = next_focus(state.focus),
            KeyCode::Left | KeyCode::Char('h') => state.focus = previous_focus(state.focus),
            KeyCode::Right | KeyCode::Char('l') => state.focus = next_focus(state.focus),
            KeyCode::Char('n') | KeyCode::Char(']') => {
                session.apply(ReviewAction::NextDiffChange);
                state.focus = FocusPane::Diff;
            }
            KeyCode::Char('p') | KeyCode::Char('[') => {
                session.apply(ReviewAction::PreviousDiffChange);
                state.focus = FocusPane::Diff;
            }
            KeyCode::Down | KeyCode::Char('j') => match state.focus {
                FocusPane::Files => session.apply(ReviewAction::NextFile),
                FocusPane::Diff => session.apply(ReviewAction::NextDiffChange),
                FocusPane::Findings => session.apply(ReviewAction::NextFinding),
            },
            KeyCode::Up | KeyCode::Char('k') => match state.focus {
                FocusPane::Files => session.apply(ReviewAction::PreviousFile),
                FocusPane::Diff => session.apply(ReviewAction::PreviousDiffChange),
                FocusPane::Findings => session.apply(ReviewAction::PreviousFinding),
            },
            _ => {}
        }
    }

    Ok(())
}

fn render(frame: &mut Frame, session: &ReviewSession, state: &mut TuiState) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(12),
            Constraint::Length(2),
        ])
        .split(frame.area());

    render_header(frame, layout[0], session);
    render_body(frame, layout[1], session, state);
    render_footer(frame, layout[2], state.focus);
}

fn render_header(frame: &mut Frame, area: Rect, session: &ReviewSession) {
    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            "Review Tool",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("repo: ", muted()),
            Span::raw(format!("{}  ", session.repo_label())),
            Span::styled("review: ", muted()),
            Span::raw(format!("{}  ", session.review_label())),
            Span::styled("ai: ", muted()),
            Span::raw(session.backend_label().to_owned()),
        ]),
    ])
    .block(Block::default().borders(Borders::BOTTOM));

    frame.render_widget(header, area);
}

fn render_body(frame: &mut Frame, area: Rect, session: &ReviewSession, state: &mut TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(32),
            Constraint::Min(36),
            Constraint::Length(42),
        ])
        .split(area);

    render_files(frame, chunks[0], session, state.focus == FocusPane::Files);
    render_diff(
        frame,
        chunks[1],
        session,
        state,
        state.focus == FocusPane::Diff,
    );
    render_findings(
        frame,
        chunks[2],
        session,
        state.focus == FocusPane::Findings,
    );
}

fn render_files(frame: &mut Frame, area: Rect, session: &ReviewSession, focused: bool) {
    let items = if session.files().is_empty() {
        vec![ListItem::new(vec![
            Line::from(Span::styled(
                "No changed files",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(session.review_summary().to_owned(), muted())),
        ])]
    } else {
        session
            .files()
            .iter()
            .map(|file| {
                ListItem::new(vec![
                    Line::from(vec![
                        Span::styled(format!("{} ", file.status), status_style(&file.status)),
                        Span::styled(
                            file.path.clone(),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(Span::styled(
                        format!("+{} -{}", file.additions, file.deletions),
                        muted(),
                    )),
                ])
            })
            .collect::<Vec<_>>()
    };
    let mut state = ListState::default();
    if !session.files().is_empty() {
        state.select(Some(session.selected_file_index()));
    }
    let list = List::new(items)
        .block(panel_block(
            format!("Changed Files ({})", session.files().len()),
            focused,
        ))
        .highlight_style(Style::default().bg(Color::Rgb(23, 37, 68)));

    frame.render_stateful_widget(list, area, &mut state);
}

fn render_diff(
    frame: &mut Frame,
    area: Rect,
    session: &ReviewSession,
    state: &mut TuiState,
    focused: bool,
) {
    let mut title = session
        .selected_file()
        .map(|file| format!("Diff - {}", file.path))
        .unwrap_or_else(|| "Diff".to_owned());
    if let Some((position, total)) = session.selected_diff_change_position() {
        title = format!("{title} - change {position}/{total}");
    }

    let mut lines = vec![Line::from(Span::styled(
        session
            .selected_file()
            .map(|file| {
                format!(
                    "Git diff for {} ({} additions, {} deletions)",
                    file.status, file.additions, file.deletions
                )
            })
            .unwrap_or_else(|| "No changed file is selected.".to_owned()),
        muted(),
    ))];

    lines.push(Line::raw(""));
    let selected_diff_line = session.selected_diff_line_index();
    lines.extend(
        session
            .diff_lines()
            .iter()
            .enumerate()
            .map(|(index, line)| render_diff_line(line, selected_diff_line == Some(index))),
    );

    ensure_selected_diff_visible(state, area, session);

    let diff = Paragraph::new(lines)
        .block(panel_block(&title, focused))
        .scroll((state.diff_scroll, 0));

    frame.render_widget(diff, area);
}

fn ensure_selected_diff_visible(state: &mut TuiState, area: Rect, session: &ReviewSession) {
    const DIFF_PREAMBLE_LINES: usize = 2;

    let Some(selected_diff_line) = session.selected_diff_line_index() else {
        state.diff_scroll = 0;
        return;
    };
    let visible_height = usize::from(area.height.saturating_sub(2));
    if visible_height == 0 {
        return;
    }

    let selected_display_line = selected_diff_line + DIFF_PREAMBLE_LINES;
    let current_scroll = usize::from(state.diff_scroll);
    let next_scroll = if selected_display_line < current_scroll {
        selected_display_line
    } else if selected_display_line >= current_scroll + visible_height {
        selected_display_line + 1 - visible_height
    } else {
        current_scroll
    };

    state.diff_scroll = next_scroll.min(usize::from(u16::MAX)) as u16;
}

fn render_findings(frame: &mut Frame, area: Rect, session: &ReviewSession, focused: bool) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Min(8),
            Constraint::Length(10),
        ])
        .split(area);

    let summary = Paragraph::new(session.review_summary())
        .block(panel_block("Review Summary", false))
        .wrap(Wrap { trim: true });
    frame.render_widget(summary, chunks[0]);

    let items = if session.findings().is_empty() {
        vec![ListItem::new(vec![
            Line::from(Span::styled(
                "No AI findings yet",
                Style::default().add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(
                "Real Git changes are loaded; a review backend is not wired in yet.",
                muted(),
            )),
        ])]
    } else {
        session
            .findings()
            .iter()
            .map(render_finding_item)
            .collect::<Vec<_>>()
    };
    let mut state = ListState::default();
    if !session.findings().is_empty() {
        state.select(Some(session.selected_finding_index()));
    }
    let findings = List::new(items)
        .block(panel_block(
            format!("AI Findings ({})", session.findings().len()),
            focused,
        ))
        .highlight_style(Style::default().bg(Color::Rgb(23, 37, 68)));
    frame.render_stateful_widget(findings, chunks[1], &mut state);

    render_selected_finding(frame, chunks[2], session.selected_finding());
}

fn render_finding_item(finding: &Finding) -> ListItem<'_> {
    ListItem::new(vec![
        Line::from(vec![
            Span::styled(
                finding.title.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                finding.severity.to_uppercase(),
                severity_style(&finding.severity),
            ),
        ]),
        Line::from(Span::styled(finding.line.clone(), muted())),
        Line::from(finding.summary.clone()),
    ])
}

fn render_selected_finding(frame: &mut Frame, area: Rect, finding: Option<&Finding>) {
    let lines = match finding {
        Some(finding) => vec![
            Line::from(vec![
                Span::styled(
                    finding.severity.to_uppercase(),
                    severity_style(&finding.severity),
                ),
                Span::raw(" "),
                Span::styled(
                    finding.title.clone(),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(Span::styled(finding.line.clone(), muted())),
            Line::raw(""),
            Line::from(finding.summary.clone()),
            Line::raw(""),
            Line::from(Span::styled(
                format!(
                    "Next: inspect {} and decide whether to comment on the active hunk.",
                    finding.line
                ),
                Style::default().fg(Color::Rgb(183, 198, 223)),
            )),
        ],
        None => vec![
            Line::raw("No AI findings yet."),
            Line::raw(""),
            Line::from(Span::styled(
                "The real Git diff is loaded; the review backend can be wired into this panel next.",
                muted(),
            )),
        ],
    };

    let selected = Paragraph::new(lines)
        .block(panel_block("Selected Finding", false))
        .wrap(Wrap { trim: true });
    frame.render_widget(selected, area);
}

fn render_diff_line(line: &DiffLine, selected: bool) -> Line<'static> {
    let style = match line.prefix.as_str() {
        "+" => Style::default().fg(Color::Green),
        "-" => Style::default().fg(Color::Red),
        "@@" => Style::default().fg(Color::Blue),
        _ => Style::default().fg(Color::White),
    };
    let style = if selected {
        style
            .bg(Color::Rgb(23, 37, 68))
            .add_modifier(Modifier::BOLD)
    } else {
        style
    };
    let marker = if selected { ">" } else { " " };

    Line::from(Span::styled(
        format!(
            "{} {:<2} {:>4} {}",
            marker, line.prefix, line.number, line.content
        ),
        style,
    ))
}

fn panel_block(title: impl Into<String>, focused: bool) -> Block<'static> {
    let border_style = if focused {
        Style::default().fg(Color::Rgb(79, 140, 255))
    } else {
        Style::default().fg(Color::Rgb(30, 42, 69))
    };

    Block::default()
        .borders(Borders::ALL)
        .title(title.into())
        .border_style(border_style)
}

fn render_footer(frame: &mut Frame, area: Rect, focus: FocusPane) {
    let focused = match focus {
        FocusPane::Files => "files",
        FocusPane::Diff => "diff",
        FocusPane::Findings => "findings",
    };
    let footer = Paragraph::new(format!(
        "focus: {} | tab/h/l switch panes | j/k move pane | n/p or ]/[ jump diff changes | q quit",
        focused
    ))
    .style(muted());

    frame.render_widget(footer, area);
}

fn next_focus(focus: FocusPane) -> FocusPane {
    match focus {
        FocusPane::Files => FocusPane::Diff,
        FocusPane::Diff => FocusPane::Findings,
        FocusPane::Findings => FocusPane::Files,
    }
}

fn previous_focus(focus: FocusPane) -> FocusPane {
    match focus {
        FocusPane::Files => FocusPane::Findings,
        FocusPane::Diff => FocusPane::Files,
        FocusPane::Findings => FocusPane::Diff,
    }
}

fn status_style(status: &str) -> Style {
    match status {
        "A" => Style::default().fg(Color::Green),
        "M" => Style::default().fg(Color::Blue),
        "D" => Style::default().fg(Color::Red),
        _ => Style::default().fg(Color::Gray),
    }
}

fn severity_style(severity: &str) -> Style {
    match severity {
        "high" => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        "medium" => Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
        "low" => Style::default()
            .fg(Color::Blue)
            .add_modifier(Modifier::BOLD),
        _ => Style::default().fg(Color::Gray),
    }
}

fn muted() -> Style {
    Style::default().fg(Color::Rgb(145, 163, 192))
}
