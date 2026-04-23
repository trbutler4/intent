use gpui::{FontWeight, Hsla, div, px, prelude::*, rgb};

pub(crate) fn render_panel_header(title: &str, subtitle: &str) -> impl IntoElement {
    div()
        .flex()
        .flex_col()
        .gap_1()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(rgb(0x1e2a45))
        .bg(rgb(0x0d1422))
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x91a3c0))
                .child(subtitle.to_owned()),
        )
}

pub(crate) fn render_badge(label: &str, color: Hsla) -> impl IntoElement {
    div()
        .rounded_full()
        .bg(color.opacity(0.18))
        .border_1()
        .border_color(color.opacity(0.5))
        .px_2()
        .py_0p5()
        .text_xs()
        .text_color(color)
        .child(label.to_uppercase())
}

pub(crate) fn render_empty_state(title: &str, body: &str) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .justify_center()
        .items_center()
        .gap_2()
        .p_6()
        .text_center()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child(title.to_owned()),
        )
        .child(
            div()
                .max_w(px(320.0))
                .text_sm()
                .text_color(rgb(0x91a3c0))
                .child(body.to_owned()),
        )
}

pub(crate) fn status_color(status: &str) -> Hsla {
    match status {
        "A" => rgb(0x7ee787).into(),
        "M" => rgb(0x79c0ff).into(),
        "D" => rgb(0xff7b72).into(),
        "R" => rgb(0xc297ff).into(),
        "U" => rgb(0xf2cc60).into(),
        _ => rgb(0xb7c6df).into(),
    }
}
