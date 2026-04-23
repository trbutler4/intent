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
    pub(crate) source: EdgeEndpoint,
    pub(crate) target: EdgeEndpoint,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) enum EdgeEndpoint {
    Row(usize),
    Output(usize),
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
        Self::walk(self.tree.root_node(), &self.source, &mut nodes, &mut cursor);
        nodes
    }

    pub(crate) fn extract_edges(&self, nodes: &[ExtractedNode]) -> Vec<ExtractedEdge> {
        let mut edges = Vec::new();
        let mut cursor = self.tree.root_node().walk();

        let func_ranges: Vec<(usize, usize)> = nodes
            .iter()
            .filter(|n| n.node_type == FlowNodeType::Function)
            .map(|n| (n.start_row, n.end_row))
            .collect();

        Self::walk_edges(self.tree.root_node(), &self.source, &func_ranges, &mut edges, &mut cursor);
        edges
    }

    fn walk(
        node: tree_sitter::Node,
        source: &str,
        nodes: &mut Vec<ExtractedNode>,
        cursor: &mut tree_sitter::TreeCursor,
    ) {
        let kind = node.kind();

        match kind {
            "function_item" => {
                if let Some(name) = Self::child_text_by_field(node, "name", source) {
                    nodes.push(ExtractedNode {
                        name,
                        node_type: FlowNodeType::Function,
                        start_row: node.start_position().row,
                        end_row: node.end_position().row,
                    });
                }
            }
            "struct_item" | "enum_item" => {
                if let Some(name) = Self::child_text_by_field(node, "name", source) {
                    nodes.push(ExtractedNode {
                        name,
                        node_type: FlowNodeType::Type,
                        start_row: node.start_position().row,
                        end_row: node.end_position().row,
                    });
                }
            }
            "impl_item" => {
                if let Some(type_name) = Self::child_text_by_field(node, "type", source) {
                    nodes.push(ExtractedNode {
                        name: format!("impl {}", type_name),
                        node_type: FlowNodeType::Type,
                        start_row: node.start_position().row,
                        end_row: node.end_position().row,
                    });
                }
            }
            "let_declaration" => {
                if let Some(name) = Self::extract_binding_name(node, source) {
                    nodes.push(ExtractedNode {
                        name,
                        node_type: FlowNodeType::Variable,
                        start_row: node.start_position().row,
                        end_row: node.end_position().row,
                    });
                }
            }
            "parameter" => {
                if let Some(name) = Self::extract_param_name(node, source) {
                    nodes.push(ExtractedNode {
                        name,
                        node_type: FlowNodeType::Input,
                        start_row: node.start_position().row,
                        end_row: node.end_position().row,
                    });
                }
            }
            "field_declaration" => {
                if let Some(name) = Self::child_text_by_field(node, "name", source) {
                    nodes.push(ExtractedNode {
                        name,
                        node_type: FlowNodeType::Input,
                        start_row: node.start_position().row,
                        end_row: node.end_position().row,
                    });
                }
            }
            _ => {}
        }

        if cursor.goto_first_child() {
            loop {
                Self::walk(cursor.node(), source, nodes, cursor);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }

    fn walk_edges(
        node: tree_sitter::Node,
        source: &str,
        func_ranges: &[(usize, usize)],
        edges: &mut Vec<ExtractedEdge>,
        cursor: &mut tree_sitter::TreeCursor,
    ) {
        let kind = node.kind();

        match kind {
            "let_declaration" => {
                Self::extract_let_edges(node, source, func_ranges, edges);
            }
            "return_expression" => {
                Self::extract_return_edges(node, source, func_ranges, edges);
            }
            "call_expression" => {
                Self::extract_call_edges(node, source, func_ranges, edges);
            }
            "assignment_expression" => {
                Self::extract_assignment_edges(node, source, func_ranges, edges);
            }
            _ => {}
        }

        if cursor.goto_first_child() {
            loop {
                Self::walk_edges(cursor.node(), source, func_ranges, edges, cursor);
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
            cursor.goto_parent();
        }
    }

    fn extract_let_edges(
        node: tree_sitter::Node,
        source: &str,
        func_ranges: &[(usize, usize)],
        edges: &mut Vec<ExtractedEdge>,
    ) {
        let Some(value_node) = node.child_by_field_name("value") else {
            return;
        };
        let let_row = node.start_position().row;
        let ident_names = Self::collect_identifiers(value_node, source);

        for ident_name in ident_names {
            if let Some(ident_row) = Self::find_ident_row_in_scope(&ident_name, let_row, func_ranges) {
                edges.push(ExtractedEdge {
                    source: EdgeEndpoint::Row(ident_row),
                    target: EdgeEndpoint::Row(let_row),
                });
            }
        }
    }

    fn extract_return_edges(
        node: tree_sitter::Node,
        source: &str,
        func_ranges: &[(usize, usize)],
        edges: &mut Vec<ExtractedEdge>,
    ) {
        let return_row = node.start_position().row;
        let Some(containing_func) = Self::containing_function(return_row, func_ranges) else {
            return;
        };

        let returned_idents = Self::collect_identifiers(node, source);

        edges.push(ExtractedEdge {
            source: EdgeEndpoint::Row(return_row),
            target: EdgeEndpoint::Output(containing_func.0),
        });

        for ident_name in returned_idents {
            if let Some(ident_row) = Self::find_ident_row_in_scope(&ident_name, return_row, func_ranges) {
                edges.push(ExtractedEdge {
                    source: EdgeEndpoint::Row(ident_row),
                    target: EdgeEndpoint::Row(return_row),
                });
            }
        }
    }

    fn extract_call_edges(
        node: tree_sitter::Node,
        source: &str,
        func_ranges: &[(usize, usize)],
        edges: &mut Vec<ExtractedEdge>,
    ) {
        let Some(func_node) = node.child_by_field_name("function") else {
            return;
        };
        let call_name = Self::resolve_call_name(func_node, source);
        let Some(call_name) = call_name else {
            return;
        };

        let call_row = node.start_position().row;

        if let Some(callee_row) = Self::find_ident_row_in_scope(&call_name, call_row, func_ranges) {
            edges.push(ExtractedEdge {
                source: EdgeEndpoint::Row(call_row),
                target: EdgeEndpoint::Row(callee_row),
            });
        }

        if let Some(args_node) = node.child_by_field_name("arguments") {
            let arg_idents = Self::collect_identifiers(args_node, source);
            for ident_name in arg_idents {
                if let Some(ident_row) = Self::find_ident_row_in_scope(&ident_name, call_row, func_ranges) {
                    edges.push(ExtractedEdge {
                        source: EdgeEndpoint::Row(ident_row),
                        target: EdgeEndpoint::Row(call_row),
                    });
                }
            }
        }
    }

    fn extract_assignment_edges(
        node: tree_sitter::Node,
        source: &str,
        func_ranges: &[(usize, usize)],
        edges: &mut Vec<ExtractedEdge>,
    ) {
        let assign_row = node.start_position().row;
        let Some(right_node) = node.child_by_field_name("right") else {
            return;
        };

        let right_idents = Self::collect_identifiers(right_node, source);
        for ident_name in right_idents {
            if let Some(ident_row) = Self::find_ident_row_in_scope(&ident_name, assign_row, func_ranges) {
                edges.push(ExtractedEdge {
                    source: EdgeEndpoint::Row(ident_row),
                    target: EdgeEndpoint::Row(assign_row),
                });
            }
        }
    }

    fn resolve_call_name(node: tree_sitter::Node, source: &str) -> Option<String> {
        match node.kind() {
            "identifier" => node.utf8_text(source.as_bytes()).ok().map(|s| s.to_string()),
            "scoped_identifier" => {
                let path = node.child_by_field_name("path")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok());
                let name = node.child_by_field_name("name")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok());
                name.map(|n| n.to_string()).or_else(|| path.map(|p| p.to_string()))
            }
            "generic_function" => {
                let inner = node.child_by_field_name("function")?;
                Self::resolve_call_name(inner, source)
            }
            "field_expression" => {
                node.child_by_field_name("field")
                    .and_then(|n| n.utf8_text(source.as_bytes()).ok())
                    .map(|s| s.to_string())
            }
            _ => None,
        }
    }

    fn collect_identifiers(node: tree_sitter::Node, source: &str) -> Vec<String> {
        let mut idents = Vec::new();
        let mut cursor = node.walk();

        fn walk(n: tree_sitter::Node, source: &str, idents: &mut Vec<String>, cursor: &mut tree_sitter::TreeCursor) {
            if n.kind() == "identifier" {
                if let Ok(text) = n.utf8_text(source.as_bytes()) {
                    let name = text.to_string();
                    if !idents.contains(&name) {
                        idents.push(name);
                    }
                }
            }
            if cursor.goto_first_child() {
                loop {
                    walk(cursor.node(), source, idents, cursor);
                    if !cursor.goto_next_sibling() {
                        break;
                    }
                }
                cursor.goto_parent();
            }
        }

        walk(node, source, &mut idents, &mut cursor);
        idents
    }

    fn find_ident_row_in_scope(_name: &str, reference_row: usize, func_ranges: &[(usize, usize)]) -> Option<usize> {
        for &(start, end) in func_ranges {
            if start <= reference_row && reference_row <= end {
                return Some(start);
            }
        }
        None
    }

    fn containing_function(row: usize, func_ranges: &[(usize, usize)]) -> Option<(usize, usize)> {
        func_ranges.iter().find(|&&range| range.0 <= row && row <= range.1).copied()
    }

    fn child_text_by_field(node: tree_sitter::Node, field: &str, source: &str) -> Option<String> {
        node.child_by_field_name(field)
            .and_then(|n| n.utf8_text(source.as_bytes()).ok())
            .map(|s| s.to_string())
    }

    fn extract_binding_name(node: tree_sitter::Node, source: &str) -> Option<String> {
        let pattern_node = node.child_by_field_name("pattern")?;
        Self::first_identifier_in(pattern_node, source)
    }

    fn extract_param_name(node: tree_sitter::Node, source: &str) -> Option<String> {
        let pattern_node = node.child_by_field_name("pattern")?;
        if pattern_node.kind() == "self" {
            return Some("self".to_string());
        }
        Self::first_identifier_in(pattern_node, source)
    }

    fn first_identifier_in(node: tree_sitter::Node, source: &str) -> Option<String> {
        let mut cursor = node.walk();
        if cursor.goto_first_child() {
            loop {
                let child = cursor.node();
                if child.kind() == "identifier" {
                    return child.utf8_text(source.as_bytes()).ok().map(|s| s.to_string());
                }
                if !cursor.goto_next_sibling() {
                    break;
                }
            }
        }
        node.utf8_text(source.as_bytes()).ok().map(|s| s.to_string())
    }
}
