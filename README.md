# Review Tool

AI-assisted code review prototype with a shared review engine and separate GPUI/TUI frontends.

## Project Goal

The goal of this project is to create a code review tool that helps engineers understand code faster. It should handle large diffs, integrate GitHub discussions, and use LLMs to help with the review process.

## Current State

- Shared review engine for files, diff lines, findings, selection actions, and reviewed-file state
- Terminal 3-pane TUI review shell, default for `cargo run`
- Nix flake package for installing/running the TUI as `intent`
- Native GPUI review shells
- Real Git working-tree changed files
- Real unified diff viewer for staged, unstaged, and untracked text files
- Expandable changed-file tree with collapsible review status sections
- SQLite-backed reviewed-file persistence
- Lazy GPUI diff loading and data-flow graph prototype from the GPUI path

## Nix Shell

The repo includes a `flake.nix` with the Rust toolchain and native libraries needed by `gpui`.

Use:

```sh
nix develop path:.
```

If `flake.nix` is tracked by Git, plain `nix develop` also works.

## Nix Package

Build the packaged TUI with:

```sh
nix build .#intent
```

Run it directly with:

```sh
nix run .#intent
```

The package installs both `intent` and `review-tui`; `intent` is the default app.

In a NixOS flake, add this repo as an input and include the package:

```nix
environment.systemPackages = [
  inputs.intent.packages.${pkgs.system}.default
];
```

## Run

Inside the dev shell, run the terminal UI:

```sh
cargo run
```

This is equivalent to:

```sh
cargo run --bin review-tui
```

Run the simpler GPUI shell with:

```sh
cargo run --bin review-gui
```

Run the GPUI app that accepts a repo path with:

```sh
cargo run --bin review-tool -- /path/to/repo
```

You can also force the display backend for that GPUI path:

```sh
cargo run --bin review-tool -- --x11 /path/to/repo
cargo run --bin review-tool -- --wayland /path/to/repo
```

From a graphical desktop session, the one-shot command is:

```sh
nix develop path:. -c cargo run
```

The GPUI apps need a Wayland or X11 session. They will not open from a plain TTY without `DISPLAY` or `WAYLAND_DISPLAY`. The TUI runs directly in a terminal.

Backend flags work by preferring one Linux display backend at startup:

1. `--x11` clears `WAYLAND_DISPLAY` before `gpui` initializes.
2. `--wayland` clears `DISPLAY` before `gpui` initializes.
3. If neither flag is passed, `gpui` keeps its default preference order.

## TUI Controls

- `tab` switches panes
- `f` hides/shows the files pane
- `R` hides/shows the review pane
- `1` shows the full changed-file tree
- `2` shows files split into `to review` and `reviewed` sections
- `e` opens the selected file tree file in `$EDITOR`; quitting the editor returns to Intent
- `r` toggles the selected file reviewed/unreviewed
- In the file tree, `h`/left collapses sections or directories
- In the file tree, `l`/right expands sections/directories or opens files in the diff pane
- In the file tree, `enter` or `space` toggles sections/directories or opens files in the diff pane
- Drag the vertical pane separators with the mouse to resize panes
- `j`, `k`, up, down move within the focused pane
- `n`, `p`, `]`, `[` jump between changed blocks in the diff
- `q` or `esc` quits

## Persistence

Reviewed-file state is persisted in SQLite at `.intent/review-tool.sqlite3` inside the repo. Entries are keyed by branch/review, file path, and the current file diff hash, so a file becomes unreviewed again when its diff changes. The `.intent/` directory is ignored by Git and can be deleted with the repo or removed to reset local review state.

## Build Check

Validated with:

```sh
nix develop path:. -c cargo check
nix build .#intent
```

Local repo behavior:

1. If the repo has uncommitted changes, the app reviews the working tree against `HEAD`.
2. If the repo is clean, the app reviews the latest commit.
3. If the repo has no commits yet, the app shows the working tree only.
4. Startup loads file metadata and line counts first, then loads the selected file's full diff on demand in the GPUI repo-path app.

## Next Steps

1. Add explicit base/head selection instead of the current auto mode.
2. Add hunk selection and context budgeting.
3. Plug in a real model backend.
4. Persist inline comments.
