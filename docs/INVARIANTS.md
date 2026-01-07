# Invariants (Mindex)

This document defines constraints that should remain true over time.
Changes here should be deliberate and usually come with an ADR.

## Data model

- Markdown files on disk are the source of truth.
- Documents are `.md` files under a configured root directory.
- The **document ID** is the normalized relative path from the root directory.

## Filesystem safety

- The server must never read or write outside the configured root directory.
- Path traversal must be prevented (`..`, absolute paths, etc.).
- Symlink policy:
  - Either do not follow symlinks, or if symlinks are followed they must resolve within root.
  - Whatever the implementation is, it must be consistent and tested.

## Editing semantics

- Edits overwrite the full file contents.
- Writes should be safe against partial writes (prefer atomic write via temp + rename).
- Concurrency/conflicts are best-effort (single-user assumption is fine), but the behavior should be well-defined.

## Security model

- Mindex assumes a trusted environment and is typically deployed behind a reverse proxy.
- Authentication/authorization is not handled inside the app unless explicitly added via ADR.
- If raw HTML is allowed in Markdown rendering, this must be called out clearly as part of the threat model.

## Dependencies

- Keep the dependency tree small.
- Prefer standard library where reasonable.
- Adding major dependencies (DB, search engine, auth framework) should be decided via ADR.

## Product philosophy

- The app should remain minimal and hackable.
- Avoid “platform” behavior: plugin systems, premature abstractions, complex configuration.