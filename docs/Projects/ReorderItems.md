# Reorder Items (Todo + General)

## Status
In progress

## Goal
Provide a convenient way to reorder TODO items within a document (including moving between separate lists), while also enabling a minimal, general-purpose “reorder blocks” feature for any markdown content.

## Context
Mindex already supports TODO checkboxes and quick-add for task lists, but there’s no way to rearrange items without editing source. The user request asks for reordering TODO items **in and across** lists, and would like the solution to be a general feature where possible.

Constraints to respect:
- No database; files remain the source of truth.
- Document ID is the normalized relative path.
- Edits overwrite full file contents (atomic writes).
- Avoid heavy dependencies or complex markdown ASTs.

## Current state
- TODO items are rendered as checkboxes via a simple line scanner (`render_task_list_markdown`).
- There is no reorder UI or API.

## Option space

### Option 1: Drag in the rendered markdown view (DOM→source mapping)
Annotate rendered HTML with source offsets and allow drag/drop directly in the normal document view.

Pros:
- Natural UX (no mode switch).
- Users reorder what they see.

Cons:
- Requires a robust source↔rendered mapping.
- Breaks easily with multi-line items, code fences, tables, etc.
- Would likely force a custom renderer or AST tracking.
- Risk of becoming non-minimal and hard to maintain.

Verdict: High complexity and fragile; not aligned with “minimal/hackable”.

### Option 2: Reorder mode (line-based)
A dedicated reorder page renders the document as a list of source lines. Dragging moves line ranges. Server splices lines and writes the file.

Pros:
- Very simple backend (splice by line range).
- Fully general (works for any markdown).
- Minimal UI complexity.

Cons:
- Easy to select a range that breaks markdown structures.
- The view is “source-like,” less user-friendly.

Verdict: Minimal and general, but risky for correctness when users select partial structures.

### Option 3: Reorder mode (block-based, lightweight scanner)
A dedicated reorder page renders **blocks** (list items, fenced code blocks, paragraphs, headings, tables). Dragging moves whole blocks. Server splices by block line ranges, validated against the current content.

Pros:
- Still general and file-backed.
- Much safer than line-based moves.
- Keeps implementation simple (line scanner, no full AST).

Cons:
- Heuristic block detection (not perfect markdown).
- Some edge cases where a block can’t be confidently detected.

Verdict: Best balance of safety and simplicity.

### Option 4: TODO-only reorder in the normal view
Add drag handles only to TODO list items and implement a TODO-specific reorder API.

Pros:
- Very focused UX for TODO lists.
- Minimal parsing: rely on TODO markers.

Cons:
- Not general.
- Still needs drag/drop mapping inside rendered HTML.

Verdict: Too narrow vs the requested “general feature”.

## Recommendation
Implement **Option 3**: a dedicated **Reorder mode** page with **block-based** dragging, plus an optional **line mode** fallback.

This satisfies the TODO list reorder request while keeping the system minimal and general:
- Reorder page avoids DOM↔source complexity.
- Block-based moves reduce markdown breakage.
- Line mode provides a universal escape hatch.
- No new dependencies; pure Rust line scanning.

## Proposed UX
- Add a “Reorder” action on document view and/or edit view.
- Route: `GET /reorder/{*path}`.
- Default view: block list with drag handles.
- If a block is a table, allow row-level reordering within that table (header + separator fixed).
- Optional toggle: “Line mode” (advanced).
- After drop: `POST /api/doc/reorder-range` and re-render the reorder page.

## Proposed API
`POST /api/doc/reorder-range` (form-encoded like existing endpoints)

Parameters:
- `doc_id` (string)
- `start_line` (0-based, inclusive)
- `end_line` (0-based, inclusive)
- `insert_before_line` (0-based, inclusive; equal to line count to append)
- `mode` = `block | line`
- Optional: `expected_line_count` or `expected_hash` for optimistic concurrency

Server behavior:
- Validate `doc_id` and resolve within root.
- Load file and split into line segments (preserve line endings).
- Validate line bounds.
- For `block` mode, verify the submitted range matches a current block boundary (reject with 409 if stale).
- Splice lines: remove [start..end], compute adjusted insertion index, insert before target.
- Atomic write and return 204 (or redirect back to reorder page).

## Block detection (lightweight scanner)
Implement a conservative line scanner that only emits blocks when it can do so confidently.

Blocks to support in v1:
- **Fenced code blocks**: from opening fence line to matching closing fence.
- **List items** (ordered/unordered, including TODO checkboxes): include continuation lines indented more than the item’s marker indent until the next list item at the same or lower indent.
- **Headings**: treat the heading line as its own block (optionally include following paragraph if desired later).
- **Paragraphs**: contiguous non-blank lines not part of the above.
- **Tables**: detect pipe tables with a header row + separator row; emit a table block and parse body rows for optional row-level reorder.

If a block cannot be confidently determined (rare), fall back to line mode.

## Non-goals
- Reordering across documents.
- WYSIWYG drag in the rendered markdown view.
- Rich multi-select or multi-block drag in v1.
- Reordering table columns.
- Markdown AST parsing or heavy dependencies.

## Risks and mitigations
- **Block detection edge cases**: use conservative heuristics; fall back to line mode.
- **Stale reorder page** (file changed while open): optional `expected_*` check to return 409 and prompt reload.
- **Markdown integrity**: block-based moves reduce (not eliminate) breakage; warn in UI if needed.

## ADR?
No ADR needed. This change does not alter architecture, security model, data model, or add a significant dependency.

## Task breakdown (PR-sized)

### Task 1: Reorder page (read-only UI)
- Add `GET /reorder/{*path}` handler and template rendering a block list.
- Status: Done.
- **Acceptance criteria**: A document can be opened in “Reorder” mode and shows line blocks with start/end line data.

### Task 2: Block scanner + reorder splice
- Implement a small block scanner and a `reorder_range` function.
- **Acceptance criteria**: Given a known markdown fixture, blocks match expected ranges; reordering produces correct content.

### Task 3: Reorder API endpoint
- Add `POST /api/doc/reorder-range` that validates and applies a reorder.
- **Acceptance criteria**: A valid reorder request updates the document on disk; invalid ranges return 400; stale block boundaries return 409.

### Task 4: Client drag/drop
- Add `assets/features/reorder.js` and minimal CSS for drag handles + drop markers.
- **Acceptance criteria**: Users can drag a block and drop it elsewhere; the document updates and the reorder page refreshes.

### Task 5: Line mode fallback
- Add a toggle to switch the reorder view into line mode (each line as a draggable block).
- **Acceptance criteria**: Line mode renders one line per row; dragging lines reorders correctly.

### Task 6: Table row reorder (within block mode)
- For detected tables, expose row-level drag handles for body rows (header + separator fixed).
- **Acceptance criteria**: Dragging a table row reorders only the body rows and preserves header/formatting.

### Task 7: Tests + docs
- Unit tests for block detection and reorder splice.
- Integration test for API endpoint.
- **Acceptance criteria**: Tests cover list-item moves, fenced code blocks, and table-row moves; docs mention the reorder feature and its limitations.
