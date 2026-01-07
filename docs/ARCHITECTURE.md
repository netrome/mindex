# Architecture (Mindex)

## Overview

Mindex is a single-process web server that:
1) reads markdown files from a configured root directory
2) renders and serves them over HTTP
3) supports search and in-browser editing
4) serves a small set of static assets for UI

## Request flow (typical)

- Request arrives at router (handlers in `src/app.rs`)
- Handler validates/normalizes the requested document path
- Handler reads from disk (under root) and renders:
  - Markdown → HTML for view routes
  - Templates → HTML pages for browse/edit/search
- Static assets are served via `src/assets.rs`

## Modules

- `src/main.rs`
  - Binary entrypoint; initializes config + state; starts server

- `src/lib.rs`
  - Library exports for reusability/testing

- `src/config.rs`
  - Defines configuration (CLI/env) and validation

- `src/state.rs`
  - Shared application state (configured root directory, caches/indexes if any)

- `src/app.rs`
  - HTTP routing and request handlers

- `src/templates.rs`
  - HTML templates and rendering helpers (server-side)

- `src/assets.rs`
  - Serves or embeds static UI assets from `static/`

## Key design choices

- Filesystem-backed storage (no DB)
- Document identity = relative path from root
- Reverse proxy handles TLS/auth in typical deployments
