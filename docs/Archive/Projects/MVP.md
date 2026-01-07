# Mindex MVP
Goal: Implement the MVP of the application.

# Mindex (Extend your mind) – MVP Specification

## Vision

A small, self-hosted web application for browsing, searching, and editing
markdown documents stored directly on disk.

It is optimized for:
- simplicity
- mobile access
- long-term maintainability
- graceful degradation (no hard dependency on external services)

## MVP Goals

The MVP must support:

1. Browsing markdown documents in a directory tree
2. Viewing rendered markdown documents
3. Full-text search across documents
4. Editing and saving markdown documents from the browser
5. Safe, atomic file writes
6. Mobile-friendly UI

## Non-Goals (Explicitly Out of Scope for MVP)

- No tags or metadata system
- No backlinks or wiki graph
- No database
- No user accounts or permissions inside the app
- No real-time collaboration
- No LLM / AI features
- No version history UI (git may be used externally)
- No publishing / public sharing features

## Data Model

- A single configured root directory (e.g. `/srv/kb`)
- All documents are `.md` files
- The **document ID** is the relative path from the root directory
- Directories may be nested arbitrarily
- For MVP: document names are assumed to be plain ASCII without special characters
- For MVP: no special normalization of document IDs beyond rejecting traversal

Example document IDs:
- `inbox.md`
- `notes/rust.md`
- `ideas/side-project.md`

## Core Concepts

- **Document**: a markdown file on disk
- **Document ID**: relative path (string)
- **Root directory**: configured at startup

## Routes (Initial)

Exact shapes may evolve slightly, but scope must remain minimal.

- `GET /`
  - List documents (recursive)
- `GET /doc/{path}`
  - Render markdown document
- `GET /edit/{path}`
  - Show markdown editor
- `POST /edit/{path}`
  - Save markdown document
- `GET /search?q=...`
  - Full-text search results

## Editing Semantics

- Edits overwrite the full file contents
- For MVP, favor the simplest correct write flow; atomic writes are optional
  - If using atomic writes, temp file + rename is sufficient (no directory fsync required)

## Rendering Semantics (MVP)

- Raw HTML in markdown is allowed (no sanitization)

## Filesystem Safety (MVP)

- Prevent path traversal (never read or write outside the root directory)
- Symlinks are ignored for simplicity; if followed, they must resolve within the root

## Deployment Assumptions

- Single-user or trusted users
- Authentication handled by reverse proxy (e.g. basic auth)
- App runs as a single binary (systemd-friendly)

## Future Extensions (Not MVP)

- Offline-first PWA support
- Backlinks / wiki links
- File rename / move via UI
- Optional LLM helpers

# Old TODO.md

## Now (only one item should be worked on)

## Next

## Later

- [ ] Add MIT license
- [ ] Simple README.md
- [ ] Push notifications
- [ ] Chat boxes
- [ ] Git integration
- [ ] TODO lists
- [ ] Math notation

## Ideas (parking lot — do NOT implement without moving to Now)

- List-only document view
- Checkbox toggle UI
- AI-assisted editing

## Done

- [x] Support multiple instances
  - Use case: Hosting one shared instance at one domain and a personal instance at another domain.
  - Allow app name in manifest.json to be configured, defaulting to "Mindex".
  - Allow icons to be configured dynamically, falling back to the existing ones.
  - If anything else also is good hygiene.

- [x] PWA support
  - Add the minimal necessary things to support turn this into an PWA.

- [x] Dark mode
  - Use dark/light mode from system preferences.
  - Add button to toggle dark/light mode.

- [x] Full-text search
  - Simple implementation acceptable (e.g. ripgrep)
  - Return matching paths + snippets

- [x] Render markdown document
  - Convert markdown → HTML
  - Safe handling of missing files

- [x] Project skeleton
  - Goal: minimal runnable server
  - Acceptance criteria:
    - `cargo run` starts a server
    - `GET /health` returns HTTP 200 and plain text `ok`
  - Out of scope:
    - no markdown rendering
    - no filesystem access

- [x] Configure root directory and list documents
  - List all `.md` files recursively
  - Display paths as links
  - Prevent path traversal

- [x] Render relative .md links as /doc/ links

- [x] Enhance sample markdown content
  - Add lists, tables, links and other markdown examples

- [x] Render markdown tables

- [x] Edit document
  - GET shows textarea with current contents
  - POST saves atomically

- [x] Basic mobile-friendly layout
  - Responsive CSS
  - No JS frameworks
  - Askama templating for maintainable HTML
