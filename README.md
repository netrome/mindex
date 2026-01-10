# Mindex

Mindex is a small, self-hosted web application for browsing, searching, and editing
Markdown documents stored directly on disk.

It’s designed to stay **minimal, simple, and hackable**.

## Features

- Browse a directory tree of `.md` documents
- Render Markdown documents for reading
- Full-text search across documents
- Edit and save Markdown from the browser
- Mobile-friendly UI
- Optional in-app authentication with a signed cookie
- Single binary. I.e. static assets (CSS/JS/icons) are embedded directly in the app.

## Non-goals

- No database
- No real-time collaboration - git can be used for syncing changes accross devices.
- No requirements of external systems. The app should be useful directly when provided a directory of `.md` documents.

## Quick start

### Run locally

```bash
cargo run -- --root ./sample-root
```

That's it. The documents are now served at `http://localhost:3000`.

## Authentication (optional)

Mindex can enable minimal in-app authentication using a signed JWT stored in an
HttpOnly cookie. If `--auth-key` / `MINDEX_AUTH_KEY` is not set, auth is disabled
and all routes behave as before.

Generate an auth key:

```bash
mindex auth-key
```

Run with auth enabled:

```bash
cargo run -- --root ./sample-root --auth-key "<base64-secret>"
```

Users are defined via `/user` directive blocks in any markdown file. A PHC
`password_hash` (Argon2id recommended) is required:

````text
/user
```toml
name = "marten"
display_name = "Marten"
password_hash = "$argon2id$v=19$m=19456,t=2,p=1$...$..."
```
````

Generate a password hash:

```bash
mindex hash-password --password "s3cr3t"
```

For better shell hygiene, pipe via stdin:

```bash
printf "%s" "s3cr3t" | mindex hash-password
```

Login is at `/login` and logout is `POST /logout`. When auth is enabled, the
service worker only caches static assets (no document content).

For more details, see `docs/Resources/Auth.md`.

## Push notifications (optional)

Push notifications require VAPID keys. Provide them via CLI flags or environment
variables:

- `--vapid-private-key` / `MINDEX_VAPID_PRIVATE_KEY`
- `--vapid-public-key` / `MINDEX_VAPID_PUBLIC_KEY`
- `--vapid-subject` / `MINDEX_VAPID_SUBJECT`

Generate keys locally with:

```bash
mindex init --subject "mailto:you@example.com"
```

If these are not set, the server runs normally but the scheduler is disabled.

To register a device, visit `/push/subscribe` and copy the generated
`/subscription` block into any markdown document. The page also includes
a "Send test" button for quick verification.
