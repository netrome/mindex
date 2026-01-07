# Agent Instructions (Mindex)

This repository is a **small, file-backed markdown knowledge base web app**.
Optimize for: simplicity, hackability, minimal dependencies, and long-term maintainability.

## Read these first
- README.md
- docs/Resources/INVARIANTS.md
- docs/Resources/ARCHITECTURE.md
- docs/Resources/Adrs/*
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
    create/update an ADR in `docs/Resources/Adrs/`.

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

### Design mode
Use when the task is exploratory/architectural or too large for a single PR.

- Default output is a single document: `docs/Projects/<project>.md`
- Do not modify code unless explicitly requested (design mode is typically docs-only)
- Consider max 2–3 options, recommend one
- End with a task breakdown of small PR-sized items, each with acceptance criteria
- If the design changes architecture/security/data model or adds a significant dependency:
  - draft/update an ADR in `docs/Resources/Adrs/`
  - call out the invariant changes explicitly

### Feature mode
- Smallest change that satisfies acceptance criteria.
- Avoid refactors unless required to implement the feature safely.

### Refactor/Engineering-excellence mode
- Must include:
  - clear motivation (what pain/risk it reduces)
  - a safety net (tests/snapshots/golden files)
  - a bounded scope (what is NOT being refactored)

## Workflow expectations (per task)

1. First respond with a plan:
   - approach, files to touch, non-goals, risks
2. Wait for confirmation/feedback, adjust the plan accordingly.
3. Implement exactly the plan.
4. Run:
   - `cargo fmt`
   - `cargo clippy --all-targets --all-features`
   - `cargo nextest run`
5. Update docs if behavior/usage changed (README/docs/*).
6. Update docs/Projects/TODO.md: check off the item, add follow-ups if needed.
  - If working on a sub-task in a project, check off the sub-task in the project document.
7. Provide:
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
