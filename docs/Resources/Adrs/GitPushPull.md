# Git Push/Pull via git CLI

## Status
Accepted

## Context
Mindex's git integration currently uses `gix` for diff/status/commit. The product
roadmap now includes git push/pull support, but `gix` does not provide push/pull
capabilities. Adding push/pull changes the git subsystem and introduces new
requirements around credentials and config handling, so a new decision is
needed.

## Decision
Use the `git` CLI for all git operations (status/diff/commit/push/pull),
replacing the current `gix` implementation. Run git with a locked-down
environment to read repo-local config only, disable hooks and external helpers,
and avoid interactive prompts (e.g., `-C <root>`, `GIT_CONFIG_NOSYSTEM=1`,
`GIT_CONFIG_GLOBAL=/dev/null`, `GIT_TERMINAL_PROMPT=0`).

## Consequences
- Adds a runtime dependency on `git`.
- Requires rewriting existing git integration and tests.
- Increases responsibility for environment hardening (config isolation, no
  hooks, no credential helpers, no interactive prompts).
- Keeps implementation minimal and aligns behavior with standard git usage.
- Supersedes the existing `docs/Resources/Adrs/GitIntegration.md` decision if
  accepted.
