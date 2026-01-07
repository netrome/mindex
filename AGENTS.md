# Agent Instructions (Mindex)

This repository is a **small, file-backed markdown knowledge base web app**.
Optimize for: simplicity, hackability, minimal dependencies, and long-term maintainability.

## Read these first
- README.md
- docs/Resources/INVARIANTS.md
- docs/Resources/ARCHITECTURE.md
- docs/Resources/adr/*
- docs/Projects/TODO.md (current task only)

## Core rules

- **NO FEATURE CREEP**
  - Implement only the explicitly requested task from `docs/Projects/TODO.md`.
  - `docs/Resources/*` defines constraints, not a feature backlog.

- **KEEP INVARIANTS TRUE**
  - Do not violate docs/Resources/INVARIANTS.md.
  - If you believe an invariant must change, propose an ADR instead of changing code.

- **DECISIONS REQUIRE ADRs**
  - If a change affects architecture, security model, data model, or introduces a significant dependency,
    create/update an ADR in `docs/adr/`.

- **FILES ARE THE SOURCE OF TRUTH**
  - No database, no background jobs unless explicitly approved via ADR.

- **DOCUMENT ID = RELATIVE PATH**
  - The canonical identifier is the normalized relative path from root.
  - No UUIDs/slugs/hashes as primary identifiers.

- **DEPENDENCY DISCIPLINE**
  - Avoid adding dependencies. If needed, justify why (and why stdlib isn’t enough).

- **SECURITY IS NOT OPTIONAL**
  - Prevent path traversal.
  - Never read/write outside configured root.
  - Be explicit about symlink policy (per invariants).

## Documentation rules
- Projects:
  - Active tasks live in `docs/Projects/`.
  - Completed work should be checked off or moved to Archive if it’s no longer relevant.

- Resources:
  - Stable knowledge and constraints live in `docs/Resources/`.
  - Changes here should be rare and deliberate.

- Archive:
  - Historical documents only.
  - Never treat Archive as current requirements.

## Work modes

### Feature mode
- Smallest change that satisfies acceptance criteria.
- Avoid refactors unless required to implement the feature safely.

### Refactor/Engineering-excellence mode
- Must include:
  - clear motivation (what pain/risk it reduces)
  - a safety net (tests/snapshots/golden files)
  - a bounded scope (what is NOT being refactored)

## Workflow expectations (per task)

1. Start with a short plan:
   - approach, files to touch, non-goals, risks
2. Implement exactly the plan.
3. Run:
   - `cargo fmt`
   - `cargo clippy --all-targets --all-features`
   - `cargo nextest run`
4. Update docs if behavior/usage changed (README/docs/*).
5. Update docs/Projects/TODO.md: check off the item, add follow-ups if needed.
6. Provide:
   - Summary of changes
   - Tests added/updated
   - Commands to run
   - Manual test checklist
   - Risks/limitations

## What NOT to do
- No drive-by refactors.
- No new architecture without ADR.
- No adding “nice UX” unless requested.
- No “future-proofing” unless part of the task.
