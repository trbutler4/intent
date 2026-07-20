# Review Tool

AI-assisted code review prototype with a shared review engine and separate GPUI/TUI frontends.

## Current state

- Shared review engine for files, diff lines, findings, and selection actions
- Native 3-pane GPUI review shell
- Terminal 3-pane TUI review shell
- Real Git working-tree changed files
- Real unified diff viewer for staged, unstaged, and untracked text files
- Empty AI findings panel ready for a review backend
- Selection state for files and findings

## Nix Shell

The repo now includes a `flake.nix` with the Rust toolchain and Linux libraries needed by `gpui`.

Use:

```sh
nix develop path:.
```

If `flake.nix` is tracked by Git, plain `nix develop` also works.

## Run

Inside the dev shell, run the terminal UI:

```sh
cargo run
```

This is equivalent to:

```sh
cargo run --bin review-tui
```

Run the GPUI app with:

```sh
cargo run --bin review-gui
```

From a graphical desktop session, the one-shot command is:

```sh
nix develop path:. -c cargo run
```

The GPUI app needs a Wayland or X11 session. It will not open from a plain TTY without `DISPLAY` or `WAYLAND_DISPLAY`. The TUI runs directly in a terminal.

## TUI Controls

- `tab` switches panes
- In the file tree, `h`/left collapses directories
- In the file tree, `l`/right expands directories or opens files in the diff pane
- In the file tree, `enter` or `space` also opens a file in the diff pane
- `j`, `k`, up, down move within the focused pane
- `n`, `p`, `]`, `[` jump between changed blocks in the diff
- `q` or `esc` quits

## Build Check

Validated with:

```sh
nix develop path:. -c cargo check
```

## Next steps

1. Add hunk selection and context budgeting.
2. Plug in a real model backend.
3. Persist review sessions and inline comments.
