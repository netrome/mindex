# Module Layout: Hybrid Categories with Feature Submodules

## Status
- Accepted

## Context
Mindex uses a ports/adapters approach for some features (push notifications),
with shared domain types and adapter interfaces. The current layout mixes these
abstractions with feature logic, and recent work highlighted a potential module
dependency cycle. We need a consistent layout that keeps dependency direction
clear, avoids cycles, and remains easy to navigate in a small codebase.

## Decision
Adopt a hybrid module layout:
- Use top-level categories for shared abstractions (`ports/`, `types/`).
- Scope those categories by feature via submodules (e.g. `ports/push.rs`,
  `types/push.rs`).
- Allow generic, cross-cutting ports to live directly under `ports/` (e.g.
  `ports/time.rs`).
- Keep feature orchestration in feature modules (e.g. `push.rs`).

## Consequences
- Clearer dependency direction and fewer cycles between ports and features.
- Some feature code will be split across top-level categories and a feature
  module, but the structure stays shallow and predictable.
- Requires a small refactor of module declarations and imports.
