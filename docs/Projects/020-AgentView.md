# Agent View

## Status
Proposed

## Goal
Provide a dedicated view for interacting with magent: browsing rendered
document content, inserting directives at any block boundary, and
accepting proposed edits. Move all magent interactivity out of the normal
document view into this purpose-built view, keeping the normal view clean.

## Context
- [Project 019](019-MagentRendering.md) delivered magent block
  pre-processing, CSS styling, and accept-edit functionality in the
  normal document view (tasks 1–5 complete).
- Task 6 ("insert-directive UI") originally called for appending
  directives to the end of the document. Discussion revealed that
  **positional insertion** (at any block boundary) is much more valuable
  — directive position is semantically meaningful to magent.
- Mapping rendered HTML back to source positions in the normal view is
  complex: the rendering pipeline (magent pre-processing → task lists →
  pulldown-cmark) makes it hard to track source line numbers through to
  the final output.
- The **reorder view** already solves this problem:
  `scan_block_ranges()` identifies blocks with source line ranges, and a
  dedicated view presents them with position-aware controls.
- Decision: create a dedicated "agent view" following the reorder
  pattern, and clean up the normal document view to ignore magent
  response blocks entirely.

## Design

### Agent view (`/agent/{*path}`)

A dedicated view accessed from the document toolbar. Shows the document
content as a sequence of **rendered blocks** with:
- **Insert points** between blocks: "+" buttons that expand into inline
  text inputs. The `@magent` prefix is auto-prepended — the user just
  types their question.
- **Magent response blocks** rendered with full structure: collapsed
  thinking/tool-calls, styled edits with accept buttons.
- **Regular content blocks** rendered through pulldown-cmark.

### Block identification

Reuse `scan_block_ranges()` from `src/documents/editing.rs`. This
identifies headings, paragraphs, lists, tables, code fences, and blank
lines with their source line ranges.

Magent response blocks need special handling: a
`<magent-response>...</magent-response>` region may span many lines that
`scan_block_ranges()` classifies as multiple separate blocks. The agent
view handler must detect magent response boundaries and **merge
consecutive scan blocks** that fall within a response into a single
logical block, tagged as a magent response.

Algorithm:
1. Run `scan_block_ranges()` on the raw document.
2. Scan the raw document for `<magent-response>` open/close line
   positions.
3. Walk the block list. When a block's start line falls within a magent
   response region, merge it with adjacent blocks into a single
   "MagentResponse" block spanning the full response.
4. The merged block carries the raw content of the entire
   `<magent-response>...</magent-response>` region.

### Per-block rendering

Each block is rendered independently for display in the agent view:

- **Regular blocks** (paragraphs, headings, lists, tables, fences):
  extract the source lines, render through pulldown-cmark with the same
  options as the main rendering pipeline.
- **Magent response blocks**: render through the existing
  `render_magent_blocks()` pre-processor (which produces structured HTML
  with thinking/tool-call/edit containers), then pass the output through
  pulldown-cmark for any inner markdown.
- **Blank blocks**: rendered as vertical spacing. No insert point
  between consecutive blanks.

**Trade-off:** rendering blocks independently means cross-block markdown
features (reference-style links, footnotes) won't resolve. This is
acceptable — such features are rare, and the agent view is for
interaction, not pixel-perfect rendering.

### Insert points

Between every two adjacent non-blank blocks, a thin row contains a small
"+" button on the left margin (similar to the reorder view's drag
handle). An insert point also appears at the end of the document.

**Collapsed state (default):** the row is minimal — just the "+" button
on the left, with the rest of the row acting as a subtle separator
between blocks. Non-intrusive; the view reads like a normal document.

**Expanded state:** clicking the "+" expands the row into an inline text
area + submit button. The text area is focused automatically. Pressing
Escape or clicking away collapses it back. Only one insert point can be
expanded at a time (expanding one collapses any other).

The form auto-prepends `@magent ` to the submitted text. The user sees
only a text area and a submit button — no mention of "magent" in the UI
chrome.

On submit, the JS POSTs to the insert-directive API with the `after_line`
value from the preceding block's `data-end-line` attribute.

### Normal document view cleanup

With all magent interactivity moving to the agent view:
- **Strip magent response blocks** during rendering: a new
  `strip_magent_blocks()` function removes
  `<magent-response>...</magent-response>` regions from the markdown
  before pulldown-cmark processes it. This replaces the current
  `render_magent_blocks()` call in the normal rendering pipeline.
- **Keep `@magent` directive lines** — these are regular markdown text
  and render naturally as paragraphs. They serve as a visible record of
  what the user asked.
- **Remove `has_magent`** from `RenderedDocument` and the document
  template. Remove the conditional `magent.js` loading.
- **Keep magent CSS** in `style.css` — it's used by the agent view
  template.

### API endpoint

`POST /api/d/insert-magent-directive`

**Request body (form-urlencoded):**
```
doc_id=notes/project.md
after_line=15
directive=summarize the key points above
```

**Behavior:**
1. Validate `doc_id` resolves within root.
2. Read the source file.
3. Insert `\n@magent {directive}\n` after the specified 0-based line
   index. `after_line` equal to the total line count means append to the
   end.
4. Atomic write.
5. Refresh push state.
6. Return `204 No Content`.

**Validation:**
- `after_line` must be in range `[0, line_count]`.
- `directive` must not be empty after trimming.

Follows the same pattern as `reorder-range`, `toggle-task`, and
`accept-magent-edit`.

### Accept-edit in agent view

The existing `POST /api/d/accept-magent-edit` endpoint stays unchanged.
The accept-button logic moves from `features/magent.js` (loaded on the
normal document view) into the agent view's JS module.
`features/magent.js` can be removed.

### Template and JS structure

| Component | File |
|---|---|
| Route | `src/app.rs` — add `/agent/{*path}` |
| Handler | `src/app/documents.rs` — `document_agent_view` |
| Block merging | `src/documents/magent.rs` or `src/documents/editing.rs` |
| Strip function | `src/documents/magent.rs` — `strip_magent_blocks()` |
| Template struct | `src/templates.rs` — `AgentViewTemplate` |
| Template | `templates/agent.html` |
| JS | `assets/features/agent.js` (insert + accept) |
| CSS | `assets/style.css` (agent view + existing magent styles) |
| Insert API | `src/app/documents.rs` — `document_insert_magent_directive` |

### Mockup

Collapsed state — insert points are minimal "+" buttons on the left:
```
┌──────────────────────────────────────────────────┐
│  Back  New  Upload  View  Edit  Reorder  Search  │
├──────────────────────────────────────────────────┤
│  Agent: notes/project.md                         │
├──────────────────────────────────────────────────┤
│     ┌──────────────────────────────────────┐     │
│     │ # Project Notes                      │     │  ← rendered heading
│     └──────────────────────────────────────┘     │
│ [+] ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─      │  ← insert point
│     ┌──────────────────────────────────────┐     │
│     │ Some introductory paragraph about    │     │  ← rendered paragraph
│     │ the project and its goals.           │     │
│     └──────────────────────────────────────┘     │
│ [+] ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─      │  ← insert point
│     ┌─magent-response─────────────────────┐      │
│     │ ▶ Thinking...                       │      │  ← collapsed
│     │ ▶ Tool: read_file                   │      │  ← collapsed
│     │                                     │      │
│     │ Here is a summary of the key...     │      │  ← rendered response
│     │                                     │      │
│     │ ┌─edit (proposed)──────────────┐    │      │
│     │ │ - old text                   │    │      │
│     │ │ + new text                   │    │      │
│     │ │                  [ Accept ]  │    │      │
│     │ └──────────────────────────────┘    │      │
│     └─────────────────────────────────────┘      │
│ [+] ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─      │  ← insert point
│     ┌──────────────────────────────────────┐     │
│     │ ## Next Steps                        │     │  ← rendered heading
│     └──────────────────────────────────────┘     │
│ [+] ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─      │  ← insert point (end)
└──────────────────────────────────────────────────┘
```

Expanded state — one insert point expanded into an input form:
```
│ [+] ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─ ─      │
│     ┌──────────────────────────────────────┐     │
│     │ Some introductory paragraph...       │     │
│     └──────────────────────────────────────┘     │
│ [-] ┌──────────────────────────────────────┐     │  ← expanded
│     │                                      │     │
│     │  [What would you like to ask?    ]   │     │  ← text area
│     │                          [ Send ]    │     │
│     └──────────────────────────────────────┘     │
│     ┌─magent-response─────────────────────┐      │
```

## Implementation plan

### [ ] Task 1: Strip magent blocks from normal view
- Add `strip_magent_blocks(markdown: &str) -> String` to
  `src/documents/magent.rs`. Same structure as `render_magent_blocks` but
  removes response blocks instead of transforming them.
- Replace the `render_magent_blocks()` call in the normal rendering
  pipeline with `strip_magent_blocks()`.
- Remove `has_magent` from `RenderedDocument` and the document template.
- Delete `assets/features/magent.js`.
- **Acceptance criteria:** Normal document view shows `@magent` directive
  lines as regular text. Magent response blocks are not visible. No
  magent JS loaded on normal pages. Existing non-magent documents are
  unaffected.

### [ ] Task 2: Agent view route, handler, and template
- Add `/agent/{*path}` route in `src/app.rs`.
- Implement `document_agent_view` handler: load document, run
  `scan_block_ranges()`, detect magent response regions, merge blocks,
  render each block independently.
- Add `AgentViewTemplate` to `src/templates.rs`.
- Create `templates/agent.html` — vertical list of rendered blocks with
  insert-point rows between them ("+").
- Add "Agent" link to document view toolbar.
- **Acceptance criteria:** `/agent/note.md` shows the document as a
  sequence of rendered blocks with "+" insert buttons between them.
  Magent responses are rendered with collapsed thinking/tool-calls and
  styled edits.

### [ ] Task 3: Insert-directive API and JS
- Implement `POST /api/d/insert-magent-directive` endpoint.
- In `assets/features/agent.js`: wire "+" buttons to expand into input
  forms, handle form submission, POST to API.
- Auto-prepend `@magent ` to the directive text.
- **Acceptance criteria:** Clicking "+" between blocks opens an input.
  Typing a question and submitting inserts `@magent <query>` at the
  correct source line. Page reload confirms the directive is in place.

### [ ] Task 4: Accept-edit in agent view
- Move accept-button logic into `assets/features/agent.js`.
- Wire accept buttons on `.magent-edit[data-status="proposed"]` elements
  to `POST /api/d/accept-magent-edit`.
- **Acceptance criteria:** Accept buttons in the agent view work
  correctly. Clicking "Accept" applies the edit and updates the UI.

### [ ] Task 5: Tests
- `strip_magent_blocks()` unit tests: responses stripped, directives
  kept, nested responses, no magent content.
- Agent view handler tests: block merging with magent responses,
  per-block rendering.
- Insert-directive API tests: valid insert, boundary insert (end of
  file), empty directive rejection, path traversal rejection.
- **Acceptance criteria:** Tests cover main paths and edge cases.

## Risks and limitations
- **Per-block rendering fidelity.** Rendering blocks independently may
  differ slightly from full-document rendering. Cross-block features
  (reference links, footnotes) won't resolve. Acceptable for an
  interaction view.
- **Magent block merging.** A `<magent-response>` region may not align
  cleanly with `scan_block_ranges()` output. The merging logic needs to
  handle partial overlaps. Mitigation: magent responses always start and
  end on their own lines, and `scan_block_ranges()` respects line
  boundaries — so alignment should be clean in practice.
- **Line number staleness.** If the file changes between render and
  form submission, the `after_line` value may be stale. Same risk as
  the reorder view — acceptable, and the user refreshes anyway.
- **`scan_block_ranges()` doesn't know about magent tags.** It will
  classify `<magent-response>` as a paragraph. The merging step handles
  this, but if `scan_block_ranges()` ever changes its heuristics, the
  merge logic may need updating.

## Non-goals
- Real-time streaming of magent responses.
- Editing document content from the agent view (beyond inserting
  directives and accepting edits).
- Drag-and-drop reordering in the agent view.
- A reject/delete button for proposed edits (follow-up).

## Follow-ups (out of scope)
- "Reject" button that removes a proposed edit from the source.
- Auto-refresh or polling for magent responses after inserting a
  directive.
- Keyboard shortcut to open insert form (e.g. `/` key).
- Syntax highlighting inside tool-result blocks.
