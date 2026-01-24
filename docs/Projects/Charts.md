# Chart Rendering (Mermaid / D2)

## Status
Proposed

## Goal
Support simple charts/diagrams in markdown using fenced code blocks, starting with the smallest, most maintainable option.

## Context
- Mindex currently renders Markdown via pulldown-cmark and outputs plain code blocks.
- The TODO list calls for "d2 or mermaidjs charts".
- The app is single-binary, file-backed, and aims to stay minimal and dependency-light.

## Options explored

### Option 1: Mermaid (client-side JS)
**Approach**
- Detect fenced code blocks labeled `mermaid`.
- Render them as `<div class="mermaid">...</div>`.
- Bundle `mermaid.min.js` as a static asset and run `mermaid.initialize({ startOnLoad: true })` on document pages.

**Pros**
- Simple integration: no server-side rendering.
- Widely used syntax; lots of examples.
- Works offline when JS is bundled.

**Cons**
- Adds a sizable JS asset.
- Requires JavaScript to view charts (FOUC possible).
- Needs careful HTML escaping of the diagram text.

### Option 2: D2 (server-side CLI)
**Approach**
- Invoke a `d2` binary on the server to render SVG from fenced `d2` blocks.
- Inline the SVG into the HTML output.

**Pros**
- No client-side JS required.
- Nice SVG output.

**Cons**
- Requires an external binary (breaks single-binary simplicity).
- Adds operational dependency and security surface area.
- Needs caching and resource limits to avoid slow renders.
- Likely requires an ADR (significant dependency/ops change).

### Option 3: D2 (WASM/JS)
**Approach**
- Bundle a WASM/JS build of D2 and run client-side, similar to Mermaid.

**Pros**
- Single-binary server still possible.
- Client-side rendering avoids server load.

**Cons**
- Unclear availability/size/maintenance of a stable WASM build.
- Larger and more complex asset pipeline than Mermaid.
- Still requires JS and careful security handling.

### Option 4: Support both Mermaid and D2
**Approach**
- Implement Option 1 plus either Option 2 or 3.

**Pros**
- Covers more user preference.

**Cons**
- More dependencies and complexity.
- Higher maintenance and testing cost.

## Recommendation
Start with **Mermaid client-side rendering**. It is the smallest and simplest option that fits Mindex's philosophy and delivers value quickly. D2 can be evaluated later if Mermaid proves insufficient; if D2 requires a binary or large WASM bundle, it should go through an ADR.

## Proposed design (Mermaid)

### Markdown handling
- Treat fenced code blocks with language `mermaid` specially.
- Convert them to an HTML container with the raw diagram text (HTML-escaped).
- All other code blocks render as normal.

### Assets and templates
- Bundle `mermaid.min.js` as a static asset (no CDN).
- Add a small inline boot script on document view pages.
- Only load Mermaid JS when the rendered document actually contains a Mermaid block.

### Security notes
- Mermaid blocks must be HTML-escaped before embedding.
- No file system access or execution; all rendering is client-side in the browser.

## Implementation plan

### Task 1: Detect Mermaid blocks in Markdown rendering
- Extend the pulldown-cmark event pipeline to wrap `mermaid` fenced blocks.
- Track whether any Mermaid blocks were found.
- **Acceptance criteria**: A document containing ` ```mermaid` renders a `<div class="mermaid">` with the diagram text, while other code blocks are unchanged.

### Task 2: Load Mermaid JS only when needed
- Add a template flag to include Mermaid assets when a document contains Mermaid blocks.
- **Acceptance criteria**: Mermaid JS is not loaded for documents without Mermaid blocks.

### Task 3: Bundle Mermaid JS asset
- Add the prebuilt `mermaid.min.js` to static assets and serve it from the embedded asset pipeline.
- **Acceptance criteria**: Document page renders charts offline with no external network calls.

### Task 4: Minimal styling
- Add CSS for `.mermaid` containers (spacing, overflow) if needed.
- **Acceptance criteria**: Charts align with existing typography and do not overflow on mobile.

### Task 5: Tests
- Add tests for Markdown-to-HTML conversion of Mermaid blocks.
- **Acceptance criteria**: Tests cover single Mermaid block, multiple blocks, and mixed code fences.

### Task 6: Documentation
- Update README and/or docs to mention Mermaid chart support and syntax.
- Update `docs/Projects/TODO.md` to check off the item.
- **Acceptance criteria**: Feature is documented for users.

## Risks and limitations
- Mermaid is JS-only; charts do not render for JS-disabled browsers.
- Asset size increases the static bundle.
- Mermaid syntax errors will render as plain text; need a reasonable fallback.

## Non-goals
- D2 support in the same PR.
- Live preview or editor integration.
- Server-side rendering or caching of diagrams.

## Follow-ups (out of scope)
- Evaluate D2 (WASM or CLI) if Mermaid is insufficient.
- Optional server-side rendering for static HTML exports.
