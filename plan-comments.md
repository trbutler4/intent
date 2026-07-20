# GitHub Comments Plan

## Goal

Import GitHub PR comments into Intent so reviewers can see existing discussion alongside the diff, then later support replies, resolving threads, and posting accepted AI findings.

## Principles

- Start read-only.
- Use `gh` CLI first for authentication and API access.
- Keep GitHub-specific data in the engine, not in TUI-only state.
- Normalize comments into file/hunk/line anchors that both TUI and GPUI can render.
- Cache fetched comments in `.intent/review-tool.sqlite3` for resilience and faster reloads.

## Prerequisite: Review Target

GitHub comments require a PR-aware `ReviewTarget`.

The target should include:

- Repo owner/name.
- Remote URL.
- Current branch.
- PR number.
- PR URL.
- Base branch and base SHA.
- Head branch and head SHA.

Initial PR detection can use:

```sh
gh pr view --json number,url,baseRefName,baseRefOid,headRefName,headRefOid
```

If no PR exists for the current branch, Intent should continue in local review mode and show that GitHub comments are unavailable.

## Comment Data Model

Add engine structs similar to:

```rust
struct ReviewComment {
    id: String,
    thread_id: Option<String>,
    author: String,
    body: String,
    path: Option<String>,
    anchor: Option<CommentAnchor>,
    url: String,
    created_at: String,
    updated_at: String,
    resolved: Option<bool>,
}

struct CommentAnchor {
    path: String,
    side: DiffSide,
    line: Option<u32>,
    start_line: Option<u32>,
    original_line: Option<u32>,
    commit_id: Option<String>,
    diff_hunk: Option<String>,
}
```

Useful enums:

- `DiffSide::Left | Right`
- `CommentSource::Github`

## API Strategy

Use `gh` first.

Fetch inline PR comments:

```sh
gh api repos/{owner}/{repo}/pulls/{pull_number}/comments --paginate
```

Fetch issue-level PR comments:

```sh
gh api repos/{owner}/{repo}/issues/{pull_number}/comments --paginate
```

For resolved/unresolved review thread state, use GraphQL later:

```graphql
repository(owner: $owner, name: $repo) {
  pullRequest(number: $number) {
    reviewThreads(first: 100) {
      nodes {
        id
        isResolved
        comments(first: 100) { ... }
      }
    }
  }
}
```

## Mapping Comments To Diffs

Map inline comments by:

- `path`
- `line`
- `start_line`
- `original_line`
- `side`
- `commit_id`
- `diff_hunk`

The engine should expose:

- All comments for current PR.
- Comments for selected file.
- Comments for selected diff line/hunk.

The TUI can render:

- A marker in the diff pane for lines with comments.
- A comments section in the review pane.
- Current selected-line comments first.

## Cache Strategy

Persist GitHub comments in `.intent/review-tool.sqlite3`.

Suggested cache key fields:

- Provider: `github`.
- Owner/repo.
- PR number.
- Comment ID.
- Head SHA.
- Updated timestamp.

Cache should be replace-on-fetch for a PR. If fetch fails, Intent can show cached comments with a stale warning.

## First Implementation Slice

1. Add PR metadata detection through `gh pr view`.
2. Add `ReviewComment` and `CommentAnchor` to the engine.
3. Add a `GithubClient` wrapper that shells out to `gh api`.
4. Fetch inline comments read-only.
5. Store comments in engine state.
6. Render per-file comments in the review pane.
7. Add diff-line comment markers after line anchoring is reliable.

## Later Posting Workflow

Posting should be explicit.

Possible actions:

- Reply to a thread.
- Add a new inline PR comment.
- Resolve/unresolve a thread.
- Post an accepted AI finding as a GitHub comment.

For posting new inline comments, we need the correct commit ID, path, side, and line. This is why the anchor model should be built before write operations.

## Open Questions

1. Should Intent require `gh` for GitHub integration, or support token-based direct API calls from the start?
2. Should issue-level PR comments appear with inline comments or in a separate discussion section?
3. Should resolved threads be hidden by default?
4. Should comment caching survive branch changes, or be scoped strictly by PR/head SHA?
5. How soon should posting/replying be supported versus read-only display?
