use std::collections::HashMap;

use super::types::{DataFlowGraph, FlowNodeType, GraphLayout, LayoutEdge, LayoutNode};

const NODE_WIDTH: f32 = 180.0;
const NODE_HEIGHT: f32 = 48.0;
const LAYER_GAP: f32 = 80.0;
const NODE_GAP: f32 = 24.0;
const PADDING: f32 = 40.0;

pub(crate) fn layout_graph(graph: &DataFlowGraph) -> GraphLayout {
    if graph.nodes.is_empty() {
        return GraphLayout {
            nodes: Vec::new(),
            edges: Vec::new(),
            labels: Vec::new(),
            total_width: 0.0,
            total_height: 0.0,
        };
    }

    let layers = assign_layers(graph);
    let order = order_nodes_in_layers(graph, &layers);

    let mut labels: Vec<String> = Vec::new();
    let mut label_map: HashMap<usize, usize> = HashMap::new();

    for (i, node) in graph.nodes.iter().enumerate() {
        label_map.insert(i, labels.len());
        labels.push(node.label.clone());
    }

    let mut layout_nodes = Vec::new();
    let mut node_positions: HashMap<usize, (f32, f32)> = HashMap::new();

    let max_layer = *layers.values().max().unwrap_or(&0) as usize;

    for layer_idx in 0..=max_layer {
        let nodes_in_layer: Vec<usize> = order
            .get(&layer_idx)
            .cloned()
            .unwrap_or_default();

        let x_start = PADDING;

        for (i, &node_id) in nodes_in_layer.iter().enumerate() {
            let x = x_start + i as f32 * (NODE_WIDTH + NODE_GAP);
            let y = PADDING + layer_idx as f32 * (NODE_HEIGHT + LAYER_GAP);
            node_positions.insert(node_id, (x, y));

            let node_type = graph.nodes.get(node_id).map(|n| n.node_type).unwrap_or(FlowNodeType::Variable);
            let li = label_map.get(&node_id).copied().unwrap_or(0);

            layout_nodes.push(LayoutNode {
                id: node_id,
                node_type,
                label_index: li,
                x,
                y,
                width: NODE_WIDTH,
                height: NODE_HEIGHT,
            });
        }
    }

    let mut layout_edges = Vec::new();
    for edge in &graph.edges {
        let source_pos = node_positions.get(&edge.source);
        let target_pos = node_positions.get(&edge.target);
        if let (Some(&(sx, sy)), Some(&(tx, ty))) = (source_pos, target_pos) {
            let start = (sx + NODE_WIDTH / 2.0, sy + NODE_HEIGHT);
            let end = (tx + NODE_WIDTH / 2.0, ty);
            let mid_y = (sy + NODE_HEIGHT + ty) / 2.0;
            let points = vec![
                start,
                (sx + NODE_WIDTH / 2.0, mid_y),
                (tx + NODE_WIDTH / 2.0, mid_y),
                end,
            ];
            layout_edges.push(LayoutEdge {
                source: edge.source,
                target: edge.target,
                points,
            });
        }
    }

    let total_width = layout_nodes.iter().map(|n| n.x + n.width).fold(0.0f32, f32::max) + PADDING;
    let total_height = layout_nodes.iter().map(|n| n.y + n.height).fold(0.0f32, f32::max) + PADDING;

    GraphLayout {
        nodes: layout_nodes,
        edges: layout_edges,
        labels,
        total_width,
        total_height,
    }
}

fn assign_layers(graph: &DataFlowGraph) -> HashMap<usize, u32> {
    let mut layers = HashMap::new();

    let targets: std::collections::HashSet<usize> = graph.edges.iter().map(|e| e.target).collect();
    let sources: std::collections::HashSet<usize> = graph.edges.iter().map(|e| e.source).collect();
    let roots: Vec<usize> = sources.difference(&targets).copied().collect();

    let roots = if roots.is_empty() {
        graph.nodes.iter().map(|n| n.id).collect()
    } else {
        roots
    };

    for &root in &roots {
        let base_layer = match graph.nodes.get(root).map(|n| n.node_type) {
            Some(FlowNodeType::Input) => 0,
            Some(FlowNodeType::Type) => 0,
            Some(FlowNodeType::Output) => u32::MAX / 2,
            _ => 0,
        };
        layers.insert(root, base_layer);
    }

    let adj: HashMap<usize, Vec<usize>> = graph.edges.iter().fold(
        HashMap::new(),
        |mut acc, e| {
            acc.entry(e.source).or_default().push(e.target);
            acc
        },
    );

    let mut queue: Vec<usize> = roots;
    while let Some(node_id) = queue.pop() {
        let current_layer = *layers.get(&node_id).unwrap_or(&0);
        if let Some(neighbors) = adj.get(&node_id) {
            for &neighbor in neighbors {
                let neighbor_type = graph.nodes.get(neighbor).map(|n| n.node_type);
                let new_layer = match neighbor_type {
                    Some(FlowNodeType::Output) => current_layer + 1,
                    _ => current_layer + 1,
                };
                let existing = layers.get(&neighbor).copied().unwrap_or(u32::MAX);
                if new_layer < existing {
                    layers.insert(neighbor, new_layer);
                    queue.push(neighbor);
                }
            }
        }
    }

    for node in &graph.nodes {
        layers.entry(node.id).or_insert(0);
    }

    layers
}

fn order_nodes_in_layers(
    graph: &DataFlowGraph,
    layers: &HashMap<usize, u32>,
) -> HashMap<usize, Vec<usize>> {
    let mut layer_nodes: HashMap<usize, Vec<usize>> = HashMap::new();

    for node in &graph.nodes {
        let layer = *layers.get(&node.id).unwrap_or(&0) as usize;
        layer_nodes.entry(layer).or_default().push(node.id);
    }

    let node_map: HashMap<usize, FlowNodeType> = graph.nodes.iter().map(|n| (n.id, n.node_type)).collect();

    for (_layer, nodes) in layer_nodes.iter_mut() {
        nodes.sort_by(|a, b| {
            let na = node_map.get(a).copied().unwrap_or(FlowNodeType::Variable);
            let nb = node_map.get(b).copied().unwrap_or(FlowNodeType::Variable);
            na.cmp(&nb)
        });
    }

    layer_nodes
}
