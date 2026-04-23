use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use super::diff::{DiffLine, build_untracked_diff, fallback_diff_for_status, parse_unified_diff};
use super::{git_command_success, run_git};
use super::status::{StatusEntry, parse_name_status, parse_porcelain_status};

#[derive(Clone)]
pub(crate) struct FileChange {
    pub(crate) path: String,
    pub(crate) status: String,
    pub(crate) additions: usize,
    pub(crate) deletions: usize,
    pub(crate) untracked: bool,
    pub(crate) diff_lines: Option<Vec<DiffLine>>,
}

#[derive(Clone)]
pub(crate) enum ReviewMode {
    WorkingTree { has_head: bool },
    LastCommit { has_parent: bool },
    Empty,
}

pub(crate) struct RepoSnapshot {
    pub(crate) root: PathBuf,
    pub(crate) review_mode: ReviewMode,
    pub(crate) files: Vec<FileChange>,
    pub(crate) load_error: Option<String>,
}

pub(crate) fn load_repo_snapshot(requested_path: Option<PathBuf>) -> RepoSnapshot {
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

pub(crate) fn load_diff_lines_for_file(
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
