# Mindex

Mindex is a small, self-hosted web application for browsing, searching, and editing
Markdown documents stored directly on disk.

It’s designed to stay **minimal, simple, and hackable**.

## Features

- Browse a directory tree of `.md` documents
- Render Markdown documents for reading
- Math expressions via LaTeX syntax (`$...$` for inline, `$$...$$` for display)
- Mermaid diagrams via fenced `mermaid` code blocks
- ABC notation rendering via fenced `abc`/`abcjs` code blocks (client-side)
- Full-text search across documents
- Edit and save Markdown from the browser
- Image uploads via `/upload` (returns markdown link)
- Paste images directly into the editor (uploads and inserts markdown)
- View PDFs stored under root, with in-app viewer and explicit open/download actions
- Reorder mode (`/reorder/<doc>.md`) with block/line drag + drop
- Mobile-friendly UI
- Optional in-app authentication with a signed cookie
- Optional git diff + commit UI (when the root is a git repo)
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

## PDF viewing

Mindex supports PDF files under the configured root directory.

- Relative markdown links to PDFs are rewritten to `/pdf/<resolved-path>`.
- The `/pdf/<path>` page embeds the PDF and includes:
  - `Open raw PDF` (`/file/<path>`)
  - `Download PDF` (`/file/<path>?download=1`)

Example in a document:

```markdown
[Concert ticket](tickets/show.pdf)
[Ticket page 2](tickets/show.pdf#page=2)
```

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

Users are defined via `/user` directive blocks in any markdown file. An `email`
and a PHC `password_hash` (Argon2id recommended) are required:

````text
/user
```toml
name = "marten"
display_name = "Marten"
email = "marten@example.com"
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

## Git integration (optional)

When the configured root contains a `.git` directory (or file that resolves
within the root), Mindex exposes `/git` to show a plain `git diff`, a minimal
commit form (stages all changes under the root), and push/pull controls.

Push/pull requires the `git` CLI, a configured upstream, and either:
- an SSH remote (uses `ssh-agent`, non-interactive; stores host keys under
  `.git/mindex_known_hosts`), or
- a local path/file remote that resolves within an allowlist configured via
  `--git-allowed-remote-root <path>` (repeatable) or
  `MINDEX_GIT_ALLOWED_REMOTE_ROOT` (comma-separated).

Push/pull always targets the branch's configured upstream (`@{u}`). If you have
multiple remotes, set the upstream on the branch you care about:

```bash
git branch --set-upstream-to origin/main
# or during push:
git push -u origin main
```

SSH setup notes:
- Ensure `ssh-agent` is running and your key is loaded (e.g., `ssh-add ~/.ssh/id_ed25519`).
- The Mindex process must inherit `SSH_AUTH_SOCK` so git can reach the agent.
- Host keys are stored in `.git/mindex_known_hosts` and new keys are accepted
  automatically on first connect (non-interactive). To pre-seed host keys, copy
  entries into that file before using push/pull.

Troubleshooting:
- If push/pull fails with auth errors, verify `ssh-agent` is running and that
  `SSH_AUTH_SOCK` is visible to the Mindex process (systemd services often need
  explicit environment propagation).
- If you see host key warnings, ensure the server key is present in
  `.git/mindex_known_hosts` or delete the entry and retry to re‑accept.

If the root is a subdirectory of a larger repo (i.e., `.git` lives above it),
git integration is disabled to preserve filesystem safety invariants.

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

When you save a document containing an `@username` mention, Mindex sends a push
notification to that user with the contents of the line containing the mention.
