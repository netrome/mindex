# Git Diff Visualization and Revert

## Status
Design

## Goal
Help users navigate and manage uncommitted changes, especially those made by
sidecar AI agents. Provide at-a-glance diff awareness in the document view,
a way to preview the previous version, per-file revert, and workspace-level
reset.

## Context
- Git integration already supports status, commit, push, and pull via the
  `git` CLI (`src/git.rs`, `src/app/git.rs`).
- The `/git` page shows a full workspace diff and commit form, but there is no
  per-document change awareness.
- AI agents (magent) can modify documents, and the user currently has no way to
  see what changed or undo changes without leaving the app.
- Documents are rendered server-side via `pulldown-cmark` in
  `src/documents/rendering.rs`.

## Constraints
- No new dependencies (unified diff parsing is simple enough for stdlib).
- No in-memory git state across requests (stateless model).
- Git features degrade gracefully when git is not detected.
- Filesystem safety invariants remain unchanged.

## Features

### 1. Gutter diff markers (helix-style)

When viewing a document with uncommitted changes, show colored markers in the
left margin indicating which blocks were added, modified, or deleted.

**How it works:**
- On document render, if git is enabled and the file is tracked, run
  `git diff -- <file>` to get a unified diff.
- Parse the unified diff to extract changed line ranges in the working copy
  (added and modified lines) and deleted-line positions (line numbers where
  deletions occurred between current lines).
- `pulldown-cmark` events carry byte-offset source ranges via
  `Event::Start(Tag::*)`. Map these offsets back to line numbers in the
  markdown source to determine which rendered HTML blocks overlap with changed
  lines.
- Wrap affected blocks in a container (or add a CSS class) to style the gutter:
  - **Added lines**: green left border.
  - **Modified lines**: yellow/amber left border.
  - **Deleted lines**: a thin red horizontal marker between blocks (a
    `<div class="diff-deleted-marker">` inserted at the position where lines
    were removed).

**Rendering approach:**
- The diff annotation happens during `render_document_html`. The function
  receives an optional `DiffInfo` struct (None when git is disabled or file is
  untracked/clean).
- After the `pulldown-cmark` event loop produces the final HTML, a post-
  processing pass wraps top-level block elements that overlap changed line
  ranges with `<div class="diff-added">` or `<div class="diff-modified">` and
  inserts `<div class="diff-deleted-marker"></div>` at deletion points.
- CSS handles the visual presentation (left border via `border-left` or
  `::before` pseudo-element). No JavaScript needed.

**Why not character-level diffs?**
Character-level diffs within rendered markdown are complex (a line-level change
in source may affect multiple rendered elements) and fragile. Line-level gutter
markers are simple, non-intrusive, and match what editors like helix do.

### 2. Historical version view (arbitrary ref)

A way to view any committed version of a document, with a convenient toggle
button to flip between the current working-tree content and the last committed
version.

**How it works:**
- Add a query parameter: `/d/<doc_id>?ref=<ref>`.
- `<ref>` can be any valid git ref: `HEAD`, `HEAD~3`, a branch name, a tag, or
  a commit SHA.
- When `ref` is present and git is enabled, read the file content from
  `git show <ref>:<file>` instead of disk.
- Validate the ref with `git rev-parse --verify <ref>` before use. Return 404
  if the ref is invalid or the file does not exist at that ref.
- Render through the same `render_document_html` pipeline (no diff markers
  when viewing a historical version since there is nothing to diff against).
- The document template shows a toggle button ("Show committed" / "Show
  current") that switches between `?ref=HEAD` and the bare URL. Only visible
  when git is enabled and the file has uncommitted changes. The toggle always
  uses HEAD as the default ref since that is the primary use case.

**Why a query param instead of a separate route?**
Keeps the URL structure clean and avoids duplicating the document handler. The
template and handler already receive the `git_enabled` flag.

**Why support arbitrary refs?**
The underlying `git show <ref>:<file>` command works for any ref. Supporting
this is essentially free and enables useful workflows like reviewing what a
document looked like at a specific tag or commit.

### 3. Per-file revert

A button on modified documents to discard uncommitted changes and restore the
last committed version.

**How it works:**
- Add `POST /api/d/restore-file` endpoint.
  - Form parameter: `doc_id` (relative path).
  - Server: validate path (same safety checks as other file operations), then
    run `git checkout HEAD -- <file>`.
  - Returns `204` on success. Error codes: `400` (invalid path), `404` (file
    not in HEAD), `500` (git error).
- The document template shows a "Revert" button (only when the file has
  uncommitted changes). Clicking it triggers a `confirm()` dialog, then POSTs
  to the endpoint. On success, the page reloads to show the restored content.
- For new/untracked files (not in HEAD), revert is not offered.

**Safety:**
- This is destructive (discards working-tree changes). The `confirm()` dialog
  is required.
- The endpoint validates the path through existing path-safety functions.
- `git checkout HEAD -- <file>` only affects the single file.

### 4. Workspace reset

A "Discard all changes" action on the `/git` page to reset the entire working
tree to HEAD.

**How it works:**
- Add `POST /git/reset` endpoint.
  - Server: run `git checkout HEAD -- .` to restore all tracked files, then
    `git clean -fd` to remove untracked files.
  - Returns the updated git template with a success/error notice.
- The `/git` page shows a "Discard all changes" button (only when there are
  uncommitted changes). It uses a `confirm()` dialog that shows the number of
  affected files.

**Safety:**
- This is the most destructive operation. The confirmation message explicitly
  states that all uncommitted changes will be lost.
- Untracked files are also removed (`git clean -fd`). This is intentional for
  the "undo everything the agent did" use case, but the confirm dialog should
  mention this.

## Non-goals
- Side-by-side diff viewer (the `/git` page already shows raw unified diff).
- Per-hunk staging or partial revert (complexity explosion).
- Tracking git state in memory across requests.
- Character-level inline diffs in rendered markdown.
- Diffing against arbitrary commits (gutter markers always diff against HEAD).

## ADR
No ADR needed. This extends the existing git integration within the current
architecture. No new dependencies, no data model changes, no new security
boundaries. The `git checkout` and `git clean` commands run through the same
locked-down `git_command()` helper that disables hooks and restricts protocols.

## Task Breakdown

### Task 1: Domain diff parsing ✅
- Add a function to run `git diff -- <file>` for a single file and parse the
  unified diff output into a struct with added/modified/deleted line ranges.
- Add a function to check whether a file has uncommitted changes
  (`git status --porcelain -- <file>`).
- Add unit tests for diff parsing (various hunk patterns: pure additions, pure
  deletions, modifications, multiple hunks).

Acceptance criteria:
- Parsing a unified diff correctly identifies added, modified, and deleted line
  ranges.
- Clean files return an empty diff info.
- Untracked files are identified as fully new (all lines added).

### Task 2: Gutter markers in rendered HTML ✅
- Extend `render_document_html` to accept optional diff info.
- Map source line ranges to rendered HTML blocks and annotate with CSS classes.
- Insert deleted-line markers at appropriate positions.
- Add CSS for gutter markers (green/yellow left border, red deletion marker).
- Add tests for annotated HTML output.

Acceptance criteria:
- Added blocks get `class="diff-added"`.
- Modified blocks get `class="diff-modified"`.
- Deleted-line positions get a `<div class="diff-deleted-marker">`.
- No annotation when diff info is None (git disabled or clean file).
- Gutter markers are visible in both light and dark themes.

### Task 3: Historical version view ✅
- Add `git_show_file(root, ref, file_path)` domain function using
  `git show <ref>:<file>`.
- Validate the ref with `git rev-parse --verify <ref>`.
- Handle `ref` query parameter in the document handler.
- Add toggle button to document template (only shown for dirty files,
  defaults to `?ref=HEAD`).
- Diff markers are omitted when viewing a historical version.
- Add tests for HEAD, tags, SHAs, and invalid refs.

Acceptance criteria:
- `/d/notes.md?ref=HEAD` renders the last committed version.
- `/d/notes.md?ref=v1.0` renders the version at tag v1.0.
- `/d/notes.md?ref=abc123` renders the version at that commit.
- Toggle button switches between current and HEAD.
- Button is hidden when the file has no uncommitted changes.
- Returns 404 if the ref is invalid or file does not exist at that ref.

### Task 4: Per-file revert
- Add `git_restore_file(root, file_path)` domain function.
- Add `POST /api/d/restore-file` endpoint with path validation.
- Add revert button to document template (only for dirty tracked files).
- Add JS handler: `confirm()` then POST, reload on success.
- Add integration tests.

Acceptance criteria:
- Reverting restores the file to its HEAD content.
- Invalid/escaping paths are rejected (400).
- Files not in HEAD return 404.
- Confirm dialog is shown before reverting.

### Task 5: Workspace reset
- Add `git_reset_workspace(root)` domain function (checkout + clean).
- Add `POST /git/reset` endpoint.
- Add "Discard all changes" button to git template (only when dirty).
- Add JS handler: `confirm()` with file count, then POST.
- Add integration tests.

Acceptance criteria:
- Reset restores all tracked files and removes untracked files.
- Confirm dialog shows the number of affected files.
- Button is hidden when workspace is clean.
- Git page reloads with updated status after reset.
