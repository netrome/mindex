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
- The **document ID** is the normalized relative path from the root directory
- Directories may be nested arbitrarily

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
- Writes must be atomic:
  - write to temp file
  - fsync
  - rename

## Deployment Assumptions

- Single-user or trusted users
- Authentication handled by reverse proxy (e.g. basic auth)
- App runs as a single binary (systemd-friendly)

## Future Extensions (Not MVP)

- Offline-first PWA support
- Backlinks / wiki links
- File rename / move via UI
- Optional LLM helpers
