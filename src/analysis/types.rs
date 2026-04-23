use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) enum FlowNodeType {
    Function,
    Variable,
    Input,
    Output,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct FlowNode {
    pub(crate) id: usize,
    pub(crate) label: String,
    pub(crate) node_type: FlowNodeType,
    pub(crate) file_path: PathBuf,
    pub(crate) start_row: usize,
    pub(crate) end_row: usize,
}

#[derive(Clone, Debug)]
pub(crate) struct FlowEdge {
    pub(crate) source: usize,
    pub(crate) target: usize,
    pub(crate) label: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct DataFlowGraph {
    pub(crate) nodes: Vec<FlowNode>,
    pub(crate) edges: Vec<FlowEdge>,
}

impl DataFlowGraph {
    pub(crate) fn empty() -> Self {
        Self {
            nodes: Vec::new(),
            edges: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct LayoutNode {
    pub(crate) id: usize,
    pub(crate) x: f32,
    pub(crate) y: f32,
    pub(crate) width: f32,
    pub(crate) height: f32,
}

#[derive(Clone, Debug)]
pub(crate) struct LayoutEdge {
    pub(crate) source: usize,
    pub(crate) target: usize,
    pub(crate) points: Vec<(f32, f32)>,
}

#[derive(Clone, Debug)]
pub(crate) struct GraphLayout {
    pub(crate) nodes: Vec<LayoutNode>,
    pub(crate) edges: Vec<LayoutEdge>,
    pub(crate) total_width: f32,
    pub(crate) total_height: f32,
}
