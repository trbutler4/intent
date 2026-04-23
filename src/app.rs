use std::collections::HashMap;
use std::collections::HashSet;

use gpui::{
    AnyElement, Context, FontWeight, Point, Render, ScrollStrategy,
    UniformListScrollHandle, Window, div, px, prelude::*, rgb,
};

use crate::analysis::{DataFlowGraph, GraphLayout, build_flow_graph, layout_graph};
use crate::git::{DiffLine, RepoSnapshot, load_diff_lines_for_file, load_repo_snapshot};
use crate::ui::diff_view::render_diff_view;
use crate::ui::file_tree::{FileTreeNode, build_file_tree, directory_id};
use crate::ui::graph_view::{GraphViewState, render_graph_view};
use crate::ui::theme::{render_badge, render_empty_state, render_panel_header, status_color};

pub(crate) struct ReviewApp {
    repo: RepoSnapshot,
    selected_file: usize,
    diff_scroll_handle: UniformListScrollHandle,
    diff_focus_mode: bool,
    collapsed_dirs: HashSet<String>,
    show_data_flow: bool,
    flow_graphs: HashMap<String, DataFlowGraph>,
    flow_layouts: HashMap<String, GraphLayout>,
    pub(crate) graph_view_state: GraphViewState,
}

impl ReviewApp {
    pub(crate) fn new(repo_path: Option<std::path::PathBuf>) -> Self {
        let mut app = Self {
            repo: load_repo_snapshot(repo_path),
            selected_file: 0,
            diff_scroll_handle: UniformListScrollHandle::new(),
            diff_focus_mode: false,
            collapsed_dirs: HashSet::new(),
            show_data_flow: false,
            flow_graphs: HashMap::new(),
            flow_layouts: HashMap::new(),
            graph_view_state: GraphViewState::new(),
        };
        app.ensure_selected_diff_loaded();
        app.ensure_flow_graph_loaded();
        app
    }

    fn set_selected_file(&mut self, index: usize, cx: &mut Context<Self>) {
        if index < self.repo.files.len() {
            self.selected_file = index;
            self.ensure_selected_diff_loaded();
            self.ensure_flow_graph_loaded();
            self.diff_scroll_handle
                .scroll_to_item_strict(0, ScrollStrategy::Top);
            self.graph_view_state.pan_offset = Point::new(px(0.0), px(0.0));
            cx.notify();
        }
    }

    fn toggle_diff_focus_mode(&mut self, cx: &mut Context<Self>) {
        self.diff_focus_mode = !self.diff_focus_mode;
        cx.notify();
    }

    fn toggle_data_flow(&mut self, cx: &mut Context<Self>) {
        self.show_data_flow = !self.show_data_flow;
        cx.notify();
    }

    fn toggle_directory(&mut self, path: String, cx: &mut Context<Self>) {
        if !self.collapsed_dirs.insert(path.clone()) {
            self.collapsed_dirs.remove(&path);
        }
        cx.notify();
    }

    fn ensure_selected_diff_loaded(&mut self) {
        let Some(file) = self.repo.files.get(self.selected_file) else {
            return;
        };

        if file.diff_lines.is_some() {
            return;
        }

        let path = file.path.clone();
        let status = file.status.clone();
        let untracked = file.untracked;
        let diff_lines = load_diff_lines_for_file(
            &self.repo.root,
            &self.repo.review_mode,
            &path,
            &status,
            untracked,
        )
        .unwrap_or_else(|error| vec![DiffLine {
            prefix: ">".to_string(),
            number: String::new(),
            content: error,
        }]);

        if let Some(file) = self.repo.files.get_mut(self.selected_file) {
            file.diff_lines = Some(diff_lines);
        }
    }

    fn ensure_flow_graph_loaded(&mut self) {
        let Some(file) = self.repo.files.get(self.selected_file) else {
            return;
        };

        let path = file.path.clone();
        if self.flow_graphs.contains_key(&path) {
            return;
        }

        if !path.ends_with(".rs") {
            self.flow_graphs.insert(path.clone(), DataFlowGraph::empty());
            self.flow_layouts.insert(path.clone(), GraphLayout {
                nodes: Vec::new(),
                edges: Vec::new(),
                labels: Vec::new(),
                total_width: 0.0,
                total_height: 0.0,
            });
            return;
        }

        let full_path = self.repo.root.join(&path);
        let source = match std::fs::read_to_string(&full_path) {
            Ok(s) => s,
            Err(_) => {
                self.flow_graphs.insert(path.clone(), DataFlowGraph::empty());
                self.flow_layouts.insert(path.clone(), GraphLayout {
                    nodes: Vec::new(),
                    edges: Vec::new(),
                    labels: Vec::new(),
                    total_width: 0.0,
                    total_height: 0.0,
                });
                return;
            }
        };

        let graph = build_flow_graph(source, full_path);
        let layout = layout_graph(&graph);

        self.flow_graphs.insert(path.clone(), graph);
        self.flow_layouts.insert(path, layout);
    }

    fn render_file_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let body = if self.repo.files.is_empty() {
            div()
                .id("file-list-scroll")
                .flex_1()
                .overflow_scroll()
                .child(render_empty_state(
                    "No changed files",
                    "The app reviews the working tree when the repo is dirty, or the latest commit when the repo is clean.",
                ))
        } else {
            let tree = build_file_tree(&self.repo.files);

            div()
                .id("file-list-scroll")
                .flex_1()
                .overflow_scroll()
                .flex()
                .flex_col()
                .children(
                    tree.into_iter().map(|node| self.render_file_tree_node(&node, 0, cx)),
                )
        };

        div()
            .w(px(320.0))
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded_lg()
            .border_1()
            .border_color(rgb(0x1e2a45))
            .bg(rgb(0x10182b))
            .child(render_panel_header(
                "Changed Files",
                &format!("{} files", self.repo.files.len()),
            ))
            .child(body)
    }

    fn render_file_tree_node(
        &self,
        node: &FileTreeNode,
        depth: usize,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let indent = 12.0 + depth as f32 * 16.0;

        if let Some(file_index) = node.file_index {
            let file = &self.repo.files[file_index];
            let selected = file_index == self.selected_file;
            let border = if selected { rgb(0x4f8cff) } else { rgb(0x10182b) };
            let bg = if selected { rgb(0x172544) } else { rgb(0x10182b) };

            div()
                .id(("file", file_index))
                .flex()
                .items_center()
                .justify_between()
                .gap_2()
                .px_3()
                .py_2()
                .pl(px(indent))
                .bg(bg)
                .border_l_2()
                .border_color(border)
                .cursor_pointer()
                .hover(|style| style.bg(rgb(0x162039)))
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.set_selected_file(file_index, cx);
                }))
                .child(
                    div()
                        .flex()
                        .justify_between()
                        .gap_2()
                        .child(
                            div()
                                .flex()
                                .gap_2()
                                .items_center()
                                .child(div().text_sm().child(node.name.clone())),
                        )
                        .child(render_badge(&file.status, status_color(&file.status))),
                )
                .into_any_element()
        } else {
            let is_collapsed = self.collapsed_dirs.contains(&node.full_path);
            let toggle_path = node.full_path.clone();
            let dir_id = directory_id(&node.full_path);

            div()
                .flex()
                .flex_col()
                .child(
                    div()
                        .id(("dir", dir_id))
                        .px_3()
                        .py_2()
                        .pl(px(indent))
                        .flex()
                        .gap_2()
                        .items_center()
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(0x121c31)))
                        .on_click(cx.listener(move |this, _, _, cx| {
                            this.toggle_directory(toggle_path.clone(), cx);
                        }))
                        .text_xs()
                        .text_color(rgb(0x6f86aa))
                        .font_weight(FontWeight::BOLD)
                        .child(if is_collapsed { "▸" } else { "▾" })
                        .child(node.name.clone()),
                )
                .children(
                    node.children
                        .iter()
                        .filter(|_| !is_collapsed)
                        .map(|child| self.render_file_tree_node(child, depth + 1, cx)),
                )
                .into_any_element()
        }
    }
}

impl Render for ReviewApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_file = self.repo.files.get(self.selected_file);
        let left_pane = if self.diff_focus_mode {
            None
        } else {
            Some(self.render_file_list(cx).into_any_element())
        };

        let current_flow_layout = selected_file
            .and_then(|f| self.flow_layouts.get(&f.path));

        let right_pane: AnyElement = if self.show_data_flow {
            render_graph_view(
                &self.graph_view_state,
                current_flow_layout,
                cx.listener(|this, _, _, cx| {
                    this.toggle_data_flow(cx);
                }),
                cx,
            )
            .into_any_element()
        } else {
            render_diff_view(
                &self.repo,
                selected_file,
                self.diff_scroll_handle.clone(),
                self.diff_focus_mode,
                cx.listener(|this, _, _, cx| {
                    this.toggle_diff_focus_mode(cx);
                }),
            )
            .into_any_element()
        };

        div()
            .size_full()
            .bg(rgb(0x0b1020))
            .text_color(rgb(0xe5eefc))
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .p_3()
                    .child(
                        div()
                            .flex()
                            .gap_2()
                            .child(
                                div()
                                    .id("btn-data-flow")
                                    .rounded_full()
                                    .border_1()
                                    .border_color(if self.show_data_flow { rgb(0x4f8cff) } else { rgb(0x22304f) })
                                    .bg(if self.show_data_flow { rgb(0x172544) } else { rgb(0x10182b) })
                                    .px_3()
                                    .py_1()
                                    .text_xs()
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(0x162039)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.toggle_data_flow(cx);
                                    }))
                                    .child("Data Flow"),
                            )
                            .child(
                                div()
                                    .id("btn-diff-view")
                                    .rounded_full()
                                    .border_1()
                                    .border_color(if !self.show_data_flow { rgb(0x4f8cff) } else { rgb(0x22304f) })
                                    .bg(if !self.show_data_flow { rgb(0x172544) } else { rgb(0x10182b) })
                                    .px_3()
                                    .py_1()
                                    .text_xs()
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(0x162039)))
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.show_data_flow = false;
                                        cx.notify();
                                    }))
                                    .child("Diff"),
                            ),
                    )
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .gap_3()
                            .overflow_hidden()
                            .children(left_pane)
                            .child(right_pane),
                    ),
            )
    }
}
