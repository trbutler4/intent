# Review Tool

Desktop AI-assisted code review prototype built with `gpui`.

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
