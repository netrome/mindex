# File Uploads

## Status
Implemented

## Goal
Allow uploading arbitrary files (PDFs, images, etc.) to a specific directory via the web UI, so that e.g. a PDF on a phone can be uploaded directly to the right place.

## Context
- Image uploads already exist (`POST /api/uploads`) but are restricted to images (PNG/JPEG/GIF/WebP) and always store files in `mindex-uploads/YYYY/MM/`.
- The directory browser shows PDFs, images, and text files, but there's no way to add files to a directory from the browser.
- The `/upload` page is image-only with no directory targeting.
- The `/file/{*path}` route already serves any recognized file type.

## Requirements
- Upload files of any recognized type to a chosen directory.
- Integrate with the directory browser so uploading feels natural (browse to a directory, upload there).
- Preserve filesystem safety invariants (no traversal, no writes outside root, no symlink escapes).
- Mobile-friendly (the primary motivating use case is uploading from a phone).
- Keep the existing image upload API and editor paste flow working.

## Options

### Option 1: Add upload button to directory browser only
**Approach**: Add a file input + upload button to each directory page. The upload target is the currently browsed directory.

**Pros**:
- Most intuitive UX — you're already looking at the directory.
- No new pages needed.
- Simple mental model: "navigate, then upload."

**Cons**:
- No standalone upload page for arbitrary paths (but the existing `/upload` page still works for images to `mindex-uploads/`).

### Option 2: Generalize the `/upload` page with a directory picker (recommended)
**Approach**: Extend the upload API to accept a target directory. Add an upload action to the directory browser that links to the upload page pre-filled with the current directory. The upload page gets a directory field (defaulting to the current directory or `mindex-uploads/`) and accepts any recognized file type.

**Pros**:
- Works from directory browser (quick action) and from the standalone page.
- One upload page handles both the old image flow and the new file flow.
- The directory field makes the target explicit.

**Cons**:
- Slightly more UI work than Option 1.

### Option 3: Inline upload form in directory browser
**Approach**: Embed the full upload form (file picker, status, result) directly in the directory listing template.

**Pros**:
- Zero navigation — upload without leaving the page.

**Cons**:
- Clutters the directory browser.
- Duplicates UI that already exists on `/upload`.
- Harder to keep mobile-friendly.

## Recommendation
Option 2. It gives the best UX across both entry points:
- From the directory browser: a visible "Upload" button links to `/upload?dir=current/path`, so the target is pre-filled.
- From the standalone page: the user can type or browse to a directory.
- Existing image paste flow continues to work (no target dir = defaults to `mindex-uploads/`).

## Design

### API changes

**`POST /api/uploads`** — extend to accept any recognized file type and an optional target directory.

New/changed headers:
- `X-Upload-Directory` (optional): relative directory path under root. If absent, defaults to `mindex-uploads/YYYY/MM/` (current behavior).

Behavior changes:
- Drop the image-only restriction. Accept any file whose extension maps to a recognized type (images, PDFs, text files — the same set the directory browser already displays).
- When `X-Upload-Directory` is set:
  - Validate the directory path (no traversal, resolves within root).
  - Store the file directly in that directory using the original filename (sanitized).
  - If a file with that name already exists, append a numeric suffix (e.g. `scan-1.pdf`).
- When `X-Upload-Directory` is absent: preserve current behavior (timestamped name in `mindex-uploads/YYYY/MM/`).
- Response JSON stays the same shape: `{ path, url, markdown }`.
  - For non-image files, `markdown` returns a regular link `[filename](path)` instead of `![](path)`.

### Upload page changes

The `/upload` page currently has a heading "Upload image" and an `accept="image/*"` file input.

Changes:
- Rename heading to "Upload file".
- Remove the `accept="image/*"` restriction.
- Add a directory field (text input) pre-filled from the `?dir=` query parameter, or empty (meaning `mindex-uploads/`).
- The JS sends the directory value as the `X-Upload-Directory` header.
- Update result display: show the URL and a markdown link snippet (image or regular link depending on type).

### Directory browser integration

Add an "Upload" link/button to the directory page that navigates to `/upload?dir=<current-directory>`.

### Domain layer changes (`src/uploads.rs`)

- Extract file type detection into a broader `FileType` enum (or extend the existing `content_type_for_path`) that covers images, PDFs, and text files.
- Add a `store_file` function (or extend `store_upload`) that:
  - Accepts a target directory parameter.
  - Sanitizes the original filename.
  - Handles collision avoidance (numeric suffix).
  - Uses `ensure_parent_dirs` + `atomic_write_bytes`.
- Keep `store_upload` (no target dir) calling the new function with `mindex-uploads/YYYY/MM/`.

### Editor paste — no changes
Paste continues to call `POST /api/uploads` without `X-Upload-Directory`, so images still go to `mindex-uploads/`. No changes needed.

## Security considerations
- **Path traversal**: `X-Upload-Directory` is validated the same way as file paths — reject `..`, absolute paths, symlinks resolving outside root.
- **Directory creation**: If the target directory doesn't exist, create it (using `ensure_parent_dirs` which already validates safety). This is consistent with how `mindex-uploads/YYYY/MM/` directories are created today.
- **Filename sanitization**: Reuse and extend `sanitize_base_name`. Preserve the original extension.
- **File type restriction**: Only accept file types that the app already knows how to serve (images, PDFs, text files). Reject unknown extensions to avoid serving arbitrary content.
- **Size limit**: The existing `DefaultBodyLimit::disable()` on the upload route should be replaced with a reasonable limit (e.g. 50 MB). This is a pre-existing gap, not new to this feature.
- **Auth**: Upload routes are already behind auth middleware when auth is enabled.

## Implementation plan

### ~~Task 1: Generalize upload storage to accept a target directory~~ Done
- Add a `store_file` function (or extend `store_upload`) that accepts an optional target directory and original filename.
- Sanitize filename, handle collisions with numeric suffix.
- Validate target directory path safety.
- Broaden accepted file types beyond images.
- Add tests for: target directory upload, filename collision, path traversal rejection, unsupported type rejection.
- **AC**: Files can be stored in a specified directory under root. Existing image upload behavior is preserved when no directory is specified.

### ~~Task 2: Extend the upload API endpoint~~ Done
- Accept `X-Upload-Directory` header.
- Return appropriate markdown (image link vs regular link).
- Add integration tests.
- **AC**: `POST /api/uploads` with `X-Upload-Directory: receipts/2026` stores the file in that directory. Without the header, behavior is unchanged.

### ~~Task 3: Update the upload page~~ Done
- Rename to "Upload file", remove image-only restriction.
- Add directory text field, pre-fill from `?dir=` query param.
- Update JS to send `X-Upload-Directory` header.
- **AC**: User can upload a PDF to a specific directory from the upload page. Page works on mobile.

### ~~Task 4: Add upload link to directory browser~~ Done
- Add an "Upload" link to the directory browser template that navigates to `/upload?dir=<path>`.
- **AC**: Clicking "Upload" from a directory page opens the upload page with the directory pre-filled.

### ~~Task 5: Documentation~~ Done
- Update README (upload section, supported file types).
- Update `docs/Projects/TODO.md`.
- **AC**: README accurately describes the file upload feature.

## Risks and limitations
- **Large files**: Uploads are read fully into memory. The size limit mitigates this. Streaming uploads would add complexity not warranted at this scale.
- **Filename collisions**: Numeric suffix approach is simple but could look odd with many collisions. Acceptable for a personal knowledge base.
- **No drag-and-drop**: The upload page uses a standard file input. Drag-and-drop could be a follow-up.

## Non-goals
- Bulk/multi-file uploads.
- Drag-and-drop on the upload page.
- Upload progress bars.
- File type conversion or compression.
- Uploading files of unrecognized types.
