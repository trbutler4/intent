pub(crate) struct StatusEntry {
    pub(crate) path: String,
    pub(crate) status: String,
    pub(crate) untracked: bool,
}

pub(crate) fn parse_porcelain_status(line: &str) -> Option<StatusEntry> {
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

pub(crate) fn parse_name_status(line: &str) -> Option<StatusEntry> {
    let mut parts = line.split('\t');
    let raw_status = parts.next()?.trim();
    let path = parts.next_back()?.trim();

    Some(StatusEntry {
        path: path.to_string(),
        status: summarize_status(raw_status),
        untracked: false,
    })
}

pub(crate) fn summarize_status(raw_status: &str) -> String {
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
