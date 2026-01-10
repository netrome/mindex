# Git Integration (Design)

## Status
Proposed

## Context
Mindex is increasingly used from mobile, and the most pressing feature request is
basic git integration: view a plain `git diff` and create commits from the UI.
This must stay minimal and align with filesystem safety invariants (no reads or
writes outside the configured root).

## Goals
- Show a plain working-tree diff for changes under the configured root.
- Provide a minimal commit form to commit changes under the root.
- Degrade gracefully when git is unavailable or the root is not a git repo.
- Avoid new dependencies; keep the implementation hackable.

## Non-goals
- Branch management, history browsing, or pushing/pulling remotes.
- Per-file or per-hunk staging, or interactive diff editing.
- Signed commits, amend, rebase, stash, or merge conflict resolution.
- Auto-committing or background git operations.

## Constraints and invariants
- Never read or write outside the configured root directory.
- Avoid adding dependencies unless strongly justified.
- No background jobs unless explicitly approved.
- Document ID remains the relative path from root.

## Options (max 3)

### Option A: Shell out to `git`
Run `git` via `std::process::Command` with fixed args and a strict availability
check. Only enable integration when a `.git` directory exists inside the root,
so all git reads/writes stay inside the configured root.

Pros: zero new dependencies, matches minimalism, easy to reason about.
Cons: requires `git` binary; uses external process.

### Option B: Use a Rust git library (`gix`) (recommended)
Embed a git library and implement diff/commit programmatically. Use `gix`
(pure Rust) to avoid runtime dependencies and to improve testability.

Pros: no external binary; deterministic tests; works in minimal runtime envs.
Cons: heavier dependency footprint; more code; higher maintenance burden.
Notes:
- `git2` depends on libgit2 and can introduce runtime/system dependency friction.
- `gix` is pure Rust but is a larger crate; still acceptable for a key feature.

### Option C: Read-only diff (no commits)
Expose only a diff view; leave committing to the CLI.

Pros: simpler and lower risk.
Cons: does not satisfy the primary request.

## Recommendation
Option B. The improved testability and removal of runtime git dependency are
worth the added implementation surface area for this feature.

## Proposed UX
Add a `/git` page (linked from the nav when available) with:
- Status line: “Clean” or “N files changed”.
- A `<pre>` block with `git diff` output (plain, no color).
- A minimal commit form:
  - multi-line commit message textarea
  - “Commit changes” button
- When git is unavailable: hide the nav link and show no git UI.

## Git availability rules
Git integration is enabled only if:
1) `git` is available on the host, and
2) the configured root contains a `.git` directory (or `.git` file that resolves
   to a path inside the root).

If root is a subdirectory of a repo (i.e., `.git` is above root), git features
are disabled to preserve the “no reads outside root” invariant.

## Library behavior (Option B)

### Diff
Use `gix` to compute a worktree diff from `HEAD` (or an empty tree if no commits).
Render the raw diff as plain text. If empty, show “No changes.”

### Status summary
Use `gix` status to count changed files and show “N files changed” vs “Clean”.

### Commit
Steps:
1) Validate commit message (trimmed, non-empty).
2) Stage all changes under root (index add).
3) Create a commit with the given message and current index.
4) Show success (commit SHA) or a readable error.

## Security considerations
- No user-controlled paths are passed to git.
- Commit message is the only user input; it is passed directly to the library.
- Root is validated as a git repo with `.git` located within the root directory.
- If `.git` is a file/symlink, resolve it and ensure it remains inside root.

## Open questions
- None.

## Task breakdown (PR-sized)
1) **Doc-only: finalize design + ADR.**
   - Acceptance: `docs/Projects/GitIntegration.md` and `docs/Resources/Adrs/GitIntegration.md` approved.
2) **Git availability helper + tests.**
   - Acceptance: detects git presence and `.git` inside root; disables when `.git` is outside root.
3) **Diff/status handler + template.**
   - Acceptance: `/git` renders status + diff or “Git not available.”
4) **Commit handler + form.**
   - Acceptance: commits staged changes under root; returns success or error; no interactive prompts.
5) **Docs update (README or Resources).**
   - Acceptance: mention git integration behavior and limitations (repo root requirement).
