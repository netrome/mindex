# ABC Notation Rendering

## Status
Proposed

## Goal
Render ABC notation in markdown documents as readable sheet music.

## Context
- Mindex already supports client-side diagram rendering (Mermaid) by detecting fenced
  code blocks and loading a JS asset only when needed.
- ABC notation is text-based, similar to Mermaid, so the same pattern can be used.
- The product philosophy favors minimal dependencies and a single binary.

## Options explored

### Option 1: Client-side rendering via abcjs (JS)
**Approach**
- Detect fenced code blocks labeled `abc` or `abcjs`.
- Replace them with an HTML container holding the escaped ABC text.
- Load a bundled abcjs asset and call its renderer on page load.

**Pros**
- Fits current rendering model (similar to Mermaid).
- No server-side rendering or external binaries.
- Works offline when the JS is bundled.
- abcjs renders sheet music directly in the browser and can optionally synthesize audio.

**Cons**
- Requires JavaScript to view notation.
- Adds a non-trivial JS asset.
- Potential FOUC before rendering.
- Must HTML-escape ABC text to prevent injection.

### Option 2: Client-side rendering via abc2svg (JS)
**Approach**
- Same as Option 1, but use abc2svg directly as the rendering engine.

**Pros**
- Smaller/lighter than a full feature library.
- Direct ABC-to-SVG rendering in the browser.

**Cons**
- Integration is more manual (multiple scripts and assets for playback).
- Still requires JS and asset bundling.

### Option 3: Server-side rendering via CLI (abcm2ps)
**Approach**
- Invoke a CLI tool to convert ABC to SVG/PS on the server.
- Inline SVG output in the HTML response, possibly cached.

**Pros**
- No client-side JS required.
- Deterministic output that works with JS disabled.

**Cons**
- Requires an external binary and operational setup.
- Adds security and sandboxing concerns.
- Needs caching and timeouts to avoid slow or repeated rendering.
- Would require an ADR (new runtime dependency).

### Option 4: Server-side conversion via Python (music21)
**Approach**
- Use a Python tool to parse ABC into MusicXML.
- Render MusicXML via a client-side renderer or server-side conversion.

**Pros**
- Rich notation support and additional tooling.

**Cons**
- Heavy dependency footprint (Python runtime + libraries).
- More moving parts and larger assets.
- Conflicts with minimal, single-binary philosophy.

## Recommendation
Start with **Option 1: client-side rendering via abcjs**. It is the smallest change
that delivers a good user experience, mirrors the existing Mermaid approach, and
avoids external binaries or server-side execution. abc2svg can be a lighter alternative
if abcjs proves too large.

If server-side rendering is desired later, it should go through an ADR due to the
dependency and security implications.

## Proposed design (Option 1)

### Markdown handling
- Treat fenced code blocks with language `abc` or `abcjs` as ABC notation.
- Convert the block to:
  - `<div class="abc-notation">` with the escaped ABC text as its text content.
- Track whether any ABC blocks were found (`has_abc` flag).

### Assets and templates
- Bundle a minified abcjs JS file as a static asset.
- Load the abcjs asset only when `has_abc` is true.

### Client-side rendering
- On DOMContentLoaded, select all `.abc-notation` nodes.
- For each node:
  - Read `textContent` as the ABC source.
  - Clear the node.
  - Call the renderer to insert SVG into the node.

### Security notes
- Always HTML-escape the ABC text before inserting into the HTML output.
- Do not allow raw HTML injection from ABC blocks.

## Implementation plan

### [x] Task 1: Detect ABC blocks in Markdown rendering
- Extend the pulldown-cmark event pipeline to capture `abc`/`abcjs` fenced blocks.
- Replace them with `<div class="abc-notation">` containers.
- Track `has_abc`.
- **Acceptance criteria**: A document containing ` ```abc` renders a `.abc-notation` container
  with the ABC text and sets `has_abc = true`.

### [ ] Task 2: Add client-side renderer and asset
- Add a bundled abcjs JS asset to `assets/`.
- Add a small feature module to render ABC on load.
- **Acceptance criteria**: ABC blocks render into SVG on document pages.

### [ ] Task 3: Load assets conditionally
- Only include the abcjs asset when `has_abc` is true.
- **Acceptance criteria**: Documents without ABC blocks do not load abcjs.

### [ ] Task 4: Minimal styling (if needed)
- Add CSS to prevent overflow and keep SVG responsive.
- **Acceptance criteria**: Sheet music fits within the content column on mobile.

### [ ] Task 5: Tests
- Markdown-to-HTML tests for ABC block detection and escaping.
- Template tests for conditional asset loading.
- **Acceptance criteria**: Tests cover single/multiple ABC blocks and mixed code fences.

### [ ] Task 6: Documentation
- Update README or docs to describe ABC syntax support.
- Check off the TODO item.
- **Acceptance criteria**: Users can find ABC usage in docs.

## Risks and limitations
- JS required for rendering.
- abcjs asset size may be non-trivial.
- Some ABC features may not render as expected.

## Non-goals
- Audio playback or MIDI export.
- Live preview in the editor.
- Server-side rendering or caching.

## Follow-ups (out of scope)
- Optional audio support using abcjs synth.
- Evaluate abc2svg if abcjs size becomes an issue.
- Optional server-side rendering behind an ADR.
