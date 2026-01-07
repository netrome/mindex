# Push Notifications: Document-Backed Metadata and In-Process Scheduler

## Status
- Accepted

## Context
Mindex stores all knowledge as markdown files on disk and avoids external persisted
state. Push notifications require a mapping from users to subscriptions and a way to
schedule delivery. The current security model expects authentication to be handled
outside the app. Any solution should preserve document identity and root sandboxing.

## Decision
- Store user definitions and notification requests as directive blocks inside
  markdown documents.
- Store push subscriptions as directive blocks in any markdown document (same as
  users and notifications), using TOML blocks for consistency.
- Add an in-process scheduler task (tokio) to deliver notifications.
- Provide VAPID keys via CLI flags (`--vapid-private-key`, `--vapid-public-key`,
  `--vapid-subject`) or environment variables (`MINDEX_VAPID_PRIVATE_KEY`,
  `MINDEX_VAPID_PUBLIC_KEY`, `MINDEX_VAPID_SUBJECT`). Keys are operational config,
  not content, and do not belong in documents.
- If VAPID keys are not configured, push features degrade gracefully: the server
  starts normally, directive blocks are still parsed, but the scheduler does not
  run and the subscription UI indicates push is unavailable.
- Expose `GET /api/push/public-key` so clients can obtain the VAPID public key
  for subscription.
- Do not introduce in-app authentication; user registry is metadata only.

## Consequences
- Adds dependencies:
  - `web-push` for push message delivery.
  - `toml` for parsing directive blocks.
  - `time` for RFC3339 datetime parsing (stdlib has no datetime parsing; `time`
    is minimal and sufficient).
- Introduces a background scheduling task, which is explicitly approved by this
  ADR to satisfy repository rules.
- No changes to document identity or root sandboxing invariants.
- Duplicate sends after restart are possible until a sent-state in documents is
  implemented (accepted trade-off for v1).
