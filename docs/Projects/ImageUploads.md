# Image Uploads

## Status
Phase 1 implemented

## Goal
Support image uploads with the best UX: paste clipboard images directly into the editor to upload and insert a markdown image link. If paste is too complex, provide a dedicated upload page that returns a link to copy into a document.

## Context
- Mindex stores Markdown files under a configured root and treats them as the source of truth.
- The editor is a plain `<textarea>` with minimal JS.
- Markdown rendering only rewrites relative links for `.md` files.
- There is no file upload endpoint or file-serving route for user content.

## Requirements
- Paste clipboard images into the editor, upload them, and insert a markdown image link.
- Provide a fallback upload page that returns a link and/or markdown snippet.
- Preserve filesystem safety invariants (no traversal, consistent symlink policy).
- Avoid new dependencies unless necessary.

## Options explored

### Option 1: Dedicated upload page only
**Approach**: Add `/upload` page with a file picker and an upload endpoint that returns a link to copy.

**Pros**:
- Smallest implementation surface.
- No editor integration complexity.

**Cons**:
- Worse UX for frequent use.
- Extra manual copy/paste.

### Option 2: Upload API + editor paste (recommended)
**Approach**: Add a single upload API used by both an upload page and editor paste. Intercept paste events in the editor, upload the image, and insert a link into markdown.

**Pros**:
- Best UX while keeping the feature scoped.
- Reusable API for future drag-and-drop.
- Still minimal and self-contained.

**Cons**:
- Requires client-side JS for paste handling.

### Option 3: External storage (S3, etc.)
**Approach**: Upload images to external object storage and store URLs in markdown.

**Pros**:
- Offloads storage.

**Cons**:
- Violates “no external systems” non-goal.
- Adds significant configuration and dependencies.

## Recommendation
Implement Option 2 in two phases:
1) Build the upload API, file-serving route, and a simple upload page.
2) Add editor paste support that uses the same API.

This delivers a usable feature quickly without expanding scope or dependencies.

## UX flows

### Paste in editor
1) User pastes an image into the editor.
2) JS detects image data in the clipboard and uploads it.
3) On success, JS inserts a markdown image link at the cursor.
4) On failure, JS shows a small error message.

### Upload page
1) User selects or drags an image file.
2) JS uploads the file and shows:
   - Raw URL
   - Markdown snippet
   - Copy buttons

## Data model and storage
- Store uploads under the configured root in a relative directory, default `uploads/`.
- Organize by date: `uploads/YYYY/MM/`.
- File naming:
  - Start with a sanitized base name from the original filename (or `paste` when missing).
  - Append a timestamp and short random suffix to avoid collisions.
  - Use extension derived from content type or filename.
- The markdown link should be **relative** (e.g., `uploads/2026/01/paste-20260124-123456-ab12.png`) for portability outside the app.

## API and routing

### Upload API
`POST /api/uploads`
- Body: raw image bytes.
- Headers:
  - `Content-Type: image/png|image/jpeg|image/gif|image/webp`
  - Optional `X-Upload-Filename` (original filename).
- Response JSON:
  - `path`: relative path under root
  - `url`: `/file/<path>`
  - `markdown`: `![](<path>)`

### File serving
`GET /file/{*path}`
- Serves files under root for allowed image extensions.
- Validates path and enforces root boundary.
- Sets content-type based on extension.

### Upload page
`GET /upload`
- Simple UI with file input and copyable output.

## Rendering changes
Extend markdown rendering to rewrite relative image links to `/file/<resolved>` when generating HTML:
- Handle `Event::Start(Tag::Image { dest_url, .. })`.
- Resolve relative paths based on the current document path (similar to `.md` links).
- Keep existing link rewriting for `.md` links unchanged.

This keeps markdown portable while ensuring images render correctly in the app.

## Security considerations
- Path traversal: reject absolute paths and `..` components.
- Symlinks: reject uploads where any parent directory is a symlink; serve only if canonicalized path is within root.
- Content validation: allow only PNG/JPEG/GIF/WebP. Check both `Content-Type` and simple magic bytes.
- Size limit: enforce a maximum upload size (e.g., 10 MB) at router-level and in handler.
- Auth: upload and file-serving routes remain protected by auth middleware (no bypass).

## Implementation plan

### Task 1: Storage helpers
- Add helpers for upload directory resolution, filename sanitization, and safe writes.
- **Acceptance criteria**: Files are written under root, parents are created safely, symlinks are rejected.

### Task 2: Upload API endpoint
- Add `POST /api/uploads` to accept raw image bytes and return JSON.
- Enforce size limit and type validation.
- **Acceptance criteria**: Valid images upload successfully; invalid type/size returns clear errors.

### Task 3: File-serving route
- Add `GET /file/{*path}` for allowed image files.
- **Acceptance criteria**: Images render in-browser; traversal and symlink paths are rejected.

### Task 4: Markdown image rewrite
- Extend renderer to rewrite relative image URLs to `/file/<resolved>`.
- **Acceptance criteria**: `![](images/a.png)` in `notes/a.md` renders correctly.

### Task 5: Upload page + editor paste
- Add `templates/upload.html` and a new JS feature module.
- Add paste handler for the editor textarea.
- **Acceptance criteria**: Paste inserts a markdown image link and upload page returns copyable markdown.

### Task 6: Documentation
- Update README/docs for new feature, supported formats, and size limit.
- Update `docs/Projects/TODO.md` when implementation is complete.

## Risks and limitations
- Uploads read into memory; mitigated via size limit.
- Clipboard APIs behave differently across browsers.
- No image resizing/compression (large files remain large).

## Non-goals
- Non-image attachments.
- Image manipulation or compression.
- Live preview of pasted images in the editor.

## Open questions
- Default upload directory name: `uploads/` vs `assets/`?
- Default max upload size: 5 MB vs 10 MB?
- Should upload page show a raw URL, markdown snippet, or both? (recommend both)
