# Git Integration via Embedded Git Library and Root-Scoped Operations

## Status
Superseded by GitPushPull.md

## Context
Users want to view diffs and commit changes from Mindex (especially on mobile).
The app must remain minimal, preserve filesystem invariants (no reads or writes
outside the configured root directory), and ideally avoid runtime dependencies
on external binaries.

Git operations typically require access to the `.git` directory, so enabling git
when the root is a subdirectory of a repo would cause reads outside root. This
must be avoided unless the invariant changes (which it should not).

## Decision
- Use the `gix` crate to implement git operations in-process.
- Enable git features only when `.git` is inside the configured root (or a `.git`
  file that resolves to a path inside root). If `.git` is above root, disable.
- Support a minimal feature set:
  - Diff against `HEAD` (plain output)
  - Status summary (count changed files)
  - Commit staged changes with a multi-line message
- Degrade gracefully when git is unavailable or the root is not a repo: show a
  clear “Git not available” message and hide git navigation.

## Consequences
- Adds a git library dependency (`gix`) to avoid runtime reliance on a `git`
  binary.
- Git integration only works when the configured root is the repo root.
- Commit actions may fail if hooks or signing require interaction; the UI will
  surface these errors without blocking the server.
- Filesystem safety invariants remain intact; no reads/writes outside root.
