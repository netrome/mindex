# File Type Support in Directory Browser

## Goal

Make the directory browser show and link to non-markdown files so users can
navigate to PDFs, images, and structured text files (JSON, YAML, TOML) without
needing an explicit markdown link.

## Current state

- `list_directory()` only includes `.md` files.
- `resolve_path()` treats any non-`.md` path as a directory.
- PDFs have a dedicated `/pdf/{path}` viewer but are invisible in directory listings.
- Images are served at `/file/{path}` but also invisible.
- JSON, YAML, and TOML have no rendering or serving support.

## Design

### File categories

Introduce a simple file categorization by extension:

| Category   | Extensions                     | Route            | Rendering                          |
|------------|--------------------------------|------------------|------------------------------------|
| Document   | `.md`                          | `/d/{path}`      | Markdown renderer (existing)       |
| PDF        | `.pdf`                         | `/pdf/{path}`    | PDF viewer (existing)              |
| Image      | `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp` | `/file/{path}` | Inline `<img>` in a viewer page |
| Text       | `.json`, `.yaml`, `.yml`, `.toml` | `/view/{path}` | Read-only with syntax highlighting |

### What changes

#### 1. `list_directory()` — include non-markdown files

Currently filters to `.md` only. Change to include all recognized extensions.
Return a richer type so the template knows how to link each file:

```rust
pub(crate) struct DirectoryFile {
    pub name: String,
    pub kind: FileKind,
}

pub(crate) enum FileKind {
    Document,  // .md
    Pdf,       // .pdf
    Image,     // .png, .jpg, .jpeg, .gif, .webp
    Text,      // .json, .yaml, .yml, .toml
}
```

`DirectoryListing.files` becomes `Vec<DirectoryFile>`.

#### 2. Template — per-type icons and links

Update `document_list.html` to:

- Use `FileKind` to pick the icon (document, PDF, image, text/code).
- Link each file to its correct route:
  - `Document` → `/d/{path}`
  - `Pdf` → `/pdf/{path}`
  - `Image` → `/file/{path}` (or a new viewer page, see below)
  - `Text` → `/view/{path}`

#### 3. Text file viewer — new route `/view/{path}`

A new read-only viewer page for structured text files. Thin handler pattern:

- Validate extension is in the allowed text set.
- Read file contents (reuse path-safety from `resolve_doc_path` / `resolve_file_path`).
- Render in a template with a `<pre><code>` block.
- Leverage the existing Highlight.js integration (already used for code blocks in markdown) for syntax highlighting.
- Include breadcrumb navigation consistent with other pages.

No editing — these are view-only. Editing text formats is out of scope.

#### 4. `resolve_path()` — route non-markdown files

Currently the catch-all `/d/{path}` only handles `.md` and directories.
Extend it to redirect/route recognized file types:

- `.pdf` → redirect to `/pdf/{path}`
- `.json`, `.yaml`, `.yml`, `.toml` → redirect to `/view/{path}`
- `.png`, `.jpg`, `.jpeg`, `.gif`, `.webp` → redirect to `/file/{path}`

This keeps direct URL navigation working (e.g., a user types `/d/config.toml`).

#### 5. `content_type_for_path()` — add text types

Extend to return content types for new text formats so `/file/{path}` can
serve them with correct MIME types if needed:

- `.json` → `application/json`
- `.yaml` / `.yml` → `text/yaml`
- `.toml` → `text/plain` (no standard MIME)

### Non-goals

- No editing of non-markdown files.
- No new file upload types (upload remains image-only).
- No search indexing of non-markdown files.
- No markdown link rewriting for text files (only PDF rewriting exists today).
- No preview/thumbnail generation for images in the grid.

### Risks

- **Extension list maintenance**: Adding a hardcoded extension list is simple but
  needs updating for new types. Acceptable at this scale — a config-driven approach
  would be over-engineering.
- **Large files**: Text viewer reads entire file into memory. The same is true for
  markdown documents today, so this is consistent. Could add a size cap later.

## Task breakdown

### Task 1: Extend `list_directory()` to return all recognized file types
- Introduce `FileKind` enum and `DirectoryFile` struct in `src/documents.rs`.
- Update `list_directory()` filter to include recognized extensions.
- Update `DirectoryListing` to use `Vec<DirectoryFile>` for files.
- Update all call sites (handler, template struct).
- Add tests for the new filtering logic.
- **AC**: Directory listing includes `.md`, `.pdf`, image, and text files. Unknown extensions are excluded.

### Task 2: Update directory template with per-type icons and links
- Update `DirectoryBrowseTemplate` in `src/templates.rs`.
- Update `document_list.html` to render different icons and link targets per `FileKind`.
- **AC**: Each file type shows a distinct icon and links to its correct route.

### Task 3: Add text file viewer (`/view/{path}`)
- New handler in `src/app/uploads.rs` (or a new `src/app/files.rs` — keep it simple).
- New template `text_view.html` with `<pre><code>` and Highlight.js class for the language.
- Breadcrumb navigation matching other pages.
- Path safety validation (reuse existing helpers).
- Route registration in `src/app.rs`.
- Add tests for the handler (valid file, missing file, disallowed extension, path traversal).
- **AC**: Navigating to `/view/config.toml` renders the file with syntax highlighting and breadcrumbs.

### Task 4: Extend `resolve_path()` to route non-markdown file types
- Update `resolve_path()` in `src/app/documents.rs` to check extension and redirect.
- **AC**: `/d/notes/config.toml` redirects to `/view/notes/config.toml`. `/d/scan.pdf` redirects to `/pdf/scan.pdf`.

### Task 5: Extend `content_type_for_path()` for text formats
- Add JSON, YAML, TOML MIME types.
- **AC**: `/file/config.json` serves with `application/json` content type.
