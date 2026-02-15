# PDF Viewing Support

## Status
Accepted

## Goal
Make PDF files that already exist under the configured root easy to open and read from Mindex, especially on mobile (for example concert tickets).

## Context
- Mindex currently serves markdown documents via `/doc/{*path}`.
- Relative markdown links ending in `.md` are rewritten to `/doc/...`.
- Relative image links are rewritten to `/file/...`.
- `/file/{*path}` currently serves only image extensions.
- Relative links to `.pdf` files are not rewritten today, so common links like `[Ticket](tickets/show.pdf)` from a document page are not reliably viewable.

## Constraints and invariants
- Do not read or write outside configured root.
- Preserve path traversal protections and symlink policy.
- Keep document ID semantics unchanged (relative `.md` path).
- Avoid major dependencies unless justified.
- Keep auth behavior unchanged (no document leaks when auth is enabled).

## Options explored

### Option 1: Minimal PDF file serving + link rewrite
Approach:
- Extend `/file/{*path}` to allow `.pdf` and return `application/pdf`.
- Rewrite relative markdown links ending in `.pdf` to `/file/<resolved-path>`.

Pros:
- Smallest implementation.
- No new dependencies.
- Works with built-in browser PDF viewers.

Cons:
- Opens raw file view without Mindex page chrome.
- UX depends entirely on browser PDF behavior.

### Option 2: Dedicated in-app PDF viewer page (accepted)
Approach:
- Do Option 1.
- Add `/pdf/{*path}` route and a small template that embeds `/file/<path>` in an `iframe` (or `object`) with fallback `Open raw` and `Download` links.
- Rewrite relative `.pdf` markdown links to `/pdf/<resolved-path>`.

Pros:
- Better mobile UX and navigation consistency.
- Keeps implementation simple and dependency-free.
- Still relies on native browser PDF rendering.

Cons:
- Slightly larger scope than Option 1.
- Still limited by browser PDF support.

### Option 3: PDF.js-based custom renderer
Approach:
- Add a dedicated PDF viewer route using bundled PDF.js assets and custom client-side rendering controls.

Pros:
- Consistent rendering and controls across browsers.
- More room for future features (page thumbnails, text search, etc.).

Cons:
- Large dependency and asset footprint.
- More JS complexity and maintenance.
- Beyond current minimal scope.
- Would require an ADR before implementation.

## Recommendation
Choose **Option 2**.

It directly solves the ticket-on-phone use case, keeps the codebase simple, preserves invariants, and avoids major dependencies. It also gives a cleaner in-app experience than raw file serving while remaining close to the current architecture.

## Proposed behavior (Option 2)
- Markdown link in a doc: `[Concert ticket](attachments/ticket.pdf)`
- On render: link becomes `/pdf/<resolved-relative-path>`
- Viewer page:
  - Embeds the PDF from `/file/<resolved-relative-path>`
  - Shows explicit actions:
    - `Open raw PDF` -> `/file/<resolved-relative-path>`
    - `Download PDF` -> `/file/<resolved-relative-path>?download=1`
- Direct file endpoint `/file/{*path}` serves:
  - Existing supported image types (unchanged)
  - `application/pdf` for `.pdf`
  - When `download=1` for PDF: include `Content-Disposition: attachment; filename="<name>.pdf"`

## Security and invariant impact
- No invariant change required.
- Continue using root-bounded path resolution and traversal checks for `/file` and `/pdf` routes.
- Continue current symlink policy enforcement.
- Keep routes behind existing auth middleware behavior.

## Task breakdown (PR-sized)

- [x] **Task 1: Extend file serving allowlist to include PDF**
  - Acceptance criteria:
    - `/file/...pdf` returns `200` with `content-type: application/pdf`
    - `/file/...pdf?download=1` returns `200` with attachment disposition
    - unsupported extensions still return `404`

- [x] **Task 2: Add relative PDF link rewriting in markdown rendering**
  - Acceptance criteria: `[x](a/b.pdf)` in `notes/doc.md` renders as `href="/pdf/notes/a/b.pdf"`; absolute/schemed links remain unchanged.

- [x] **Task 3: Add `/pdf/{*path}` viewer route and template**
  - Acceptance criteria:
    - valid in-root PDF path renders an embedded viewer
    - page includes `Open raw PDF` and `Download PDF` actions
    - invalid/traversal paths return `404`

- [x] **Task 4: Add tests for correctness and safety**
  - Acceptance criteria: Tests cover link rewriting, PDF content type, and path safety behavior (including traversal rejection).

- [ ] **Task 5: Update docs**
  - Acceptance criteria: README includes PDF viewing behavior and basic usage example.

## Risks and limitations
- Some mobile browsers have limited embedded PDF behavior.
- Current file-serving path reads whole files into memory; this is acceptable for typical ticket-size PDFs but not ideal for very large files.
- No PDF text extraction/search in v1.

## Non-goals
- PDF upload endpoint changes.
- PDF editing, annotation, signing, or OCR.
- Full-text indexing of PDF contents.
- Offline caching strategy changes for `/file` or `/pdf`.

## ADR impact
- **No ADR required** for Option 2 as proposed (no architecture/data model/security model change and no significant new dependency).
- If Option 3 is chosen later, add an ADR first due to dependency and complexity impact.
