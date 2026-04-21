# Review Tool

Desktop AI-assisted code review prototype built with `gpui`.

# Project Goal

The goal of this project is to create a pretty good code review tool that helps engineers understand code faster. It must be performant to handle very large diffs, integrate github discussions, and LLMs to help with the code review process.

The primary function is faster understanding of the code being changed.

## Current state

- Native 3-pane review shell
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

Inside the dev shell, run:

```sh
cargo run
```

From a graphical desktop session, the one-shot command is:

```sh
nix develop path:. -c cargo run
```

This app needs a Wayland or X11 session. It will not open from a plain TTY without `DISPLAY` or `WAYLAND_DISPLAY`.

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
