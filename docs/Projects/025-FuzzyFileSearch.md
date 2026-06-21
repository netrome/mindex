# Fuzzy File Search (Command Palette)

## Status
Proposed

## Goal
Add a Helix-style command palette to Mindex: a quick overlay, opened from any
view page, offering an **fzf-style fuzzy file picker** and the existing
**content search**. Keyboard-first on desktop, tappable on mobile.

## Feasibility
**Low-to-moderate** complexity, almost entirely client-side:
- Backend: one new read-only listing endpoint. No new dependencies.
- Fuzzy matching: a small vanilla-JS scorer (~40 lines). No dependency.
- Palette UI: one new feature module following the existing `assets/features/*`
  pattern, plus modal CSS.

## Context
- The frontend is vanilla ES modules; each feature is a self-contained
  `init*()` wired up in `assets/app.js` (e.g. `file_manage.js`, `reorder.js`).
- Content search already exists server-side: `GET /search?q=` →
  `search_documents` → full-page template. This is the "search file contents"
  mode and is reused as-is.
- `paths::collect_markdown_paths(root)` exists but is **not** exposed over HTTP.
  The fuzzy picker needs a list of all browsable files.
- The directory browser already classifies file kinds (md/pdf/text/image) and
  knows the correct view route for each.
- All view pages share the same template chrome and load `app.js`.

## Constraints
- No new dependencies (no fzf library, no fuzzy-match npm package).
- Keep filesystem safety invariants: the listing endpoint must respect root,
  symlink policy, and auth (no leaking when auth is enabled).
- Keep the minimal/hackable philosophy: start with exactly two modes; the
  palette is mode-extensible but is **not** a plugin platform.

## Decisions (settled with maintainer)
- **Desktop trigger:** `Ctrl-K` (`⌘-K` on macOS). `Ctrl-P` is rejected — it is
  the browser print shortcut. `space` is rejected — it fights page scroll on
  reading views.
- **Palette modes (chosen by first keystroke when the query is empty):**
  - `f` → fuzzy file open.
  - `/` → content search.
- **Mobile trigger:** a small persistent floating launcher button. Swipe-up is
  rejected (competes with scroll/pull gestures). Once open, the palette behaves
  identically to desktop with tappable results.
- **Picker scope:** all browsable files (md, pdf, text, image) — Enter opens the
  correct view route for the file's kind.
- **Content search in v1:** reuse the existing `/search` page (Enter navigates
  to `/search?q=…`). Inline AJAX content search is a deliberate follow-up.

## Options considered

### Backend file listing
- **A — JSON endpoint `GET /api/files` (recommended).** Client fetches once,
  fuzzy-matches in JS. Small, cacheable, isolates the only new surface.
- **B — Embed the file list in every page.** Bloats every rendered page; couples
  the palette to server templates. Rejected.

### Fuzzy matching
- **A — Tiny vanilla-JS subsequence scorer (recommended).** Subsequence match
  with boundary/contiguity bonuses (path separators, word starts). No
  dependency, fully hackable.
- **B — Add a fuzzy-match library.** Violates dependency discipline for a small
  amount of code. Rejected.

## Proposed Design (v1)

### Server
- Add `GET /api/files` → JSON: `[{ "path": "notes/todo.md", "kind": "markdown" }, …]`.
  - Lists every file the directory browser would show, recursively under root.
  - Reuses existing path-collection + file-kind classification in
    `documents/paths.rs`.
  - Goes through existing auth middleware (no leak when auth is enabled).
  - Read-only and listing-only: it never accepts a client path, so there is no
    traversal surface.

### Client
- `assets/features/fuzzy.js` — pure scorer: `score(query, candidate) -> number|null`
  and a `filter(query, items)` helper that ranks and truncates. No DOM, no I/O;
  unit-testable in isolation.
- `assets/features/palette.js` — `initPalette()`:
  - Mounts a hidden modal overlay + a floating launcher button into the page.
  - Opens on `Ctrl/⌘-K` (desktop) or button tap (mobile). Never triggers while
    focused in an input/textarea/contenteditable/editor.
  - Empty-query first keystroke selects the mode: `f` (file open) or `/`
    (content search). A small hint row shows the available modes.
  - **File mode:** lazy-fetches `/api/files` once (cached for the session),
    fuzzy-filters live, renders ranked results; ↑/↓ to move, Enter/tap to
    navigate to the file's view route, Esc to close.
  - **Search mode:** Enter navigates to `/search?q=<query>`.
- Wire `initPalette()` into `assets/app.js`.
- Add the two new asset routes in `app.rs` and serve functions in `assets.rs`
  (mirroring existing `features/*.js` registration).
- Modal + launcher-button styles in `assets/style.css`.

## Non-goals (v1)
- Inline AJAX content search (results without leaving the palette) — follow-up.
- Additional palette modes (new file, recent files, git, settings, …).
- Configurable keybindings.
- Swipe-up gesture on mobile.
- Server-side fuzzy ranking.
- Searching file *contents* in the fuzzy picker (it matches paths/names only;
  content search remains the `/` mode).

## ADR
No ADR needed. This does not change the data model, security model, or
architecture, and adds no dependency. The one new endpoint only exposes an
existing listing through the existing auth/safety boundary. The keyboard/mobile
trigger choices are recorded above for traceability.

## Task Breakdown

### Task 1: File listing endpoint ✓
- Add a domain helper that returns `(relative_path, file_kind)` for all
  browsable files under root (reuse `paths.rs` collection + classification).
- Add `GET /api/files` route + thin handler returning JSON.
- Tests: domain helper lists expected files with correct kinds and excludes
  unsupported/hidden entries; integration test asserts JSON shape and that the
  route is auth-gated when auth is enabled.

Acceptance criteria:
- Returns every file the directory browser shows, as `{path, kind}`.
- Paths are normalized relative paths (the document-ID convention).
- Requires auth when auth is enabled.

Implementation notes:
- `documents::paths::collect_browsable_files` walks root recursively, mirroring
  the directory browser's rules (skips symlinks, hidden entries, unrecognized
  extensions) and returns sorted normalized relative paths with their
  `FileKind`.
- `kind` serializes via `FileKind::as_str` as `document` / `pdf` / `image` /
  `text`. The client can navigate to `/d/<path>` for any kind (it already
  resolves to the correct view), so `kind` is for display/icons.
- Handler: `document_file_list` in `app/documents.rs` → `Json<Vec<FileListEntry>>`.
  Auth-gating is inherited from the existing `/api/*` middleware.

### Task 2: Fuzzy scorer module
- Add `assets/features/fuzzy.js` with `score`/`filter`.
- Subsequence match, case-insensitive, with bonuses for matches at path
  separators and word boundaries and for contiguous runs; returns `null` on no
  match.
- Add a tiny test harness (or document manual test cases) for ranking order.

Acceptance criteria:
- `"todo"` ranks `notes/todo.md` above `t/o/d/other.md`.
- Non-subsequence queries return no match.
- Basename matches outrank deep-path incidental matches.

### Task 3: Palette overlay + file mode
- Add `assets/features/palette.js` (`initPalette()`), register the asset route
  (`app.rs` + `assets.rs`), wire into `app.js`.
- Implement open/close (Ctrl/⌘-K + Esc), input-focus guard, file mode with
  lazy `/api/files` fetch, fuzzy filtering, keyboard navigation, and
  navigation-on-select to the per-kind view route.
- Add modal styles to `style.css`.

Acceptance criteria:
- `Ctrl/⌘-K` opens the palette on directory, document, and text view pages.
- Does not trigger while typing in an input/textarea/editor.
- `f` enters file mode; typing filters; Enter opens the highlighted file in the
  correct view; Esc closes.

### Task 4: Content-search mode + mobile launcher
- Add `/` mode → navigate to `/search?q=<query>`.
- Add the floating launcher button (visible/comfortable on mobile) that opens
  the same palette; ensure results are tappable.

Acceptance criteria:
- `/` mode reaches the existing content search results.
- On a phone viewport, the launcher button opens the palette and results can be
  tapped to navigate.

### Task 5: Docs
- Update README (a "Command palette / fuzzy search" subsection) and check off
  the TODO item; note v1 non-goals (inline search, extra modes).

Acceptance criteria:
- README documents the trigger keys, modes, and mobile launcher.
- `docs/Projects/TODO.md` "fzf style file search" item links here and is checked
  off when implementation lands.
