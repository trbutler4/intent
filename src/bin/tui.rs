use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    time::Duration,
};

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
use review_tool::engine::{DiffLine, FileChange, Finding, ReviewAction, ReviewSession};

const BG: Color = Color::Rgb(40, 40, 40);
const BG_SOFT: Color = Color::Rgb(60, 56, 54);
const FG: Color = Color::Rgb(235, 219, 178);
const MUTED: Color = Color::Rgb(146, 131, 116);
const RED: Color = Color::Rgb(251, 73, 52);
const GREEN: Color = Color::Rgb(184, 187, 38);
const YELLOW: Color = Color::Rgb(250, 189, 47);
const BLUE: Color = Color::Rgb(131, 165, 152);
const AQUA: Color = Color::Rgb(142, 192, 124);
const ORANGE: Color = Color::Rgb(254, 128, 25);
const PURPLE: Color = Color::Rgb(211, 134, 155);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusPane {
    Files,
    Diff,
    Findings,
}

struct TuiState {
    focus: FocusPane,
    diff_scroll: u16,
    expanded_dirs: BTreeSet<String>,
    file_cursor: usize,
}

impl TuiState {
    fn new(session: &ReviewSession) -> Self {
        let expanded_dirs = directory_paths(session.files());
        let rows = visible_file_rows(session.files(), &expanded_dirs);
        let file_cursor = rows
            .iter()
            .position(|row| match row {
                FileTreeRow::File { index, .. } => *index == session.selected_file_index(),
                FileTreeRow::Directory { .. } => false,
            })
            .unwrap_or(0);

        Self {
            focus: FocusPane::Files,
            diff_scroll: 0,
            expanded_dirs,
            file_cursor,
        }
    }
}

#[derive(Default)]
struct FileTreeNode {
    name: String,
    path: String,
    dirs: BTreeMap<String, FileTreeNode>,
    files: Vec<usize>,
    file_count: usize,
    additions: usize,
    deletions: usize,
}

impl FileTreeNode {
    fn new(name: impl Into<String>, path: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            path: path.into(),
            ..Default::default()
        }
    }
}

enum FileTreeRow {
    Directory {
        path: String,
        name: String,
        depth: usize,
        expanded: bool,
        file_count: usize,
        additions: usize,
        deletions: usize,
    },
    File {
        index: usize,
        path: String,
        name: String,
        depth: usize,
    },
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
    let mut state = TuiState::new(&session);

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
            KeyCode::Left | KeyCode::Char('h') => {
                if state.focus == FocusPane::Files {
                    collapse_file_tree_row(&session, &mut state);
                } else {
                    state.focus = previous_focus(state.focus);
                }
            }
            KeyCode::Right | KeyCode::Char('l') => {
                if state.focus == FocusPane::Files {
                    expand_file_tree_row(&mut session, &mut state);
                } else {
                    state.focus = next_focus(state.focus);
                }
            }
            KeyCode::Enter | KeyCode::Char(' ') if state.focus == FocusPane::Files => {
                toggle_file_tree_row(&mut session, &mut state);
            }
            KeyCode::Char('n') | KeyCode::Char(']') => {
                session.apply(ReviewAction::NextDiffChange);
                state.focus = FocusPane::Diff;
            }
            KeyCode::Char('p') | KeyCode::Char('[') => {
                session.apply(ReviewAction::PreviousDiffChange);
                state.focus = FocusPane::Diff;
            }
            KeyCode::Down | KeyCode::Char('j') => match state.focus {
                FocusPane::Files => move_file_cursor(&mut session, &mut state, 1),
                FocusPane::Diff => session.apply(ReviewAction::NextDiffChange),
                FocusPane::Findings => session.apply(ReviewAction::NextFinding),
            },
            KeyCode::Up | KeyCode::Char('k') => match state.focus {
                FocusPane::Files => move_file_cursor(&mut session, &mut state, -1),
                FocusPane::Diff => session.apply(ReviewAction::PreviousDiffChange),
                FocusPane::Findings => session.apply(ReviewAction::PreviousFinding),
            },
            _ => {}
        }
    }

    Ok(())
}

fn render(frame: &mut Frame, session: &ReviewSession, state: &mut TuiState) {
    frame.render_widget(Block::default().style(base()), frame.area());

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(12),
            Constraint::Length(1),
        ])
        .split(frame.area());

    render_header(frame, layout[0], session);
    render_body(frame, layout[1], session, state);
    render_footer(frame, layout[2], state.focus);
}

fn render_header(frame: &mut Frame, area: Rect, session: &ReviewSession) {
    let header = Paragraph::new(vec![
        Line::from(vec![
            Span::styled("intent", base().fg(YELLOW).add_modifier(Modifier::BOLD)),
            Span::styled("  code review", muted()),
        ]),
        Line::from(vec![
            Span::styled(session.repo_label().to_owned(), base().fg(AQUA)),
            Span::styled(" / ", muted()),
            Span::styled(session.review_label().to_owned(), base().fg(BLUE)),
            Span::styled(" / ai ", muted()),
            Span::styled(session.backend_label().to_owned(), base().fg(MUTED)),
        ]),
    ])
    .style(base());

    frame.render_widget(header, area);
}

fn render_body(frame: &mut Frame, area: Rect, session: &ReviewSession, state: &mut TuiState) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28),
            Constraint::Min(42),
            Constraint::Length(34),
        ])
        .split(area);

    render_files(
        frame,
        chunks[0],
        session,
        state,
        state.focus == FocusPane::Files,
    );
    render_diff(
        frame,
        chunks[1],
        session,
        state,
        state.focus == FocusPane::Diff,
    );
    render_review(
        frame,
        chunks[2],
        session,
        state.focus == FocusPane::Findings,
    );
}

fn render_files(
    frame: &mut Frame,
    area: Rect,
    session: &ReviewSession,
    state: &mut TuiState,
    focused: bool,
) {
    let rows = visible_file_rows(session.files(), &state.expanded_dirs);
    clamp_file_cursor(state, rows.len());

    let items = if session.files().is_empty() {
        vec![ListItem::new(vec![
            Line::from(Span::styled(
                "clean working tree",
                base().fg(FG).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(session.review_summary().to_owned(), muted())),
        ])]
    } else {
        rows.iter()
            .map(|row| render_file_tree_row(row, session.files()))
            .collect::<Vec<_>>()
    };
    let mut list_state = ListState::default();
    if !session.files().is_empty() {
        list_state.select(Some(state.file_cursor));
    }
    let list = List::new(items)
        .block(panel_block(
            format!("files ({})", session.files().len()),
            focused,
        ))
        .style(base())
        .highlight_style(base().fg(FG).bg(BG_SOFT).add_modifier(Modifier::BOLD));

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn render_file_tree_row(row: &FileTreeRow, files: &[FileChange]) -> ListItem<'static> {
    match row {
        FileTreeRow::Directory {
            name,
            depth,
            expanded,
            file_count,
            additions,
            deletions,
            ..
        } => {
            let marker = if *expanded { "[-]" } else { "[+]" };
            ListItem::new(Line::from(vec![
                Span::styled(indent(*depth), muted()),
                Span::styled(format!("{marker} "), base().fg(YELLOW)),
                Span::styled(name.clone(), base().fg(AQUA).add_modifier(Modifier::BOLD)),
                Span::styled(
                    format!("  {file_count} files  +{additions} -{deletions}"),
                    muted(),
                ),
            ]))
        }
        FileTreeRow::File {
            index, name, depth, ..
        } => {
            let file = &files[*index];
            ListItem::new(Line::from(vec![
                Span::styled(indent(*depth), muted()),
                Span::styled(format!("{} ", file.status), status_style(&file.status)),
                Span::styled(name.clone(), base().fg(FG)),
                Span::styled(
                    format!("  +{} -{}", file.additions, file.deletions),
                    muted(),
                ),
            ]))
        }
    }
}

fn move_file_cursor(session: &mut ReviewSession, state: &mut TuiState, delta: isize) {
    let rows = visible_file_rows(session.files(), &state.expanded_dirs);
    if rows.is_empty() {
        state.file_cursor = 0;
        return;
    }

    let max_index = rows.len() - 1;
    let next = (state.file_cursor.min(max_index) as isize + delta).clamp(0, max_index as isize);
    state.file_cursor = next as usize;
    select_file_row_if_file(session, &rows[state.file_cursor]);
}

fn toggle_file_tree_row(session: &mut ReviewSession, state: &mut TuiState) {
    let rows = visible_file_rows(session.files(), &state.expanded_dirs);
    let Some(row) = rows.get(state.file_cursor) else {
        return;
    };

    match row {
        FileTreeRow::Directory { path, expanded, .. } => {
            if *expanded {
                state.expanded_dirs.remove(path);
            } else {
                state.expanded_dirs.insert(path.clone());
            }
            clamp_file_cursor(
                state,
                visible_file_rows(session.files(), &state.expanded_dirs).len(),
            );
        }
        FileTreeRow::File { .. } => select_file_row_if_file(session, row),
    }
}

fn expand_file_tree_row(session: &mut ReviewSession, state: &mut TuiState) {
    let rows = visible_file_rows(session.files(), &state.expanded_dirs);
    let Some(row) = rows.get(state.file_cursor) else {
        return;
    };

    match row {
        FileTreeRow::Directory { path, .. } => {
            state.expanded_dirs.insert(path.clone());
        }
        FileTreeRow::File { .. } => select_file_row_if_file(session, row),
    }
}

fn collapse_file_tree_row(session: &ReviewSession, state: &mut TuiState) {
    let rows = visible_file_rows(session.files(), &state.expanded_dirs);
    let Some(row) = rows.get(state.file_cursor) else {
        return;
    };

    match row {
        FileTreeRow::Directory { path, expanded, .. } => {
            if *expanded {
                state.expanded_dirs.remove(path);
            } else {
                collapse_parent_directory(session, state, path);
            }
        }
        FileTreeRow::File { path, .. } => collapse_parent_directory(session, state, path),
    }

    clamp_file_cursor(
        state,
        visible_file_rows(session.files(), &state.expanded_dirs).len(),
    );
}

fn collapse_parent_directory(session: &ReviewSession, state: &mut TuiState, path: &str) {
    let Some(parent) = parent_dir_path(path) else {
        return;
    };

    state.expanded_dirs.remove(&parent);
    let rows = visible_file_rows(session.files(), &state.expanded_dirs);
    if let Some(index) = rows.iter().position(|row| match row {
        FileTreeRow::Directory { path, .. } => path == &parent,
        FileTreeRow::File { .. } => false,
    }) {
        state.file_cursor = index;
    }
}

fn select_file_row_if_file(session: &mut ReviewSession, row: &FileTreeRow) {
    if let FileTreeRow::File { index, .. } = row {
        session.apply(ReviewAction::SelectFile(*index));
    }
}

fn clamp_file_cursor(state: &mut TuiState, row_count: usize) {
    if row_count == 0 {
        state.file_cursor = 0;
    } else if state.file_cursor >= row_count {
        state.file_cursor = row_count - 1;
    }
}

fn visible_file_rows(files: &[FileChange], expanded_dirs: &BTreeSet<String>) -> Vec<FileTreeRow> {
    let tree = build_file_tree(files);
    let mut rows = Vec::new();
    collect_file_tree_rows(&tree, files, expanded_dirs, &mut rows, 0);
    rows
}

fn build_file_tree(files: &[FileChange]) -> FileTreeNode {
    let mut root = FileTreeNode::default();

    for (index, file) in files.iter().enumerate() {
        let parts = file.path.split('/').collect::<Vec<_>>();
        let mut node = &mut root;

        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            let path = if node.path.is_empty() {
                (*part).to_owned()
            } else {
                format!("{}/{}", node.path, part)
            };
            node = node
                .dirs
                .entry((*part).to_owned())
                .or_insert_with(|| FileTreeNode::new(*part, path));
            node.file_count += 1;
            node.additions += file.additions;
            node.deletions += file.deletions;
        }

        node.files.push(index);
    }

    root
}

fn collect_file_tree_rows(
    node: &FileTreeNode,
    files: &[FileChange],
    expanded_dirs: &BTreeSet<String>,
    rows: &mut Vec<FileTreeRow>,
    depth: usize,
) {
    for dir in node.dirs.values() {
        let expanded = expanded_dirs.contains(&dir.path);
        rows.push(FileTreeRow::Directory {
            path: dir.path.clone(),
            name: dir.name.clone(),
            depth,
            expanded,
            file_count: dir.file_count,
            additions: dir.additions,
            deletions: dir.deletions,
        });

        if expanded {
            collect_file_tree_rows(dir, files, expanded_dirs, rows, depth + 1);
        }
    }

    let mut file_indices = node.files.clone();
    file_indices.sort_by(|left, right| files[*left].path.cmp(&files[*right].path));

    for index in file_indices {
        let path = files[index].path.clone();
        let name = path
            .rsplit_once('/')
            .map_or_else(|| path.clone(), |(_, name)| name.to_owned());
        rows.push(FileTreeRow::File {
            index,
            path,
            name,
            depth,
        });
    }
}

fn directory_paths(files: &[FileChange]) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();

    for file in files {
        let mut path = String::new();
        let parts = file.path.split('/').collect::<Vec<_>>();

        for part in parts.iter().take(parts.len().saturating_sub(1)) {
            if !path.is_empty() {
                path.push('/');
            }
            path.push_str(part);
            dirs.insert(path.clone());
        }
    }

    dirs
}

fn parent_dir_path(path: &str) -> Option<String> {
    path.rsplit_once('/').map(|(parent, _)| parent.to_owned())
}

fn indent(depth: usize) -> String {
    "  ".repeat(depth)
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
        .map(|file| file.path.clone())
        .unwrap_or_else(|| "diff".to_owned());
    if let Some((position, total)) = session.selected_diff_change_position() {
        title = format!("{title}  {position}/{total}");
    }

    let mut lines = vec![Line::from(Span::styled(
        session
            .selected_file()
            .map(|file| format!("{}  +{} -{}", file.status, file.additions, file.deletions))
            .unwrap_or_else(|| "no changed file selected".to_owned()),
        muted(),
    ))];

    lines.push(Line::raw(""));
    let selected_diff_change_range = session.selected_diff_change_range();
    lines.extend(
        session
            .diff_lines()
            .iter()
            .enumerate()
            .map(|(index, line)| {
                render_diff_line(
                    line,
                    selected_diff_change_range
                        .as_ref()
                        .is_some_and(|range| range.contains(&index)),
                )
            }),
    );

    ensure_selected_diff_visible(state, area, session);

    let diff = Paragraph::new(lines)
        .block(panel_block(title, focused))
        .style(base())
        .scroll((state.diff_scroll, 0));

    frame.render_widget(diff, area);
}

fn ensure_selected_diff_visible(state: &mut TuiState, area: Rect, session: &ReviewSession) {
    const DIFF_PREAMBLE_LINES: usize = 2;

    let Some(selected_diff_line) = session.selected_diff_line_index() else {
        state.diff_scroll = 0;
        return;
    };
    let visible_height = usize::from(area.height.saturating_sub(1));
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

fn render_review(frame: &mut Frame, area: Rect, session: &ReviewSession, focused: bool) {
    let mut lines = vec![
        Line::from(Span::styled("summary", label_style())),
        Line::from(Span::styled(
            session.review_summary().to_owned(),
            base().fg(FG),
        )),
        Line::raw(""),
        Line::from(Span::styled(
            format!("findings ({})", session.findings().len()),
            label_style(),
        )),
    ];

    if session.findings().is_empty() {
        lines.push(Line::from(Span::styled("none yet", muted())));
        lines.push(Line::from(Span::styled(
            "ai backend is not wired in",
            muted(),
        )));
    } else {
        lines.extend(
            session
                .findings()
                .iter()
                .enumerate()
                .map(|(index, finding)| {
                    render_finding_line(finding, index == session.selected_finding_index())
                }),
        );
    }

    lines.push(Line::raw(""));
    lines.push(Line::from(Span::styled("selected", label_style())));
    lines.extend(selected_finding_lines(session.selected_finding()));

    let review = Paragraph::new(lines)
        .block(panel_block("review", focused))
        .style(base())
        .wrap(Wrap { trim: true });

    frame.render_widget(review, area);
}

fn render_finding_line(finding: &Finding, selected: bool) -> Line<'static> {
    let marker = if selected { "> " } else { "  " };
    let style = if selected {
        base().fg(FG).bg(BG_SOFT).add_modifier(Modifier::BOLD)
    } else {
        base().fg(FG)
    };

    Line::from(vec![
        Span::styled(marker, style),
        Span::styled(
            finding.severity.to_uppercase(),
            severity_style(&finding.severity),
        ),
        Span::styled(format!("  {}", finding.title), style),
    ])
}

fn selected_finding_lines(finding: Option<&Finding>) -> Vec<Line<'static>> {
    match finding {
        Some(finding) => vec![
            Line::from(vec![
                Span::styled(
                    finding.severity.to_uppercase(),
                    severity_style(&finding.severity),
                ),
                Span::styled(format!("  {}", finding.title), base().fg(FG)),
            ]),
            Line::from(Span::styled(finding.line.clone(), muted())),
            Line::from(Span::styled(finding.summary.clone(), base().fg(FG))),
        ],
        None => vec![Line::from(Span::styled("no ai finding selected", muted()))],
    }
}

fn render_diff_line(line: &DiffLine, selected: bool) -> Line<'static> {
    let style = match line.prefix.as_str() {
        "+" => base().fg(GREEN),
        "-" => base().fg(RED),
        "@@" => base().fg(ORANGE).add_modifier(Modifier::BOLD),
        _ => base().fg(FG),
    };
    let style = if selected {
        style.bg(BG_SOFT).add_modifier(Modifier::BOLD)
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
        base().fg(YELLOW)
    } else {
        base().fg(BG_SOFT)
    };

    Block::default()
        .borders(Borders::TOP)
        .title(title.into())
        .style(base())
        .border_style(border_style)
}

fn render_footer(frame: &mut Frame, area: Rect, focus: FocusPane) {
    let focused = match focus {
        FocusPane::Files => "files",
        FocusPane::Diff => "diff",
        FocusPane::Findings => "review",
    };
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(format!("{focused} "), base().fg(YELLOW)),
        Span::styled(
            "tab panes  files: h/l or enter collapse/expand  j/k move  n/p diff  q quit",
            muted(),
        ),
    ]))
    .style(base());

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
        "A" => base().fg(GREEN).add_modifier(Modifier::BOLD),
        "M" => base().fg(BLUE).add_modifier(Modifier::BOLD),
        "D" => base().fg(RED).add_modifier(Modifier::BOLD),
        "R" => base().fg(PURPLE).add_modifier(Modifier::BOLD),
        _ => base().fg(MUTED),
    }
}

fn severity_style(severity: &str) -> Style {
    match severity {
        "high" => base().fg(RED).add_modifier(Modifier::BOLD),
        "medium" => base().fg(YELLOW).add_modifier(Modifier::BOLD),
        "low" => base().fg(BLUE).add_modifier(Modifier::BOLD),
        _ => base().fg(MUTED),
    }
}

fn label_style() -> Style {
    base().fg(ORANGE).add_modifier(Modifier::BOLD)
}

fn muted() -> Style {
    base().fg(MUTED)
}

fn base() -> Style {
    Style::default().fg(FG).bg(BG)
}
