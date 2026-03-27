# Magent Interaction Rendering

## Status
Proposed

## Goal
Render magent directives and responses in mindex documents with proper
structure: collapsible thinking/tool-call blocks, styled response sections,
accept buttons for proposed edits, and an easy way to insert new directives
from the document view.

## Context
- [Magent](https://github.com/marten-sova/magent) is a standalone LLM agent
  that watches a markdown directory and responds to `@magent` directives by
  writing `<magent-*>` custom elements back into the same files.
- Mindex already renders these files but treats the custom elements as raw
  HTML. Due to how CommonMark handles HTML blocks (type 6 — custom tags end
  at the next blank line), the content inside magent blocks is parsed
  inconsistently: pulldown-cmark flips between HTML-block and markdown mode
  at every blank line, producing garbled output.
- Mindex has an established pattern for custom content types: detect in the
  rendering pipeline, emit structured HTML, conditionally load a JS feature
  module. Mermaid, ABC notation, syntax highlighting, and task checkboxes
  all follow this pattern.
- The accept-edit flow has a direct analogue in the existing task-toggle
  endpoint (`/api/d/toggle-task`), which mutates the source file and returns
  the result.

## Options explored

### Option 1: CSS-only (style custom elements in-place)
**Approach**
- Add CSS rules targeting `magent-response`, `magent-thinking`, etc.
- No backend or JS changes.

**Pros**
- Zero code changes.

**Cons**
- Does not work. The CommonMark HTML-block parsing issue means the elements
  are not emitted cleanly — blank lines inside a `<magent-response>` cause
  pulldown-cmark to exit the HTML block and re-enter markdown mode, garbling
  the output.
- Cannot provide collapsible sections or accept buttons without JS.

**Verdict:** Not viable.

### Option 2: Pre-processor + feature module (follows existing patterns)
**Approach**
- Add a pre-processing step that runs before pulldown-cmark. It extracts
  `<magent-*>` blocks from the raw markdown, renders their inner content
  independently, and emits structured HTML that pulldown-cmark won't touch.
- Add a small JS feature module for interactivity (collapse/expand, accept
  buttons).
- Add one API endpoint for applying accepted edits.

**Pros**
- Follows the exact same pattern as task lists (pre-process before
  pulldown-cmark), mermaid/abc (detect, emit container, conditional JS), and
  task toggle (API endpoint to mutate source file).
- Magent-specific code is isolated: one pre-processor function, one JS
  module, one API endpoint, some CSS.
- No new Rust dependencies needed.

**Cons**
- Couples mindex to magent's `<magent-*>` tag vocabulary. If magent changes
  its format, the pre-processor needs updating.
- The pre-processor needs to handle nested structures (e.g. tool calls
  inside responses, edits inside responses).

**Complexity:** Medium. The pre-processor is the main new code (~100-200
lines of Rust for parsing and emitting HTML). The JS and API endpoint are
small and follow established patterns.

### Option 3: Generic custom-element extension system
**Approach**
- Generalize the mermaid/abc/magent pattern into a registry where each
  custom block type declares a detection function, a transform, and optional
  assets.

**Pros**
- Would make adding future custom block types trivial.

**Cons**
- Three content types (soon four) does not justify an abstraction.
- Adds indirection and configuration surface for minimal benefit.
- Violates the project invariant: "no plugin systems, premature
  abstractions."

**Verdict:** Overkill. Revisit if we ever reach five or six custom types.

## Recommendation
**Option 2: pre-processor + feature module.** It's the smallest change that
delivers a good experience, follows every existing pattern in the codebase,
and keeps magent-specific code isolated.

## Proposed design

### Magent block vocabulary

Magent writes these elements into markdown files:

| Element | Contains | Rendering |
|---|---|---|
| `<magent-response>` | Full agent response (may contain any of the below) | Styled container with left accent border |
| `<magent-thinking>` | Agent reasoning | Collapsed `<details>` block |
| `<magent-tool-call tool="X">` | Tool input (`<magent-input>`) | Collapsed `<details>` block showing tool name |
| `<magent-tool-result tool="X">` | Tool output | Collapsed `<details>` block showing tool name |
| `<magent-edit status="proposed/accepted">` | A search-and-replace pair (`<magent-search>`, `<magent-replace>`) | Diff-style view with accept button when `status="proposed"` |

### Pre-processor (`render_magent_blocks`)

Runs **first** in the rendering pipeline, before task-list rendering and
before pulldown-cmark.

**Input:** raw markdown string.
**Output:** markdown string with magent blocks replaced by self-contained
HTML that pulldown-cmark will pass through untouched, plus a `has_magent`
flag.

Algorithm:
1. Scan for `<magent-response>` open/close pairs.
2. For each response block, parse the inner structure into a tree of
   magent elements.
3. Render the inner markdown content (text outside magent sub-elements) by
   passing it through pulldown-cmark independently.
4. Emit structured HTML:
   - Wrap the whole response in `<div class="magent-response">`.
   - Wrap thinking blocks in
     `<details class="magent-thinking"><summary>Thinking</summary>...</details>`.
   - Wrap tool calls in
     `<details class="magent-tool-call"><summary>Tool: {name}</summary>...</details>`,
     with input and result as nested collapsed blocks.
   - Wrap edit blocks in `<div class="magent-edit" data-status="proposed">`
     with the search/replace content shown as a simple before/after or
     inline diff.
5. Replace the original `<magent-response>...</magent-response>` region in
   the markdown with the emitted HTML.
6. Leave `@magent` directive lines (the user's input) as regular markdown
   — they render naturally as paragraphs.

Because the emitted HTML contains no blank-line boundaries that would
confuse pulldown-cmark (it is a single HTML block), it passes through the
rest of the pipeline untouched.

### RenderedDocument changes

Add `has_magent: bool` to `RenderedDocument`, following the `has_mermaid` /
`has_abc` / `has_code` pattern.

### Template changes

In `document.html`, conditionally load magent assets when `has_magent` is
true:
```html
{% if has_magent %}
<script type="module" src="/static/magent.js"></script>
{% endif %}
```

Magent CSS can either be a separate conditional stylesheet or folded into
the main `style.css` since the selectors are scoped (`.magent-*` classes
won't match anything in non-magent documents, so the cost is just a few
extra CSS rules).

### Frontend feature module (`assets/features/magent.js`)

Responsibilities:
- **Collapse/expand:** Already handled natively by `<details>/<summary>`
  — no JS needed for basic toggle. JS can optionally add "expand all /
  collapse all" controls.
- **Accept button:** For each `.magent-edit[data-status="proposed"]`,
  inject an "Accept" button. On click, POST to the accept-edit API
  endpoint.
- **Insert directive (stretch):** Add a small input form (e.g. at the
  bottom of the document, or triggered by a button in the toolbar) that
  appends `@magent <query>\n` to the source file via the API. This is a
  convenience — users can always just edit the file directly.

### API endpoint

`POST /api/d/accept-magent-edit`

**Request body:**
```json
{
  "document_id": "notes/project.md",
  "search": "- [Rust](htps://rust-lang.org)",
  "replace": "- [Rust](https://rust-lang.org)"
}
```

**Behavior:**
1. Load the document source.
2. Find the `<magent-edit>` block containing the given search/replace pair.
3. Apply the replacement in the document body (replace the `search` text
   with the `replace` text).
4. Update the edit block's `status` from `"proposed"` to `"accepted"`.
5. Write the file back.
6. Return the updated rendered document (or a success status).

This follows the same pattern as `/api/d/toggle-task`: load file, apply
targeted mutation, write back.

**Security:** Validate that `document_id` resolves within the configured
root (existing path safety). The search string must match exactly in the
document — no regex, no wildcards.

### Directive insertion endpoint (stretch)

`POST /api/d/append-magent-directive`

**Request body:**
```json
{
  "document_id": "notes/project.md",
  "directive": "@magent summarize the key points above"
}
```

**Behavior:** Append the directive text (plus a trailing newline) to the
end of the document. Magent picks it up on the next file-watch cycle.

## Implementation plan

### [x] Task 1: Magent block pre-processor
- Implement `render_magent_blocks(markdown: &str) -> (String, bool)` that
  parses `<magent-*>` blocks and emits structured HTML.
- Handle nested structures: thinking, tool calls, and edits inside
  responses.
- Render inner markdown content via pulldown-cmark.
- Wire it as the first step in `render_document_html`, before
  `render_task_list_markdown`.
- **Acceptance criteria:** A document containing magent response blocks
  produces well-structured HTML with `.magent-response`, `.magent-thinking`,
  `.magent-tool-call`, and `.magent-edit` containers. The `has_magent` flag
  is set correctly. Documents without magent blocks are unaffected.

### [x] Task 2: Template and conditional loading
- Add `has_magent` to `RenderedDocument` and pass it to the template.
- Conditionally load `magent.js` when `has_magent` is true.
- **Acceptance criteria:** Magent JS is loaded only on pages with magent
  content. Pages without magent content are unchanged.

### [x] Task 3: CSS styling
- Style `.magent-response` (accent border, background).
- Style `.magent-thinking`, `.magent-tool-call`, `.magent-tool-result`
  `<details>` blocks (subtle, de-emphasized when collapsed).
- Style `.magent-edit` blocks with before/after or diff presentation.
- **Acceptance criteria:** Magent interactions are visually distinct from
  regular document content. Thinking and tool blocks are collapsed by
  default. Edit proposals are clearly readable.

### [x] Task 4: Accept-edit API endpoint and JS
- Implement `POST /api/d/accept-magent-edit`.
- In `magent.js`, inject "Accept" buttons on proposed edits and wire them
  to the endpoint.
- **Acceptance criteria:** Clicking "Accept" applies the edit to the source
  file, updates the status to "accepted", and the UI reflects the change
  (button disappears or shows "Accepted").

### [ ] Task 5: Tests
- Pre-processor unit tests: simple response, nested thinking, tool calls,
  edit blocks, mixed content, documents with no magent content.
- API endpoint tests: successful accept, edit not found, path traversal
  rejection.
- **Acceptance criteria:** Tests cover the main cases and edge cases
  (multiple edits in one response, already-accepted edits, malformed
  blocks).

### [ ] Task 6: Insert-directive UI (stretch)
- Implement `POST /api/d/append-magent-directive`.
- Add a minimal input form in the document view for inserting directives.
- **Acceptance criteria:** Users can type a magent directive in the UI and
  it appears in the source file. Magent picks it up on its next watch
  cycle.

### [ ] Task 7: Documentation
- Update README to describe magent rendering support.
- Check off the TODO item.
- **Acceptance criteria:** Users know what magent blocks look like and how
  the accept flow works.

## Risks and limitations
- **Format coupling.** If magent changes its tag vocabulary, the
  pre-processor needs updating. Mitigation: magent's format is stable and
  under the same author's control.
- **Nested parsing complexity.** The pre-processor must handle arbitrary
  nesting of magent elements. Mitigation: the nesting is shallow and
  well-defined (response > thinking/tool-call/edit > search/replace).
- **Edit conflicts.** If the document is modified between magent writing
  an edit and the user clicking "Accept", the search string may no longer
  match. Mitigation: the endpoint requires an exact match and returns an
  error if not found — the user sees the failure and can resolve manually.
- **JS required.** Collapse/expand uses native `<details>` (works without
  JS). Accept buttons require JS. This is consistent with the task-toggle
  precedent.

## Non-goals
- Rendering magent config (`.magent/config.toml`, `.magent/state.json`).
- Real-time streaming of magent responses (magent writes complete blocks).
- Editing or managing magent's schedule/timing from the mindex UI.
- A generic plugin/extension system.

## Follow-ups (out of scope)
- Syntax highlighting inside tool-result blocks (depends on content type).
- "Reject" button that deletes a proposed edit block from the source.
- Inline diff view for edits (instead of simple before/after).
- Collapse/expand-all toggle for documents with many magent interactions.
