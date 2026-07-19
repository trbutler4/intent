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
    Findings,
}

fn main() -> io::Result<()> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run_app(&mut terminal, ReviewSession::mock());

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut session: ReviewSession) -> io::Result<()> {
    let mut focus = FocusPane::Files;

    loop {
        terminal.draw(|frame| render(frame, &session, focus))?;

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
            KeyCode::Tab => focus = toggle_focus(focus),
            KeyCode::Left | KeyCode::Char('h') => focus = FocusPane::Files,
            KeyCode::Right | KeyCode::Char('l') => focus = FocusPane::Findings,
            KeyCode::Down | KeyCode::Char('j') => match focus {
                FocusPane::Files => session.apply(ReviewAction::NextFile),
                FocusPane::Findings => session.apply(ReviewAction::NextFinding),
            },
            KeyCode::Up | KeyCode::Char('k') => match focus {
                FocusPane::Files => session.apply(ReviewAction::PreviousFile),
                FocusPane::Findings => session.apply(ReviewAction::PreviousFinding),
            },
            _ => {}
        }
    }

    Ok(())
}

fn render(frame: &mut Frame, session: &ReviewSession, focus: FocusPane) {
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(4),
            Constraint::Min(12),
            Constraint::Length(2),
        ])
        .split(frame.area());

    render_header(frame, layout[0]);
    render_body(frame, layout[1], session, focus);
    render_footer(frame, layout[2], focus);
}

fn render_header(frame: &mut Frame, area: Rect) {
    let header = Paragraph::new(vec![
        Line::from(Span::styled(
            "Review Tool",
            Style::default()
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled("repo: ", muted()),
            Span::raw("zed-industries/zed  "),
            Span::styled("review: ", muted()),
            Span::raw("main...feature/ai-review  "),
            Span::styled("model: ", muted()),
            Span::raw("mock:gpt-reviewer"),
        ]),
    ])
    .block(Block::default().borders(Borders::BOTTOM));

    frame.render_widget(header, area);
}

fn render_body(frame: &mut Frame, area: Rect, session: &ReviewSession, focus: FocusPane) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(32),
            Constraint::Min(36),
            Constraint::Length(42),
        ])
        .split(area);

    render_files(frame, chunks[0], session, focus == FocusPane::Files);
    render_diff(frame, chunks[1], session);
    render_findings(frame, chunks[2], session, focus == FocusPane::Findings);
}

fn render_files(frame: &mut Frame, area: Rect, session: &ReviewSession, focused: bool) {
    let items = session
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
        .collect::<Vec<_>>();
    let mut state = ListState::default();
    state.select(Some(session.selected_file_index()));
    let list = List::new(items)
        .block(panel_block(
            format!("Changed Files ({})", session.files().len()),
            focused,
        ))
        .highlight_style(Style::default().bg(Color::Rgb(23, 37, 68)));

    frame.render_stateful_widget(list, area, &mut state);
}

fn render_diff(frame: &mut Frame, area: Rect, session: &ReviewSession) {
    let title = session
        .selected_file()
        .map(|file| format!("Diff - {}", file.path))
        .unwrap_or_else(|| "Diff".to_owned());
    let mut lines = vec![Line::from(Span::styled(
        session
            .selected_file()
            .map(|file| {
                format!(
                    "Mock review context for {} ({} additions, {} deletions)",
                    file.status, file.additions, file.deletions
                )
            })
            .unwrap_or_else(|| "No changed file is selected.".to_owned()),
        muted(),
    ))];

    lines.push(Line::raw(""));
    lines.extend(session.diff_lines().iter().map(render_diff_line));

    let diff = Paragraph::new(lines)
        .block(panel_block(&title, false))
        .wrap(Wrap { trim: false });

    frame.render_widget(diff, area);
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

    let items = session
        .findings()
        .iter()
        .map(render_finding_item)
        .collect::<Vec<_>>();
    let mut state = ListState::default();
    state.select(Some(session.selected_finding_index()));
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
                    "Next: bind {} to a real git diff parser and send only the active hunk plus nearby context to the model.",
                    finding.line
                ),
                Style::default().fg(Color::Rgb(183, 198, 223)),
            )),
        ],
        None => vec![Line::raw("Select a finding to see details.")],
    };

    let selected = Paragraph::new(lines)
        .block(panel_block("Selected Finding", false))
        .wrap(Wrap { trim: true });
    frame.render_widget(selected, area);
}

fn render_diff_line(line: &DiffLine) -> Line<'_> {
    let style = match line.prefix.as_str() {
        "+" => Style::default().fg(Color::Green),
        "-" => Style::default().fg(Color::Red),
        "@@" => Style::default().fg(Color::Blue),
        _ => Style::default().fg(Color::White),
    };

    Line::from(Span::styled(
        format!("{:<2} {:>4} {}", line.prefix, line.number, line.content),
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
        FocusPane::Findings => "findings",
    };
    let footer = Paragraph::new(format!(
        "focus: {} | tab/left/right switch panes | j/k or arrows move | q quit",
        focused
    ))
    .style(muted());

    frame.render_widget(footer, area);
}

fn toggle_focus(focus: FocusPane) -> FocusPane {
    match focus {
        FocusPane::Files => FocusPane::Findings,
        FocusPane::Findings => FocusPane::Files,
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
