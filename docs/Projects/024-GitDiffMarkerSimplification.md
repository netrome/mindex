# Git Diff Marker Simplification

## Status
Accepted

## Goal
Remove ambient gutter diff markers from normal document reading while keeping
the lightweight git review tools that are useful after agent edits.

## Context
- `023-GitDiffAndRevert.md` added gutter markers to rendered document pages,
  plus "Show committed", per-file revert, and workspace reset.
- In practice, the markers add visual noise during normal reading.
- The main review workflow is intentional: inspect a page after an agent or
  another tool has changed it.
- Editor gutter markers, especially in Helix, already cover the active editing
  workflow well.

## Constraints
- No new dependencies.
- No new diff viewer in this task.
- Keep existing filesystem and git safety invariants unchanged.
- Preserve useful low-noise controls for changed files.

## Recommendation
Remove gutter diff markers from the normal document view and keep the existing
comparison/recovery actions:

- `Show committed` / `Show current`
- `Revert`
- `/git` for exact unified diff inspection

This restores a clean reading experience while preserving a simple way to
inspect and undo uncommitted changes.

## Proposed Design

- Stop rendering gutter marker annotations on `/d/<doc>`.
- Keep the dirty-file detection used by the document template so the existing
  git actions can still be shown when a file has uncommitted changes.
- Remove the unused rendered diff marker CSS if no remaining page uses those
  classes.
- Remove renderer tests that only cover gutter marker annotation behavior, if
  the renderer no longer supports diff annotations.
- Keep git diff parsing/domain tests if they are still needed by other git
  behavior; otherwise remove only the code made dead by this simplification.

## Non-goals
- Adding a file-specific diff page.
- Adding textarea/editor gutter markers to the browser edit page.
- Changing git commit, push, pull, reset, or restore behavior.
- Comparing already committed revisions or showing author-level history.

## ADR
No ADR needed. This narrows an existing UI feature and does not change the
architecture, security model, data model, or dependency set.

## Task Breakdown

### Task 1: Remove document-view gutter annotations
- Update the document view handler so `render_document_html` is called without
  diff marker data.
- If possible, simplify the renderer signature back to not accepting diff
  information.

Acceptance criteria:
- `/d/<doc>` does not render `diff-added`, `diff-modified`, or
  `diff-deleted-marker` markup for dirty files.
- Dirty files still show `Show committed` and `Revert` when applicable.
- Historical views such as `/d/<doc>?ref=HEAD` continue to work.

### Task 2: Remove dead marker UI code
- Remove unused diff marker CSS.
- Remove unused rendered-diff annotation helpers and tests if the renderer no
  longer exposes that behavior.

Acceptance criteria:
- No unused gutter marker styling remains.
- Tests cover the remaining git review behavior without retaining marker-only
  expectations.

### Task 3: Verify the simplified workflow
- Run `cargo fmt`.
- Run `cargo clippy --all-targets --all-features`.
- Run `cargo nextest run`.

Acceptance criteria:
- Formatting, clippy, and tests pass.
- `docs/Projects/TODO.md` is updated when the implementation is complete.
