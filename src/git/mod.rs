mod diff;
mod snapshot;
mod status;

use std::path::Path;
use std::process::Command;

pub(crate) use diff::DiffLine;
pub(crate) use snapshot::{FileChange, RepoSnapshot, load_diff_lines_for_file, load_repo_snapshot};

pub(crate) fn run_git(repo_path: &Path, args: &[&str]) -> Result<String, String> {
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

pub(crate) fn git_command_success(repo_path: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .arg("-C")
        .arg(repo_path)
        .args(args)
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}
