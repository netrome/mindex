# File Deletion

## Status
Proposed

## Goal
Allow deleting files from the directory browser UI, including on phone.

## Feasibility
This is **low** complexity:
- Backend delete operation is small (validate path, remove file).
- UI can reuse the existing move page which already serves as a file
  management view.

## Context
- The move page (`/move/`) already shows file cards with drag handles for
  file management operations.
- There is currently no API or UI to delete files.
- Git integration provides recovery for users who have it enabled.

## Constraints
- No new dependencies.
- Keep filesystem safety invariants: no traversal, no root escape, no symlink
  escape.
- Only delete files shown in the directory browser (supported file types).
- No directory deletion.

## Options

### Option A: Delete button on the move page (recommended)
Add a delete button (x) to each file card on the existing move page.
Clicking it shows a browser `confirm()` dialog, then calls the delete API.

Pros:
- No new pages or views.
- The move page is already the "file management" view.
- Minimal UI addition (one button per card).
- Works on desktop and mobile.

Cons:
- File management actions only available from the move page, not the main
  directory browser. (Acceptable: keeps the main view clean.)

### Option B: Trash bin
Move deleted files to a `.trash/` directory under root instead of removing
them. Add a trash view to browse and restore.

Pros:
- Recoverable without git.

Cons:
- Significant extra complexity (trash management, restore, cleanup).
- Git already provides recovery for most users.
- Conflicts with the minimal file-backed philosophy.

### Option C: Delete from the main directory browser
Add delete buttons directly to file cards in the main directory listing.

Pros:
- Most discoverable.

Cons:
- Adds destructive actions to the browse view where accidental clicks
  are more likely.
- Adds JS to a page that currently needs none.

## Recommendation
Implement **Option A**. The move page is already the file management view,
and adding a delete button there is minimal and natural. Git provides
recovery for users who need it.

## Proposed Design

- Add `POST /api/d/delete-file` (form encoded):
  - `file_path` (relative file path, e.g. `notes/todo.md`)
- Server behavior:
  - Validate path (normal components only, supported extension).
  - Resolve and ensure file is within root, not a symlink escape.
  - Remove via `std::fs::remove_file`.
- Client behavior:
  - Delete button (x) on each file card in the move page.
  - Browser `confirm()` before calling API.
  - On success, remove the card from the DOM (no full reload needed).
  - Show error notice on failure.

## Non-goals
- Deleting directories.
- Trash/undo (git covers recovery).
- Batch deletion.
- Delete from the main directory browser view.

## ADR
No ADR needed. This adds one bounded file operation within existing
invariants, similar to the move feature.

## Task Breakdown

### Task 1: Domain delete function ✓
- Add a domain function to validate and delete a file within root.
- Add unit tests for success, not-found, traversal rejection, unsupported
  extension, and symlink escape rejection.

Acceptance criteria:
- Deleting `a/b.md` removes the file.
- Invalid or escaping paths are rejected.
- Missing file returns not-found.

### Task 2: Delete API endpoint ✓
- Add `POST /api/d/delete-file` route + handler.
- Map domain errors to HTTP status codes.
- Add integration tests.

Acceptance criteria:
- Valid request returns `204`.
- Invalid path returns `400`, missing file returns `404`.

### Task 3: Delete button UI on move page
- Add a delete button (x) to each file card in `file_move.html`.
- Add JS handler: `confirm()` then POST, remove card from DOM on success.
- Style the delete button.

Acceptance criteria:
- Clicking delete shows confirmation dialog.
- Confirmed delete removes file and updates the UI.
- Cancelled delete does nothing.
- Error message shown on failed delete.
