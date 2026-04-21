use std::{
    collections::{HashMap, HashSet},
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

use gpui::{
    AnyElement, App, Application, Bounds, ClickEvent, Context, FontWeight, Hsla,
    ListHorizontalSizingBehavior, ScrollStrategy, UniformListScrollHandle, Window, WindowBounds,
    WindowOptions, div, prelude::*, px, rgb, size, uniform_list,
};

#[derive(Clone)]
struct FileChange {
    path: String,
    status: String,
    additions: usize,
    deletions: usize,
    untracked: bool,
    diff_lines: Option<Vec<DiffLine>>,
}

#[derive(Clone)]
struct DiffLine {
    prefix: String,
    number: String,
    content: String,
}

struct StatusEntry {
    path: String,
    status: String,
    untracked: bool,
}

#[derive(Clone)]
enum ReviewMode {
    WorkingTree { has_head: bool },
    LastCommit { has_parent: bool },
    Empty,
}

struct RepoSnapshot {
    root: PathBuf,
    review_mode: ReviewMode,
    files: Vec<FileChange>,
    load_error: Option<String>,
}

struct ReviewApp {
    repo: RepoSnapshot,
    selected_file: usize,
    diff_scroll_handle: UniformListScrollHandle,
    diff_focus_mode: bool,
    collapsed_dirs: HashSet<String>,
}

#[derive(Clone)]
struct FileTreeNode {
    name: String,
    full_path: String,
    children: Vec<FileTreeNode>,
    file_index: Option<usize>,
}

impl ReviewApp {
    fn new(repo_path: Option<PathBuf>) -> Self {
        let mut app = Self {
            repo: load_repo_snapshot(repo_path),
            selected_file: 0,
            diff_scroll_handle: UniformListScrollHandle::new(),
            diff_focus_mode: false,
            collapsed_dirs: HashSet::new(),
        };
        app.ensure_selected_diff_loaded();
        app
    }

    fn set_selected_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.repo.files.len() {
            self.selected_file = index;
            self.ensure_selected_diff_loaded();
            self.diff_scroll_handle
                .scroll_to_item_strict(0, ScrollStrategy::Top);
            cx.notify();
        }
    }

    fn toggle_diff_focus_mode(&mut self, cx: &mut Context<Self>) {
        self.diff_focus_mode = !self.diff_focus_mode;
        cx.notify();
    }

    fn toggle_directory(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.collapsed_dirs.insert(path.clone()) {
            self.collapsed_dirs.remove(&path);
        }
        cx.notify();
    }

    fn ensure_selected_diff_loaded(&mut self) {
        let Some(file) = self.repo.files.get(self.selected_file) else {
            return;
        };

        if file.diff_lines.is_some() {
            return;
        }

        let path = file.path.clone();
        let status = file.status.clone();
        let untracked = file.untracked;
        let diff_lines = load_diff_lines_for_file(
            &self.repo.root,
            &self.repo.review_mode,
            &path,
            &status,
            untracked,
        )
        .unwrap_or_else(|error| vec![DiffLine {
            prefix: ">".to_string(),
            number: String::new(),
            content: error,
        }]);

        if let Some(file) = self.repo.files.get_mut(self.selected_file) {
            file.diff_lines = Some(diff_lines);
        }
    }

    fn render_file_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let body = if self.repo.files.is_empty() {
            div()
                .id("file-list-scroll")
                .flex_1()
                .overflow_scroll()
                .child(render_empty_state(
                    "No changed files",
                    "The app reviews the working tree when the repo is dirty, or the latest commit when the repo is clean.",
                ))
        } else {
            let tree = build_file_tree(&self.repo.files);

            div()
                .id("file-list-scroll")
                .flex_1()
                .overflow_scroll()
                .flex()
                .flex_col()
                .children(
                    tree.into_iter().map(|node| self.render_file_tree_node(&node, 0, cx)),
                )
        };

        div()
            .w(px(320.0))
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded_lg()
            .border_1()
            .border_color(rgb(0x1e2a45))
            .bg(rgb(0x10182b))
            .child(render_panel_header(
                "Changed Files",
                &format!("{} files", self.repo.files.len()),
            ))
            .child(body)
    }

    fn render_file_tree_node(
        &self,
        node: &FileTreeNode,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let indent = 12.0 + depth as f32 * 16.0;

        if let Some(file_index) = node.file_index {
            let file = &self.repo.files[file_index];
            let selected = file_index == self.selected_file;
            let border = if selected { rgb(0x4f8cff) } else { rgb(0x10182b) };
            let bg = if selected { rgb(0x172544) } else { rgb(0x10182b) };

            div()
                .id(("file", file_index))
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_3()
                .py_2()
                .pl(px(indent))
                .bg(bg)
                .border_l_2()
                .border_color(border)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0x162039)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_selected_file(file_index, cx);
                }))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .items_center()
                                .child(div().text_sm().child(node.name.clone())),
                        )
                        .child(render_badge(&file.status, status_color(&file.status))),
                )
                .into_any_element()
        } else {
            let is_collapsed = self.collapsed_dirs.contains(&node.full_path);
            let toggle_path = node.full_path.clone();
            let dir_id = directory_id(&node.full_path);

            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .id(("dir", dir_id))
                        .px_3()
                        .py_2()
                        .pl(px(indent))
                        .flex()
                        .gap_2()
                        .items_center()
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(0x121c31)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_directory(toggle_path.clone(), cx);
                        }))
                        .text_xs()
                        .text_color(rgb(0x6f86aa))
                        .font_weight(FontWeight::BOLD)
                        .child(if is_collapsed { "▸" } else { "▾" })
                        .child(node.name.clone()),
                )
                .children(
                    node.children
                        .iter()
                        .filter(|_| !is_collapsed)
                        .map(|child| self.render_file_tree_node(child, depth + 1, cx)),
                )
                .into_any_element()
        }
    }
}

impl Render for ReviewApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_file = self.repo.files.get(self.selected_file);
        let left_pane = if self.diff_focus_mode {
            None
        } else {
            Some(self.render_file_list(cx).into_any_element())
        };

        div()
            .size_full()
            .bg(rgb(0x0b1020))
            .text_color(rgb(0xe5eefc))
            .child(
                div()
                    .size_full()
                    .flex()
                    .gap_3()
                    .p_3()
                    .children(left_pane)
                    .child(render_diff_view(
                        &self.repo,
                        selected_file,
                        self.diff_scroll_handle.clone(),
                        self.diff_focus_mode,
                        cx.listener(|this, _, _, cx| {
                            this.toggle_diff_focus_mode(cx);
                        }),
                    )),
            )
    }
}

fn load_repo_snapshot(requested_path: Option<PathBuf>) -> RepoSnapshot {
    let repo_path = requested_path
        .or_else(|| env::var_os("REVIEW_REPO").map(PathBuf::from))
        .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    match load_repo_snapshot_inner(&repo_path) {
        Ok(repo) => repo,
        Err(error) => RepoSnapshot {
            root: repo_path.clone(),
            review_mode: ReviewMode::Empty,
            files: Vec::new(),
            load_error: Some(error),
        },
    }
}

fn load_repo_snapshot_inner(repo_path: &Path) -> Result<RepoSnapshot, String> {
    let top_level = PathBuf::from(run_git(repo_path, &["rev-parse", "--show-toplevel"])?);
    let status_output = run_git(&top_level, &["status", "--porcelain=v1", "--untracked-files=all"])?;
    let has_head = git_command_success(&top_level, &["rev-parse", "--verify", "HEAD"]);
    let has_worktree_changes = !status_output.trim().is_empty();

    let (review_mode, files) = if has_worktree_changes {
        (
            ReviewMode::WorkingTree { has_head },
            load_worktree_changes(&top_level, &status_output)?,
        )
    } else if has_head {
        let files = load_last_commit_changes(&top_level)?;
        let has_parent = git_command_success(&top_level, &["rev-parse", "--verify", "HEAD^"]);
        (ReviewMode::LastCommit { has_parent }, files)
    } else {
        (ReviewMode::Empty, Vec::new())
    };

    Ok(RepoSnapshot {
        root: top_level.clone(),
        review_mode,
        files,
        load_error: None,
    })
}

fn load_worktree_changes(repo_path: &Path, status_output: &str) -> Result<Vec<FileChange>, String> {
    let mut files = Vec::new();
    let has_head = git_command_success(repo_path, &["rev-parse", "--verify", "HEAD"]);
    let mut diff_stats = load_worktree_diff_stats(repo_path, has_head)?;

    for line in status_output.lines() {
        let Some(entry) = parse_porcelain_status(line) else {
            continue;
        };
        let (additions, deletions) = diff_stats.remove(&entry.path).unwrap_or_else(|| {
            if entry.untracked {
                count_untracked_file_stats(repo_path, &entry.path)
            } else {
                (0, 0)
            }
        });

        files.push(FileChange {
            path: entry.path,
            status: entry.status,
            additions,
            deletions,
            untracked: entry.untracked,
            diff_lines: None,
        });
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(files)
}

fn load_last_commit_changes(repo_path: &Path) -> Result<Vec<FileChange>, String> {
    let commit_count: usize = run_git(repo_path, &["rev-list", "--count", "HEAD"])?
        .trim()
        .parse()
        .unwrap_or(1);
    let diff_stats = load_last_commit_diff_stats(repo_path, commit_count > 1)?;

    let name_status = if commit_count > 1 {
        run_git(repo_path, &["show", "--format=", "--name-status", "HEAD"])?
    } else {
        run_git(repo_path, &["show", "--format=", "--name-status", "--root", "HEAD"])?
    };

    let mut files = Vec::new();

    for line in name_status.lines() {
        let Some(entry) = parse_name_status(line) else {
            continue;
        };
        let (additions, deletions) = diff_stats.get(&entry.path).copied().unwrap_or((0, 0));

        files.push(FileChange {
            path: entry.path,
            status: entry.status,
            additions,
            deletions,
            untracked: false,
            diff_lines: None,
        });
    }

    files.sort_by(|left, right| left.path.cmp(&right.path));

    Ok(files)
}

fn parse_porcelain_status(line: &str) -> Option<StatusEntry> {
    if line.len() < 4 {
        return None;
    }

    let raw_status = &line[..2];
    if raw_status == "!!" {
        return None;
    }

    let path = line[3..].trim();
    let path = path.split(" -> ").last().unwrap_or(path).to_string();

    Some(StatusEntry {
        path,
        status: summarize_status(raw_status),
        untracked: raw_status == "??",
    })
}

fn parse_name_status(line: &str) -> Option<StatusEntry> {
    let mut parts = line.split('\t');
    let raw_status = parts.next()?.trim();
    let path = parts.last()?.trim();

    Some(StatusEntry {
        path: path.to_string(),
        status: summarize_status(raw_status),
        untracked: false,
    })
}

fn load_worktree_diff_stats(
    repo_path: &Path,
    has_head: bool,
) -> Result<HashMap<String, (usize, usize)>, String> {
    if has_head {
        run_git(
            repo_path,
            &[
                "diff",
                "--no-ext-diff",
                "--find-renames",
                "--numstat",
                "HEAD",
            ],
        )
        .map(|output| parse_numstat_output(&output))
    } else {
        run_git(
            repo_path,
            &[
                "diff",
                "--no-ext-diff",
                "--find-renames",
                "--cached",
                "--numstat",
            ],
        )
        .map(|output| parse_numstat_output(&output))
    }
}

fn load_last_commit_diff_stats(
    repo_path: &Path,
    has_parent: bool,
) -> Result<HashMap<String, (usize, usize)>, String> {
    if has_parent {
        run_git(repo_path, &["show", "--format=", "--numstat", "HEAD"]).map(|output| {
            parse_numstat_output(&output)
        })
    } else {
        run_git(repo_path, &["show", "--format=", "--numstat", "--root", "HEAD"]).map(
            |output| parse_numstat_output(&output),
        )
    }
}

fn parse_numstat_output(output: &str) -> HashMap<String, (usize, usize)> {
    let mut stats = HashMap::new();

    for line in output.lines() {
        let mut parts = line.split('\t');
        let additions = parts.next().unwrap_or_default();
        let deletions = parts.next().unwrap_or_default();
        let path = parts.next().unwrap_or_default().trim();

        if path.is_empty() {
            continue;
        }

        let path = path.split(" -> ").last().unwrap_or(path).to_string();
        let counts = (
            parse_numstat_count(additions),
            parse_numstat_count(deletions),
        );
        merge_diff_stat(&mut stats, path, counts);
    }

    stats
}

fn merge_diff_stat(
    stats: &mut HashMap<String, (usize, usize)>,
    path: String,
    counts: (usize, usize),
) {
    stats
        .entry(path)
        .and_modify(|existing| {
            existing.0 += counts.0;
            existing.1 += counts.1;
        })
        .or_insert(counts);
}

fn parse_numstat_count(value: &str) -> usize {
    value.parse().unwrap_or(0)
}

fn count_untracked_file_stats(repo_path: &Path, relative_path: &str) -> (usize, usize) {
    let full_path = repo_path.join(relative_path);
    let bytes = match fs::read(full_path) {
        Ok(bytes) => bytes,
        Err(_) => return (0, 0),
    };

    if bytes.contains(&0) {
        return (0, 0);
    }

    let additions = String::from_utf8_lossy(&bytes).lines().count();
    (additions, 0)
}

fn load_diff_lines_for_file(
    repo_path: &Path,
    review_mode: &ReviewMode,
    path: &str,
    status: &str,
    untracked: bool,
) -> Result<Vec<DiffLine>, String> {
    let entry = StatusEntry {
        path: path.to_string(),
        status: status.to_string(),
        untracked,
    };

    if untracked {
        return Ok(build_untracked_diff(repo_path, path));
    }

    let diff = match review_mode {
        ReviewMode::WorkingTree { has_head: true } => run_git(
            repo_path,
            &[
                "diff",
                "--no-ext-diff",
                "--find-renames",
                "--unified=3",
                "HEAD",
                "--",
                path,
            ],
        )
        .or_else(|_| {
            run_git(
                repo_path,
                &[
                    "diff",
                    "--no-ext-diff",
                    "--find-renames",
                    "--cached",
                    "--unified=3",
                    "--",
                    path,
                ],
            )
        })?,
        ReviewMode::WorkingTree { has_head: false } => run_git(
            repo_path,
            &[
                "diff",
                "--no-ext-diff",
                "--find-renames",
                "--cached",
                "--unified=3",
                "--",
                path,
            ],
        )?,
        ReviewMode::LastCommit { has_parent: true } => run_git(
            repo_path,
            &["show", "--format=", "--unified=3", "HEAD", "--", path],
        )?,
        ReviewMode::LastCommit { has_parent: false } => run_git(
            repo_path,
            &[
                "show",
                "--format=",
                "--unified=3",
                "--root",
                "HEAD",
                "--",
                path,
            ],
        )?,
        ReviewMode::Empty => String::new(),
    };

    if diff.trim().is_empty() {
        Ok(fallback_diff_for_status(repo_path, &entry))
    } else {
        Ok(parse_unified_diff(&diff))
    }
}

fn summarize_status(raw_status: &str) -> String {
    if raw_status == "??" {
        return "A".to_string();
    }

    for ch in raw_status.chars() {
        match ch {
            'A' => return "A".to_string(),
            'D' => return "D".to_string(),
            'R' => return "R".to_string(),
            'C' => return "R".to_string(),
            'U' => return "U".to_string(),
            'M' => return "M".to_string(),
            _ => {}
        }
    }

    "M".to_string()
}

fn fallback_diff_for_status(repo_path: &Path, entry: &StatusEntry) -> Vec<DiffLine> {
    match entry.status.as_str() {
        "A" => build_untracked_diff(repo_path, &entry.path),
        "D" => vec![
            DiffLine {
                prefix: ">".to_string(),
                number: String::new(),
                content: format!("diff --git a/{0} b/{0}", entry.path),
            },
            DiffLine {
                prefix: ">".to_string(),
                number: String::new(),
                content: format!("deleted file {}", entry.path),
            },
        ],
        _ => vec![DiffLine {
            prefix: ">".to_string(),
            number: String::new(),
            content: "No textual diff available for this change.".to_string(),
        }],
    }
}

fn build_untracked_diff(repo_path: &Path, relative_path: &str) -> Vec<DiffLine> {
    let full_path = repo_path.join(relative_path);
    let bytes = match fs::read(&full_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return vec![DiffLine {
                prefix: ">".to_string(),
                number: String::new(),
                content: format!("Unable to read {}: {}", relative_path, error),
            }];
        }
    };

    let mut diff = vec![
        DiffLine {
            prefix: ">".to_string(),
            number: String::new(),
            content: format!("diff --git a/{0} b/{0}", relative_path),
        },
        DiffLine {
            prefix: ">".to_string(),
            number: String::new(),
            content: "new file mode 100644".to_string(),
        },
        DiffLine {
            prefix: ">".to_string(),
            number: String::new(),
            content: "--- /dev/null".to_string(),
        },
        DiffLine {
            prefix: ">".to_string(),
            number: String::new(),
            content: format!("+++ b/{}", relative_path),
        },
    ];

    if bytes.contains(&0) {
        diff.push(DiffLine {
            prefix: ">".to_string(),
            number: String::new(),
            content: "Binary or non-text file preview unavailable.".to_string(),
        });
        return diff;
    }

    let contents = String::from_utf8_lossy(&bytes);
    let lines: Vec<&str> = contents.lines().collect();

    diff.push(DiffLine {
        prefix: "@@".to_string(),
        number: if lines.is_empty() {
            String::new()
        } else {
            "1".to_string()
        },
        content: format!("@@ -0,0 +1,{} @@", lines.len()),
    });

    if lines.is_empty() {
        diff.push(DiffLine {
            prefix: ">".to_string(),
            number: String::new(),
            content: "Empty file".to_string(),
        });
        return diff;
    }

    for (index, line) in lines.iter().enumerate() {
        diff.push(DiffLine {
            prefix: "+".to_string(),
            number: (index + 1).to_string(),
            content: (*line).to_string(),
        });
    }

    diff
}

fn parse_unified_diff(diff: &str) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    let mut old_line = 0usize;
    let mut new_line = 0usize;

    for raw_line in diff.lines() {
        if raw_line.starts_with("@@") {
            let (parsed_old, parsed_new) = parse_hunk_header(raw_line);
            old_line = parsed_old;
            new_line = parsed_new;
            lines.push(DiffLine {
                prefix: "@@".to_string(),
                number: format!("{}:{}", old_line, new_line),
                content: raw_line.to_string(),
            });
            continue;
        }

        if is_diff_metadata(raw_line) {
            lines.push(DiffLine {
                prefix: ">".to_string(),
                number: String::new(),
                content: raw_line.to_string(),
            });
            continue;
        }

        if let Some(content) = raw_line.strip_prefix('+') {
            lines.push(DiffLine {
                prefix: "+".to_string(),
                number: new_line.to_string(),
                content: content.to_string(),
            });
            new_line += 1;
            continue;
        }

        if let Some(content) = raw_line.strip_prefix('-') {
            lines.push(DiffLine {
                prefix: "-".to_string(),
                number: old_line.to_string(),
                content: content.to_string(),
            });
            old_line += 1;
            continue;
        }

        if let Some(content) = raw_line.strip_prefix(' ') {
            lines.push(DiffLine {
                prefix: " ".to_string(),
                number: format!("{}:{}", old_line, new_line),
                content: content.to_string(),
            });
            old_line += 1;
            new_line += 1;
            continue;
        }

        lines.push(DiffLine {
            prefix: ">".to_string(),
            number: String::new(),
            content: raw_line.to_string(),
        });
    }

    lines
}

fn parse_hunk_header(header: &str) -> (usize, usize) {
    let mut parts = header.split_whitespace();
    let _ = parts.next();
    let old = parts.next().unwrap_or("-0,0");
    let new = parts.next().unwrap_or("+0,0");

    (parse_hunk_start(old), parse_hunk_start(new))
}

fn parse_hunk_start(range: &str) -> usize {
    range[1..]
        .split(',')
        .next()
        .unwrap_or("0")
        .parse()
        .unwrap_or(0)
}

fn is_diff_metadata(line: &str) -> bool {
    line.starts_with("diff --git")
        || line.starts_with("index ")
        || line.starts_with("--- ")
        || line.starts_with("+++ ")
        || line.starts_with("new file mode ")
        || line.starts_with("deleted file mode ")
        || line.starts_with("similarity index ")
        || line.starts_with("rename from ")
        || line.starts_with("rename to ")
        || line.starts_with("Binary files ")
}

fn run_git(repo_path: &Path, args: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .map_err(|error| format!("Failed to start git for {}: {}", repo_path.display(), error))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout)
            .trim_end()
            .to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !stderr.is_empty() {
            Err(stderr)
        } else if !stdout.is_empty() {
            Err(stdout)
        } else {
            Err(format!("git command failed for {}", repo_path.display()))
        }
    }
}

fn git_command_success(repo_path: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn build_file_tree(files: &[FileChange]) -> Vec<FileTreeNode> {
    let mut roots = Vec::new();

    for (index, file) in files.iter().enumerate() {
        let parts: Vec<&str> = file.path.split('/').collect();
        insert_file_tree_node(&mut roots, &parts, String::new(), &file.path, index);
    }

    sort_file_tree_nodes(&mut roots);
    roots
}

fn insert_file_tree_node(
    nodes: &mut Vec<FileTreeNode>,
    parts: &[&str],
    parent_path: String,
    full_path: &str,
    file_index: usize,
) {
    let Some((part, rest)) = parts.split_first() else {
        return;
    };

    let node_path = if parent_path.is_empty() {
        (*part).to_string()
    } else {
        format!("{}/{}", parent_path, part)
    };

    if let Some(existing) = nodes.iter_mut().find(|node| node.name == *part) {
        if rest.is_empty() {
            existing.file_index = Some(file_index);
            existing.full_path = full_path.to_string();
        } else {
            insert_file_tree_node(
                &mut existing.children,
                rest,
                existing.full_path.clone(),
                full_path,
                file_index,
            );
        }
        return;
    }

    let mut node = FileTreeNode {
        name: (*part).to_string(),
        full_path: if rest.is_empty() {
            full_path.to_string()
        } else {
            node_path.clone()
        },
        children: Vec::new(),
        file_index: rest.is_empty().then_some(file_index),
    };

    if !rest.is_empty() {
        insert_file_tree_node(&mut node.children, rest, node_path, full_path, file_index);
    }

    nodes.push(node);
}

fn sort_file_tree_nodes(nodes: &mut [FileTreeNode]) {
    nodes.sort_by(|left, right| match (left.file_index.is_some(), right.file_index.is_some()) {
        (false, true) => std::cmp::Ordering::Less,
        (true, false) => std::cmp::Ordering::Greater,
        _ => left.name.cmp(&right.name),
    });

    for node in nodes {
        sort_file_tree_nodes(&mut node.children);
    }
}

fn directory_id(path: &str) -> u64 {
    let mut hash = 1469598103934665603u64;
    for byte in path.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}

fn render_panel_header(title: &str, subtitle: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(rgb(0x1e2a45))
        .bg(rgb(0x0d1422))
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x91a3c0))
                .child(subtitle.to_owned()),
        )
}

fn render_badge(label: &str, color: Hsla) -> impl IntoElement {
    div()
        .rounded_full()
        .bg(color.opacity(0.18))
        .border_1()
        .border_color(color.opacity(0.5))
        .px_2()
        .py_0p5()
        .text_xs()
        .text_color(color)
        .child(label.to_uppercase())
}

fn render_empty_state(title: &str, body: &str) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .justify_center()
        .items_center()
        .gap_2()
        .p_6()
        .text_center()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child(title.to_owned()),
        )
        .child(
            div()
                .max_w(px(320.0))
                .text_sm()
                .text_color(rgb(0x91a3c0))
                .child(body.to_owned()),
        )
}

fn render_diff_view(
    repo: &RepoSnapshot,
    selected_file: Option<&FileChange>,
    diff_scroll_handle: UniformListScrollHandle,
    diff_focus_mode: bool,
    toggle_focus_listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let subtitle = selected_file
        .map(|file| file.path.as_str())
        .unwrap_or("No file selected");

    let summary = if let Some(error) = &repo.load_error {
        error.clone()
    } else if let Some(file) = selected_file {
        format!("{}  +{}  -{}", file.status, file.additions, file.deletions)
    } else {
        "No diff selected".to_string()
    };

    let diff_body: AnyElement = if let Some(file) = selected_file {
        if file.diff_lines.as_ref().is_none_or(|lines| lines.is_empty()) {
            div()
                .id("diff-scroll")
                .flex_1()
                .overflow_scroll()
                .child(render_empty_state(
                    "No textual diff available",
                    "This file may be binary or only have metadata changes.",
                ))
                .into_any_element()
        } else {
            let diff_lines = file.diff_lines.clone().unwrap_or_default();
            let item_count = diff_lines.len();

            uniform_list(
                "diff-scroll",
                item_count,
                move |range, _window, _cx| {
                    range
                        .map(|index| render_diff_line(diff_lines[index].clone()))
                        .collect::<Vec<_>>()
                },
            )
            .flex_1()
            .text_sm()
            .track_scroll(diff_scroll_handle)
            .with_width_from_item(Some(0))
            .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
            .into_any_element()
        }
    } else if let Some(error) = &repo.load_error {
        div()
            .id("diff-scroll")
            .flex_1()
            .overflow_scroll()
            .child(render_empty_state("Unable to load repository", error))
            .into_any_element()
    } else {
        div()
            .id("diff-scroll")
            .flex_1()
            .overflow_scroll()
            .child(render_empty_state(
                "No diff selected",
                "Pick a file from the left panel to inspect the real git diff.",
            ))
            .into_any_element()
    };

    div()
        .flex_1()
        .h_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x1e2a45))
        .bg(rgb(0x0f1728))
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .px_4()
                .py_3()
                .border_b_1()
                .border_color(rgb(0x1e2a45))
                .bg(rgb(0x0d1422))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(FontWeight::BOLD)
                                .child("Diff"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x91a3c0))
                                .child(subtitle.to_owned()),
                        ),
                )
                .child(
                    div()
                        .id("toggle-diff-focus")
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(0x22304f))
                        .bg(rgb(0x10182b))
                        .px_3()
                        .py_1()
                        .text_xs()
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(0x162039)))
                        .on_click(toggle_focus_listener)
                        .child(if diff_focus_mode {
                            "Restore Layout"
                        } else {
                            "Focus Diff"
                        }),
                ),
        )
        .child(
            div()
                .px_4()
                .py_3()
                .border_b_1()
                .border_color(rgb(0x1e2a45))
                .child(div().text_sm().text_color(rgb(0x91a3c0)).child(summary)),
        )
        .child(diff_body)
}

fn render_diff_line(line: DiffLine) -> impl IntoElement {
    let (color, background) = match line.prefix.as_str() {
        "+" => (rgb(0x7ee787), rgb(0x12261a)),
        "-" => (rgb(0xff7b72), rgb(0x2a1417)),
        "@@" => (rgb(0x79c0ff), rgb(0x0f2238)),
        ">" => (rgb(0xb7c6df), rgb(0x121c31)),
        _ => (rgb(0xe5eefc), rgb(0x0f1728)),
    };

    div()
        .flex()
        .gap_3()
        .items_start()
        .min_w(px(960.0))
        .rounded_sm()
        .bg(background)
        .px_2()
        .py_1()
        .child(div().w(px(22.0)).text_color(color).child(line.prefix))
        .child(
            div()
                .w(px(72.0))
                .text_color(rgb(0x6f86aa))
                .child(line.number),
        )
        .child(div().text_color(color).child(line.content))
}

fn status_color(status: &str) -> Hsla {
    match status {
        "A" => rgb(0x7ee787).into(),
        "M" => rgb(0x79c0ff).into(),
        "D" => rgb(0xff7b72).into(),
        "R" => rgb(0xc297ff).into(),
        "U" => rgb(0xf2cc60).into(),
        _ => rgb(0xb7c6df).into(),
    }
}

#[derive(Clone, Copy)]
enum DisplayBackend {
    X11,
    Wayland,
}

fn requested_repo_path_and_backend() -> (Option<PathBuf>, Option<DisplayBackend>) {
    let mut repo_path = None;
    let mut backend = None;

    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--x11" => backend = Some(DisplayBackend::X11),
            "--wayland" => backend = Some(DisplayBackend::Wayland),
            _ if arg.starts_with("--") => {}
            _ if repo_path.is_none() => repo_path = Some(PathBuf::from(arg)),
            _ => {}
        }
    }

    (repo_path, backend)
}

fn apply_backend_override(backend: Option<DisplayBackend>) {
    match backend {
        Some(DisplayBackend::X11) => {
            unsafe {
                env::remove_var("WAYLAND_DISPLAY");
            }
        }
        Some(DisplayBackend::Wayland) => {
            unsafe {
                env::remove_var("DISPLAY");
            }
        }
        None => {}
    }
}

fn main() {
    let (repo_path, backend) = requested_repo_path_and_backend();
    apply_backend_override(backend);

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1440.0), px(920.0)), cx);
        let requested_path = repo_path.clone();

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |_, cx| {
                let requested_path = requested_path.clone();
                cx.new(|_| ReviewApp::new(requested_path))
            },
        )
        .unwrap();

        cx.activate(true);
    });
}
