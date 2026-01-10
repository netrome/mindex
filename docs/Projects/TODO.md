# TODO

## In progress
- [ ] auth: Introduce authentication [./Authentication.md]
  - [x] Config plumbing
  - [x] Refactor: extract directive parsing/registries from push
  - [x] `/user` parsing update (require `password_hash`)
  - [x] Auth middleware + JWT
  - [x] Login/logout handlers + cookie issuance

## To do
- [ ] refactor: Break out endpoints from `app.rs` to keep the main file focused on only the top-level functionality.
- [ ] Math notation
- [ ] Chat boxes
- [ ] Git integration
- [ ] TODO lists

## Recently done
- [x] auth: accept ADR and config plumbing
- [x] refactor: Update tests to use //given //when //then sections
- [x] refactor: Move CLI subcommand handling out of `main.rs`
- [x] refactor: Centralize VAPID key generation helpers for CLI init
- [x] feat: VAPID credential generation
  - Currently it's inconvenient to have to go to a third party generator to get VAPID credentials. It would be better if the application could generate them.
  - Acceptance criteria: Add a `mindex init` subcommand that generates VAPID credentials.
- [x] feat: TODO lists
  - When rendering lists with the `- [ ] <text>` structure, render checkboxes and allow ticking off items directly from the view page.
- [x] feat: Create new document
  - Allow creating new documents from the UI.
- [x] Module layout refactor [./ModuleLayoutRefactor.md]
- [x] Push notifications [./PushNotifications.md]
  - [x] Directive parser + registry loader
  - [x] Scheduler + delivery
  - [x] Minimal subscription UI page
  - [x] Subscription nav link + test helper
  - [x] Harden scheduler delay conversion + extract doc helpers
  - [x] Refactor: extract push domain types to break the `ports`/`push` dependency cycle
  - [x] Feature: store scheduler handles and add a debug view for scheduled notifications
  - [x] Refresh push registries + scheduler on in-app save

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
