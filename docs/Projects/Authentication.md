# Authentication (Design)

## Status
In progress

## Context
Mindex currently assumes a trusted environment and typically relies on a reverse
proxy for auth. That does not play well with PWA workflows and makes it harder to
use the app directly on personal devices. We already have a lightweight user
concept for push notifications, and that can serve as the minimal foundation for
in-app authentication without introducing a database or session store.

The goal is to restrict access to the UI and APIs while keeping the app file-backed,
minimal, and easy to self-host.

## Goals
- Provide optional in-app authentication that gates all document/UI/API routes.
- Keep the system stateless (no server-side session store).
- Reuse the existing user registry where possible.
- Stay minimal and file-backed; no DB or background auth workers.
- Preserve existing filesystem safety invariants.

## Non-goals
- Multi-tenant roles, per-document ACLs, or user groups.
- User self-service (signup, password reset, email flows).
- OAuth/SSO integration.
- Encrypting documents at rest.
- Guaranteeing offline access to private docs.

## Constraints and invariants
- Documents on disk remain the source of truth; doc IDs are relative paths.
- No reads/writes outside the configured root.
- No new background jobs.
- Dependency additions must be justified and minimal.
- Security model change requires an ADR.

## Options (max 3)

### Option A: Single shared access secret (config only)
Require a single shared secret configured via CLI/env; clients submit it on a
login form, receive a signed JWT cookie, and proceed.
Pros: minimal, no document changes, no user registry changes.
Cons: no per-user identity, no easy revocation beyond rotating the secret.

### Option B: Per-user credentials in `/user` blocks (recommended)
Extend `/user` TOML blocks to include a required `password_hash` (PHC string).
Login with username/password issues a signed JWT cookie with `sub = username`.
Pros: aligns with file-backed philosophy; reuses user registry; supports per-user
identity for future features (audit, push ownership, etc.).
Cons: needs a password-hashing dependency and careful UX to generate hashes.

### Option C: Bearer token in docs
Store a long random token in a directive block and require clients to send it as
`Authorization: Bearer <token>`.
Pros: no hashing dependency; minimal logic.
Cons: poor UX; tokens are bearer secrets; no user identity; easy to leak.

## Recommendation
Option B: extend `/user` blocks with a required `password_hash`, add a simple
login page, and issue stateless JWTs stored in an HttpOnly cookie. This keeps
the system file-backed and minimal while enabling PWA-friendly auth.

## Proposed design (Option B)

### Data model
Extend existing `/user` directive blocks:

````
/user
```toml
name = "marten"
display_name = "Marten"
password_hash = "$argon2id$v=19$m=19456,t=2,p=1$...$..."
```
````

Rules:
- `password_hash` is required; blocks missing it are ignored with a warning.
- Hash format is PHC (Argon2id recommended).
- Existing `/user` blocks without `password_hash` will be treated as invalid.

### Configuration
Add optional auth config (exact names TBD in implementation):
- `--auth-key` / `MINDEX_AUTH_KEY`: HMAC signing key (base64-encoded secret).
- `--auth-token-ttl` / `MINDEX_AUTH_TOKEN_TTL`: token lifetime (default `14d`).
- `--auth-cookie-name` (default `mindex_auth`).
- `--auth-cookie-secure` / `MINDEX_AUTH_COOKIE_SECURE`: when true, set `Secure`
  on auth cookies (needed behind TLS-terminating reverse proxies).

Auth is disabled if `auth-key` is missing. When disabled, behavior is unchanged.

### JWT details
- Sign with HS256 (HMAC) using `MINDEX_AUTH_KEY` (decoded from base64).
- Claims: `sub` (username), `iat`, `exp`, `iss` (app name).
- Store token in an HttpOnly, SameSite=Lax cookie.
- No server-side sessions; logout clears the cookie.
- Rotation/revocation is via key rotation or short TTLs.

### Login flow
1. `GET /login` renders a minimal login form.
2. `POST /login` verifies username + password.
3. On success, set auth cookie and redirect to the original URL (validated `next`).
4. `POST /logout` clears cookie and redirects to `/login`.

API endpoints should return `401` JSON errors instead of redirects.

### Route protection
Add a lightweight auth middleware that:
- Bypasses auth for `/login`, `/logout`, `/static/*`, `/sw.js`, `/health`.
- Requires a valid JWT for all other routes.

### Service worker and caching
The current service worker caches document views for offline access. With auth
enabled, this risks storing sensitive content on-device. Proposed behavior:
- When auth is enabled, only cache static assets.
- Do not cache `/doc/*`, `/search`, or `/` responses.
- Optionally clear caches on logout.

### Password hashing
Use Argon2id (via a minimal crate) to verify PHC strings.
Provide documentation or a CLI helper to generate hashes; no in-app user creation.

### Directive extraction
Auth will need `/user` data outside of push. To avoid coupling auth to push,
extract directive parsing/registry loading into a standalone module (e.g.
`src/directives/` or `src/registries/`). Push and auth should depend on that
module instead of each other.

## Security considerations
- Tokens are bearer credentials; always set `HttpOnly` and `SameSite=Lax`.
- `Secure` should be set when behind TLS (likely via reverse proxy).
- Avoid leaking auth state through cached content in the service worker.
- Keep `auth-key` separate from VAPID keys; do not reuse.

## Invariant impact
- No changes to document identity or root sandboxing.
- Security model changes (in-app auth becomes optional) require an ADR.
- No DB or background jobs introduced.

## Open questions
- Should we add a small CLI helper to generate password hashes?
- Token TTL defaults (short vs "remember me").

## Task breakdown (PR-sized)
1. [x] ADR: define optional in-app auth and credential storage. Acceptance: ADR merged
   with decision, consequences, and dependency rationale.
2. [x] Config plumbing. Acceptance: auth config loads, validation rejects missing
   `auth-key` when auth is enabled, default behavior unchanged.
3. [x] Refactor: extract directive parsing/registries from push. Acceptance: push and
   auth depend on shared directives module; module layout stays consistent with
   existing ADR; tests updated.
4. [x] `/user` parsing update. Acceptance: `password_hash` is required; missing
   hashes emit warnings and the user entry is skipped; tests updated.
5. [ ] Auth middleware + JWT. Acceptance: protected routes require auth; JWT cookie
   validated; API returns 401; HTML routes redirect to `/login`.
6. [ ] Login/logout UI. Acceptance: minimal login form; success sets cookie; logout
   clears cookie; safe `next` handling.
7. [ ] Service worker changes. Acceptance: when auth enabled, no document caching;
   static assets still cached.
8. [ ] CLI helper for auth secrets. Acceptance: `mindex auth-key` (or similar) prints
   a base64-encoded secret; docs updated with usage.
9. [ ] Docs update. Acceptance: README and auth setup docs describe configuration,
   hash generation, and PWA considerations.
