# Review Tool

Desktop AI-assisted code review prototype built with `gpui`.

# Project Goal

The goal of this project is to create a pretty good code review tool that helps engineers understand code faster. It must be performant to handle very large diffs, integrate github discussions, and LLMs to help with the code review process.

The primary function is faster understanding of the code being changed.

## Current state

- Minimal changed-files tree
- Real local git repo loading
- Real unified diff viewer from git
- Lazy diff loading for faster startup
- Virtualized diff rendering for large files
- Diff focus mode for expanded review

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

To review a specific local checkout, pass its path as the first argument:

```sh
cargo run -- /path/to/repo
```

You can also force the display backend:

```sh
cargo run -- --x11 /path/to/repo
cargo run -- --wayland /path/to/repo
```

From a graphical desktop session, the one-shot command is:

```sh
nix develop path:. -c cargo run
```

For the `intent` repo you mentioned:

```sh
nix develop path:. -c cargo run -- /home/trbiv/Projects/intent
```

If Wayland is unstable on your machine, force X11:

```sh
nix develop path:. -c cargo run -- --x11 /home/trbiv/Projects/intent
```

You can also set a default repo with `REVIEW_REPO`:

```sh
REVIEW_REPO=/path/to/repo nix develop path:. -c cargo run
```

This app needs a Wayland or X11 session. It will not open from a plain TTY without `DISPLAY` or `WAYLAND_DISPLAY`.

Backend flags work by preferring one Linux display backend at startup:

1. `--x11` clears `WAYLAND_DISPLAY` before `gpui` initializes.
2. `--wayland` clears `DISPLAY` before `gpui` initializes.
3. If neither flag is passed, `gpui` keeps its default preference order.

## Build Check

Validated with:

```sh
nix develop path:. -c cargo check
```

Local repo behavior:

1. If the repo has uncommitted changes, the app reviews the working tree against `HEAD`.
2. If the repo is clean, the app reviews the latest commit.
3. If the repo has no commits yet, the app shows the working tree only.
4. Startup loads file metadata and line counts first, then loads the selected file's full diff on demand.

## Next steps

1. Add explicit base/head selection instead of the current auto mode.
2. Add hunk selection and context budgeting.
3. Plug in a real model backend.
4. Persist review sessions and inline comments.
