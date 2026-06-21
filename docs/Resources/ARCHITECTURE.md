# Architecture (Mindex)

## Overview

Mindex is a single-process web server that:
1) reads markdown files from a configured root directory
2) renders and serves them over HTTP
3) supports search and in-browser editing
4) serves a small set of static assets for UI

## Request flow (typical)

- Request arrives at router (routes defined in `src/app.rs`)
- Auth middleware checks credentials when auth is enabled
- Handler validates/normalizes the requested document path
- Handler calls domain functions and renders:
  - Markdown → HTML for view routes
  - Templates → HTML pages for browse/edit/search
- Static assets are served via `src/assets.rs`

## Modules

### Entrypoints

- `src/main.rs` — binary entrypoint; parses CLI args, initializes config, starts server
- `src/lib.rs` — library root; declares modules, exports `serve()`
- `src/cli.rs` — CLI argument parsing and validation

### Domain logic

- `src/documents.rs` — document loading/creation, shared utilities, re-exports
  - `src/documents/paths.rs` — path resolution, directory listing, file kind detection
  - `src/documents/rendering.rs` — markdown-to-HTML rendering, link rewriting, heading IDs
  - `src/documents/editing.rs` — block scanning and reordering
  - `src/documents/tasks.rs` — task list toggle/add, mention extraction
  - `src/documents/search.rs` — full-text search
- `src/directives.rs` — parses user/notification directive blocks from markdown files
- `src/uploads.rs` — image upload storage and path resolution
- `src/git.rs` — git status, commit, and remote operations
- `src/auth.rs` — password hashing and auth key/token generation
- `src/math.rs` — LaTeX-to-MathML rendering for inline/display math
- `src/fs.rs` — shared filesystem utilities (atomic writes, safe directory creation)

### HTTP layer

- `src/app.rs` — router setup, middleware, and route definitions
- `src/app/auth.rs` — login/logout handlers
- `src/app/documents.rs` — document view, edit, search, reorder handlers
- `src/app/git.rs` — git status/commit/pull/push handlers
- `src/app/push.rs` — push notification debug/subscription handlers
- `src/app/uploads.rs` — image upload and file serving handlers

### Infrastructure

- `src/config.rs` — application configuration (CLI/env) and validation
- `src/state.rs` — shared application state (`AppState`)
- `src/templates.rs` — HTML templates and rendering helpers (server-side)
- `src/assets.rs` — serves or embeds static UI assets from `static/`
- `src/adapters.rs` — concrete implementations of port traits (e.g. `WebPushSender`)

### Push notifications

- `src/push.rs` — push notification dispatch (mention notifications)
- `src/push/scheduler.rs` — scheduled notification delivery
- `src/push/vapid.rs` — VAPID key generation for web push

### Shared abstractions

- `src/ports/` — trait interfaces for external dependencies
  - `src/ports/push.rs` — `PushSender` trait
  - `src/ports/time.rs` — `Clock` trait
- `src/types/` — domain types
  - `src/types/directives.rs` — directive registry types (users, subscriptions, notifications)
  - `src/types/push.rs` — VAPID configuration type

### Test support

- `src/test_support.rs` — shared test utilities (`create_temp_root`); compiled only under `#[cfg(test)]`
- `assets/**/*.test.mjs` — Node.js built-in test runner unit tests for pure browser-asset logic (run with `node --test assets/`); no third-party dependencies, not part of the Rust build or the shipped binary

## Key design choices

- Filesystem-backed storage (no DB)
- Document identity = relative path from root
- Reverse proxy handles TLS/auth in typical deployments; in-app auth is optional when configured
