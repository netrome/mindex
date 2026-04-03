# File Move Drag and Drop

## Status
Done

## Goal
Allow moving a file from one directory to another directly in the directory
browser UI, including on phone.

## Feasibility
This is **moderate** complexity:
- Backend move operation is small.
- Desktop drag and drop is straightforward.
- Mobile drag behavior is the main complexity (touch/pointer handling).

## Context
- The directory browser already has clear file cards and folder cards.
- The reorder feature already implements both native drag events and pointer
  drag handling for touch devices.
- There is currently no API/domain function to move files between directories.

## Constraints
- No new dependencies.
- Keep filesystem safety invariants: no traversal, no root escape, no symlink
  escape.
- Keep file-backed model; no DB/background jobs.

## Options

### Option A: Native HTML drag and drop only
Use `dragstart/dragover/drop` on cards.

Pros:
- Lowest implementation effort.

Cons:
- Unreliable on mobile browsers; poor fit for the phone-first use case.

### Option B: Hybrid drag model (recommended)
Use native drag for mouse + pointer-based drag for touch, following the
existing reorder pattern.

Pros:
- Works on desktop and phone.
- Reuses known code patterns in `assets/features/reorder.js`.
- No dependencies.

Cons:
- More JS than Option A.

### Option C: Non-drag "Move to..." action
Add a per-file move button and destination picker (no drag).

Pros:
- Most robust mobile behavior.
- Smallest UX risk.

Cons:
- Does not satisfy the drag-and-drop interaction request.

## Recommendation
Implement **Option B** with a narrow v1 scope.

## Proposed Design (v1)
- Add a directory-browser "Move mode" toggle.
- In move mode:
  - File cards are draggable sources.
  - Folder cards (and `..` parent card) are drop targets.
  - Clicking cards for navigation is disabled to avoid accidental navigation while dragging.
- Add `POST /api/d/move-file` (form encoded):
  - `source_path` (relative file path, e.g. `notes/todo.md`)
  - `target_dir` (relative directory path, empty string for root)
- Server behavior:
  - Validate source and target paths.
  - Ensure source is a supported file type shown in the directory browser.
  - Ensure source/target resolve within root, respecting symlink policy.
  - Reject overwrite conflicts (`409`).
  - Move via `std::fs::rename`.
- Client behavior:
  - On successful drop, reload current page.
  - Show inline error notice on failure.

## Non-goals
- Moving directories.
- Cross-root moves.
- Rename-on-move UI.
- Auto-updating markdown links after moves.
- Multi-file selection.

## ADR
No ADR needed. This does not change architecture, security model, or data model;
it adds one bounded file operation within existing invariants.

## Task Breakdown

### Task 1: Domain move function ✓
- Add a domain function to validate and move a file within root.
- Add unit tests for success, conflict, not-found, traversal rejection, and
  symlink escape rejection.

Acceptance criteria:
- Moving `a/b.md` to `x/` yields `x/b.md`.
- Invalid or escaping paths are rejected.
- Existing destination file returns conflict.

### Task 2: Move API endpoint ✓
- Add `POST /api/d/move-file` route + handler in `app/documents.rs`.
- Map domain errors to HTTP status codes.
- Add integration tests for success and error cases.

Acceptance criteria:
- Valid request returns `204`.
- Invalid path returns `400`, missing source returns `404`, conflict returns `409`.

### Task 3: Directory browser move mode UI ✓
- Add separate `/move/` page (like `/reorder/`) with drag handles on file cards.
- Implement drag/drop JS module for file cards and folder drop targets.
- Reuse reorder-style pointer handling for touch drag.
- Add "Move" link in directory browser nav bar.

Acceptance criteria:
- Desktop: drag file card onto folder card moves file.
- Mobile: touch drag via handle moves file.
- Standard navigation remains unchanged (move is a separate page).

### Task 4: UX polish and docs ✓
- Add drop-target highlight and busy/error states.
- Update README (directory browser section) after implementation.

Acceptance criteria:
- Clear visual drop feedback.
- Error message shown on failed move.
- Docs mention move mode and limitation (no directory moves).
