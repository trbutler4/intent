# Review Tool

AI-assisted code review prototype with a shared review engine and separate GPUI/TUI frontends.

## Current state

- Shared review engine for files, diff lines, findings, and selection actions
- Native 3-pane GPUI review shell
- Terminal 3-pane TUI review shell
- Mock changed files list
- Mock unified diff viewer
- Mock AI findings panel
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

## Build Check

Validated with:

```sh
nix develop path:. -c cargo check
```

## Next steps

1. Replace mock data with `git diff` output.
2. Add hunk selection and context budgeting.
3. Plug in a real model backend.
4. Persist review sessions and inline comments.
