use std::fs;
use std::path::Path;

use super::status::StatusEntry;

#[derive(Clone)]
pub(crate) struct DiffLine {
    pub(crate) prefix: String,
    pub(crate) number: String,
    pub(crate) content: String,
}

pub(crate) fn fallback_diff_for_status(repo_path: &Path, entry: &StatusEntry) -> Vec<DiffLine> {
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

pub(crate) fn build_untracked_diff(repo_path: &Path, relative_path: &str) -> Vec<DiffLine> {
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

pub(crate) fn parse_unified_diff(diff: &str) -> Vec<DiffLine> {
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
