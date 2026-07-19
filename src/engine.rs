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
    NextFile,
    PreviousFile,
    NextFinding,
    PreviousFinding,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewSession {
    selected_file: usize,
    selected_finding: usize,
    files: Vec<FileChange>,
    findings: Vec<Finding>,
    diff_lines: Vec<DiffLine>,
}

impl ReviewSession {
    pub fn mock() -> Self {
        Self {
            selected_file: 0,
            selected_finding: 0,
            files: mock_files(),
            findings: mock_findings(),
            diff_lines: mock_diff_lines(),
        }
    }

    pub fn apply(&mut self, action: ReviewAction) {
        match action {
            ReviewAction::SelectFile(index) => self.select_file(index),
            ReviewAction::SelectFinding(index) => self.select_finding(index),
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

    pub fn files(&self) -> &[FileChange] {
        &self.files
    }

    pub fn findings(&self) -> &[Finding] {
        &self.findings
    }

    pub fn diff_lines(&self) -> &[DiffLine] {
        &self.diff_lines
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

    pub fn review_summary(&self) -> &'static str {
        "Two correctness risks and one UI consistency issue detected in the current mock diff."
    }

    fn select_file(&mut self, index: usize) {
        if index < self.files.len() {
            self.selected_file = index;
            self.selected_finding = 0;
        }
    }

    fn select_finding(&mut self, index: usize) {
        if index < self.findings.len() {
            self.selected_finding = index;
        }
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

fn mock_files() -> Vec<FileChange> {
    vec![
        FileChange {
            path: "src/review/session.rs".to_owned(),
            status: "M".to_owned(),
            additions: 28,
            deletions: 7,
        },
        FileChange {
            path: "src/review/prompts.rs".to_owned(),
            status: "A".to_owned(),
            additions: 74,
            deletions: 0,
        },
        FileChange {
            path: "src/git/diff_parser.rs".to_owned(),
            status: "M".to_owned(),
            additions: 19,
            deletions: 11,
        },
        FileChange {
            path: "src/ui/review_panel.rs".to_owned(),
            status: "M".to_owned(),
            additions: 46,
            deletions: 13,
        },
    ]
}

fn mock_findings() -> Vec<Finding> {
    vec![
        Finding {
            severity: "high".to_owned(),
            title: "Unbounded prompt context".to_owned(),
            summary: "The prompt builder appends entire hunks without a token budget, which can silently exceed model limits on large reviews.".to_owned(),
            line: "prompts.rs:48".to_owned(),
        },
        Finding {
            severity: "medium".to_owned(),
            title: "Session cache misses branch changes".to_owned(),
            summary: "The cache key omits base and head refs, so results can be reused after the user switches comparison branches.".to_owned(),
            line: "session.rs:91".to_owned(),
        },
        Finding {
            severity: "low".to_owned(),
            title: "Inline comment count is stale".to_owned(),
            summary: "The sidebar still renders a hard-coded comment count instead of deriving it from the active review state.".to_owned(),
            line: "review_panel.rs:22".to_owned(),
        },
    ]
}

fn mock_diff_lines() -> Vec<DiffLine> {
    vec![
        DiffLine {
            prefix: "@@".to_owned(),
            number: "12".to_owned(),
            content: "fn build_review_prompt(session: &ReviewSession) -> String {".to_owned(),
        },
        DiffLine {
            prefix: " ".to_owned(),
            number: "13".to_owned(),
            content: "    let mut prompt = String::from(SYSTEM_PROMPT);".to_owned(),
        },
        DiffLine {
            prefix: "-".to_owned(),
            number: "14".to_owned(),
            content: "    prompt.push_str(&session.diff);".to_owned(),
        },
        DiffLine {
            prefix: "+".to_owned(),
            number: "14".to_owned(),
            content: "    for hunk in &session.selected_hunks {".to_owned(),
        },
        DiffLine {
            prefix: "+".to_owned(),
            number: "15".to_owned(),
            content: "        prompt.push_str(&hunk.header);".to_owned(),
        },
        DiffLine {
            prefix: "+".to_owned(),
            number: "16".to_owned(),
            content: "        prompt.push_str(&hunk.diff);".to_owned(),
        },
        DiffLine {
            prefix: "+".to_owned(),
            number: "17".to_owned(),
            content: "    }".to_owned(),
        },
        DiffLine {
            prefix: " ".to_owned(),
            number: "18".to_owned(),
            content: "    prompt".to_owned(),
        },
        DiffLine {
            prefix: " ".to_owned(),
            number: "19".to_owned(),
            content: "}".to_owned(),
        },
        DiffLine {
            prefix: "@@".to_owned(),
            number: "48".to_owned(),
            content: "fn build_cache_key(repo: &Repo, session: &ReviewSession) -> String {"
                .to_owned(),
        },
        DiffLine {
            prefix: " ".to_owned(),
            number: "49".to_owned(),
            content: "    format!(\"{}:{}\", repo.path.display(), session.commit_sha)".to_owned(),
        },
        DiffLine {
            prefix: "@@".to_owned(),
            number: "88".to_owned(),
            content: "fn render_sidebar(state: &ReviewState) -> SidebarSummary {".to_owned(),
        },
        DiffLine {
            prefix: "+".to_owned(),
            number: "89".to_owned(),
            content: "    let inline_comment_count = 3;".to_owned(),
        },
        DiffLine {
            prefix: " ".to_owned(),
            number: "90".to_owned(),
            content: "    SidebarSummary::new(state.files_changed)".to_owned(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{ReviewAction, ReviewSession};

    #[test]
    fn selecting_file_resets_finding_selection() {
        let mut session = ReviewSession::mock();

        session.apply(ReviewAction::SelectFinding(2));
        session.apply(ReviewAction::SelectFile(1));

        assert_eq!(session.selected_file_index(), 1);
        assert_eq!(session.selected_finding_index(), 0);
    }

    #[test]
    fn navigation_wraps() {
        let mut session = ReviewSession::mock();

        session.apply(ReviewAction::PreviousFile);
        assert_eq!(session.selected_file_index(), session.files().len() - 1);

        session.apply(ReviewAction::NextFile);
        assert_eq!(session.selected_file_index(), 0);
    }
}
