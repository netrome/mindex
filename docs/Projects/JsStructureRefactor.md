# JS Structure Refactor

## Status
Proposed

## Goal
Keep client-side JavaScript organized as small features are added (e.g., TODO reorder, quick add) while preserving Mindex’s minimal, server-rendered, progressive-enhancement model.

## Context
Current JS lives in three places:
- `assets/theme.js` (global theme toggling, loaded in `templates/base.html`)
- Inline scripts in `templates/document.html` (TODO toggle)
- Inline scripts in `templates/push_subscribe.html` (push subscription flow)
- Inline script in `templates/base.html` (service worker registration)

This is fine today, but upcoming TODO-related features will introduce more JS and make inline scripts harder to manage, reuse, and test.

## Constraints and invariants
- Keep the server-rendered HTML + progressive enhancement model.
- No bundler, no framework, no new dependencies.
- Static assets remain embedded via `src/assets.rs`.
- No behavioral changes in this refactor; move code only.

## Options (max 3)

### Option 1: Keep inline scripts
**Pros**
- No changes to asset plumbing.
- Zero additional requests.

**Cons**
- Script sprawl across templates.
- Harder to reuse helpers or share patterns.
- Harder to keep consistent as features grow.

### Option 2: One `static/app.js` with all feature code
**Pros**
- Single file and request.
- No modules or globals required.
- Minimal change to assets/routes.

**Cons**
- One large file becomes a “misc pile.”
- Feature boundaries are only comments.

### Option 3: Tiny bootstrap + feature modules (ES modules)
**Pros**
- Clear, obvious feature boundaries.
- No bundler; plain files, easy to hack.
- Keeps the app.js entrypoint tiny and readable.

**Cons**
- Multiple small JS files to serve.
- Requires adding a small static JS map in `src/assets.rs`.

## Recommendation
**Option 3: Tiny bootstrap + feature modules (ES modules).**

This keeps JS simple and obvious without introducing a build step. Each feature is isolated in a small file, and `app.js` only wires them up if the relevant DOM exists. This stays true to the project’s minimal ethos while avoiding inline-script sprawl.

## Proposed structure

```
assets/
  theme.js
  app.js
  features/
    todo_toggle.js
    push_subscribe.js
    sw_register.js
```

Notes:
- Keep `assets/theme.js` as-is to keep early theme initialization behavior.
- `assets/app.js` is loaded globally (once) and calls feature `init()` functions.
- Features are no-op if their DOM hooks are not present.
- No shared “utils” module unless real duplication emerges.

## HTML/JS contract (minimal)
- TODO checkboxes keep `data-task-index`.
- Feature modules only rely on stable IDs/data attributes (no class-name coupling).
- No client-side templating; HTML remains the source of truth.

## Implementation plan (PR-sized tasks)

1) **Add JS entrypoint + feature modules**
   - Create `assets/app.js` and `assets/features/*.js`
   - Move existing inline code into feature modules
   - Acceptance: behavior unchanged for TODO toggle, push subscribe, and service worker registration

2) **Wire templates to the new JS**
   - Add `<script type="module" src="/static/app.js"></script>` in `templates/base.html`
   - Remove inline scripts from `templates/document.html` and `templates/push_subscribe.html`
   - Acceptance: pages render without inline scripts and behavior is unchanged

3) **Update asset serving + service worker cache list**
   - Add routes and embed strings for the new JS files in `src/app.rs` + `src/assets.rs`
   - Add the new JS files to `templates/sw.js` pre-cache list
   - Acceptance: JS files are served with `content-type: application/javascript` and cached by the service worker

## Non-goals
- Implementing TODO reorder or quick-add features.
- Changing the server’s document rendering or data model.
- Introducing a bundler, framework, or external JS dependencies.

## Risks and limitations
- Slightly more static files to embed and serve.
- ES module support is required (modern browsers only; acceptable for Mindex’s current targets).

## ADR?
Not required. This refactor does not change architecture, security, data model, or add dependencies. If a future change introduces a bundler or external JS library, that should go through an ADR.
