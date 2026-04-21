use gpui::{
    App, Application, Bounds, Context, FontWeight, Hsla, Window, WindowBounds, WindowOptions,
    div, prelude::*, px, rgb, size,
};

#[derive(Clone, Copy)]
struct FileChange {
    path: &'static str,
    status: &'static str,
    additions: usize,
    deletions: usize,
}

#[derive(Clone, Copy)]
struct Finding {
    severity: &'static str,
    title: &'static str,
    summary: &'static str,
    line: &'static str,
}

#[derive(Clone, Copy)]
struct DiffLine {
    prefix: &'static str,
    number: &'static str,
    content: &'static str,
}

struct ReviewApp {
    selected_file: usize,
    selected_finding: usize,
    files: Vec<FileChange>,
    findings: Vec<Finding>,
}

impl ReviewApp {
    fn new() -> Self {
        Self {
            selected_file: 0,
            selected_finding: 0,
            files: vec![
                FileChange {
                    path: "src/review/session.rs",
                    status: "M",
                    additions: 28,
                    deletions: 7,
                },
                FileChange {
                    path: "src/review/prompts.rs",
                    status: "A",
                    additions: 74,
                    deletions: 0,
                },
                FileChange {
                    path: "src/git/diff_parser.rs",
                    status: "M",
                    additions: 19,
                    deletions: 11,
                },
                FileChange {
                    path: "src/ui/review_panel.rs",
                    status: "M",
                    additions: 46,
                    deletions: 13,
                },
            ],
            findings: vec![
                Finding {
                    severity: "high",
                    title: "Unbounded prompt context",
                    summary: "The prompt builder appends entire hunks without a token budget, which can silently exceed model limits on large reviews.",
                    line: "prompts.rs:48",
                },
                Finding {
                    severity: "medium",
                    title: "Session cache misses branch changes",
                    summary: "The cache key omits base and head refs, so results can be reused after the user switches comparison branches.",
                    line: "session.rs:91",
                },
                Finding {
                    severity: "low",
                    title: "Inline comment count is stale",
                    summary: "The sidebar still renders a hard-coded comment count instead of deriving it from the active review state.",
                    line: "review_panel.rs:22",
                },
            ],
        }
    }

    fn set_selected_file(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected_file = index;
        self.selected_finding = 0;
        cx.notify();
    }

    fn set_selected_finding(&mut self, index: usize, cx: &mut Context<Self>) {
        self.selected_finding = index;
        cx.notify();
    }
}

impl Render for ReviewApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let selected_file = self.files[self.selected_file];
        let selected_finding = self.findings[self.selected_finding];

        div()
            .size_full()
            .bg(rgb(0x0b1020))
            .text_color(rgb(0xe5eefc))
            .child(
                div()
                    .size_full()
                    .flex()
                    .flex_col()
                    .child(render_header())
                    .child(
                        div()
                            .flex_1()
                            .flex()
                            .gap_3()
                            .p_3()
                            .child(self.render_file_list(cx))
                            .child(render_diff_view(selected_file))
                            .child(self.render_findings_panel(selected_finding, cx)),
                    ),
            )
    }
}

impl ReviewApp {
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
                &format!("{} files", self.files.len()),
            ))
            .child(
                div()
                    .id("file-list-scroll")
                    .flex_1()
                    .overflow_scroll()
                    .flex()
                    .flex_col()
                    .children(self.files.iter().enumerate().map(|(index, file)| {
                        let selected = index == self.selected_file;
                        let border = if selected { rgb(0x4f8cff) } else { rgb(0x10182b) };
                        let bg = if selected { rgb(0x172544) } else { rgb(0x10182b) };
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
                                this.set_selected_file(index, cx);
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
                                            .child(file.path),
                                    )
                                    .child(render_badge(file.status, status_color(file.status))),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x91a3c0))
                                    .child(stats),
                            )
                    })),
            )
    }

    fn render_findings_panel(
        &self,
        selected_finding: Finding,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
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
                &format!("{} issues", self.findings.len()),
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
                    .child(
                        div()
                            .text_sm()
                            .child("Two correctness risks and one UI consistency issue detected in the current mock diff."),
                    ),
            )
            .child(
                div()
                    .id("findings-scroll")
                    .flex_1()
                    .overflow_scroll()
                    .p_3()
                    .flex()
                    .flex_col()
                    .gap_2()
                    .children(self.findings.iter().enumerate().map(|(index, finding)| {
                        let selected = index == self.selected_finding;
                        let bg = if selected { rgb(0x172544) } else { rgb(0x121c31) };
                        let border = if selected { rgb(0x4f8cff) } else { rgb(0x22304f) };

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
                                this.set_selected_finding(index, cx);
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
                                            .child(finding.title),
                                    )
                                    .child(render_badge(
                                        finding.severity,
                                        severity_color(finding.severity),
                                    )),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(rgb(0x91a3c0))
                                    .child(finding.line),
                            )
                            .child(div().text_sm().child(finding.summary))
                    })),
            )
            .child(
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
                            .child(render_badge(
                                selected_finding.severity,
                                severity_color(selected_finding.severity),
                            ))
                            .child(
                                div()
                                    .text_sm()
                                    .font_weight(FontWeight::BOLD)
                                    .child(selected_finding.title),
                            ),
                    )
                    .child(div().text_sm().child(selected_finding.summary))
                    .child(
                        div()
                            .rounded_md()
                            .bg(rgb(0x0d1422))
                            .border_1()
                            .border_color(rgb(0x22304f))
                            .p_3()
                            .text_sm()
                            .text_color(rgb(0xb7c6df))
                            .child(format!(
                                "Suggested next step: bind {} to a real git diff parser and send only the active hunk plus nearby context to the model.",
                                selected_finding.line
                            )),
                    ),
            )
    }
}

fn render_header() -> impl IntoElement {
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
                        .child("AI-assisted review shell built with gpui"),
                ),
        )
        .child(
            div()
                .flex()
                .gap_2()
                .child(render_pill("repo", "zed-industries/zed"))
                .child(render_pill("review", "main...feature/ai-review"))
                .child(render_pill("model", "mock:gpt-reviewer")),
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

fn render_diff_view(selected_file: FileChange) -> impl IntoElement {
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
        .child(render_panel_header("Diff", selected_file.path))
        .child(
            div()
                .px_4()
                .py_3()
                .border_b_1()
                .border_color(rgb(0x1e2a45))
                .child(
                    div()
                        .text_sm()
                        .text_color(rgb(0x91a3c0))
                        .child(format!(
                            "Mock review context for {} ({} additions, {} deletions)",
                            selected_file.status, selected_file.additions, selected_file.deletions
                        )),
                ),
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
                .children(mock_diff_lines().into_iter().map(render_diff_line)),
        )
}

fn render_diff_line(line: DiffLine) -> impl IntoElement {
    let color = match line.prefix {
        "+" => rgb(0x7ee787),
        "-" => rgb(0xff7b72),
        "@@" => rgb(0x79c0ff),
        _ => rgb(0xe5eefc),
    };

    let background = match line.prefix {
        "+" => rgb(0x12261a),
        "-" => rgb(0x2a1417),
        "@@" => rgb(0x0f2238),
        _ => rgb(0x0f1728),
    };

    div()
        .flex()
        .gap_3()
        .items_start()
        .rounded_sm()
        .bg(background)
        .px_2()
        .py_1()
        .child(div().w(px(22.0)).text_color(color).child(line.prefix))
        .child(
            div()
                .w(px(48.0))
                .text_color(rgb(0x6f86aa))
                .child(line.number),
        )
        .child(div().text_color(color).child(line.content))
}

fn mock_diff_lines() -> Vec<DiffLine> {
    vec![
        DiffLine {
            prefix: "@@",
            number: "12",
            content: "fn build_review_prompt(session: &ReviewSession) -> String {",
        },
        DiffLine {
            prefix: " ",
            number: "13",
            content: "    let mut prompt = String::from(SYSTEM_PROMPT);",
        },
        DiffLine {
            prefix: "-",
            number: "14",
            content: "    prompt.push_str(&session.diff);",
        },
        DiffLine {
            prefix: "+",
            number: "14",
            content: "    for hunk in &session.selected_hunks {",
        },
        DiffLine {
            prefix: "+",
            number: "15",
            content: "        prompt.push_str(&hunk.header);",
        },
        DiffLine {
            prefix: "+",
            number: "16",
            content: "        prompt.push_str(&hunk.diff);",
        },
        DiffLine {
            prefix: "+",
            number: "17",
            content: "    }",
        },
        DiffLine {
            prefix: " ",
            number: "18",
            content: "    prompt",
        },
        DiffLine {
            prefix: " ",
            number: "19",
            content: "}",
        },
        DiffLine {
            prefix: "@@",
            number: "48",
            content: "fn build_cache_key(repo: &Repo, session: &ReviewSession) -> String {",
        },
        DiffLine {
            prefix: " ",
            number: "49",
            content: "    format!(\"{}:{}\", repo.path.display(), session.commit_sha)",
        },
        DiffLine {
            prefix: "@@",
            number: "88",
            content: "fn render_sidebar(state: &ReviewState) -> SidebarSummary {",
        },
        DiffLine {
            prefix: "+",
            number: "89",
            content: "    let inline_comment_count = 3;",
        },
        DiffLine {
            prefix: " ",
            number: "90",
            content: "    SidebarSummary::new(state.files_changed)",
        },
    ]
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
            |_, cx| cx.new(|_| ReviewApp::new()),
        )
        .unwrap();

        cx.activate(true);
    });
}
