use crate::git::FileChange;

#[derive(Clone)]
pub(crate) struct FileTreeNode {
    pub(crate) name: String,
    pub(crate) full_path: String,
    pub(crate) children: Vec<FileTreeNode>,
    pub(crate) file_index: Option<usize>,
}

pub(crate) fn build_file_tree(files: &[FileChange]) -> Vec<FileTreeNode> {
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

pub(crate) fn directory_id(path: &str) -> u64 {
    let mut hash = 1469598103934665603u64;
    for byte in path.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(1099511628211);
    }
    hash
}
