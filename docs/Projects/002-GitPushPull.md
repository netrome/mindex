# Git Push/Pull and Git Library Choice

## Status
Implemented

## Goal
Add minimal git push/pull support while preserving filesystem safety invariants and keeping the git integration maintainable.

## Context
- Current git integration uses `gix` and supports diff/status/commit only.
- `docs/Projects/TODO.md` includes "Git push/pull support".
- `gix` does not provide push/pull support today, so we need a new approach.
- Any change in git library choice affects architecture and dependencies (ADR required).

## Constraints and invariants
- Never read or write outside the configured root directory.
- Git integration remains optional; the app must function without git.
- No background jobs; push/pull should be user-triggered and synchronous.
- Dependency discipline: avoid major deps unless clearly justified.

## Minimal requirements
- Push and pull operate on the current branch and its configured upstream.
- No branch selection UI, no merge UI, and no interactive prompts.
- Fast-forward only for pull; surface errors when merge/conflicts would be required.
- Preserve existing diff/status/commit behavior.
- Support SSH remotes using `ssh-agent` when available.
- Allow SSH and local path/file remotes **only** when the remote path resolves
  within an explicit allowlist configured by the operator.

## Options (max 3)

### Option 1: Replace `gix` with `git2` (libgit2) for all git operations
Use `git2` for status/diff/commit/push/pull in-process.

Pros:
- Supports push/pull without shelling out.
- Single in-process implementation with one library.
- Easier to control behavior (no hooks, no external binaries).

Cons:
- Adds a C dependency (`libgit2`), increasing build complexity.
- Requires credential callbacks and explicit config handling.
- Migration cost: rewrite the existing `gix`-based logic and tests.

Notes:
- We must ensure only repo-local config is read (no system/global configs).
- Credentials should be provided explicitly per request or embedded in the remote URL.

### Option 2: Keep `gix` for local ops; shell out to `git` for push/pull
Retain `gix` for status/diff/commit and use `git` CLI for push/pull only.

Pros:
- Minimal changes to existing logic.
- Push/pull behavior matches standard git CLI.

Cons:
- Introduces a runtime dependency on `git`.
- Harder to guarantee "no reads outside root" due to global config, SSH, and credential helpers.
- Mixes two implementations, increasing operational complexity.

Notes:
- To satisfy invariants, we would need strict environment isolation (disable global/system config, hooks, and external helpers), and likely restrict to HTTPS.

### Option 3: Replace `gix` with `git` CLI for all git operations
Use `git` for status/diff/commit/push/pull via `std::process::Command`.

Pros:
- Minimal Rust-side git code.
- Best feature parity with git itself.

Cons:
- Runtime dependency on `git`.
- Must harden environment to avoid filesystem reads outside root.
- External process management, parsing, and error handling complexity.
Notes:
- Run with `-C <root>` and a locked-down environment (`GIT_CONFIG_GLOBAL=/dev/null`,
  `GIT_CONFIG_SYSTEM=/dev/null`, `GIT_CONFIG_NOSYSTEM=1`, `GIT_TERMINAL_PROMPT=0`).
- Disable hooks and external helpers (`-c core.hooksPath=/dev/null`,
  `-c credential.helper=`), and use `--no-verify` where supported.

## Recommendation
**Option 3: Replace `gix` with the `git` CLI.**

Rationale:
- Minimal Rust-side implementation and long-term maintenance burden.
- Uses the canonical git behavior users already expect for push/pull.
- Avoids the libgit2 dependency while still enabling the required features.

This decision would supersede the current `docs/Resources/Adrs/GitIntegration.md` choice of `gix`.

## Proposed behavior (if Option 3 is accepted)

### Git availability
- Keep existing root checks: git features are enabled only when `.git` is inside the configured root.
- Run all commands with `-C <root>` and avoid user-controlled paths.

### Push
- Push the current branch to its configured upstream.
- If no upstream is configured, return a clear error.

### Pull
- Fetch and fast-forward only to the upstream.
- If a merge would be required, return a clear error (no auto-merge).

### Credentials
- No credential storage in Mindex.
- Support SSH remotes via `ssh-agent` (use `SSH_AUTH_SOCK`).
- Optionally allow credentials embedded in the repo's remote URL.
- No interactive prompts; errors are surfaced to the user.

### Config isolation
- Read repo-local config only; disable global/system config paths.
- No hooks or external helpers.
- Disable interactive prompts (`GIT_TERMINAL_PROMPT=0`).
- Prefer `-c` overrides for `core.hooksPath` and `credential.helper` when invoking git.
- Restrict allowed protocols (`GIT_ALLOW_PROTOCOL=ssh:file`) and validate local
  path/file remotes resolve within the allowlist before invoking git.
- For SSH: use non-interactive mode (`BatchMode=yes`) and a repo-scoped
  `known_hosts` file (or an explicit host key provided in the request) to avoid
  reads from `~/.ssh`.

## Config
- `--git-allowed-remote-root <path>` (repeatable)
- `MINDEX_GIT_ALLOWED_REMOTE_ROOT` (comma-separated)

## Task breakdown (PR-sized)
- [x] **ADR draft + decision**
  - Acceptance: `docs/Resources/Adrs/GitPushPull.md` approved and marked Accepted or Rejected.
- [x] **Swap `gix` for `git` CLI with parity tests**
  - Acceptance: status/diff/commit features match current behavior; tests updated.
- [x] **Add push/pull endpoints + UI**
  - Acceptance: `/git/push` and `/git/pull` work for upstream-configured branches; errors are clear and non-interactive.
- [x] **Credential handling + config isolation**
  - Acceptance: push/pull works with SSH via ssh-agent and local remotes under the allowlist; no reads from global/system config; no external helpers.
- [x] **Docs + TODO update**
  - Acceptance: README/Resources updated; `docs/Projects/TODO.md` item checked off with any follow-ups listed.
