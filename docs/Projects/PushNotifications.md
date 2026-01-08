# Push Notifications (Design)

## Status
Accepted

## Context
Mindex is a file-backed markdown knowledge base. Push notifications should follow the
same philosophy: no database, no external persisted state, and minimal dependencies.
Users and notification data live inside markdown files.

## Goals
- Store push subscriptions and notification definitions in markdown documents.
- Allow users to be defined inside the wiki (not for authentication, only for routing).
- Schedule one-shot notifications from document content.
- Keep implementation minimal and consistent with existing invariants.

## Non-goals
- In-app authentication/authorization or multi-tenant user accounts.
- Guaranteed delivery, retries, or at-least-once semantics.
- Complex scheduling rules (recurrence, cron syntax) in v1.

## Constraints and invariants
- Documents on disk are the source of truth; doc ID is relative path from root.
- No reads or writes outside the configured root.
- No background jobs unless explicitly approved via ADR.
- Dependency additions must be justified.

## Options (max 3)

### Option A: Directive blocks in any doc
- Use explicit directive lines (e.g. `/user`, `/notify`, `/subscription`) followed by
  fenced TOML code blocks.
- All directive types can appear in any document.
- Parse all docs at startup; rescan on in-app saves.
Pros: matches desired workflow, minimal UI changes, data stays in docs.
Cons: no durable "sent" state unless we write back into docs.

### Option B: Dedicated notifications document with mutable status
- All `/notify` blocks live in a single `notifications.md` file.
- Server updates `status = "sent"` (or `sent_at`) after delivery.
Pros: avoids duplicate sends after restart, simpler updates.
Cons: loses "any doc" flexibility; server mutates a user document.

### Option C: Inline directives with front-matter
- Use YAML/TOML front-matter in docs for users and notifications.
Pros: familiar to some users.
Cons: more parsing complexity; increases risk of accidental collisions.

## Decision
Start with Option A for minimal surface area and to align with the requested workflow.
Accept the trade-off that notifications may re-send after a restart until we add
explicit sent-state in docs (a later, opt-in enhancement).

## Configuration

Push notifications require VAPID keys. These are provided via environment variables
or CLI flags — not stored in documents (keys are operational config, not content).

### Required settings

| Setting | CLI | Env | Description |
|---------|-----|-----|-------------|
| Private key | `--vapid-private-key` | `MINDEX_VAPID_PRIVATE_KEY` | Base64-URL encoded EC P-256 private key |
| Public key | `--vapid-public-key` | `MINDEX_VAPID_PUBLIC_KEY` | Base64-URL encoded EC P-256 public key |
| Subject | `--vapid-subject` | `MINDEX_VAPID_SUBJECT` | Contact URI (`mailto:you@example.com` or `https://example.com`) |

### Key generation

Users can generate a key pair with:

```bash
openssl ecparam -genkey -name prime256v1 -noout -out vapid_private.pem
openssl ec -in vapid_private.pem -pubout -out vapid_public.pem
# Then base64url-encode for config
```

Or use an online generator / the `web-push` CLI if available.

### Graceful degradation

If VAPID keys are not configured:
- The server starts normally (no error).
- `/notify` and `/subscription` blocks are still parsed (allows editing docs without full setup).
- The scheduler does not start.
- The subscription UI page shows a message: "Push notifications are not configured."
- `GET /api/push/public-key` returns 503 with an explanation.

This allows running Mindex without push support and enabling it later without code changes.

## Proposed data model (Option A)

### User blocks (in any markdown file)
Directive line followed by a TOML code block:

````
/user
```toml
name = "marten"
display_name = "Marten"
```
````

Rules:
- `name` is the unique username. Later duplicates are ignored with a warning.
- User registry is informational only (no auth).

### Subscription blocks (in any markdown file)
Store a subscription per device. Multiple blocks for the same user are allowed.

````
/subscription
```toml
user = "marten"
endpoint = "https://push.example/..."
p256dh = "base64-url-encoded"
auth = "base64-url-encoded"
```
````

Notes:
- TOML avoids JSON escaping and keeps parsing uniform.
- These values map directly to `web_push::SubscriptionInfo`.

### Notification blocks (in any markdown file)

````
/notify
```toml
to = ["marten"]
at = "2025-01-12T09:30:00Z"
message = "Check the daily log."
```
````

Rules:
- `to` can be a string or list of strings (users).
- `at` uses RFC3339. If a dependency is rejected, use unix epoch seconds.
- Notifications in the past fire immediately on load.

## Parsing and loading
- Reuse `collect_markdown_paths` to respect current symlink policy.
- Scan documents line-by-line for:
  1) a directive line (`/user`, `/subscription`, `/notify`) on its own line
  2) the next fenced code block with language `toml`
- Ignore malformed blocks and log warnings with `doc_id` and line number.
- Build in-memory registries:
  - `users: HashMap<String, User>`
  - `subscriptions: HashMap<String, Vec<SubscriptionInfo>>`
  - `notifications_by_doc: HashMap<DocId, HashSet<ContentHash>>`
  - `scheduled_tasks: HashMap<ContentHash, TaskHandle>`

### Debug endpoint (dev/test)
- `GET /api/debug/push/registry` returns the current directive registries as JSON.
- Intended for local testing; the response includes subscription details.

## Scheduling and execution
- Create an in-process scheduler task at startup (tokio).
- For each notification, compute delay and `sleep_until`.
- Send via `web-push` to all subscriptions for each recipient.
- On in-app save (`document_save`), re-parse that document and refresh affected
  notifications/users. External edits require a restart or a manual reload endpoint.

## Notification identity and bookkeeping

**Identity:** A notification is identified by the hash of its TOML block content
(trimmed). Two blocks with identical content have the same identity.

**Bookkeeping:** Notifications are tracked per-document in memory:
```
notifications_by_doc: HashMap<DocId, HashSet<ContentHash>>
scheduled_tasks: HashMap<ContentHash, TaskHandle>
```

**On document re-parse:**
1. Compute `new_hashes` from the parsed `/notify` blocks.
2. Retrieve `old_hashes` from `notifications_by_doc`.
3. For each hash in `old_hashes - new_hashes`: cancel the scheduled task.
4. For each hash in `new_hashes - old_hashes`: schedule a new task.
5. Update `notifications_by_doc` with `new_hashes`.

**Duplicate blocks across documents:** Each document's notifications are tracked
independently. If the same block appears in two documents, two tasks are scheduled.
This is acceptable for v1 — a well-maintained wiki shouldn't duplicate this kind of
information anyway, so duplicates serve as a gentle reminder to clean up.

## Client subscription flow

### New API endpoint

`GET /api/push/public-key`

- Returns: `{ "publicKey": "<base64url>" }` (200 OK)
- If not configured: `{ "error": "Push notifications not configured" }` (503)

### Subscription page behavior

1. Check if push is supported (`'PushManager' in window`).
2. Fetch `/api/push/public-key`.
   - If 503 → show "Push not configured on this server".
3. Request notification permission (`Notification.requestPermission()`).
   - If denied → show "Permission denied" message.
4. Get or create subscription via service worker:
   ```js
   const subscription = await registration.pushManager.subscribe({
     userVisibleOnly: true,
     applicationServerKey: urlBase64ToUint8Array(publicKey)
   });
   ```
5. Extract `endpoint`, `keys.p256dh`, `keys.auth` from subscription.
6. Render a `/subscription` block for the user to copy:
   ```
   /subscription
   ```toml
   user = "your-username"
   endpoint = "https://fcm.googleapis.com/..."
   p256dh = "BNcRd..."
   auth = "tBHI..."
   ```
   ```
7. User pastes this into any document (e.g., a personal settings page) and saves.

### Why not auto-save?

Auto-writing the subscription would require:
- Knowing which user is subscribing (no auth → no identity).
- Server-side mutation of `notification_endpoints.md`.

Manual paste keeps the flow auth-free and mutation-free, consistent with Mindex philosophy.

## Security considerations
- Subscriptions are sensitive; treat `notification_endpoints.md` as private.
- No auth changes are implied; rely on reverse proxy if access control is needed.
- Ensure all file reads/writes go through existing path validation helpers.

## Invariant impact
- No change to document identity or root sandboxing.
- Background scheduling requires an ADR approval.
- No auth model changes; user registry is non-authoritative metadata.

## Open questions
- Should we allow opt-in "sent" markers in docs to avoid re-sends on restart?
  - Decision: We'll start without this and accept the risk of re-sending or missing notifications.
- Should subscriptions be per-user or per-document (e.g., notify watchers of a doc)?
  - Decision: Subscriptions are keyed per user but can be declared anywhere.

## Decisions

- **Time format:** RFC3339 (e.g., `2025-01-12T09:30:00Z`). Use the `time` crate for parsing.
  Justification: stdlib has no datetime parsing; `time` is minimal and well-maintained.
- **Subscriptions location:** `/subscription` blocks can live in any document, just like
  `/user` and `/notify` blocks. No dedicated file required.

## Task breakdown (PR-sized)
1. [x] Directive parser + registry loader. Acceptance: loads `/user`, `/subscription`,
   `/notify` blocks; logs on invalid blocks; respects root and symlink policy.
2. [x] Scheduler + delivery. Acceptance: pending notifications send via web-push and
   respect `to` and `at`; no writes outside root.
3. [x] Minimal subscription UI page. Acceptance: can generate a `/subscription` block
   without writing to disk; works with existing service worker.
4. [ ] Refactor: extract push domain types to break the `ports`/`push` dependency cycle.
   Acceptance: `ports` no longer imports `push`; shared types live in a dedicated module.
5. [ ] Docs: document ports/adapters boundaries for push + time abstractions.
   Acceptance: `docs/Resources/ARCHITECTURE.md` (or another canonical doc) reflects the
   updated module and boundary layout.
6. [ ] Refactor: extract directive parsing into `push/directives.rs`.
   Acceptance: parsing logic + tests move; `push.rs` becomes a small public surface.
7. [ ] Refactor: extract registry loading into `push/registry.rs`.
   Acceptance: `DirectiveRegistries::load` and helpers live in the registry module.
8. [ ] Refactor: extract scheduling into `push/scheduler.rs`.
   Acceptance: `PushScheduler`, delay computation, and notification runner are isolated.
9. [ ] Refactor: centralize VAPID validation/construction.
   Acceptance: a single helper is used by scheduler startup and API handlers.
10. [ ] Refactor: make directive parsing return warnings instead of `eprintln!` in core logic.
    Acceptance: logging happens at the call boundary; tests cover warning collection.
11. [ ] Feature: store scheduler handles and add a debug view for scheduled notifications.
    Acceptance: app state keeps handles; debug endpoint returns scheduled entries and
    current server time for timezone inspection.
