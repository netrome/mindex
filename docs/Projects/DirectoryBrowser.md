# Directory Browser

## Status
Done

## Goal
Replace the flat document list at `/` with a directory browser that shows the
contents of a single directory at a time — subdirectories and files — so users
can navigate a large knowledge base the same way they navigate a file system.

## Context
- The current `/` route calls `collect_markdown_paths` which recursively walks
  the entire root and returns a flat, sorted list of all `.md` files.
- With a handful of files this is fine. With dozens of files in nested
  directories it becomes hard to scan and navigate.
- The existing "New document" form at `/new` requires the user to type the full
  relative path (e.g. `notes/idea.md`). When browsing a directory, the form
  should pre-fill the directory prefix so the user only needs to type the
  filename.
- Document identity is already "normalized relative path from root", so
  directory paths follow the same convention (just without the `.md` suffix).

## Requirements
1. See all subdirectories of the current directory.
2. See all `.md` files in the current directory (non-recursive).
3. Navigate into any subdirectory.
4. Create a new file scoped to the current directory.

## Options explored

### Option A: Directory browser at `/`, subdirectories at `/dir/{*path}`

**Approach**
- Change the `/` handler to list only the root directory's immediate children
  (subdirectories + `.md` files), instead of recursively collecting all files.
- Add a `/dir/{*path}` route for browsing subdirectories.
- Both routes use the same handler and template: show subdirectories as folder
  links (pointing to `/dir/<subdir>`), and `.md` files as document links
  (pointing to `/doc/<doc_id>` as today).
- Add a "parent directory" link (`..`) when not at root.
- The "New" link passes the current directory as a query param:
  `/new?dir=some/dir`, and the `/new` form pre-fills `some/dir/` in the path
  input so the user only types the filename.
- Hidden directories (starting with `.`) are excluded, matching the existing
  `collect_markdown_paths` behavior.

**Domain layer changes**
- New function `list_directory(root, relative_dir) -> Result<DirectoryListing>`
  in `documents.rs`. Returns `{ directories: Vec<String>, files: Vec<String> }`.
  Uses the same path validation as `resolve_doc_path` (no traversal, must
  resolve within root, no symlinks).

**Pros**
- Minimal change: one new domain function, one new route, one template update.
- URL scheme is intuitive: `/` is root, `/dir/notes` is the `notes/` folder.
- No JS required — pure server-rendered HTML with regular links.
- Pre-filling the directory in `/new` makes file creation much faster.
- The flat list is not lost — `/search` with an empty query effectively serves
  the same purpose. (Or we could add a "list all" link if needed.)

**Cons**
- Users who liked the flat list lose it as the default view. (Mitigated by
  search still being available.)
- Adds a new route (`/dir/{*path}`) to the router.

**Complexity:** Low. ~50 lines in `documents.rs`, ~20 lines in handler, template
update. No new dependencies.

### Option B: Collapsible tree sidebar

**Approach**
- Keep the flat list at `/` but add a collapsible directory tree in a sidebar.
- Tree nodes expand/collapse on click, lazily loading children via AJAX or
  eagerly rendering the full tree.

**Pros**
- Shows the full structure at a glance.
- No page navigation needed to browse.

**Cons**
- Significantly more complex: needs JS for expand/collapse, CSS for tree
  layout, potentially AJAX endpoints for lazy loading.
- Doesn't work well on mobile (sidebars are awkward on small screens).
- The full tree can be overwhelming for large knowledge bases — the opposite of
  the problem we're trying to solve.
- Eager rendering defeats the purpose (still loading everything); lazy loading
  adds API endpoints and client state.

**Complexity:** Medium-high. New JS module, CSS, potentially a new API endpoint.

### Option C: Client-side grouping of the flat list

**Approach**
- Keep the flat list but add client-side JS that groups documents by their
  directory prefix, rendering collapsible sections.

**Pros**
- No backend changes.
- Flat list still available (just visually grouped).

**Cons**
- Requires JS for basic navigation to work well.
- Still loads all documents upfront — doesn't scale.
- Grouping logic in JS duplicates what the filesystem already provides.
- Doesn't solve the "create file in current directory" use case.

**Complexity:** Low backend, but adds a non-trivial JS module.

## Recommendation
**Option A** (directory browser at `/` and `/dir/{*path}`).

1. **Simplest.** One domain function, one route, one template. No JS.
2. **Scales.** Only reads one directory at a time, not the entire tree.
3. **Consistent.** Follows the existing URL pattern (`/doc/{*path}`,
   `/edit/{*path}`) and the same path validation logic.
4. **Solves all requirements.** Subdirectory listing, file listing, navigation,
   and scoped file creation.

## Design

### URL scheme

| URL | Shows |
|---|---|
| `/` | Root directory contents |
| `/d/notes` | Contents of `notes/` |
| `/d/notes/work` | Contents of `notes/work/` |
| `/d/notes/todo.md` | View document |
| `/new?dir=notes` | New document form, pre-filled with `notes/` |

The `/d/{*path}` route is unified: if the path ends in `.md` it renders the
document, otherwise it renders the directory browser. This means navigating
between directories and documents is transparent — users don't need to know
whether a path is a file or directory.

### Domain function

```rust
pub struct DirectoryListing {
    pub directories: Vec<String>,  // sorted, names only (e.g. "notes", "work")
    pub files: Vec<String>,        // sorted, names only (e.g. "todo.md", "ideas.md")
}

pub fn list_directory(root: &Path, relative_dir: &str) -> Result<DirectoryListing, DocError>
```

- Validates `relative_dir` using the same path normalization as `resolve_doc_path`.
- Empty string means root.
- Reads immediate children with `std::fs::read_dir`.
- Skips hidden entries (starting with `.`) and symlinks.
- Directories go in `directories`, `.md` files go in `files`.
- Both lists sorted alphabetically.

### Template

The `document_list.html` template becomes a directory browser:
- Breadcrumb trail showing the current path (each segment links to its `/dir/` URL).
- Subdirectories listed first (as folder links).
- Files listed second (as document links).
- "New" link points to `/new?dir=<current_dir>`.
- ".." link when not at root.

### Handler

```
GET /         -> directory_browse(dir="")
GET /dir/*path -> directory_browse(dir=path)
```

Both use the same `directory_browse` handler function and
`DirectoryListingTemplate`.

## Task breakdown

### Task 1: Domain function `list_directory`
Add `DirectoryListing` struct and `list_directory` function to `documents.rs`.
Reuse existing path validation. Unit tests covering:
- Root listing
- Subdirectory listing
- Hidden directories excluded
- Symlinks excluded
- Path traversal rejected
- Non-existent directory returns error

### Task 2: Handler and template
- Add `directory_browse` handler in `app/documents.rs`.
- Update `DocumentListTemplate` (or create `DirectoryBrowseTemplate`) with
  fields for current path, directories, files, breadcrumbs.
- Update `document_list.html` to render directory browser UI.
- Add `/dir/{*path}` route in `app.rs`.
- Wire `/` to use the new handler.

### Task 3: Scoped "New" form
- Accept `dir` query param in `/new` handler.
- Pre-fill the `doc_id` input with `<dir>/` when `dir` is provided.
- Update the "New" link in the directory browser template to pass `?dir=`.

## Non-goals
- Showing non-`.md` files in the directory listing (PDFs, images, etc.). These
  are accessible via other routes but not browsed here.
- Recursive/flat listing mode toggle. Search covers the "find anything" case.
- Drag-and-drop file moving between directories.
- Creating directories explicitly (they are created implicitly when creating a
  document in a new path).

## Risks and limitations
- Users accustomed to the flat list will need to use `/search` to find
  documents across the entire tree. This is already the natural workflow for
  large knowledge bases.
- Directory listing reads the filesystem on every request (no caching). This is
  fine for the expected scale and consistent with how document loading works
  today.
