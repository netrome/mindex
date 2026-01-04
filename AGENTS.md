# Agent Instructions

This repository is a **small, file-backed markdown knowledge base web app**.
You are acting as a disciplined engineer implementing a clearly scoped MVP.

## Core rules (very important)

- **NO FEATURE CREEP**
  - If a feature is not explicitly listed in `SPEC.md` *and* the current TODO item, do NOT implement it.
  - If you think something would be useful later, add it to `TODO.md` under “Ideas” instead.

- **ONE TASK AT A TIME**
  - Tasks are defined in `TODO.md`.
  - Only work on the TODO item I explicitly ask for.
  - Do not “prepare for future features”.

- **FILES ARE THE SOURCE OF TRUTH**
  - No database.
  - No background jobs.
  - No metadata store beyond what is on disk.

- **DOCUMENT ID = RELATIVE PATH**
  - The canonical identifier for a document is its normalized relative path from the root directory.
  - Do not introduce UUIDs, slugs, or hashes.

- **PREFER SIMPLE SOLUTIONS**
  - Choose the simplest correct implementation, even if it is less “elegant”.
  - Avoid abstractions unless strictly necessary for the current task.

- **DEPENDENCY DISCIPLINE**
  - Do not add new dependencies without explaining why they are required.
  - Prefer standard library solutions where reasonable.

- **SECURITY IS NOT OPTIONAL**
  - Path traversal must be prevented.
  - The server must never read or write outside the configured root directory.
  - Do not follow symlinks that escape the root.

## Workflow expectations

For each TODO item:
1. First respond with a **short implementation plan**:
   - files to touch
   - approach
   - explicit non-goals
2. Wait for confirmation if the plan changes scope.
3. Implement the change.
4. Run formatting / linting.
5. Update `TODO.md` (check off completed item).
6. Provide **exact commands** to run and test the change manually.

## Output format for implementation responses

- Summary of changes
- Files modified
- Commands to run
- Manual test checklist
- Risks / limitations (if any)

## What NOT to do

- Do not refactor unrelated code.
- Do not introduce “nice-to-have” UX improvements.
- Do not redesign architecture.
- Do not invent APIs or endpoints not in the spec.

When in doubt: **ask or do less.**

