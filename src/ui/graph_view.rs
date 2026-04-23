use gpui::{
    App, Bounds, ClickEvent, Context, Hsla, Point, Window,
    div, px, prelude::*, rgb,
};

use crate::analysis::{FlowNodeType, GraphLayout, LayoutEdge, LayoutNode};
use crate::ui::theme::render_empty_state;

const NODE_CORNER_RADIUS: f32 = 6.0;
const EDGE_WIDTH: f32 = 1.5;
const ARROW_SIZE: f32 = 6.0;

fn node_border_color(node_type: FlowNodeType) -> Hsla {
    match node_type {
        FlowNodeType::Function => rgb(0x79c0ff).into(),
        FlowNodeType::Variable => rgb(0x7ee787).into(),
        FlowNodeType::Input => rgb(0xf2cc60).into(),
        FlowNodeType::Output => rgb(0xc297ff).into(),
    }
}

fn node_bg_color(node_type: FlowNodeType) -> Hsla {
    match node_type {
        FlowNodeType::Function => rgb(0x0f2238).into(),
        FlowNodeType::Variable => rgb(0x12261a).into(),
        FlowNodeType::Input => rgb(0x2a2410).into(),
        FlowNodeType::Output => rgb(0x1a1030).into(),
    }
}

pub(crate) struct GraphViewState {
    pub(crate) layout: Option<GraphLayout>,
    pub(crate) pan_offset: Point<gpui::Pixels>,
    pub(crate) zoom: f32,
    pub(crate) hovered_node: Option<usize>,
}

impl GraphViewState {
    pub(crate) fn new() -> Self {
        Self {
            layout: None,
            pan_offset: Point::new(px(0.0), px(0.0)),
            zoom: 1.0,
            hovered_node: None,
        }
    }
}

pub(crate) fn render_graph_view(
    state: &GraphViewState,
    graph_layout: Option<&GraphLayout>,
    toggle_listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
    cx: &mut Context<crate::app::ReviewApp>,
) -> impl gpui::IntoElement {
    let body = if let Some(layout) = graph_layout {
        let layout_clone = layout.clone();
        let pan = state.pan_offset;
        let zoom = state.zoom;

        div()
            .id("graph-scroll")
            .flex_1()
            .overflow_scroll()
            .on_scroll_wheel(cx.listener(|this, ev: &gpui::ScrollWheelEvent, _, cx| {
                let delta = match &ev.delta {
                    gpui::ScrollDelta::Pixels(p) => *p,
                    gpui::ScrollDelta::Lines(l) => Point::new(px(l.x * 20.0), px(l.y * 20.0)),
                };
                this.graph_view_state.pan_offset += delta;
                cx.notify();
            }))
            .child(
                gpui::canvas(
                    move |_bounds, _window, _cx| {
                        layout_clone.clone()
                    },
                    move |bounds: Bounds<gpui::Pixels>, layout: GraphLayout, window: &mut Window, _cx: &mut App| {
                        let origin_x = bounds.origin.x + pan.x;
                        let origin_y = bounds.origin.y + pan.y;

                        for edge in &layout.edges {
                            draw_edge(window, edge, origin_x, origin_y, zoom);
                        }

                        for node in &layout.nodes {
                            draw_node(window, node, origin_x, origin_y, zoom);
                        }
                    },
                )
                .size_full(),
            )
    } else {
        div()
            .id("graph-scroll")
            .flex_1()
            .overflow_scroll()
            .child(render_empty_state(
                "No data flow available",
                "Select a Rust source file to visualize its data flow graph.",
            ))
    };

    div()
        .flex_1()
        .h_full()
        .flex()
        .flex_col()
        .overflow_hidden()
        .rounded_lg()
        .border_1()
        .border_color(rgb(0x1e2a45))
        .bg(rgb(0x0f1728))
        .child(
            div()
                .flex()
                .justify_between()
                .items_center()
                .px_4()
                .py_3()
                .border_b_1()
                .border_color(rgb(0x1e2a45))
                .bg(rgb(0x0d1422))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .text_sm()
                                .font_weight(gpui::FontWeight::BOLD)
                                .child("Data Flow"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x91a3c0))
                                .child("Per-file data flow graph"),
                        ),
                )
                .child(
                    div()
                        .id("toggle-data-flow")
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(0x22304f))
                        .bg(rgb(0x10182b))
                        .px_3()
                        .py_1()
                        .text_xs()
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(0x162039)))
                        .on_click(toggle_listener)
                        .child("Show Diff"),
                ),
        )
        .child(body)
}

fn draw_node(
    window: &mut Window,
    node: &LayoutNode,
    origin_x: gpui::Pixels,
    origin_y: gpui::Pixels,
    zoom: f32,
) {
    let x = origin_x + px(node.x * zoom);
    let y = origin_y + px(node.y * zoom);
    let w = px(node.width * zoom);
    let h = px(node.height * zoom);

    let node_type = FlowNodeType::Function;
    let bg = node_bg_color(node_type);
    let border = node_border_color(node_type);

    let bounds = Bounds::new(Point::new(x, y), gpui::size(w, h));
    let corner = gpui::Corners::all(px(NODE_CORNER_RADIUS * zoom));

    window.paint_quad(gpui::quad(
        bounds,
        corner,
        bg,
        gpui::Edges::all(px(1.0)),
        border,
        gpui::BorderStyle::Solid,
    ));
}

fn draw_edge(
    window: &mut Window,
    edge: &LayoutEdge,
    origin_x: gpui::Pixels,
    origin_y: gpui::Pixels,
    zoom: f32,
) {
    if edge.points.len() < 2 {
        return;
    }

    let color: Hsla = rgb(0x4f8cff).into();

    let mut builder = gpui::PathBuilder::stroke(px(EDGE_WIDTH * zoom));

    let first = &edge.points[0];
    builder.move_to(Point::new(
        origin_x + px(first.0 * zoom),
        origin_y + px(first.1 * zoom),
    ));

    for point in &edge.points[1..] {
        builder.line_to(Point::new(
            origin_x + px(point.0 * zoom),
            origin_y + px(point.1 * zoom),
        ));
    }

    if let Ok(path) = builder.build() {
        window.paint_path(path, color);
    }

    if edge.points.len() >= 2 {
        let last = &edge.points[edge.points.len() - 1];
        let prev = &edge.points[edge.points.len() - 2];
        let dx = last.0 - prev.0;
        let dy = last.1 - prev.1;
        let len = (dx * dx + dy * dy).sqrt().max(0.001);
        let ndx = dx / len;
        let ndy = dy / len;

        let tip_x = origin_x + px(last.0 * zoom);
        let tip_y = origin_y + px(last.1 * zoom);
        let s = ARROW_SIZE * zoom;

        let left = Point::new(
            tip_x - px(ndx * s) + px(ndy * s * 0.5),
            tip_y - px(ndy * s) - px(ndx * s * 0.5),
        );
        let right = Point::new(
            tip_x - px(ndx * s) - px(ndy * s * 0.5),
            tip_y - px(ndy * s) + px(ndx * s * 0.5),
        );
        let tip = Point::new(tip_x, tip_y);

        let mut arrow = gpui::PathBuilder::fill();
        arrow.move_to(tip);
        arrow.line_to(left);
        arrow.line_to(right);
        arrow.close();

        if let Ok(arrow_path) = arrow.build() {
            window.paint_path(arrow_path, color);
        }
    }
}
