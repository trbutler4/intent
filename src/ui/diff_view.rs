use gpui::{
    AnyElement, ClickEvent, App, FontWeight, Hsla, ListHorizontalSizingBehavior,
    UniformListScrollHandle, Window, div, px, prelude::*, rgb, uniform_list,
};

use crate::git::{DiffLine, FileChange, RepoSnapshot};
use super::theme::render_empty_state;

pub(crate) fn render_diff_view(
    repo: &RepoSnapshot,
    selected_file: Option<&FileChange>,
    diff_scroll_handle: UniformListScrollHandle,
    diff_focus_mode: bool,
    toggle_focus_listener: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> impl IntoElement {
    let subtitle = selected_file
        .map(|file| file.path.as_str())
        .unwrap_or("No file selected");

    let summary = if let Some(error) = &repo.load_error {
        error.clone()
    } else if let Some(file) = selected_file {
        format!("{}  +{}  -{}", file.status, file.additions, file.deletions)
    } else {
        "No diff selected".to_string()
    };

    let diff_body: AnyElement = if let Some(file) = selected_file {
        if file.diff_lines.as_ref().is_none_or(|lines| lines.is_empty()) {
            div()
                .id("diff-scroll")
                .flex_1()
                .overflow_scroll()
                .child(render_empty_state(
                    "No textual diff available",
                    "This file may be binary or only have metadata changes.",
                ))
                .into_any_element()
        } else {
            let diff_lines = file.diff_lines.clone().unwrap_or_default();
            let item_count = diff_lines.len();

            uniform_list(
                "diff-scroll",
                item_count,
                move |range, _window, _cx| {
                    range
                        .map(|index| render_diff_line(diff_lines[index].clone()))
                        .collect::<Vec<_>>()
                },
            )
            .flex_1()
            .text_sm()
            .track_scroll(diff_scroll_handle)
            .with_width_from_item(Some(0))
            .with_horizontal_sizing_behavior(ListHorizontalSizingBehavior::Unconstrained)
            .into_any_element()
        }
    } else if let Some(error) = &repo.load_error {
        div()
            .id("diff-scroll")
            .flex_1()
            .overflow_scroll()
            .child(render_empty_state("Unable to load repository", error))
            .into_any_element()
    } else {
        div()
            .id("diff-scroll")
            .flex_1()
            .overflow_scroll()
            .child(render_empty_state(
                "No diff selected",
                "Pick a file from the left panel to inspect the real git diff.",
            ))
            .into_any_element()
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
                                .font_weight(FontWeight::BOLD)
                                .child("Diff"),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(rgb(0x91a3c0))
                                .child(subtitle.to_owned()),
                        ),
                )
                .child(
                    div()
                        .id("toggle-diff-focus")
                        .rounded_full()
                        .border_1()
                        .border_color(rgb(0x22304f))
                        .bg(rgb(0x10182b))
                        .px_3()
                        .py_1()
                        .text_xs()
                        .cursor_pointer()
                        .hover(|style| style.bg(rgb(0x162039)))
                        .on_click(toggle_focus_listener)
                        .child(if diff_focus_mode {
                            "Restore Layout"
                        } else {
                            "Focus Diff"
                        }),
                ),
        )
        .child(
            div()
                .px_4()
                .py_3()
                .border_b_1()
                .border_color(rgb(0x1e2a45))
                .child(div().text_sm().text_color(rgb(0x91a3c0)).child(summary)),
        )
        .child(diff_body)
}

fn render_diff_line(line: DiffLine) -> impl IntoElement {
    let (color, background): (Hsla, Hsla) = match line.prefix.as_str() {
        "+" => (rgb(0x7ee787).into(), rgb(0x12261a).into()),
        "-" => (rgb(0xff7b72).into(), rgb(0x2a1417).into()),
        "@@" => (rgb(0x79c0ff).into(), rgb(0x0f2238).into()),
        ">" => (rgb(0xb7c6df).into(), rgb(0x121c31).into()),
        _ => (rgb(0xe5eefc).into(), rgb(0x0f1728).into()),
    };

    div()
        .flex()
        .gap_3()
        .items_start()
        .min_w(px(960.0))
        .rounded_sm()
        .bg(background)
        .px_2()
        .py_1()
        .child(div().w(px(22.0)).text_color(color).child(line.prefix))
        .child(
            div()
                .w(px(72.0))
                .text_color(rgb(0x6f86aa))
                .child(line.number),
        )
        .child(div().text_color(color).child(line.content))
}
