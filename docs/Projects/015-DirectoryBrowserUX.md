# Directory Browser UX

## Status
Done

## Goal
Improve the directory browser with breadcrumb navigation and a card-based layout
with type icons, making it easier to scan and navigate a growing knowledge base.

## Context
- The directory browser (`document_list.html`) currently renders a plain `<ul>`
  with text links — subdirectories and files are visually identical except for a
  trailing `/` on directory names.
- There is no way to see where you are in the hierarchy at a glance. The `<h1>`
  shows the full directory path but none of the segments are clickable.
- As the knowledge base grows, scanning a list of unstyled text links becomes
  slower. Visual differentiation (icons, card layout) helps users locate entries
  faster.
- The recent visual facelift established a design system with surface tokens,
  card patterns, and a color palette — this project reuses those patterns.

## Constraints
- **No new dependencies.** Icons are inline SVGs or CSS-only, not an icon
  library.
- **No new JS.** Both features are pure server-rendered HTML + CSS.
- **No layout architecture changes.** The single-column 800px layout stays.
- **Mobile-friendly.** Cards must work at the 600px breakpoint (single-column
  grid on small screens).
- **No backend logic changes** beyond passing breadcrumb data to the template.

## Design

### Breadcrumbs

Replace the `<h1>{{ current_dir }}/</h1>` heading with a breadcrumb trail where
each path segment is a clickable link.

Example for path `Projects/Resources/Adrs`:

```
Documents / Projects / Resources / Adrs
^^^^^^^^    ^^^^^^^^   ^^^^^^^^^   ^^^^
link to /   /d/Projects /d/Projects/Resources  current (not linked)
```

**Implementation:**
- Add a `Vec<BreadcrumbSegment>` field to `DirectoryBrowseTemplate`, where each
  segment has a `name: String` and `url: String`.
- The handler builds this by splitting `current_dir` on `/` and accumulating
  the path prefix.
- The last segment (current directory) is rendered as plain text, not a link.
- "Documents" is always the first breadcrumb, linking to `/`.
- At root level, just show `<h1>Documents</h1>` (no breadcrumb trail needed).
- Style: horizontal list with a separator (` / ` or `›`), using secondary text
  color for non-current segments and primary for the current one.

### Directory cards

Replace the `<ul>` file list with a CSS grid of cards. Each card contains:
- A **type icon** (inline SVG) — folder for directories, document for `.md`
  files.
- The **entry name** as a link.

**Grid layout:**
- CSS grid with `auto-fill` columns, min ~200px per card.
- On mobile (<600px), collapses to a single column.
- Cards use `--surface-elevated` background, matching the existing card pattern
  from the design system (subtle shadow, rounded corners).

**Icons:**
- Two small inline SVGs: a folder icon and a document icon.
- Defined once in the template (or as CSS background-image data URIs) to avoid
  repetition.
- Color follows `--color-text-secondary` so they adapt to the theme.

**Parent directory:**
- The `..` entry becomes a card as well, with a distinct "up arrow" or "back
  folder" icon to differentiate it from regular folders.

## What this is NOT
- Not a sidebar or tree navigation. That is a separate, larger effort.
- Not a file metadata display (size, modified date). That could come later but
  is not in scope.
- Not a `dir.md` / `dir.json` feature. Directory descriptions and config is
  planned as a separate project.

## Task breakdown

### Task 1: Breadcrumb navigation [x]
- Add `BreadcrumbSegment` struct (or tuple) with `name` and `url` fields.
- Add `breadcrumbs: Vec<BreadcrumbSegment>` to `DirectoryBrowseTemplate`.
- Build the breadcrumb list in the `directory_browse` handler from `current_dir`.
- Update `document_list.html` to render breadcrumbs when not at root, plain
  `<h1>Documents</h1>` at root.
- Add CSS for breadcrumb styling (horizontal list, separator, color treatment).

**Acceptance criteria:**
- Each path segment except the last is a clickable link to its directory.
- "Documents" links to `/`.
- Current directory name is rendered as non-linked text.
- Root page shows `<h1>Documents</h1>` without breadcrumb trail.
- Breadcrumbs wrap gracefully on narrow screens.
- Existing tests pass, no visual regression on other pages.

### Task 2: Card grid layout [x]
- Replace the `<ul>` in `document_list.html` with a CSS grid of card elements.
- Add inline SVG icons: folder (directories), document (`.md` files), back/up
  (parent `..` link).
- Style cards using existing design system tokens (`--surface-elevated`,
  `--color-border`, `--color-text-secondary` for icons).
- Ensure cards are responsive: multi-column on desktop, single column on mobile.

**Acceptance criteria:**
- Directories and files are visually distinct via icons.
- Cards use the established surface/shadow/radius pattern.
- The `..` parent link is visually distinct from regular folders.
- Grid is responsive — collapses to single column below 600px.
- Empty directory state ("No documents found.") still works.
- Existing tests pass.

### Task 3: Polish and verify [x]
- Test both themes (light + dark) at desktop and mobile widths.
- Ensure breadcrumbs + cards work together for deeply nested directories.
- Verify no regressions on document view, edit, search, or other pages.

**Acceptance criteria:**
- Both themes look correct.
- Cards and breadcrumbs are visually cohesive with the rest of the app.
- No regressions on any other page.

## Non-goals
- Sidebar / file tree navigation.
- Directory descriptions or metadata (`dir.md`, `dir.json`).
- File metadata display (size, date, author).
- Sorting or filtering options.
- Drag-and-drop or file management actions.

## Risks and limitations
- Inline SVGs add a small amount of template verbosity. This is preferable to
  an icon dependency.
- The card grid takes more vertical space than a plain list. For directories
  with many entries this could mean more scrolling — but the improved
  scannability should offset this.
- Breadcrumb paths can get long for deeply nested directories. The wrapping
  behavior should handle this gracefully but is worth testing.
