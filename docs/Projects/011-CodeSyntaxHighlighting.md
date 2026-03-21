# Code Syntax Highlighting

## Status
Done

## Goal
Render fenced code blocks with syntax highlighting so that keywords, strings,
comments, etc. are visually distinct.

## Context
- Mindex renders markdown via pulldown-cmark. Fenced code blocks currently emit
  plain `<pre><code>` with no color.
- The event-stream pipeline already intercepts special code blocks (mermaid, abc)
  by language tag, so the interception point exists.
- The project has light/dark theme support via CSS variables.
- Assets are embedded at compile time (`include_str!`). Currently 12 direct
  Rust dependencies.

## Options explored

### Option A: Server-side via syntect (Rust)
**Approach**
- Add the `syntect` crate.
- In `render_document_html`, intercept `CodeBlock` events that have a recognized
  language tag.
- Use syntect to tokenize the code and emit `<span>` elements with class names
  (class-based highlighting, not inline styles).
- Ship a small CSS theme (light + dark variants) as part of `style.css`.

**Pros**
- No client-side JS; highlighted HTML arrives ready to render.
- Works with JS disabled and in RSS/feed readers.
- Follows the same interception pattern as mermaid/abc — well-understood.
- syntect is a mature, widely-used Rust crate (used by bat, delta, Zola, etc.).

**Cons**
- Adds a direct dependency (`syntect`) which pulls in `onig` or `fancy-regex`
  and a set of bundled `.sublime-syntax` / `.tmTheme` files. This noticeably
  increases compile time and binary size (~2-4 MB for embedded syntax sets).
- The default syntax set covers ~150 languages. Trimming it to a smaller set
  requires a custom build step or using `syntect::dumps`.
- Requires an ADR per INVARIANTS.md (significant new dependency).
- Generating class-based HTML requires building a `ClassedHTMLGenerator`, which
  means we also need to maintain a CSS theme mapping those classes to colors.

**Complexity:** Medium. The interception pattern exists; the new code is ~30-50
lines in `documents.rs` plus a CSS theme. The dependency weight is the main cost.

### Option B: Client-side via highlight.js
**Approach**
- Bundle a highlight.js distribution (core + selected languages) as a static
  asset, similar to how mermaid.js and abcjs are bundled today.
- Add a small JS module that runs `hljs.highlightAll()` on page load.
- Load the JS + CSS only when the page contains code blocks (`has_code` flag).

**Pros**
- Zero Rust code changes beyond setting a `has_code` flag and passing it to the
  template (same pattern as `has_mermaid` / `has_abc`).
- highlight.js auto-detects language when no tag is specified.
- Easy to swap themes — just replace the CSS file.
- No new Rust dependency; no ADR needed.

**Cons**
- Requires JS; no highlighting with JS disabled.
- Asset size: core + common languages ≈ 40-50 KB minified + gzipped.
  Including many languages grows this further.
- Brief FOUC (un-highlighted code visible until JS runs).
- Adds a vendored JS file to the repo (same as mermaid/abcjs).

**Complexity:** Low. Follows the exact mermaid/abc pattern end-to-end.

## Recommendation
**Option B (highlight.js)**, for the following reasons:

1. **Minimal Rust changes.** The only backend change is adding a `has_code` flag
   — the same one-line pattern used for mermaid and abc. No new Rust dependency,
   no ADR required.
2. **Proven pattern.** This is identical to how mermaid.js and abcjs are
   integrated. The approach is well-understood in this codebase.
3. **Theme flexibility.** highlight.js ships dozens of CSS themes. Swapping a
   theme is a single file replacement, and it's easy to provide separate
   light/dark themes that hook into the existing CSS variable / `data-theme`
   system.
4. **Language auto-detection.** highlight.js can guess the language when no tag
   is provided, which is a nice UX bonus for documents that use bare ``` blocks.
5. **Dependency discipline.** syntect is a substantial dependency that would
   increase binary size by several MB and compile time significantly. That weight
   isn't justified for a feature that highlight.js handles well client-side —
   especially given that mermaid and abc already set the precedent for
   client-side rendering of specialized blocks.

syntect would be the better choice if JS-free rendering were a hard requirement
or if we were building a static site generator. For an interactive web app that
already ships JS for mermaid, abc, and other features, client-side highlighting
is the pragmatic choice.

## Proposed design (Option B)

### Markdown handling
- In `render_document_html`, detect whether any fenced code block with a
  language tag is present (exclude mermaid/abc which are already handled).
- Set a `has_code` flag on `RenderedDocument`.
- No changes to the HTML output — pulldown-cmark already emits
  `<pre><code class="language-X">` which is exactly what highlight.js expects.

### Assets
- Bundle `highlight.min.js` (core + common languages) in `assets/vendor/`.
- Bundle a highlight.js CSS theme in `assets/vendor/` (one that works for both
  light and dark modes, or two themes toggled by `data-theme`).
- Register both in `src/assets.rs` following the existing pattern.

### Template
- In `document.html`, conditionally load the highlight.js script and CSS when
  `has_code` is true.
- Add a small inline script or module: `hljs.highlightAll()`.

### Theme integration
- Use a highlight.js theme that respects `prefers-color-scheme` or ship two
  theme files toggled by the existing `data-theme` attribute.
- Alternatively, use a single neutral theme that looks acceptable in both modes.

## Implementation plan

### [x] Task 1: Detect code blocks and set `has_code` flag
- In the pulldown-cmark event loop, track whether any non-mermaid, non-abc
  fenced code block is encountered.
- Add `has_code: bool` to `RenderedDocument`.
- Pass it through to the template.
- **Acceptance criteria**: `has_code` is true when a document contains a fenced
  code block with a language tag; false otherwise.

### [x] Task 2: Bundle highlight.js assets
- Download highlight.js (core + common languages subset).
- Add `highlight.min.js` and a CSS theme to `assets/vendor/`.
- Register in `src/assets.rs`.
- **Acceptance criteria**: Assets are served at `/static/highlight.min.js` and
  `/static/highlight.min.css`.

### [x] Task 3: Conditional loading in template
- Load highlight.js JS and CSS only when `has_code` is true.
- Call `hljs.highlightAll()` after load.
- **Acceptance criteria**: Documents with code blocks load highlight.js;
  documents without code blocks do not.

### [x] Task 4: Theme integration
- Ensure highlighting looks good in both light and dark modes.
- **Acceptance criteria**: Code blocks have distinct syntax colors in both
  themes; no contrast or readability issues.

### [x] Task 5: Tests
- Markdown rendering test: `has_code` is set correctly for various block types.
- Verify mermaid/abc blocks do not trigger `has_code`.
- **Acceptance criteria**: Tests cover code blocks with language tags, bare code
  blocks, and mixed documents.

### [x] Task 6: Documentation
- Update README to mention syntax highlighting support.
- Check off the TODO item.
- **Acceptance criteria**: Users know which languages are supported.

## Risks and limitations
- JS required for highlighting (consistent with mermaid/abc precedent).
- Language coverage depends on which highlight.js languages are bundled. Can be
  expanded later by adding language modules.
- Brief FOUC on slow connections before JS executes.

## Non-goals
- Line numbers in code blocks.
- Copy-to-clipboard button.
- Server-side rendering.
- Highlighting in the editor/preview.

## Follow-ups (out of scope)
- Add more highlight.js language packs if users need niche languages.
- Consider a copy-to-clipboard button for code blocks.
- Re-evaluate server-side highlighting if JS-free rendering becomes a goal.
