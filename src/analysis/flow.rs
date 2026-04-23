use std::collections::HashMap;
use std::path::PathBuf;

use super::ast::{EdgeEndpoint, ParsedSource};
use super::types::{DataFlowGraph, FlowEdge, FlowNode, FlowNodeType};

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

    let mut output_func_rows: HashMap<usize, usize> = HashMap::new();
    for ext_edge in &extracted_edges {
        if let EdgeEndpoint::Output(func_row) = ext_edge.target {
            output_func_rows.entry(func_row).or_insert_with(|| {
                let output_id = nodes.len();
                let func_name = row_to_id
                    .get(&func_row)
                    .and_then(|&id| nodes.get(id))
                    .map(|n| n.label.clone())
                    .unwrap_or_else(|| "output".to_string());
                nodes.push(FlowNode {
                    id: output_id,
                    label: format!("{} →", func_name),
                    node_type: FlowNodeType::Output,
                    file_path: file_path.clone(),
                    start_row: func_row,
                    end_row: func_row,
                });
                output_id
            });
        }
    }

    let mut edges: Vec<FlowEdge> = Vec::new();
    let mut seen: std::collections::HashSet<(usize, usize)> = std::collections::HashSet::new();

    for ext_edge in &extracted_edges {
        let source_id = match &ext_edge.source {
            EdgeEndpoint::Row(row) => row_to_id.get(row).copied(),
            EdgeEndpoint::Output(_) => None,
        };
        let target_id = match &ext_edge.target {
            EdgeEndpoint::Row(row) => row_to_id.get(row).copied(),
            EdgeEndpoint::Output(func_row) => output_func_rows.get(func_row).copied(),
        };

        if let (Some(s), Some(t)) = (source_id, target_id) {
            if s == t {
                continue;
            }
            if seen.insert((s, t)) {
                edges.push(FlowEdge {
                    source: s,
                    target: t,
                    label: None,
                });
            }
        }
    }

    DataFlowGraph { nodes, edges }
}
