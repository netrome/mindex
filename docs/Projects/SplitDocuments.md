# Split `documents.rs` into submodules

## Status
Design

## Goal
Break `src/documents.rs` (2,301 LOC) into focused submodules to improve
readability, reviewability, and testability — without changing the public API.

## Context
`documents.rs` is by far the largest file in the codebase. It mixes five
distinct concerns that have different change cadences and cognitive loads:

| Concern                  | ~LOC | Examples                                     |
|--------------------------|------|----------------------------------------------|
| Path resolution & I/O    | 280  | `resolve_doc_path`, `list_directory`         |
| Markdown rendering       | 750  | `render_document_html`, link rewriting       |
| Block editing/reordering | 450  | `scan_block_ranges`, `reorder_range`         |
| Task list manipulation   | 280  | `toggle_task_item`, `add_task_item_in_list`  |
| Search & mentions        | 190  | `search_documents`, `collect_mentions`       |

(Remaining ~350 lines are shared types, errors, and utilities. Tests add
another ~914 lines.)

The rendering logic requires understanding pulldown-cmark's event model.
The block editing code has its own line-segment abstraction. Search is
self-contained. These shouldn't live in the same file.

The rest of the codebase is well-structured — no other module comes close
in size or mixed concerns. `app.rs` is the second-largest at 1,441 LOC,
but it's already split into `app/*.rs` submodules and the parent primarily
handles routing and middleware. No other refactoring targets offer comparable
impact.

## Proposed structure

Follow the existing convention (`push.rs` + `push/*.rs`, not `mod.rs`):

```
src/documents.rs          parent module (~350 LOC)
src/documents/paths.rs    path resolution, directory listing, file kind
src/documents/rendering.rs   markdown → HTML, link rewriting, heading IDs
src/documents/editing.rs     block ranges, reordering
src/documents/tasks.rs       task list toggle/add, task line parsing
src/documents/search.rs      full-text search, mention extraction
```

### `documents.rs` (parent, ~350 LOC)
Owns the public API surface via re-exports. Contains:
- `DocError` enum and `map_io_to_doc_error`
- `load_document`, `create_document`, `load_text_file`
- `normalize_newlines`, `detect_line_ending`, `is_fence_line`
- Shared small utilities used across submodules
- All `pub(crate) use` re-exports so callers don't change

### `documents/paths.rs` (~280 LOC)
- `collect_markdown_paths`, `collect_markdown_paths_recursive`
- `doc_id_from_path`, `resolve_doc_path`, `doc_id_to_path`
- `resolve_text_file_path`, `text_file_id_to_path`
- `is_text_extension`, `highlight_lang_for_extension`
- `FileKind`, `DirectoryFile`, `DirectoryListing`, `list_directory`
- `dir_to_path`
- Tests for path resolution, directory listing, hidden dir skipping

### `documents/rendering.rs` (~750 LOC)
- `RenderedDocument`, `render_document_html`
- `render_task_list_markdown`, `render_task_list_form`
- `rewrite_relative_md_links`, `rewrite_relative_image_links`
- Private: `rewrite_relative_md_link`, `rewrite_relative_image_link`,
  `resolve_relative_path`, `split_link_fragment`,
  `is_absolute_or_scheme`, `has_extension_ignore_ascii_case`
- Private: `is_mermaid_info`, `is_abc_info`, `heading_slug`, `unique_slug`
- Tests for rendering, link rewriting, heading slugs, mermaid/abc detection

### `documents/editing.rs` (~450 LOC)
- `BlockKind`, `BlockRange`, `ReorderError`
- `line_count`, `lines_for_display`
- `scan_block_ranges`, `reorder_range`
- Private: `split_lines_preserve`, `join_lines_preserve`, `LineSegment`
- Private: block detection helpers (`detect_table_block`,
  `detect_list_item_block`, `is_heading_line`, `is_table_*`, etc.)
- Tests for block scanning and reordering

### `documents/tasks.rs` (~280 LOC)
- `toggle_task_item`, `add_task_item_in_list`
- `collect_mentions`
- Private: `parse_task_line`, `TaskLineParts`, `is_task_list_marker`
- Private: `extract_mentions_from_line`, `is_mention_boundary`,
  `is_username_start`, `is_username_char`
- Tests for task toggling, task adding, mention extraction

### `documents/search.rs` (~190 LOC)
- `SearchResult`, `search_documents`
- Private: `find_match_snippet`
- Tests for search

## What does NOT change
- **Public API**: all `pub(crate)` items are re-exported from `documents.rs`,
  so no caller (`app/documents.rs`, `app/text_files.rs`, `directives.rs`, etc.)
  needs modification.
- **No new abstractions**: no traits, no generics, no new types.
- **No logic changes**: pure structural move of functions between files.
- **No dependency changes**.

## Non-goals
- Refactoring internals of any moved function.
- Changing the `app/documents.rs` handler layer.
- Deduplicating `assets.rs` (separate follow-up if desired).
- Adding new tests beyond what exists today (tests move with their functions).

## Risks
- **Low**: this is a mechanical move. Each function moves with its private
  helpers and tests. The re-export layer ensures compile errors surface
  immediately if anything is missed.
- **Shared private helpers**: a few small utilities (`is_fence_line`,
  `split_line_ending`, `leading_indent`) are used across concerns. These
  stay in the parent `documents.rs` as `pub(super)` or are placed in the
  submodule that is their primary consumer and re-used via `super::`.
  The exact placement is decided during implementation — the guiding
  principle is: put it where it's used most, avoid circular dependencies.

## Task breakdown

Each task is a single commit within one PR. The PR is kept as a single
unit because the intermediate states (half-extracted module) aren't
meaningful to review independently.

- [ ] **T1: Create `documents/paths.rs`**
  Move path resolution functions and directory listing. Re-export from parent.
  - AC: `cargo nextest run` passes, no changes to any file outside
    `src/documents.rs` and `src/documents/`.

- [ ] **T2: Create `documents/search.rs`**
  Move search and mention extraction. Re-export from parent.
  - AC: search tests pass.

- [ ] **T3: Create `documents/tasks.rs`**
  Move task list manipulation and mention helpers. Re-export from parent.
  - AC: task toggle/add tests pass.

- [ ] **T4: Create `documents/editing.rs`**
  Move block scanning, reordering, and line-segment utilities. Re-export from parent.
  - AC: reorder tests pass.

- [ ] **T5: Create `documents/rendering.rs`**
  Move rendering, link rewriting, and heading ID generation. Re-export from parent.
  - AC: rendering tests pass.

- [ ] **T6: Clean up parent `documents.rs`**
  Verify re-exports are complete. Remove dead code. Ensure parent is
  ~350 LOC or less.
  - AC: `cargo fmt && cargo clippy --all-targets --all-features && cargo nextest run`
    all green. No caller changes outside `src/documents*`.

- [ ] **T7: Update docs**
  Update `docs/Resources/ARCHITECTURE.md` to reflect the new submodule
  structure. Check off task in TODO.md.
  - AC: ARCHITECTURE.md lists all `documents/*.rs` submodules.

## ADR?
Not required. This is a pure structural refactor within one module. No
changes to architecture, security model, data model, or dependencies.
