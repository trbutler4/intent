use tree_sitter::Parser;

use super::types::FlowNodeType;

pub(crate) struct ParsedSource {
    tree: tree_sitter::Tree,
    source: String,
}

pub(crate) struct ExtractedNode {
    pub(crate) name: String,
    pub(crate) node_type: FlowNodeType,
    pub(crate) start_row: usize,
    pub(crate) end_row: usize,
}

pub(crate) struct ExtractedEdge {
    pub(crate) source_row: usize,
    pub(crate) target_row: usize,
}

impl ParsedSource {
    pub(crate) fn parse(source: String) -> Option<Self> {
        let mut parser = Parser::new();
        parser.set_language(&tree_sitter_rust::LANGUAGE.into())
            .ok()?;

        let tree = parser.parse(&source, None)?;
        Some(Self { tree, source })
    }

    pub(crate) fn extract_nodes(&self) -> Vec<ExtractedNode> {
        let mut nodes = Vec::new();
        let mut cursor = self.tree.root_node().walk();

        fn walk(node: tree_sitter::Node, source: &str, nodes: &mut Vec<ExtractedNode>, cursor: &mut tree_sitter::TreeCursor) {
            let kind = node.kind();

            if kind == "function_item" || kind == "impl_item" || kind == "trait_item" {
                if let Some(name) = extract_name(node, source) {
                    nodes.push(ExtractedNode {
                        name,
                        node_type: FlowNodeType::Function,
                        start_row: node.start_position().row,
                        end_row: node.end_position().row,
                    });
                }
            } else if kind == "let_declaration" || kind == "let_mut_pattern" || kind == "field_declaration" {
                if let Some(name) = extract_binding_name(node, source) {
                    nodes.push(ExtractedNode {
                        name,
                        node_type: FlowNodeType::Variable,
                        start_row: node.start_position().row,
                        end_row: node.end_position().row,
                    });
                }
            } else if kind == "function_signature" || kind == "parameter" {
                if let Some(name) = extract_name(node, source) {
                    nodes.push(ExtractedNode {
                        name,
                        node_type: FlowNodeType::Input,
                        start_row: node.start_position().row,
                        end_row: node.end_position().row,
                    });
                }
            }

            if cursor.goto_first_child() {
                loop {
                    walk(cursor.node(), source, nodes, cursor);
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
                cursor.goto_parent();
            }
        }

        walk(self.tree.root_node(), &self.source, &mut nodes, &mut cursor);
        nodes
    }

    pub(crate) fn extract_edges(&self, nodes: &[ExtractedNode]) -> Vec<ExtractedEdge> {
        let mut edges = Vec::new();
        let functions: Vec<&ExtractedNode> = nodes.iter().filter(|n| n.node_type == FlowNodeType::Function).collect();
        let variables: Vec<&ExtractedNode> = nodes.iter().filter(|n| n.node_type == FlowNodeType::Variable).collect();

        for var in &variables {
            for func in &functions {
                if var.start_row >= func.start_row && var.end_row <= func.end_row {
                    edges.push(ExtractedEdge {
                        source_row: func.start_row,
                        target_row: var.start_row,
                    });
                }
            }
        }

        let source_lines: Vec<&str> = self.source.lines().collect();
        for caller in &functions {
            for callee in &functions {
                if caller.start_row == callee.start_row {
                    continue;
                }
                if function_references(&source_lines, caller, callee) {
                    edges.push(ExtractedEdge {
                        source_row: caller.start_row,
                        target_row: callee.start_row,
                    });
                }
            }
        }

        edges
    }
}

fn extract_name(node: tree_sitter::Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "identifier" || child.kind() == "type_identifier" {
                return Some(child.utf8_text(source.as_bytes()).ok()?.to_string());
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

fn extract_binding_name(node: tree_sitter::Node, source: &str) -> Option<String> {
    let mut cursor = node.walk();
    if cursor.goto_first_child() {
        loop {
            let child = cursor.node();
            if child.kind() == "identifier" {
                return Some(child.utf8_text(source.as_bytes()).ok()?.to_string());
            }
            if child.kind() == "let_pattern" {
                let mut pat_cursor = child.walk();
                if pat_cursor.goto_first_child() {
                    loop {
                        let inner = pat_cursor.node();
                        if inner.kind() == "identifier" {
                            return Some(inner.utf8_text(source.as_bytes()).ok()?.to_string());
                        }
                        if !pat_cursor.goto_next_sibling() {
                            break;
                        }
                    }
                }
            }
            if !cursor.goto_next_sibling() {
                break;
            }
        }
    }
    None
}

fn function_references(lines: &[&str], caller: &ExtractedNode, callee: &ExtractedNode) -> bool {
    let start = caller.start_row;
    let end = (caller.end_row + 1).min(lines.len());
    let callee_name = &callee.name;

    for line in &lines[start..end] {
        if line.contains(callee_name.as_str()) {
            let pos = line.find(callee_name.as_str()).unwrap();
            let after = &line[pos + callee_name.len()..];
            if after.starts_with('(') || after.starts_with("::") || after.starts_with('.') || after.starts_with('<') || after.is_empty() || after.starts_with(' ') {
                return true;
            }
        }
    }
    false
}
