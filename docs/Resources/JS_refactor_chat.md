You can absolutely keep this “server-rendered HTML + sprinkle of JS” model for a long time without drifting into SPA-land. The trick is to treat JS as **progressive enhancement** + keep a **small, explicit contract** between the rendered HTML and the JS.

Here are a few approaches that scale nicely while staying lean.

## 1) Stay server-rendered, add “islands” via data-attributes

You’re already doing the right thing with `data-task-index`. Lean into that pattern:

* Server renders *semantic HTML* (lists, forms, buttons), plus `data-*` hooks.
* JS enhances specific parts *if present*.
* No client-side routing, no client-side templating needed.

A good “contract” for TODO items is to render enough metadata so the client can do edits without needing to re-parse Markdown:

* `data-task-index` (you have)
* **and ideally** something like `data-line-start` / `data-line-end` or a stable `data-task-id` (more on that below)

That lets you implement reorder / insert with very little client complexity.

## 2) Put your JS on a diet: one tiny “runtime” + feature modules

As JS grows, the main failure mode is “random scripts everywhere”.

A simple structure that stays tiny:

* `static/js/app.js` (tiny bootstrap)
* `static/js/features/todo.js`
* `static/js/features/drag_reorder.js`
* `static/js/features/quick_add.js`

And a micro pattern:

* feature module exports `init(root)` (or just `init()`).
* bootstrap calls all `init`s once on `DOMContentLoaded`.
* feature modules use **event delegation** (attach 1 handler to a parent) instead of many listeners.

This keeps your code size small and avoids “listener leaks” when you later introduce partial refreshes.

## 3) Reordering TODOs: avoid full Markdown parsing in the browser

There are two sane minimal routes:

### A) Best UX, still minimal: reorder on the client, persist via API

* Use native HTML5 drag+drop or pointer-based sorting.
* After drop, send **the new order** to the server.

But the payload should be something stable. If you only have `task_index`, reordering changes indices, so you need a “before” mapping. Two easy options:

**Option 1: stable IDs rendered by server**

* When rendering, assign each task a stable id (e.g. hash of its source line + doc id + occurrence).
* Send `ordered_ids: [...]` to server.
* Server rewrites the Markdown list accordingly.

**Option 2: line ranges**

* Render `data-line-start`/`data-line-end` for each list item in the original Markdown.
* On reorder, send the list of ranges in new order.
* Server splices text segments. (Very robust, very little “parsing”.)

### B) Even simpler: “Move up / move down” buttons

It’s surprisingly nice on mobile too.

* Two buttons per item.
* Calls `/api/doc/move-task` with index + direction.
* Server rewrites markdown.
  This avoids DnD complexity entirely, and is still “fast add / reorder”.

If “drag reorder” is a priority, I’d still keep **Move up/down** as a fallback for touch devices and accessibility.

## 4) Quick add without textarea: use a real `<form>` + enhance it

Make it work with **no JS** first:

* Render a small form above a TODO list:

  * input text
  * submit button
* POST to `/api/doc/add-task` (or a normal route), then redirect back to the doc.

Then add a tiny JS enhancement:

* intercept submit
* POST via `fetch`
* on success, either:

  * reload the page (simple + consistent), or
  * insert the new `<li>` into the DOM (nicer, slightly more code)

Reloading after successful write is totally fine in a minimalist app and keeps client logic tiny.

## 5) Consider one “tiny helper library” only if it truly pays for itself

If you want to keep it ultra-minimal, vanilla JS is fine.

If you want *slightly* higher-level ergonomics without becoming an SPA, these are in the “sprinkles” category:

* **htmx**: lets you do “submit this form, swap that HTML fragment” with almost no JS.
* **Stimulus**: tiny controller pattern for DOM behavior (very compatible with server-rendered HTML).

Both can be used narrowly (only on TODO pages) and don’t force an SPA architecture.

But: you don’t *need* them. Your current style is already basically “Stimulus-lite”.

## 6) Testing without pain: do a small pyramid

You can get great confidence without heavy infra:

### Unit-ish tests (fast)

* Use something like **Vitest** with **jsdom** to test:

  * event delegation
  * DOM updates
  * payloads sent to `fetch` (mock fetch)

This keeps logic in small pure-ish functions (e.g. “compute reorder payload”, “extract task index”, “apply optimistic UI”).

### E2E tests (few, high value)

* Use **Playwright** to run 5–10 core flows:

  * open doc
  * tick checkbox
  * add todo item
  * reorder item
  * verify persistence after reload

This catches real integration bugs (templates + JS + server endpoints) and doesn’t require you to over-test internals.

If you only choose one: Playwright gives the best “don’t rely on manual testing in prod” ROI.

## 7) A few practical details that’ll save you later

* **Event delegation**: attach one `change` handler on the doc content container, not per checkbox.
* **Make HTML the source of truth**: after any mutation, reloading should always show correct state.
* **Prefer “server rewrites markdown”** for correctness: client sends intent; server owns the file.
* **Keep endpoints small and specific** (`toggle-task`, `add-task`, `reorder-tasks`), like you’re already doing.

---

If I were you, I’d do:

1. Refactor current inline script into `static/js/features/todo_toggle.js` + delegated handler.
2. Add “Quick add” as a plain form first + optional fetch enhancement (or just reload).
3. Implement reorder either:

   * move up/down first (super small), then
   * drag/drop later using stable IDs or line ranges.

If you want, paste (or link) how you currently render TODO list items in HTML (the markdown-to-HTML output for tasks), and I can suggest the cleanest “contract” (IDs vs line ranges) for reorder + add with minimal server complexity.
