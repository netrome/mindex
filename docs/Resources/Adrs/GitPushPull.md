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

For push/pull credentials:
- SSH is supported via `ssh-agent` (use `SSH_AUTH_SOCK`) with non-interactive
  SSH (`BatchMode=yes`), and repo-scoped host key handling (explicit host key or
  a repo-local `known_hosts` file).
- Local path/file remotes are allowed only when the resolved path is within an
  explicit operator-configured allowlist; otherwise they are rejected.
- Restrict protocols to `ssh` and `file`, and validate local remotes against
  the allowlist before invoking git.

## Consequences
- Adds a runtime dependency on `git`.
- Requires rewriting existing git integration and tests.
- Increases responsibility for environment hardening (config isolation, no
  hooks, no credential helpers, no interactive prompts).
- Introduces a new configuration surface (allowlist of local remote roots) and
  requires an invariant update before implementation to permit bounded
  read/write outside the configured root.
- Keeps implementation minimal and aligns behavior with standard git usage.
- Supersedes the existing `docs/Resources/Adrs/GitIntegration.md` decision if
  accepted.
