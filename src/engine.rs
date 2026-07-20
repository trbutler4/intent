use std::{
    collections::BTreeSet,
    error::Error,
    fmt, fs, io,
    ops::Range,
    path::{Path, PathBuf},
    process::Command,
};

use rusqlite::{params, Connection};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileChange {
    pub path: String,
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Finding {
    pub severity: String,
    pub title: String,
    pub summary: String,
    pub line: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiffLine {
    pub prefix: String,
    pub number: String,
    pub content: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewAction {
    SelectFile(usize),
    SelectFinding(usize),
    ToggleSelectedFileReviewed,
    NextDiffChange,
    PreviousDiffChange,
    NextFile,
    PreviousFile,
    NextFinding,
    PreviousFinding,
}

#[derive(Debug)]
pub enum ReviewError {
    Io(io::Error),
    Sqlite(rusqlite::Error),
    Git { command: String, stderr: String },
}

impl fmt::Display for ReviewError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(formatter, "{error}"),
            Self::Sqlite(error) => write!(formatter, "sqlite error: {error}"),
            Self::Git { command, stderr } => {
                if stderr.trim().is_empty() {
                    write!(formatter, "git command failed: {command}")
                } else {
                    write!(
                        formatter,
                        "git command failed: {command}: {}",
                        stderr.trim()
                    )
                }
            }
        }
    }
}

impl Error for ReviewError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io(error) => Some(error),
            Self::Sqlite(error) => Some(error),
            Self::Git { .. } => None,
        }
    }
}

impl From<io::Error> for ReviewError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<rusqlite::Error> for ReviewError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Sqlite(error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewSession {
    selected_file: usize,
    selected_finding: usize,
    selected_diff_line: usize,
    repo_label: String,
    review_label: String,
    backend_label: String,
    review_summary: String,
    files: Vec<FileChange>,
    diffs: Vec<Vec<DiffLine>>,
    findings: Vec<Finding>,
    reviewed_files: BTreeSet<String>,
    persistence: Option<PersistenceContext>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PersistenceContext {
    db_path: PathBuf,
    review_key: String,
    file_hashes: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct StatusEntry {
    status: String,
    path: String,
}

impl ReviewSession {
    pub fn load_current_worktree() -> Result<Self, ReviewError> {
        Self::load_from_repo(".")
    }

    pub fn load_from_repo(path: impl AsRef<Path>) -> Result<Self, ReviewError> {
        let repo_root = repo_root(path.as_ref())?;
        let review_key = review_key(&repo_root);
        let status_entries = git_status_entries(&repo_root)?;
        let mut files = Vec::with_capacity(status_entries.len());
        let mut diffs = Vec::with_capacity(status_entries.len());

        for entry in status_entries {
            let diff = if entry.status == "??" {
                untracked_file_diff(&repo_root, &entry.path)
            } else {
                tracked_file_diff(&repo_root, &entry.path)?
            };
            let (additions, deletions) = diff_stats(&diff);

            files.push(FileChange {
                path: entry.path,
                status: display_status(&entry.status),
                additions,
                deletions,
            });
            diffs.push(diff);
        }
        let file_hashes = diffs.iter().map(|diff| diff_hash(diff)).collect::<Vec<_>>();
        let persistence = PersistenceContext {
            db_path: review_db_path(&repo_root)?,
            review_key,
            file_hashes,
        };
        let reviewed_files = load_reviewed_files(&persistence, &files)?;

        let review_summary = if files.is_empty() {
            "Working tree is clean. No changed files to review.".to_owned()
        } else {
            format!(
                "Loaded {} changed files from Git. AI findings are not wired yet.",
                files.len()
            )
        };

        let selected_diff_line = diffs
            .first()
            .and_then(|diff| first_diff_change_index(diff))
            .unwrap_or(0);

        Ok(Self {
            selected_file: 0,
            selected_finding: 0,
            selected_diff_line,
            repo_label: repo_label(&repo_root),
            review_label: review_label(&repo_root),
            backend_label: "not configured".to_owned(),
            review_summary,
            files,
            diffs,
            findings: Vec::new(),
            reviewed_files,
            persistence: Some(persistence),
        })
    }

    pub fn from_error(error: impl Into<String>) -> Self {
        Self {
            selected_file: 0,
            selected_finding: 0,
            selected_diff_line: 0,
            repo_label: "unknown repo".to_owned(),
            review_label: "load error".to_owned(),
            backend_label: "not configured".to_owned(),
            review_summary: error.into(),
            files: Vec::new(),
            diffs: Vec::new(),
            findings: Vec::new(),
            reviewed_files: BTreeSet::new(),
            persistence: None,
        }
    }

    pub fn apply(&mut self, action: ReviewAction) {
        match action {
            ReviewAction::SelectFile(index) => self.select_file(index),
            ReviewAction::SelectFinding(index) => self.select_finding(index),
            ReviewAction::ToggleSelectedFileReviewed => self.toggle_selected_file_reviewed(),
            ReviewAction::NextDiffChange => self.next_diff_change(),
            ReviewAction::PreviousDiffChange => self.previous_diff_change(),
            ReviewAction::NextFile => {
                self.select_file(self.next_index(self.selected_file, self.files.len()))
            }
            ReviewAction::PreviousFile => {
                self.select_file(self.previous_index(self.selected_file, self.files.len()))
            }
            ReviewAction::NextFinding => {
                self.select_finding(self.next_index(self.selected_finding, self.findings.len()))
            }
            ReviewAction::PreviousFinding => {
                self.select_finding(self.previous_index(self.selected_finding, self.findings.len()))
            }
        }
    }

    pub fn repo_label(&self) -> &str {
        &self.repo_label
    }

    pub fn review_label(&self) -> &str {
        &self.review_label
    }

    pub fn backend_label(&self) -> &str {
        &self.backend_label
    }

    pub fn files(&self) -> &[FileChange] {
        &self.files
    }

    pub fn is_file_reviewed(&self, index: usize) -> bool {
        self.files
            .get(index)
            .is_some_and(|file| self.reviewed_files.contains(&file.path))
    }

    pub fn reviewed_file_count(&self) -> usize {
        self.files
            .iter()
            .filter(|file| self.reviewed_files.contains(&file.path))
            .count()
    }

    pub fn pending_file_count(&self) -> usize {
        self.files.len().saturating_sub(self.reviewed_file_count())
    }

    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    pub fn diff_lines(&self) -> &[DiffLine] {
        self.diffs
            .get(self.selected_file)
            .map_or(&[], Vec::as_slice)
    }

    pub fn selected_diff_line_index(&self) -> Option<usize> {
        if self.selected_diff_line < self.diff_lines().len() {
            Some(self.selected_diff_line)
        } else {
            None
        }
    }

    pub fn selected_diff_change_position(&self) -> Option<(usize, usize)> {
        let indices = diff_change_indices(self.diff_lines());
        let position = indices
            .iter()
            .position(|index| *index == self.selected_diff_line)?;

        Some((position + 1, indices.len()))
    }

    pub fn selected_diff_change_range(&self) -> Option<Range<usize>> {
        selected_diff_change_range(self.diff_lines(), self.selected_diff_line)
    }

    pub fn selected_file_index(&self) -> usize {
        self.selected_file
    }

    pub fn selected_finding_index(&self) -> usize {
        self.selected_finding
    }

    pub fn selected_file(&self) -> Option<&FileChange> {
        self.files.get(self.selected_file)
    }

    pub fn selected_finding(&self) -> Option<&Finding> {
        self.findings.get(self.selected_finding)
    }

    pub fn review_summary(&self) -> &str {
        &self.review_summary
    }

    fn select_file(&mut self, index: usize) {
        if index < self.files.len() {
            self.selected_file = index;
            self.selected_finding = 0;
            self.selected_diff_line = first_diff_change_index(self.diff_lines()).unwrap_or(0);
        }
    }

    fn select_finding(&mut self, index: usize) {
        if index < self.findings.len() {
            self.selected_finding = index;
        }
    }

    fn toggle_selected_file_reviewed(&mut self) {
        let Some(path) = self.selected_file().map(|file| file.path.clone()) else {
            return;
        };
        let reviewed = !self.reviewed_files.contains(&path);

        if let Err(error) = self.persist_file_reviewed(&path, reviewed) {
            self.review_summary = format!("Failed to persist reviewed state: {error}");
            return;
        }

        if reviewed {
            self.reviewed_files.insert(path);
        } else {
            self.reviewed_files.remove(&path);
        }
    }

    fn persist_file_reviewed(&self, path: &str, reviewed: bool) -> Result<(), ReviewError> {
        let Some(persistence) = &self.persistence else {
            return Ok(());
        };
        let Some(file_index) = self.files.iter().position(|file| file.path == path) else {
            return Ok(());
        };
        let Some(diff_hash) = persistence.file_hashes.get(file_index) else {
            return Ok(());
        };

        set_file_reviewed(persistence, path, diff_hash, reviewed)
    }

    fn next_diff_change(&mut self) {
        let indices = diff_change_indices(self.diff_lines());
        if indices.is_empty() {
            self.selected_diff_line = 0;
            return;
        }

        self.selected_diff_line = indices
            .iter()
            .copied()
            .find(|index| *index > self.selected_diff_line)
            .unwrap_or(indices[0]);
    }

    fn previous_diff_change(&mut self) {
        let indices = diff_change_indices(self.diff_lines());
        if indices.is_empty() {
            self.selected_diff_line = 0;
            return;
        }

        self.selected_diff_line = indices
            .iter()
            .copied()
            .rev()
            .find(|index| *index < self.selected_diff_line)
            .unwrap_or_else(|| *indices.last().expect("indices is not empty"));
    }

    fn next_index(&self, current: usize, len: usize) -> usize {
        if len == 0 {
            0
        } else {
            (current + 1) % len
        }
    }

    fn previous_index(&self, current: usize, len: usize) -> usize {
        if len == 0 {
            0
        } else if current == 0 {
            len - 1
        } else {
            current - 1
        }
    }
}

fn repo_root(path: &Path) -> Result<PathBuf, ReviewError> {
    let output = git_output(path, &["rev-parse", "--show-toplevel"])?;
    Ok(PathBuf::from(output.trim()))
}

fn repo_label(repo_root: &Path) -> String {
    repo_root
        .file_name()
        .and_then(|name| name.to_str())
        .map_or_else(|| repo_root.display().to_string(), str::to_owned)
}

fn review_label(repo_root: &Path) -> String {
    match git_output(repo_root, &["branch", "--show-current"]) {
        Ok(branch) if !branch.trim().is_empty() => format!("{} working tree", branch.trim()),
        _ => "working tree".to_owned(),
    }
}

fn review_key(repo_root: &Path) -> String {
    match git_output(repo_root, &["branch", "--show-current"]) {
        Ok(branch) if !branch.trim().is_empty() => format!("branch:{}", branch.trim()),
        _ => match git_output(repo_root, &["rev-parse", "HEAD"]) {
            Ok(head) => format!("detached:{}", head.trim()),
            Err(_) => "working-tree".to_owned(),
        },
    }
}

fn review_db_path(repo_root: &Path) -> Result<PathBuf, ReviewError> {
    let intent_dir = repo_root.join(".intent");
    fs::create_dir_all(&intent_dir)?;

    Ok(intent_dir.join("review-tool.sqlite3"))
}

fn load_reviewed_files(
    persistence: &PersistenceContext,
    files: &[FileChange],
) -> Result<BTreeSet<String>, ReviewError> {
    let connection = open_review_db(&persistence.db_path)?;
    let mut reviewed_files = BTreeSet::new();

    for (index, file) in files.iter().enumerate() {
        let Some(diff_hash) = persistence.file_hashes.get(index) else {
            continue;
        };
        let reviewed = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM reviewed_files
                WHERE review_key = ?1 AND path = ?2 AND diff_hash = ?3
            )",
            params![persistence.review_key, file.path, diff_hash],
            |row| row.get::<_, bool>(0),
        )?;

        if reviewed {
            reviewed_files.insert(file.path.clone());
        }
    }

    Ok(reviewed_files)
}

fn set_file_reviewed(
    persistence: &PersistenceContext,
    path: &str,
    diff_hash: &str,
    reviewed: bool,
) -> Result<(), ReviewError> {
    let connection = open_review_db(&persistence.db_path)?;

    if reviewed {
        connection.execute(
            "INSERT OR REPLACE INTO reviewed_files
                (review_key, path, diff_hash, reviewed_at)
             VALUES (?1, ?2, ?3, unixepoch())",
            params![persistence.review_key, path, diff_hash],
        )?;
    } else {
        connection.execute(
            "DELETE FROM reviewed_files
             WHERE review_key = ?1 AND path = ?2 AND diff_hash = ?3",
            params![persistence.review_key, path, diff_hash],
        )?;
    }

    Ok(())
}

fn open_review_db(path: &Path) -> Result<Connection, ReviewError> {
    let connection = Connection::open(path)?;
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS reviewed_files (
            review_key TEXT NOT NULL,
            path TEXT NOT NULL,
            diff_hash TEXT NOT NULL,
            reviewed_at INTEGER NOT NULL,
            PRIMARY KEY (review_key, path, diff_hash)
        );",
    )?;

    Ok(connection)
}

fn git_status_entries(repo_root: &Path) -> Result<Vec<StatusEntry>, ReviewError> {
    let output = git_output_bytes(repo_root, &["status", "--porcelain=v1", "-z", "-uall"])?;
    let records = output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    let mut entries = Vec::new();
    let mut index = 0;

    while let Some(record) = records.get(index) {
        index += 1;

        if record.len() < 4 {
            continue;
        }

        let status = String::from_utf8_lossy(&record[..2]).into_owned();
        if status == "!!" {
            continue;
        }

        let path = String::from_utf8_lossy(&record[3..]).into_owned();

        if status.contains('R') || status.contains('C') {
            index += 1;
        }

        entries.push(StatusEntry { status, path });
    }

    Ok(entries)
}

fn display_status(status: &str) -> String {
    if status == "??" || status.contains('A') {
        "A".to_owned()
    } else if status.contains('D') {
        "D".to_owned()
    } else if status.contains('R') {
        "R".to_owned()
    } else if status.contains('C') {
        "C".to_owned()
    } else if status.contains('U') {
        "U".to_owned()
    } else if status.contains('M') {
        "M".to_owned()
    } else {
        status.trim().to_owned()
    }
}

fn tracked_file_diff(repo_root: &Path, path: &str) -> Result<Vec<DiffLine>, ReviewError> {
    let cached = git_output(
        repo_root,
        &["diff", "--cached", "--no-ext-diff", "--", path],
    )?;
    let working = git_output(repo_root, &["diff", "--no-ext-diff", "--", path])?;
    let mut combined = String::new();

    if !cached.trim().is_empty() {
        combined.push_str(&cached);
    }

    if !working.trim().is_empty() {
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(&working);
    }

    if combined.trim().is_empty() {
        Ok(vec![plain_diff_line(
            "No textual diff available for this file.",
        )])
    } else {
        Ok(parse_unified_diff(&combined))
    }
}

fn untracked_file_diff(repo_root: &Path, path: &str) -> Vec<DiffLine> {
    let full_path = repo_root.join(path);
    let Ok(bytes) = fs::read(&full_path) else {
        return vec![plain_diff_line("Unable to read untracked file.")];
    };

    if bytes.contains(&0) {
        return vec![
            plain_diff_line(format!("diff --git a/{path} b/{path}")),
            plain_diff_line("new binary file; content omitted"),
        ];
    }

    let Ok(contents) = String::from_utf8(bytes) else {
        return vec![
            plain_diff_line(format!("diff --git a/{path} b/{path}")),
            plain_diff_line("new non-UTF-8 file; content omitted"),
        ];
    };
    let content_lines = contents.lines().collect::<Vec<_>>();
    let mut diff = vec![
        plain_diff_line(format!("diff --git a/{path} b/{path}")),
        plain_diff_line("new file"),
        plain_diff_line("--- /dev/null"),
        plain_diff_line(format!("+++ b/{path}")),
        DiffLine {
            prefix: "@@".to_owned(),
            number: "1".to_owned(),
            content: format!("@@ -0,0 +1,{} @@", content_lines.len()),
        },
    ];

    diff.extend(
        content_lines
            .iter()
            .enumerate()
            .map(|(index, line)| DiffLine {
                prefix: "+".to_owned(),
                number: (index + 1).to_string(),
                content: (*line).to_owned(),
            }),
    );

    diff
}

fn parse_unified_diff(diff: &str) -> Vec<DiffLine> {
    let mut lines = Vec::new();
    let mut old_line = 0;
    let mut new_line = 0;

    for raw_line in diff.lines() {
        if raw_line.starts_with("@@") {
            if let Some((old_start, new_start)) = parse_hunk_header(raw_line) {
                old_line = old_start;
                new_line = new_start;
            }

            lines.push(DiffLine {
                prefix: "@@".to_owned(),
                number: new_line.to_string(),
                content: raw_line.to_owned(),
            });
        } else if raw_line.starts_with('+') && !raw_line.starts_with("+++") {
            lines.push(DiffLine {
                prefix: "+".to_owned(),
                number: new_line.to_string(),
                content: raw_line.strip_prefix('+').unwrap_or(raw_line).to_owned(),
            });
            new_line += 1;
        } else if raw_line.starts_with('-') && !raw_line.starts_with("---") {
            lines.push(DiffLine {
                prefix: "-".to_owned(),
                number: old_line.to_string(),
                content: raw_line.strip_prefix('-').unwrap_or(raw_line).to_owned(),
            });
            old_line += 1;
        } else if raw_line.starts_with(' ') {
            lines.push(DiffLine {
                prefix: " ".to_owned(),
                number: new_line.to_string(),
                content: raw_line.strip_prefix(' ').unwrap_or(raw_line).to_owned(),
            });
            old_line += 1;
            new_line += 1;
        } else {
            lines.push(plain_diff_line(raw_line));
        }
    }

    if lines.is_empty() {
        vec![plain_diff_line("No textual diff available for this file.")]
    } else {
        lines
    }
}

fn parse_hunk_header(header: &str) -> Option<(usize, usize)> {
    let mut parts = header.split_whitespace();
    parts.next()?;
    let old_range = parts.next()?.strip_prefix('-')?;
    let new_range = parts.next()?.strip_prefix('+')?;

    Some((parse_range_start(old_range)?, parse_range_start(new_range)?))
}

fn parse_range_start(range: &str) -> Option<usize> {
    range.split(',').next()?.parse().ok()
}

fn diff_stats(diff: &[DiffLine]) -> (usize, usize) {
    diff.iter().fold((0, 0), |(additions, deletions), line| {
        match line.prefix.as_str() {
            "+" => (additions + 1, deletions),
            "-" => (additions, deletions + 1),
            _ => (additions, deletions),
        }
    })
}

fn diff_hash(diff: &[DiffLine]) -> String {
    let mut hash = 0xcbf29ce484222325u64;

    for line in diff {
        hash_bytes(&mut hash, line.prefix.as_bytes());
        hash_bytes(&mut hash, &[0]);
        hash_bytes(&mut hash, line.number.as_bytes());
        hash_bytes(&mut hash, &[0]);
        hash_bytes(&mut hash, line.content.as_bytes());
        hash_bytes(&mut hash, &[0xff]);
    }

    format!("{hash:016x}")
}

fn hash_bytes(hash: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *hash ^= u64::from(*byte);
        *hash = hash.wrapping_mul(0x100000001b3);
    }
}

fn first_diff_change_index(diff: &[DiffLine]) -> Option<usize> {
    diff_change_indices(diff).into_iter().next()
}

fn diff_change_indices(diff: &[DiffLine]) -> Vec<usize> {
    diff.iter()
        .enumerate()
        .filter_map(|(index, line)| {
            if is_changed_line(line) && !diff[..index].last().is_some_and(is_changed_line) {
                Some(index)
            } else {
                None
            }
        })
        .collect()
}

fn selected_diff_change_range(
    diff: &[DiffLine],
    selected_diff_line: usize,
) -> Option<Range<usize>> {
    if selected_diff_line >= diff.len() || !is_changed_line(&diff[selected_diff_line]) {
        return None;
    }

    let mut start = selected_diff_line;
    while start > 0 && is_changed_line(&diff[start - 1]) {
        start -= 1;
    }

    let mut end = selected_diff_line + 1;
    while end < diff.len() && is_changed_line(&diff[end]) {
        end += 1;
    }

    Some(start..end)
}

fn is_changed_line(line: &DiffLine) -> bool {
    matches!(line.prefix.as_str(), "+" | "-")
}

fn plain_diff_line(content: impl Into<String>) -> DiffLine {
    DiffLine {
        prefix: " ".to_owned(),
        number: String::new(),
        content: content.into(),
    }
}

fn git_output(repo_root: &Path, args: &[&str]) -> Result<String, ReviewError> {
    let output = git_output_bytes(repo_root, args)?;
    Ok(String::from_utf8_lossy(&output).into_owned())
}

fn git_output_bytes(repo_root: &Path, args: &[&str]) -> Result<Vec<u8>, ReviewError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo_root)
        .args(args)
        .output()?;

    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(ReviewError::Git {
            command: format!("git -C {} {}", repo_root.display(), args.join(" ")),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        parse_hunk_header, parse_unified_diff, DiffLine, FileChange, ReviewAction, ReviewSession,
    };

    fn test_session() -> ReviewSession {
        ReviewSession {
            selected_file: 0,
            selected_finding: 0,
            selected_diff_line: 1,
            repo_label: "repo".to_owned(),
            review_label: "working tree".to_owned(),
            backend_label: "not configured".to_owned(),
            review_summary: "test".to_owned(),
            files: vec![
                FileChange {
                    path: "a.rs".to_owned(),
                    status: "M".to_owned(),
                    additions: 1,
                    deletions: 0,
                },
                FileChange {
                    path: "b.rs".to_owned(),
                    status: "M".to_owned(),
                    additions: 1,
                    deletions: 0,
                },
            ],
            diffs: vec![
                vec![
                    DiffLine {
                        prefix: "@@".to_owned(),
                        number: "1".to_owned(),
                        content: "@@ -1,2 +1,3 @@".to_owned(),
                    },
                    DiffLine {
                        prefix: "+".to_owned(),
                        number: "1".to_owned(),
                        content: "a".to_owned(),
                    },
                    DiffLine {
                        prefix: " ".to_owned(),
                        number: "2".to_owned(),
                        content: "context".to_owned(),
                    },
                    DiffLine {
                        prefix: "-".to_owned(),
                        number: "3".to_owned(),
                        content: "old".to_owned(),
                    },
                    DiffLine {
                        prefix: "+".to_owned(),
                        number: "3".to_owned(),
                        content: "new".to_owned(),
                    },
                ],
                vec![
                    DiffLine {
                        prefix: "@@".to_owned(),
                        number: "1".to_owned(),
                        content: "@@ -1,1 +1,1 @@".to_owned(),
                    },
                    DiffLine {
                        prefix: " ".to_owned(),
                        number: "1".to_owned(),
                        content: "context".to_owned(),
                    },
                    DiffLine {
                        prefix: "+".to_owned(),
                        number: "2".to_owned(),
                        content: "b".to_owned(),
                    },
                ],
            ],
            findings: Vec::new(),
            reviewed_files: BTreeSet::new(),
            persistence: None,
        }
    }

    #[test]
    fn selecting_file_resets_finding_selection() {
        let mut session = test_session();

        session.apply(ReviewAction::SelectFinding(2));
        session.apply(ReviewAction::SelectFile(1));

        assert_eq!(session.selected_file_index(), 1);
        assert_eq!(session.selected_finding_index(), 0);
        assert_eq!(session.selected_diff_line_index(), Some(2));
    }

    #[test]
    fn navigation_wraps() {
        let mut session = test_session();

        session.apply(ReviewAction::PreviousFile);
        assert_eq!(session.selected_file_index(), session.files().len() - 1);

        session.apply(ReviewAction::NextFile);
        assert_eq!(session.selected_file_index(), 0);
    }

    #[test]
    fn parses_hunk_line_numbers() {
        assert_eq!(
            parse_hunk_header("@@ -12,2 +20,3 @@ fn main()"),
            Some((12, 20))
        );
    }

    #[test]
    fn parses_diff_line_kinds_and_numbers() {
        let diff = parse_unified_diff(
            "diff --git a/a.rs b/a.rs\n@@ -3,2 +3,2 @@\n old\n-removed\n+added\n",
        );

        assert_eq!(diff[1].prefix, "@@");
        assert_eq!(diff[2].number, "3");
        assert_eq!(diff[3].prefix, "-");
        assert_eq!(diff[3].number, "4");
        assert_eq!(diff[4].prefix, "+");
        assert_eq!(diff[4].number, "4");
    }

    #[test]
    fn diff_change_navigation_wraps_between_change_blocks() {
        let mut session = test_session();

        assert_eq!(session.selected_diff_line_index(), Some(1));
        assert_eq!(session.selected_diff_change_position(), Some((1, 2)));
        assert_eq!(session.selected_diff_change_range(), Some(1..2));

        session.apply(ReviewAction::NextDiffChange);
        assert_eq!(session.selected_diff_line_index(), Some(3));
        assert_eq!(session.selected_diff_change_position(), Some((2, 2)));
        assert_eq!(session.selected_diff_change_range(), Some(3..5));

        session.apply(ReviewAction::NextDiffChange);
        assert_eq!(session.selected_diff_line_index(), Some(1));

        session.apply(ReviewAction::PreviousDiffChange);
        assert_eq!(session.selected_diff_line_index(), Some(3));
    }

    #[test]
    fn toggles_selected_file_reviewed_state() {
        let mut session = test_session();

        assert_eq!(session.pending_file_count(), 2);
        assert_eq!(session.reviewed_file_count(), 0);
        assert!(!session.is_file_reviewed(0));

        session.apply(ReviewAction::ToggleSelectedFileReviewed);
        assert!(session.is_file_reviewed(0));
        assert_eq!(session.pending_file_count(), 1);
        assert_eq!(session.reviewed_file_count(), 1);

        session.apply(ReviewAction::ToggleSelectedFileReviewed);
        assert!(!session.is_file_reviewed(0));
        assert_eq!(session.pending_file_count(), 2);
        assert_eq!(session.reviewed_file_count(), 0);
    }
}
