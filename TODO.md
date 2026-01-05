# TODO

## Now (only one item should be worked on)

## Next

## Later

- [ ] Recently opened documents
- [ ] Pin documents
- [ ] Image upload support
- [ ] Offline caching (PWA)

## Ideas (parking lot — do NOT implement without moving to Now)

- List-only document view
- Checkbox toggle UI
- Backlinks
- Wiki-style links
- Git integration
- AI-assisted editing

## Done

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
