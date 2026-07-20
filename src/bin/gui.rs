use gpui::{
    div, prelude::*, px, rgb, size, App, Application, Bounds, Context, FontWeight, Hsla, Window,
    WindowBounds, WindowOptions,
};
use review_tool::engine::{DiffLine, FileChange, Finding, ReviewAction, ReviewSession};

struct ReviewGui {
    session: ReviewSession,
}

impl ReviewGui {
    fn new() -> Self {
        Self {
            session: ReviewSession::load_current_worktree()
                .unwrap_or_else(|error| ReviewSession::from_error(error.to_string())),
        }
    }

    fn apply(&mut self, action: ReviewAction, cx: &mut Context<Self>) {
        self.session.apply(action);
        cx.notify();
    }
}

impl Render for ReviewGui {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .size_full()
            .bg(rgb(0x0b1020))
            .text_color(rgb(0xe5eefc))
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(render_header(&self.session))
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .gap_3()
                            .p_3()
                            .child(self.render_file_list(cx))
                            .child(render_diff_view(
                                self.session.selected_file(),
                                self.session.diff_lines(),
                                self.session.selected_diff_change_range(),
                            ))
                            .child(self.render_findings_panel(cx)),
                    ),
            )
    }
}

impl ReviewGui {
    fn render_file_list(&self, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .w(px(300.0))
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
                &format!("{} files", self.session.files().len()),
            ))
            .child(if self.session.files().is_empty() {
                div()
                    .id("file-list-scroll")
                    .flex_1()
                    .p_4()
                    .child(render_empty_state(
                        "No changed files",
                        self.session.review_summary(),
                    ))
            } else {
                div()
                    .id("file-list-scroll")
                    .flex_1()
                    .overflow_scroll()
                    .flex()
                    .flex_col()
                    .children(
                        self.session
                            .files()
                            .iter()
                            .enumerate()
                            .map(|(index, file)| {
                                let selected = index == self.session.selected_file_index();
                                let border = if selected {
                                    rgb(0x4f8cff)
                                } else {
                                    rgb(0x10182b)
                                };
                                let bg = if selected {
                                    rgb(0x172544)
                                } else {
                                    rgb(0x10182b)
                                };
                                let path = file.path.clone();
                                let status = file.status.clone();
                                let stats = format!("+{} -{}", file.additions, file.deletions);

                                div()
                                    .id(index)
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .p_3()
                                    .bg(bg)
                                    .border_l_2()
                                    .border_color(border)
                                    .cursor_pointer()
                                    .hover(|style| style.bg(rgb(0x162039)))
                                    .on_click(cx.listener(move |this, _, _, cx| {
                                        this.apply(ReviewAction::SelectFile(index), cx);
                                    }))
                                    .child(
                                        div()
                                            .flex()
                                            .justify_between()
                                            .gap_2()
                                            .child(
                                                div()
                                                    .text_sm()
                                                    .font_weight(FontWeight::BOLD)
                                                    .child(path),
                                            )
                                            .child(render_badge(&status, status_color(&status))),
                                    )
                                    .child(div().text_xs().text_color(rgb(0x91a3c0)).child(stats))
                            }),
                    )
            })
    }

    fn render_findings_panel(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_finding = self.session.selected_finding();

        div()
            .w(px(360.0))
            .h_full()
            .flex()
            .flex_col()
            .overflow_hidden()
            .rounded_lg()
            .border_1()
            .border_color(rgb(0x1e2a45))
            .bg(rgb(0x10182b))
            .child(render_panel_header(
                "AI Findings",
                &format!("{} issues", self.session.findings().len()),
            ))
            .child(
                div()
                    .border_b_1()
                    .border_color(rgb(0x1e2a45))
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .child(
                        div()
                            .text_xs()
                            .text_color(rgb(0x91a3c0))
                            .child("Review summary"),
                    )
                    .child(div().text_sm().child(self.session.review_summary().to_owned())),
            )
            .child(if self.session.findings().is_empty() {
                div()
                    .id("findings-scroll")
                    .flex_1()
                    .p_3()
                    .child(render_empty_state(
                        "No AI findings yet",
                        "The review engine is showing real Git changes. An AI findings backend is not wired in yet.",
                    ))
            } else {
                div()
                    .id("findings-scroll")
                    .flex_1()
                    .overflow_scroll()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(self.session.findings().iter().enumerate().map(
                        |(index, finding)| {
                            let selected = index == self.session.selected_finding_index();
                            let bg = if selected {
                                rgb(0x172544)
                            } else {
                                rgb(0x121c31)
                            };
                            let border = if selected {
                                rgb(0x4f8cff)
                            } else {
                                rgb(0x22304f)
                            };
                            let title = finding.title.clone();
                            let severity = finding.severity.clone();
                            let line = finding.line.clone();
                            let summary = finding.summary.clone();

                            div()
                                .id(("finding", index))
                                .rounded_md()
                                .border_1()
                                .border_color(border)
                                .bg(bg)
                                .p_3()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .cursor_pointer()
                                .hover(|style| style.bg(rgb(0x172544)))
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    this.apply(ReviewAction::SelectFinding(index), cx);
                                }))
                                .child(
                                    div()
                                        .flex()
                                        .justify_between()
                                        .gap_2()
                                        .child(
                                            div()
                                                .text_sm()
                                                .font_weight(FontWeight::BOLD)
                                                .child(title),
                                        )
                                        .child(render_badge(&severity, severity_color(&severity))),
                                )
                                .child(div().text_xs().text_color(rgb(0x91a3c0)).child(line))
                                .child(div().text_sm().child(summary))
                        },
                    ))
            })
            .child(render_selected_finding(selected_finding))
    }
}

fn render_header(session: &ReviewSession) -> impl IntoElement {
    div()
        .flex()
        .justify_between()
        .items_center()
        .px_4()
        .py_3()
        .border_b_1()
        .border_color(rgb(0x1e2a45))
        .child(
            div()
                .flex()
                .flex_col()
                .gap_1()
                .child(
                    div()
                        .text_xl()
                        .font_weight(FontWeight::BOLD)
                        .child("Review Tool"),
                )
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x91a3c0))
                        .child("AI-assisted review shell backed by the current Git working tree"),
                ),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(render_pill("repo", session.repo_label()))
                .child(render_pill("review", session.review_label()))
                .child(render_pill("ai", session.backend_label())),
        )
}

fn render_empty_state(title: &str, body: &str) -> impl IntoElement {
    div()
        .rounded_md()
        .border_1()
        .border_color(rgb(0x22304f))
        .bg(rgb(0x0d1422))
        .p_3()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_sm()
                .font_weight(FontWeight::BOLD)
                .child(title.to_owned()),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x91a3c0))
                .child(body.to_owned()),
        )
}

fn render_panel_header(title: &str, subtitle: &str) -> impl IntoElement {
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

fn render_pill(label: &str, value: &str) -> impl IntoElement {
    div()
        .rounded_full()
        .border_1()
        .border_color(rgb(0x22304f))
        .bg(rgb(0x10182b))
        .px_3()
        .py_1()
        .text_xs()
        .child(format!("{}: {}", label, value))
}

fn render_badge(label: &str, color: Hsla) -> impl IntoElement {
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

fn render_diff_view(
    selected_file: Option<&FileChange>,
    diff_lines: &[DiffLine],
    selected_diff_change_range: Option<std::ops::Range<usize>>,
) -> impl IntoElement {
    let subtitle = selected_file
        .map(|file| file.path.as_str())
        .unwrap_or("No file selected");
    let context = selected_file
        .map(|file| {
            format!(
                "Git diff for {} ({} additions, {} deletions)",
                file.status, file.additions, file.deletions
            )
        })
        .unwrap_or_else(|| "No changed file is selected.".to_owned());

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
        .child(render_panel_header("Diff", subtitle))
        .child(
            div()
                .px_4()
                .py_3()
                .border_b_1()
                .border_color(rgb(0x1e2a45))
                .child(div().text_sm().text_color(rgb(0x91a3c0)).child(context)),
        )
        .child(
            div()
                .id("diff-scroll")
                .flex_1()
                .overflow_scroll()
                .text_sm()
                .p_4()
                .flex()
                .flex_col()
                .gap_1()
                .children(diff_lines.iter().enumerate().map(|(index, line)| {
                    render_diff_line(
                        line,
                        selected_diff_change_range
                            .as_ref()
                            .is_some_and(|range| range.contains(&index)),
                    )
                })),
        )
}

fn render_diff_line(line: &DiffLine, selected: bool) -> impl IntoElement {
    let color = match line.prefix.as_str() {
        "+" => rgb(0x7ee787),
        "-" => rgb(0xff7b72),
        "@@" => rgb(0x79c0ff),
        _ => rgb(0xe5eefc),
    };

    let background = if selected {
        rgb(0x172544)
    } else {
        match line.prefix.as_str() {
            "+" => rgb(0x12261a),
            "-" => rgb(0x2a1417),
            "@@" => rgb(0x0f2238),
            _ => rgb(0x0f1728),
        }
    };
    let border = if selected { rgb(0x4f8cff) } else { background };

    div()
        .flex()
        .gap_3()
        .items_start()
        .rounded_sm()
        .bg(background)
        .border_l_2()
        .border_color(border)
        .px_2()
        .py_1()
        .child(
            div()
                .w(px(22.0))
                .text_color(color)
                .child(line.prefix.clone()),
        )
        .child(
            div()
                .w(px(48.0))
                .text_color(rgb(0x6f86aa))
                .child(line.number.clone()),
        )
        .child(div().text_color(color).child(line.content.clone()))
}

fn render_selected_finding(selected_finding: Option<&Finding>) -> impl IntoElement {
    let title = selected_finding
        .map(|finding| finding.title.clone())
        .unwrap_or_else(|| "No finding selected".to_owned());
    let severity = selected_finding
        .map(|finding| finding.severity.clone())
        .unwrap_or_else(|| "none".to_owned());
    let summary = selected_finding
        .map(|finding| finding.summary.clone())
        .unwrap_or_else(|| "No AI findings yet. The real Git diff is loaded, but a review backend is not configured.".to_owned());
    let next_step = selected_finding
        .map(|finding| {
            format!(
                "Suggested next step: inspect {} and decide whether to comment on the active hunk.",
                finding.line
            )
        })
        .unwrap_or_else(|| {
            "Suggested next step: wire an AI review backend that consumes the selected file diff."
                .to_owned()
        });

    div()
        .border_t_1()
        .border_color(rgb(0x1e2a45))
        .p_3()
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .text_xs()
                .text_color(rgb(0x91a3c0))
                .child("Selected finding"),
        )
        .child(
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(render_badge(&severity, severity_color(&severity)))
                .child(div().text_sm().font_weight(FontWeight::BOLD).child(title)),
        )
        .child(div().text_sm().child(summary))
        .child(
            div()
                .rounded_md()
                .bg(rgb(0x0d1422))
                .border_1()
                .border_color(rgb(0x22304f))
                .p_3()
                .text_sm()
                .text_color(rgb(0xb7c6df))
                .child(next_step),
        )
}

fn status_color(status: &str) -> Hsla {
    match status {
        "A" => rgb(0x7ee787).into(),
        "M" => rgb(0x79c0ff).into(),
        "D" => rgb(0xff7b72).into(),
        _ => rgb(0xb7c6df).into(),
    }
}

fn severity_color(severity: &str) -> Hsla {
    match severity {
        "high" => rgb(0xff7b72).into(),
        "medium" => rgb(0xf2cc60).into(),
        "low" => rgb(0x79c0ff).into(),
        _ => rgb(0xb7c6df).into(),
    }
}

fn main() {
    Application::new().run(|cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1440.0), px(920.0)), cx);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            |_, cx| cx.new(|_| ReviewGui::new()),
        )
        .unwrap();

        cx.activate(true);
    });
}
