# Review Tool — Feature Roadmap

Goal: Increase speed at which an engineer understands code during PR review.

## Completed

- [x] Modular codebase (git/, ui/, analysis/, app.rs, main.rs)
- [x] Per-file data flow DAG (tree-sitter AST → graph → Sugiyama layout → gpui canvas)
- [x] Data Flow / Diff view toggle
- [x] Proper AST-based flow extraction (let bindings, return values, call resolution, struct/enum/impl nodes)
- [x] Cross-platform flake (Linux + macOS)

---

## 1. Hunk Collapsing & Context Budget

**Problem:** Large diffs drown the reviewer in irrelevant context.

**Scope:** Diff view only.

- Collapsible hunk sections — each `@@ ... @@` block becomes a clickable region that expands/collapses
- Context budget slider (0–10) in the diff header — controls `--unified=N` arg passed to `git diff`/`git show`
- "Changes only" preset — context=0, strips all context lines
- State per file: track which hunks are collapsed in `ReviewApp`

**Files changed:** `git/diff.rs` (unified arg), `ui/diff_view.rs` (hunk UI), `app.rs` (state), `analysis/types.rs` (HunkMeta)

## 2. Semantic Diff Annotations

**Problem:** Red/green coloring tells you what lines changed, not what structures changed.

**Scope:** Diff view + tree-sitter.

- Parse both old and new file versions with tree-sitter
- Diff the ASTs at the function/struct/enum level
- Render annotation badges above each hunk: "parameter added", "return type changed", "new struct field", "removed match arm"
- Annotations are derived from comparing node kinds in the hunk region

**Files changed:** New `analysis/semantic.rs`, `ui/diff_view.rs` (badge rendering), `git/diff.rs` (expose old/new source)

## 3. Related Changes Navigation

**Problem:** "Where else does this matter?" requires mental context-switching.

**Scope:** File tree + flow graph.

- "Callers" panel: when viewing a function in the diff, show which other changed files call it (using flow graph edges scoped to the PR's changed files)
- Auto-detect test files: match `foo.rs` → `foo_test.rs` / `tests/foo.rs` patterns, show as linked pair
- "Jump to definition": click a function name in diff → jump to its definition line (even in another changed file)
- File tree badges: show link count (how many changed files reference this one)

**Files changed:** `analysis/flow.rs` (cross-file edge resolution), `ui/file_tree.rs` (link badges), `ui/diff_view.rs` (clickable identifiers), `app.rs` (navigation state)

## 4. Change Impact Heatmap

**Problem:** Reviewer doesn't know which files are high-risk to review first.

**Scope:** File tree sidebar.

- Count outgoing edges from each changed function in the flow graph (how many callers depend on it)
- Sum per file → heat score
- Color-code file tree entries: cool (blue, isolated changes) → hot (red, widely-used changes)
- Sort option: by heat score descending
- Tooltip: "3 functions changed, 12 callers affected"

**Files changed:** `analysis/flow.rs` (edge counting), `ui/file_tree.rs` (heat coloring + sort), `app.rs` (sort mode state)

## 5. Two-Column Before/After View

**Problem:** Unified diff is hard to parse for large rewrites and refactors.

**Scope:** New view mode alongside unified diff.

- Split pane: left = old file version, right = new file version
- Alignment by function/method boundaries (tree-sitter), not just line numbers
- Synced scrolling (linked scroll handles)
- Unchanged regions dimmed, changed regions highlighted
- Toggle: unified | side-by-side | data flow

**Files changed:** New `ui/side_by_side.rs`, `git/snapshot.rs` (load old file version via `git show HEAD:file`), `app.rs` (view mode enum)

## 6. Pattern Detection

**Problem:** Reviewers always look for the same risky patterns manually.

**Scope:** Diff view + tree-sitter queries.

- Lightweight AST checks on new/changed code:
  - New `unsafe` block
  - New `pub` item (API surface expansion)
  - `unwrap()` / `expect()` in non-test code
  - Mutex/lock introduction
  - Error handling: is the error propagated or swallowed?
  - TODO/FIXME/HACK comments
- Render as small inline flags next to the relevant line
- Clickable: expand to show a brief explanation
- Disable-able per-pattern in a settings panel

**Files changed:** New `analysis/patterns.rs` (tree-sitter queries), `ui/diff_view.rs` (flag rendering), `analysis/types.rs` (PatternFlag)

## 7. Commit Story / Change Timeline

**Problem:** Flattened diff hides the author's reasoning across multiple commits.

**Scope:** New panel for multi-commit PRs.

- Detect commit count in the review range
- Show commit list with messages as a timeline sidebar
- Click a commit → filter diff to only that commit's changes
- "All commits" = current flattened view
- Single-commit reviews: timeline hidden (no noise)

**Files changed:** `git/snapshot.rs` (commit list loading), New `ui/timeline.rs`, `app.rs` (selected commit state), `git/diff.rs` (per-commit diff loading)

## 8. Diff Minimap

**Problem:** No at-a-glance sense of the change's shape and distribution.

**Scope:** Right edge of diff view.

- Vertical bar (~8px wide) showing colored bands: green=additions, red=deletions, gray=context
- Proportional to total diff length
- Click a band → scroll to that section
- Current viewport shown as a semi-transparent highlight
- Rendered via `gpui::canvas` (paint_quad for each band)

**Files changed:** `ui/diff_view.rs` (minimap column), `app.rs` (viewport tracking)

---

## Execution Priority

| Order | Feature | Dependency |
|-------|---------|------------|
| 1 | Hunk collapsing & context budget | None — pure diff view improvement |
| 2 | Semantic diff annotations | Needs tree-sitter on both old/new versions |
| 3 | Pattern detection | Shares tree-sitter infra with #2 |
| 4 | Related changes navigation | Needs cross-file flow graph |
| 5 | Change impact heatmap | Needs edge counting from #4 |
| 6 | Two-column before/after | Needs old-file loading from git |
| 7 | Commit timeline | Needs commit-list git plumbing |
| 8 | Diff minimap | Pure UI, no analysis dependency |

Features 2+3 and 4+5 are natural pairs to build together.
