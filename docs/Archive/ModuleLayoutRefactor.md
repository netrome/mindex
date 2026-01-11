# Module Layout Refactor (Design)

## Status
Done

## Context
Mindex recently introduced push notifications and supporting abstractions (ports,
domain types). The current layout mixes feature logic and shared abstractions,
and there is an open question whether the top-level module organization should
be category-based (ports/types/services) or feature-based (push/, documents/, ...).
We want a layout that keeps dependency direction clear, avoids cycles, and stays
simple/hackable.

## Goals
- Clarify module layout conventions for shared abstractions vs feature logic.
- Reduce the chance of dependency cycles by making boundaries explicit.
- Keep navigation easy for a small codebase.

## Non-goals
- No behavior changes.
- No new dependencies.
- No large refactors beyond module moves and import updates.

## Options

### Option A: Category-based top-level (ports/, types/, services/)
Place shared abstractions in top-level categories, e.g.:
- `src/ports/` for traits and interfaces
- `src/types/` for domain types
- `src/push.rs` for push orchestration logic

Pros: clear layering; easy to find boundaries; reduces cycles.
Cons: feature logic is split across multiple folders.

### Option B: Feature-based top-level (push/, documents/, ...)
Group everything by feature, e.g.:
- `src/push/` contains `types.rs`, `ports.rs`, `scheduler.rs`, etc.

Pros: strong cohesion; easy to reason about a feature in one place.
Cons: shared abstractions can be harder to discover; cross-feature reuse can
become ad hoc and may reintroduce cycles.

### Option C: Hybrid (category-based with feature submodules)
Keep top-level categories, but scope them by feature inside:
- `src/ports/push.rs`, `src/ports/time.rs`
- `src/types/push.rs`
- `src/push.rs` for orchestration
Generic ports live directly under `src/ports/` when they are not feature-specific.

Pros: explicit boundaries + reasonable cohesion; avoids cycles.
Cons: still some scattering, but less than Option A.

## Recommendation
Option C (Hybrid). It keeps the ports/adapters boundaries clear without creating
deep folder hierarchies, and it scales as additional features arrive.

## Safety net
- Run `cargo test` and `cargo clippy` after module moves.
- If the refactor touches behavior, add targeted tests before making changes.

## Task breakdown (PR-sized)
1. [x] **Adopt module layout decision (docs-only).**
   Acceptance: ADR accepted; `docs/Resources/ARCHITECTURE.md` mentions the
   chosen layout and boundaries.
2. [x] **Move push domain types to `src/types/push.rs`.**
   Acceptance: no behavior changes; tests pass; imports updated.
3. [x] **Split ports into `src/ports/push.rs` and `src/ports/time.rs`.**
   Acceptance: `PushSender` lives in `ports/push.rs`; `TimeProvider` is a
   shared port in `ports/time.rs`; module paths updated; no dependency cycles.
4. [x] **Add module wiring in `src/lib.rs` (and any new `mod.rs`).**
   Acceptance: module declarations match the new layout; no unused modules.
5. [x] **Update imports and clean up old paths.**
   Acceptance: no `push_types` or `ports.rs` references remain; build succeeds.
6. [x] **Run safety net checks.**
   Acceptance: `cargo test` and `cargo clippy` pass.
