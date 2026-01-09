# TODO

## In progress
- [ ] Push notifications [./PushNotifications.md]
  - [x] Directive parser + registry loader
  - [x] Scheduler + delivery
  - [x] Minimal subscription UI page
  - [x] Subscription nav link + test helper
  - [x] Harden scheduler delay conversion + extract doc helpers
  - [x] Refactor: extract push domain types to break the `ports`/`push` dependency cycle

## To do
- [ ] Module layout refactor [./ModuleLayoutRefactor.md]
- [ ] refactor: Update tests to use //given //when //then sections
- [ ] refactor: Break out endpoints from `app.rs` to keep the main file focused on only the top-level functionality.
- [ ] Math notation
- [ ] Chat boxes
- [ ] Git integration
- [ ] TODO lists

## Recently done
- [x] Push notifications design doc
- [x] Add MIT license
- [x] Add a minimal ADR template in docs/Resources/Adrs
- [x] Align ADR folder path in AGENTS.md with docs/Resources/Adrs
- [x] Simple README.md
- [x] Refactor: extract directive parsing into `push/directives.rs`
- [x] Refactor: extract registry loading into `push/registry.rs`
- [x] Refactor: extract scheduling into `push/scheduler.rs`
- [x] Refactor: centralize VAPID validation/construction
- [x] Refactor: make directive parsing return warnings instead of `eprintln!`
