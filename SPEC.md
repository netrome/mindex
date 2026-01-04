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
