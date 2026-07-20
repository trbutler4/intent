# AI Review Plan

## Goal

Add an AI review feature that can inspect the current review target, produce structured findings, and eventually help draft GitHub review comments.

## Principles

- Keep model/provider logic behind an engine abstraction.
- Keep AI findings local by default until the user explicitly posts them.
- Anchor findings to files, hunks, and line ranges instead of storing only free-form text.
- Cache outputs by diff hash, model, prompt version, and review target to avoid repeated work.
- Include existing GitHub comments in context so the AI avoids duplicate feedback.

## Prerequisite: Review Target

AI review should consume a normalized `ReviewTarget` from the shared engine.

The target should include:

- Review mode: working tree, latest commit, or GitHub PR.
- Repo root and remote metadata.
- Base SHA and head SHA when available.
- PR number and URL when available.
- Changed files and parsed hunks.
- Stable file diff hashes.

## Engine Model

Add an AI backend abstraction, roughly:

```rust
trait AiReviewBackend {
    fn label(&self) -> &str;
    fn review_file(&self, request: AiReviewRequest) -> Result<Vec<ReviewFinding>, ReviewError>;
}
```

`AiReviewRequest` should include:

- Review target metadata.
- Selected file path and diff hunks.
- Nearby code context when available.
- Existing GitHub comments for that file/hunk.
- Existing local AI findings for deduplication.

Expand the current `Finding` model into something like:

```rust
struct ReviewFinding {
    id: String,
    path: String,
    line_range: Option<LineRange>,
    severity: FindingSeverity,
    title: String,
    body: String,
    confidence: Option<f32>,
    source: FindingSource,
    status: FindingStatus,
}
```

Useful supporting enums:

- `FindingSource::Ai { provider, model, prompt_version }`
- `FindingStatus::Open | Dismissed | Accepted | Posted`
- `FindingSeverity::High | Medium | Low | Info`

## First Implementation Slice

1. Add structured `ReviewFinding` and anchors in the engine.
2. Add an `AiReviewBackend` trait and a disabled/no-op backend.
3. Add a TUI action like `a` to review the selected file.
4. Run the review synchronously at first only if the backend is fast/simple, otherwise add a background job state.
5. Store findings in `.intent/review-tool.sqlite3` keyed by diff hash and model metadata.
6. Render findings in the review pane and allow selecting/dismissing them.

## Provider Strategy

Start with one backend only.

Good first options:

- OpenAI-compatible HTTP endpoint for broad compatibility.
- Anthropic if that is the intended primary provider.
- Ollama if local/offline review is more important.

Avoid wiring multiple providers until the request/response schema is stable.

## Prompting Strategy

The AI prompt should request JSON output only.

Each finding should include:

- File path.
- Line or line range if applicable.
- Severity.
- Title.
- Explanation.
- Suggested fix if useful.
- Confidence.

The prompt should instruct the model to:

- Focus on correctness, security, data loss, regressions, and missing tests.
- Avoid style-only comments unless they affect maintainability materially.
- Avoid duplicating existing GitHub comments.
- Prefer fewer, higher-confidence findings.

## Background Jobs

Once whole-PR review is added, the TUI should not block.

Engine state should track:

- Idle.
- Reviewing selected file.
- Reviewing all files.
- Completed with findings.
- Failed with error.

The TUI can poll job state or receive messages through a channel.

## Posting Drafts Later

Do not auto-post AI output.

Later workflow:

1. AI creates local draft findings.
2. User selects a finding.
3. User accepts, edits, dismisses, or posts it.
4. Posting uses GitHub integration and marks the finding as `Posted`.

## Open Questions

1. Which provider should be implemented first: OpenAI-compatible, Anthropic, Ollama, or something else?
2. Should the first review action cover only the selected file or the whole PR/worktree?
3. Should findings be editable before posting?
4. Should AI review run automatically on load, or only on explicit user action?
5. What local config format should hold model settings and API endpoint data?
