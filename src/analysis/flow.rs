use std::collections::HashMap;
use std::path::PathBuf;

use super::ast::ParsedSource;
use super::types::{DataFlowGraph, FlowEdge, FlowNode};

pub(crate) fn build_flow_graph(source: String, file_path: PathBuf) -> DataFlowGraph {
    let Some(parsed) = ParsedSource::parse(source) else {
        return DataFlowGraph::empty();
    };

    let extracted_nodes = parsed.extract_nodes();
    let extracted_edges = parsed.extract_edges(&extracted_nodes);

    let mut row_to_id: HashMap<usize, usize> = HashMap::new();
    let mut nodes = Vec::new();

    for (i, ext) in extracted_nodes.iter().enumerate() {
        row_to_id.insert(ext.start_row, i);
        nodes.push(FlowNode {
            id: i,
            label: ext.name.clone(),
            node_type: ext.node_type,
            file_path: file_path.clone(),
            start_row: ext.start_row,
            end_row: ext.end_row,
        });
    }

    let mut edges: Vec<FlowEdge> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for ext_edge in &extracted_edges {
        if let (Some(&source_id), Some(&target_id)) = (row_to_id.get(&ext_edge.source_row), row_to_id.get(&ext_edge.target_row)) {
            if source_id == target_id {
                continue;
            }
            let key = (source_id, target_id);
            if seen.insert(key) {
                edges.push(FlowEdge {
                    source: source_id,
                    target: target_id,
                    label: None,
                });
            }
        }
    }

    DataFlowGraph { nodes, edges }
}
