mod app;
mod git;
mod ui;

use std::env;
use std::path::PathBuf;

use gpui::{App, AppContext, Application, Bounds, WindowBounds, WindowOptions, size, px};

use app::ReviewApp;

#[derive(Clone, Copy)]
enum DisplayBackend {
    X11,
    Wayland,
}

fn requested_repo_path_and_backend() -> (Option<PathBuf>, Option<DisplayBackend>) {
    let mut repo_path = None;
    let mut backend = None;

    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--x11" => backend = Some(DisplayBackend::X11),
            "--wayland" => backend = Some(DisplayBackend::Wayland),
            _ if arg.starts_with("--") => {}
            _ if repo_path.is_none() => repo_path = Some(PathBuf::from(arg)),
            _ => {}
        }
    }

    (repo_path, backend)
}

fn apply_backend_override(backend: Option<DisplayBackend>) {
    match backend {
        Some(DisplayBackend::X11) => {
            unsafe {
                env::remove_var("WAYLAND_DISPLAY");
            }
        }
        Some(DisplayBackend::Wayland) => {
            unsafe {
                env::remove_var("DISPLAY");
            }
        }
        None => {}
    }
}

fn main() {
    let (repo_path, backend) = requested_repo_path_and_backend();
    apply_backend_override(backend);

    Application::new().run(move |cx: &mut App| {
        let bounds = Bounds::centered(None, size(px(1440.0), px(920.0)), cx);
        let requested_path = repo_path.clone();

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(bounds)),
                ..Default::default()
            },
            move |_, cx| {
                let requested_path = requested_path.clone();
                cx.new(|_| ReviewApp::new(requested_path))
            },
        )
        .unwrap();

        cx.activate(true);
    });
}
