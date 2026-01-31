# PWA Refresh Button

## Status
Implemented

## Goal
Provide a user-triggered way to refresh cached assets so PWAs reliably pick up
new JS/CSS without uninstalling the app.

## Context
- The service worker pre-caches static assets and uses cache-first for many
  requests.
- Cache names are fixed (`mindex-v1` / `mindex-auth-v1`), so stale assets can
  persist across updates.
- There is no in-app control to force a refresh when a PWA gets “stuck.”

## Constraints and invariants
- Keep it minimal: no new dependencies, no build step.
- Preserve current SW caching rules (especially the auth/no-auth split).
- Avoid feature creep (no auto-update banner, no background polling).

## Options (max 3)

### Option 1: Plain reload button
**Approach**
- Add a button that calls `location.reload()` (optionally `registration.update()`).

**Pros**
- Very small change.
- No SW changes required.

**Cons**
- Often doesn’t fix cached JS, since the SW can still serve stale assets.

### Option 2: Refresh button that clears Mindex caches + reloads
**Approach**
- Add a button that:
  - forces a SW update check,
  - clears all Mindex caches,
  - activates a waiting SW if present,
  - reloads the page.

**Pros**
- Directly addresses stale JS/CSS by clearing caches.
- Still small, no new dependencies.

**Cons**
- Clears offline caches for documents until re-fetched.
- Requires a small SW message handler or window-side cache deletion.

### Option 3: Versioned cache names
**Approach**
- Inject a build/version string into `sw.js` and use it in `CACHE_NAME`.

**Pros**
- Automatic cache busting on deploys.

**Cons**
- Requires a version source (build-time or config).
- More plumbing than needed for a simple “refresh now” button.

## Recommendation
**Option 2: Refresh button that clears Mindex caches and reloads.**

It is the smallest change that actually fixes the stale-asset problem. It keeps
the app minimal and leaves room for future versioned caches if needed.

## Proposed design (Option 2)

### UI
- Add a `Refresh` (or `Update`) button next to the existing Theme button in nav.
- Show it unconditionally; if SW is unsupported, it just reloads the page.

### Client-side behavior
- Add `assets/features/pwa_refresh.js` and wire it in `assets/app.js`.
- On click:
  1) If `navigator.serviceWorker` is missing, call `location.reload()`.
  2) `navigator.serviceWorker.getRegistration()`; if none, reload.
  3) Attach a one-time `controllerchange` listener to reload when a new SW
     takes control.
  4) Call `registration.update()` to check for a new SW.
  5) Clear caches:
     - `caches.keys()` and delete any cache whose name starts with `mindex-`.
  6) If `registration.waiting` exists, send `{ type: "SKIP_WAITING" }`.
  7) After cache deletion (or a short timeout), `location.reload()`.

### Service worker support (optional but clean)
- Add a `message` handler in `sw.js`:
  - `{ type: "SKIP_WAITING" }` → `self.skipWaiting()`.
  - `{ type: "CLEAR_CACHES" }` → delete Mindex caches and `clients.claim()`.
- The client can either clear caches directly or ask the SW to do it.

### Assets
- Add `assets/features/pwa_refresh.js` to the embedded static assets.
- Include it in the SW `STATIC_ASSETS` list so it’s available offline.

## Implementation plan (PR-sized tasks)

1) **UI + JS hook**
   - Add a Refresh button in nav templates.
   - Add `assets/features/pwa_refresh.js` and wire it in `assets/app.js`.
   - **Acceptance**: Clicking Refresh reloads the page and attempts SW update.

2) **Cache clearing**
   - Implement cache deletion via `caches.keys()` for names starting with `mindex-`.
   - Optional: add SW `message` handler for `SKIP_WAITING` / `CLEAR_CACHES`.
   - **Acceptance**: After clicking Refresh, cached assets are cleared and new
     JS/CSS are fetched on reload.

3) **Service worker asset list**
   - Add the new JS file to `STATIC_ASSETS`.
   - **Acceptance**: The Refresh feature works offline and in PWA context.

## Non-goals
- Automatic “update available” banners or background polling.
- Asset hashing/versioning or build pipelines.
- Changing the caching strategy or auth model.

## Risks and limitations
- Refreshing clears offline caches until assets/documents are re-fetched.
- If the network is down, reload may show the offline fallback page.

## ADR?
Not required. This change does not affect architecture, data model, or introduce
new dependencies.
