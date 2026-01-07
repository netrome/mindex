# Mindex

Mindex is a small, self-hosted web application for browsing, searching, and editing
Markdown documents stored directly on disk.

It’s designed to stay **minimal, simple, and hackable**.

## Features

- Browse a directory tree of `.md` documents
- Render Markdown documents for reading
- Full-text search across documents
- Edit and save Markdown from the browser
- Mobile-friendly UI
- Single binary. I.e. static assets (CSS/JS/icons) are embedded directly in the app.

## Non-goals

- No database
- No in-app user accounts - deployments can be protected with basic auth using a reverse proxy
- No real-time collaboration - git can be used for syncing changes accross devices.
- No requirements of external systems. The app should be useful directly when provided a directory of `.md` documents.

## Quick start

### Run locally

```bash
cargo run -- --root ./sample-root
```

That's it. The documents are now served at `http://localhost:3000`.
