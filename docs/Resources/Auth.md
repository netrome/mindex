# Authentication (Mindex)

## Overview

Mindex supports optional in-app authentication using a signed JWT stored in an
HttpOnly cookie. Auth is disabled by default; when disabled, all routes behave
as before.

## Enable auth

Provide an auth signing key (base64, URL-safe recommended):

- CLI: `--auth-key`
- Env: `MINDEX_AUTH_KEY`

Generate a key:

```bash
mindex auth-key
```

Optional settings:

- `--auth-token-ttl` / `MINDEX_AUTH_TOKEN_TTL` (default `14d`)
- `--auth-cookie-name` (default `mindex_auth`)
- `--auth-cookie-secure` / `MINDEX_AUTH_COOKIE_SECURE`

## Users

Users are defined in `/user` directive blocks in any markdown file. A PHC
`password_hash` is required (Argon2id recommended).

````text
/user
```toml
name = "marten"
display_name = "Marten"
password_hash = "$argon2id$v=19$m=19456,t=2,p=1$...$..."
```
````

Blocks missing `password_hash` are ignored with a warning.

Generate a password hash:

```bash
mindex hash-password --password "s3cr3t"
```

Prefer stdin to avoid shell history:

```bash
printf "%s" "s3cr3t" | mindex hash-password
```

If you prefer third-party tooling, the `argon2` CLI can also emit PHC strings:

```bash
printf "%s" "s3cr3t" | argon2 "$(openssl rand -base64 16)" -id -t 2 -m 15 -p 1
```

## Login/logout

- `GET /login` renders a login form.
- `POST /login` verifies username + password and sets the auth cookie.
- `POST /logout` clears the cookie and redirects to `/login`.

API requests under `/api/*` return `401` JSON when unauthenticated; HTML routes
redirect to `/login`.

## Cookies

The auth cookie is:

- `HttpOnly`, `SameSite=Lax`
- `Secure` when `--auth-cookie-secure` is enabled
- `Max-Age` is derived from the configured token TTL

## Service worker caching

When auth is enabled, the service worker only caches static assets (no document
content or search results).
