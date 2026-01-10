# In-App Authentication: Optional JWT Cookie with Document-Backed Credentials

## Status
- Proposed

## Context
Mindex is typically deployed behind a reverse proxy for auth, but PWAs and
mobile usage make basic auth awkward. We want an optional, minimal in-app auth
mechanism that keeps the system file-backed, avoids a database, and preserves
existing filesystem safety invariants.

## Decision
- Add optional in-app authentication guarded by a signed JWT stored in an
  HttpOnly cookie.
- Store user credentials in existing `/user` directive blocks as a
  `password_hash` (PHC string). Blocks missing a hash are invalid and ignored
  with a warning.
- Use a dedicated auth signing key configured via CLI/env; do not reuse VAPID keys.
  The key is a base64-encoded HMAC secret used to sign HS256 JWTs.
- Make `Secure` cookies configurable to support TLS termination at a reverse proxy
  while keeping local HTTP development simple.
- Keep the system stateless (no server-side sessions). Logout clears the cookie.

## Consequences
- Security model changes: Mindex now supports built-in auth when configured.
- Adds a minimal password hashing dependency (Argon2id recommended).
- Token revocation is limited to expiry or key rotation.
- Service worker caching must avoid persisting sensitive content when auth is enabled.
- Existing `/user` blocks without `password_hash` will no longer be accepted.
