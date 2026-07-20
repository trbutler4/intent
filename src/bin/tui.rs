use std::{
    collections::{BTreeMap, BTreeSet},
    io,
    time::Duration,
};

use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, MouseButton,
        MouseEvent, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
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
const DEFAULT_FILES_WIDTH: u16 = 28;
const DEFAULT_REVIEW_WIDTH: u16 = 34;
const MIN_FILES_WIDTH: u16 = 18;
const MIN_DIFF_WIDTH: u16 = 24;
const MIN_REVIEW_WIDTH: u16 = 20;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PaneBoundary {
    FilesDiff,
    DiffReview,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusPane {
    Files,
    Diff,
    Findings,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FileTreeMode {
    ReviewStatus,
    FullTree,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum ReviewSection {
    Pending,
    Reviewed,
}

struct TuiState {
    focus: FocusPane,
    diff_scroll: u16,
    expanded_dirs: BTreeSet<String>,
    collapsed_review_sections: BTreeSet<ReviewSection>,
    file_cursor: usize,
    file_tree_mode: FileTreeMode,
    last_selected_file: Option<usize>,
    files_width: u16,
    review_width: u16,
    files_visible: bool,
    review_visible: bool,
    dragging_boundary: Option<PaneBoundary>,
    last_body_area: Option<Rect>,
}

impl TuiState {
    fn new(session: &ReviewSession) -> Self {
        let expanded_dirs = directory_paths(session.files());
        let mut state = Self {
            focus: FocusPane::Files,
            diff_scroll: 0,
            expanded_dirs,
            collapsed_review_sections: BTreeSet::new(),
            file_cursor: 0,
            file_tree_mode: FileTreeMode::ReviewStatus,
            last_selected_file: selected_file_index(session),
            files_width: DEFAULT_FILES_WIDTH,
            review_width: DEFAULT_REVIEW_WIDTH,
            files_visible: true,
            review_visible: true,
            dragging_boundary: None,
            last_body_area: None,
        };

        sync_file_cursor_to_selected(session, &mut state);
        state
    }
}

#[derive(Clone, Copy)]
struct BodyChunks {
    files: Option<Rect>,
    files_separator: Option<Rect>,
    diff: Rect,
    review_separator: Option<Rect>,
    review: Option<Rect>,
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
    Section {
        section: ReviewSection,
        name: String,
        expanded: bool,
        file_count: usize,
        additions: usize,
        deletions: usize,
    },
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
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let result = run_app(&mut terminal, session);

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        DisableMouseCapture,
        LeaveAlternateScreen
    )?;
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

        match event::read()? {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => break,
                    KeyCode::Tab => state.focus = next_focus(&state),
                    KeyCode::Char('f') => toggle_files_pane(&mut state),
                    KeyCode::Char('R') => toggle_review_pane(&mut state),
                    KeyCode::Char('1') => {
                        state.file_tree_mode = FileTreeMode::FullTree;
                        sync_file_cursor_to_selected(&session, &mut state);
                    }
                    KeyCode::Char('2') => {
                        state.file_tree_mode = FileTreeMode::ReviewStatus;
                        sync_file_cursor_to_selected(&session, &mut state);
                    }
                    KeyCode::Char('r') => {
                        session.apply(ReviewAction::ToggleSelectedFileReviewed);
                        expand_selected_review_section(&session, &mut state);
                        sync_file_cursor_to_selected(&session, &mut state);
                    }
                    KeyCode::Left | KeyCode::Char('h') => {
                        if state.focus == FocusPane::Files {
                            collapse_file_tree_row(&session, &mut state);
                        } else {
                            state.focus = previous_focus(&state);
                        }
                    }
                    KeyCode::Right | KeyCode::Char('l') => {
                        if state.focus == FocusPane::Files {
                            state.focus = if expand_file_tree_row(&mut session, &mut state) {
                                FocusPane::Diff
                            } else {
                                state.focus
                            };
                        } else {
                            state.focus = next_focus(&state);
                        }
                    }
                    KeyCode::Enter | KeyCode::Char(' ') if state.focus == FocusPane::Files => {
                        state.focus = if activate_file_tree_row(&mut session, &mut state) {
                            FocusPane::Diff
                        } else {
                            state.focus
                        };
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
            Event::Mouse(mouse) => handle_mouse_event(mouse, &mut state),
            _ => {}
        }
    }

    Ok(())
}

fn render(frame: &mut Frame, session: &ReviewSession, state: &mut TuiState) {
    reset_diff_scroll_on_file_change(session, state);
    ensure_focus_visible(state);
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
    render_footer(frame, layout[2], state);
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
    state.last_body_area = Some(area);
    clamp_pane_widths(area, state);
    let chunks = body_chunks(area, state);
    let files_focused = state.focus == FocusPane::Files;
    let diff_focused = state.focus == FocusPane::Diff;
    let review_focused = state.focus == FocusPane::Findings;

    if let Some(area) = chunks.files {
        render_files(frame, area, session, state, files_focused);
    }
    if let Some(area) = chunks.files_separator {
        render_separator(
            frame,
            area,
            state.dragging_boundary == Some(PaneBoundary::FilesDiff),
        );
    }
    render_diff(frame, chunks.diff, session, state, diff_focused);
    if let Some(area) = chunks.review_separator {
        render_separator(
            frame,
            area,
            state.dragging_boundary == Some(PaneBoundary::DiffReview),
        );
    }
    if let Some(area) = chunks.review {
        render_review(frame, area, session, review_focused);
    }
}

fn body_chunks(area: Rect, state: &TuiState) -> BodyChunks {
    let mut constraints = Vec::new();
    if state.files_visible {
        constraints.push(Constraint::Length(state.files_width));
        constraints.push(Constraint::Length(1));
    }
    constraints.push(Constraint::Min(MIN_DIFF_WIDTH));
    if state.review_visible {
        constraints.push(Constraint::Length(1));
        constraints.push(Constraint::Length(state.review_width));
    }

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(constraints)
        .split(area);
    let mut index = 0;
    let files = state.files_visible.then(|| {
        let area = chunks[index];
        index += 1;
        area
    });
    let files_separator = state.files_visible.then(|| {
        let area = chunks[index];
        index += 1;
        area
    });
    let diff = chunks[index];
    index += 1;
    let review_separator = state.review_visible.then(|| {
        let area = chunks[index];
        index += 1;
        area
    });
    let review = state.review_visible.then(|| chunks[index]);

    BodyChunks {
        files,
        files_separator,
        diff,
        review_separator,
        review,
    }
}

fn render_separator(frame: &mut Frame, area: Rect, active: bool) {
    let color = if active { YELLOW } else { BG_SOFT };
    frame.render_widget(Block::default().style(base().bg(color)), area);
}

fn handle_mouse_event(mouse: MouseEvent, state: &mut TuiState) {
    let Some(area) = state.last_body_area else {
        return;
    };

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            state.dragging_boundary = pane_boundary_at(area, state, mouse.column, mouse.row);
            if state.dragging_boundary.is_none() {
                update_focus_from_mouse(area, state, mouse.column, mouse.row);
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            if let Some(boundary) = state.dragging_boundary {
                resize_panes(area, state, boundary, mouse.column);
            }
        }
        MouseEventKind::Up(MouseButton::Left) => state.dragging_boundary = None,
        _ => {}
    }
}

fn pane_boundary_at(area: Rect, state: &TuiState, column: u16, row: u16) -> Option<PaneBoundary> {
    if !contains_point(area, column, row) {
        return None;
    }

    let chunks = body_chunks(area, state);
    if chunks
        .files_separator
        .is_some_and(|area| contains_point(area, column, row))
    {
        Some(PaneBoundary::FilesDiff)
    } else if chunks
        .review_separator
        .is_some_and(|area| contains_point(area, column, row))
    {
        Some(PaneBoundary::DiffReview)
    } else {
        None
    }
}

fn update_focus_from_mouse(area: Rect, state: &mut TuiState, column: u16, row: u16) {
    if !contains_point(area, column, row) {
        return;
    }

    let chunks = body_chunks(area, state);
    state.focus = if chunks
        .files
        .is_some_and(|area| contains_point(area, column, row))
    {
        FocusPane::Files
    } else if contains_point(chunks.diff, column, row) {
        FocusPane::Diff
    } else if chunks
        .review
        .is_some_and(|area| contains_point(area, column, row))
    {
        FocusPane::Findings
    } else {
        state.focus
    };
}

fn resize_panes(area: Rect, state: &mut TuiState, boundary: PaneBoundary, column: u16) {
    if area.width == 0 {
        return;
    }

    let right = area.x.saturating_add(area.width);
    let column = column.clamp(area.x, right.saturating_sub(1));

    match boundary {
        PaneBoundary::FilesDiff if state.files_visible => {
            state.files_width = column.saturating_sub(area.x);
        }
        PaneBoundary::DiffReview => {
            if state.review_visible {
                state.review_width = right.saturating_sub(column.saturating_add(1));
            }
        }
        PaneBoundary::FilesDiff => {}
    }

    clamp_pane_widths(area, state);
}

fn clamp_pane_widths(area: Rect, state: &mut TuiState) {
    let separator_count = u16::from(state.files_visible) + u16::from(state.review_visible);
    let side_budget = area
        .width
        .saturating_sub(separator_count)
        .saturating_sub(MIN_DIFF_WIDTH);
    if side_budget == 0 {
        return;
    }

    let min_files = if state.files_visible {
        MIN_FILES_WIDTH.min(side_budget)
    } else {
        0
    };
    let min_review = if state.review_visible {
        MIN_REVIEW_WIDTH.min(side_budget.saturating_sub(min_files))
    } else {
        0
    };

    if state.files_visible {
        let max_files = side_budget.saturating_sub(min_review);
        state.files_width = state.files_width.clamp(min_files, max_files);
    }

    if state.review_visible {
        let used_files = if state.files_visible {
            state.files_width
        } else {
            0
        };
        let max_review = side_budget.saturating_sub(used_files);
        let min_review = MIN_REVIEW_WIDTH.min(max_review);
        state.review_width = state.review_width.clamp(min_review, max_review);
    }
}

fn toggle_files_pane(state: &mut TuiState) {
    state.files_visible = !state.files_visible;
    state.dragging_boundary = None;
    ensure_focus_visible(state);
}

fn toggle_review_pane(state: &mut TuiState) {
    state.review_visible = !state.review_visible;
    state.dragging_boundary = None;
    ensure_focus_visible(state);
}

fn ensure_focus_visible(state: &mut TuiState) {
    if !is_focus_visible(state.focus, state) {
        state.focus = FocusPane::Diff;
    }
}

fn is_focus_visible(focus: FocusPane, state: &TuiState) -> bool {
    match focus {
        FocusPane::Files => state.files_visible,
        FocusPane::Diff => true,
        FocusPane::Findings => state.review_visible,
    }
}

fn contains_point(area: Rect, column: u16, row: u16) -> bool {
    column >= area.x
        && column < area.x.saturating_add(area.width)
        && row >= area.y
        && row < area.y.saturating_add(area.height)
}

fn render_files(
    frame: &mut Frame,
    area: Rect,
    session: &ReviewSession,
    state: &mut TuiState,
    focused: bool,
) {
    clear_area(frame, area);

    let rows = visible_file_rows(session, state);
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
            .map(|row| render_file_tree_row(row, session))
            .collect::<Vec<_>>()
    };
    let mut list_state = ListState::default();
    if !session.files().is_empty() {
        list_state.select(Some(state.file_cursor));
    }
    let list = List::new(items)
        .block(panel_block(
            file_panel_title(session, state.file_tree_mode),
            focused,
        ))
        .style(base())
        .highlight_style(base().fg(FG).bg(BG_SOFT).add_modifier(Modifier::BOLD));

    frame.render_stateful_widget(list, area, &mut list_state);
}

fn file_panel_title(session: &ReviewSession, mode: FileTreeMode) -> String {
    match mode {
        FileTreeMode::ReviewStatus => format!(
            "files  {} todo / {} done",
            session.pending_file_count(),
            session.reviewed_file_count()
        ),
        FileTreeMode::FullTree => format!("files  tree ({})", session.files().len()),
    }
}

fn render_file_tree_row(row: &FileTreeRow, session: &ReviewSession) -> ListItem<'static> {
    match row {
        FileTreeRow::Section {
            section: _,
            name,
            expanded,
            file_count,
            additions,
            deletions,
        } => {
            let marker = if *expanded { "[-]" } else { "[+]" };
            ListItem::new(Line::from(vec![
                Span::styled(format!("{marker} "), base().fg(YELLOW)),
                Span::styled(name.clone(), label_style()),
                Span::styled(
                    format!("  {file_count} files  +{additions} -{deletions}"),
                    muted(),
                ),
            ]))
        }
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
            let file = &session.files()[*index];
            let reviewed = if session.is_file_reviewed(*index) {
                "[x] "
            } else {
                "[ ] "
            };
            ListItem::new(Line::from(vec![
                Span::styled(indent(*depth), muted()),
                Span::styled(reviewed, muted()),
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
    let rows = visible_file_rows(session, state);
    if rows.is_empty() {
        state.file_cursor = 0;
        return;
    }

    let max_index = rows.len() - 1;
    let next = (state.file_cursor.min(max_index) as isize + delta).clamp(0, max_index as isize);
    state.file_cursor = next as usize;
    select_file_row_if_file(session, &rows[state.file_cursor]);
}

fn activate_file_tree_row(session: &mut ReviewSession, state: &mut TuiState) -> bool {
    let rows = visible_file_rows(session, state);
    let Some(row) = rows.get(state.file_cursor) else {
        return false;
    };

    match row {
        FileTreeRow::Section { section, .. } => {
            toggle_review_section(state, *section);
            clamp_file_cursor(state, visible_file_rows(session, state).len());
            false
        }
        FileTreeRow::Directory { path, expanded, .. } => {
            if *expanded {
                state.expanded_dirs.remove(path);
            } else {
                state.expanded_dirs.insert(path.clone());
            }
            clamp_file_cursor(state, visible_file_rows(session, state).len());
            false
        }
        FileTreeRow::File { .. } => {
            select_file_row_if_file(session, row);
            true
        }
    }
}

fn expand_file_tree_row(session: &mut ReviewSession, state: &mut TuiState) -> bool {
    let rows = visible_file_rows(session, state);
    let Some(row) = rows.get(state.file_cursor) else {
        return false;
    };

    match row {
        FileTreeRow::Section { section, .. } => {
            state.collapsed_review_sections.remove(section);
            false
        }
        FileTreeRow::Directory { path, .. } => {
            state.expanded_dirs.insert(path.clone());
            false
        }
        FileTreeRow::File { .. } => {
            select_file_row_if_file(session, row);
            true
        }
    }
}

fn collapse_file_tree_row(session: &ReviewSession, state: &mut TuiState) {
    let rows = visible_file_rows(session, state);
    let Some(row) = rows.get(state.file_cursor) else {
        return;
    };

    match row {
        FileTreeRow::Section { section, .. } => {
            state.collapsed_review_sections.insert(*section);
        }
        FileTreeRow::Directory { path, expanded, .. } => {
            if *expanded {
                state.expanded_dirs.remove(path);
            } else {
                collapse_parent_directory(session, state, path);
            }
        }
        FileTreeRow::File { path, .. } => collapse_parent_directory(session, state, path),
    }

    clamp_file_cursor(state, visible_file_rows(session, state).len());
}

fn toggle_review_section(state: &mut TuiState, section: ReviewSection) {
    if !state.collapsed_review_sections.insert(section) {
        state.collapsed_review_sections.remove(&section);
    }
}

fn expand_selected_review_section(session: &ReviewSession, state: &mut TuiState) {
    if state.file_tree_mode != FileTreeMode::ReviewStatus {
        return;
    }

    let Some(index) = selected_file_index(session) else {
        return;
    };
    let section = if session.is_file_reviewed(index) {
        ReviewSection::Reviewed
    } else {
        ReviewSection::Pending
    };
    state.collapsed_review_sections.remove(&section);
}

fn collapse_parent_directory(session: &ReviewSession, state: &mut TuiState, path: &str) {
    let Some(parent) = parent_dir_path(path) else {
        return;
    };

    state.expanded_dirs.remove(&parent);
    let rows = visible_file_rows(session, state);
    if let Some(index) = rows.iter().position(|row| match row {
        FileTreeRow::Directory { path, .. } => path == &parent,
        FileTreeRow::Section { .. } | FileTreeRow::File { .. } => false,
    }) {
        state.file_cursor = index;
    }
}

fn select_file_row_if_file(session: &mut ReviewSession, row: &FileTreeRow) {
    if let FileTreeRow::File { index, .. } = row {
        session.apply(ReviewAction::SelectFile(*index));
    }
}

fn sync_file_cursor_to_selected(session: &ReviewSession, state: &mut TuiState) {
    let rows = visible_file_rows(session, state);
    state.file_cursor = rows
        .iter()
        .position(|row| match row {
            FileTreeRow::File { index, .. } => *index == session.selected_file_index(),
            FileTreeRow::Section { .. } | FileTreeRow::Directory { .. } => false,
        })
        .unwrap_or_else(|| state.file_cursor.min(rows.len().saturating_sub(1)));
}

fn clamp_file_cursor(state: &mut TuiState, row_count: usize) {
    if row_count == 0 {
        state.file_cursor = 0;
    } else if state.file_cursor >= row_count {
        state.file_cursor = row_count - 1;
    }
}

fn visible_file_rows(session: &ReviewSession, state: &TuiState) -> Vec<FileTreeRow> {
    match state.file_tree_mode {
        FileTreeMode::FullTree => visible_tree_rows(
            session.files(),
            &state.expanded_dirs,
            &(0..session.files().len()).collect::<Vec<_>>(),
            0,
        ),
        FileTreeMode::ReviewStatus => visible_review_status_rows(session, state),
    }
}

fn visible_review_status_rows(session: &ReviewSession, state: &TuiState) -> Vec<FileTreeRow> {
    let pending = file_indices_by_reviewed(session, false);
    let reviewed = file_indices_by_reviewed(session, true);
    let mut rows = Vec::new();
    let pending_expanded = !state
        .collapsed_review_sections
        .contains(&ReviewSection::Pending);
    let reviewed_expanded = !state
        .collapsed_review_sections
        .contains(&ReviewSection::Reviewed);

    push_review_section(
        session.files(),
        &mut rows,
        ReviewSection::Pending,
        "to review",
        pending_expanded,
        &pending,
    );
    if pending_expanded {
        rows.extend(visible_tree_rows(
            session.files(),
            &state.expanded_dirs,
            &pending,
            1,
        ));
    }
    push_review_section(
        session.files(),
        &mut rows,
        ReviewSection::Reviewed,
        "reviewed",
        reviewed_expanded,
        &reviewed,
    );
    if reviewed_expanded {
        rows.extend(visible_tree_rows(
            session.files(),
            &state.expanded_dirs,
            &reviewed,
            1,
        ));
    }

    rows
}

fn visible_tree_rows(
    files: &[FileChange],
    expanded_dirs: &BTreeSet<String>,
    indices: &[usize],
    depth: usize,
) -> Vec<FileTreeRow> {
    let tree = build_file_tree(files, indices);
    let mut rows = Vec::new();
    collect_file_tree_rows(&tree, files, expanded_dirs, &mut rows, depth);
    rows
}

fn build_file_tree(files: &[FileChange], indices: &[usize]) -> FileTreeNode {
    let mut root = FileTreeNode::default();

    for index in indices {
        let file = &files[*index];
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

        node.files.push(*index);
    }

    root
}

fn push_review_section(
    files: &[FileChange],
    rows: &mut Vec<FileTreeRow>,
    section: ReviewSection,
    name: &str,
    expanded: bool,
    indices: &[usize],
) {
    let (additions, deletions) = indices
        .iter()
        .fold((0, 0), |(additions, deletions), index| {
            (
                additions + files[*index].additions,
                deletions + files[*index].deletions,
            )
        });

    rows.push(FileTreeRow::Section {
        section,
        name: name.to_owned(),
        expanded,
        file_count: indices.len(),
        additions,
        deletions,
    });
}

fn file_indices_by_reviewed(session: &ReviewSession, reviewed: bool) -> Vec<usize> {
    session
        .files()
        .iter()
        .enumerate()
        .filter_map(|(index, _)| (session.is_file_reviewed(index) == reviewed).then_some(index))
        .collect()
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
    clear_area(frame, area);

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
    clear_area(frame, area);

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

fn clear_area(frame: &mut Frame, area: Rect) {
    frame.render_widget(Clear, area);
    frame.render_widget(Block::default().style(base()), area);
}

fn reset_diff_scroll_on_file_change(session: &ReviewSession, state: &mut TuiState) {
    let selected_file = selected_file_index(session);
    if state.last_selected_file != selected_file {
        state.diff_scroll = 0;
        state.last_selected_file = selected_file;
    }
}

fn selected_file_index(session: &ReviewSession) -> Option<usize> {
    session
        .selected_file()
        .is_some()
        .then_some(session.selected_file_index())
}

fn render_footer(frame: &mut Frame, area: Rect, state: &TuiState) {
    clear_area(frame, area);

    let focused = match state.focus {
        FocusPane::Files => "files",
        FocusPane::Diff => "diff",
        FocusPane::Findings => "review",
    };
    let files_visible = if state.files_visible { "on" } else { "off" };
    let review_visible = if state.review_visible { "on" } else { "off" };
    let footer = Paragraph::new(Line::from(vec![
        Span::styled(format!("{focused} "), base().fg(YELLOW)),
        Span::styled(
            format!(
                "tab panes  f files:{files_visible}  R review:{review_visible}  r reviewed  h/l tree  q quit"
            ),
            muted(),
        ),
    ]))
    .style(base());

    frame.render_widget(footer, area);
}

fn next_focus(state: &TuiState) -> FocusPane {
    step_focus(state, 1)
}

fn previous_focus(state: &TuiState) -> FocusPane {
    step_focus(state, 2)
}

fn step_focus(state: &TuiState, delta: usize) -> FocusPane {
    let order = [FocusPane::Files, FocusPane::Diff, FocusPane::Findings];
    let current = order
        .iter()
        .position(|pane| *pane == state.focus)
        .unwrap_or(1);

    for offset in 1..=order.len() {
        let candidate = order[(current + offset * delta) % order.len()];
        if is_focus_visible(candidate, state) {
            return candidate;
        }
    }

    FocusPane::Diff
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
