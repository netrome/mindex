# Math Expression Rendering

## Status
Design

## Goal
Enable rendering of LaTeX-style math expressions in markdown documents.

## Context

Mathematical notation is common in technical documentation, notes, and knowledge bases. Users want to write expressions like `$E = mc^2$` for inline math and `$$\int_0^\infty e^{-x^2} dx = \frac{\sqrt{\pi}}{2}$$` for display (block) math.

### Current state

Mindex uses `pulldown-cmark = "0.13"` for markdown rendering. The current rendering code in `src/app/documents.rs` enables only `Options::ENABLE_TABLES`:

```rust
let mut options = Options::empty();
options.insert(Options::ENABLE_TABLES);
let parser = Parser::new_ext(&rendered, options)
    .map(|event| rewrite_relative_md_links(event, &doc_id));
pulldown_cmark::html::push_html(&mut body, parser);
```

### What pulldown-cmark 0.13 provides

Pulldown-cmark already supports math syntax via `Options::ENABLE_MATH`. When enabled:
- `$...$` emits `Event::InlineMath(CowStr)` 
- `$$...$$` emits `Event::DisplayMath(CowStr)`

However, pulldown-cmark's HTML renderer outputs these as raw text with no special handling. We need to:
1. Enable the math parsing option
2. Convert the LaTeX content to something browsers can render

## Options explored

### Option 1: Client-side rendering (KaTeX/MathJax)

**Approach**: Pass math events through as `<span class="math inline">...</span>` / `<div class="math display">...</div>` and load a JavaScript library to render them in the browser.

**Pros**:
- Full LaTeX support (especially MathJax)
- No new Rust dependencies
- Well-tested, widely used libraries

**Cons**:
- Adds ~200KB+ JavaScript (KaTeX) or ~1MB+ (MathJax) 
- Requires external CDN or bundling JS
- Violates product philosophy: "app should remain minimal"
- FOUC (flash of unstyled content) before JS runs
- Breaks offline-first PWA goal unless JS is bundled

### Option 2: Server-side MathML via `latex2mathml`

**Approach**: Convert LaTeX to MathML at render time. Modern browsers (Chrome 109+, Firefox 71+, Safari 14.1+) render MathML natively.

**Pros**:
- Pure Rust, small dependency (~15KB compiled)
- No JavaScript required
- Native browser rendering (no FOUC)
- Works offline by default
- Aligns with minimal/hackable philosophy

**Cons**:
- Limited LaTeX support compared to KaTeX/MathJax (covers ~90% of common math)
- MathML rendering varies slightly between browsers
- `latex2mathml` crate is less actively maintained (last release 2022, but stable)

### Option 3: Server-side MathML via `pulldown-latex`

**Approach**: Use `pulldown-latex` crate which provides more comprehensive LaTeX parsing and MathML output.

**Pros**:
- More actively maintained than `latex2mathml`
- Better LaTeX compatibility (aims for ~95% KaTeX coverage)
- Pull-parser architecture matches pulldown-cmark style

**Cons**:
- Larger dependency
- Still in active development (may have breaking changes)
- Requires Rust 1.74.1+

### Option 4: Switch to `comrak`

**Approach**: Replace pulldown-cmark with comrak, which is GitHub's markdown parser.

**Pros**:
- GitHub-compatible rendering
- Well-maintained by GitHub engineers
- May simplify future GFM feature additions

**Cons**:
- Comrak does NOT have built-in math support (would still need one of the above)
- Major dependency change for no math benefit
- Different parsing model (AST vs pull parser)
- Larger dependency (~30KB vs ~15KB for pulldown-cmark)
- Would require refactoring existing code (link rewriting, task lists)

**Verdict**: Switching to comrak provides no advantage for math rendering and adds migration cost.

## Recommendation

**Option 2: Server-side MathML via `latex2mathml`**

This is the best fit for mindex's philosophy:
- Minimal: single small dependency
- Hackable: simple integration point
- No JavaScript: aligns with PWA/offline goals
- Good enough: covers the vast majority of math notation needs

If `latex2mathml` proves insufficient, we can later upgrade to `pulldown-latex` or add client-side KaTeX as an optional enhancement.

## Implementation plan

### Task 1: Enable math parsing in pulldown-cmark
- Add `Options::ENABLE_MATH` to the parser options
- Verify math events are emitted correctly
- **Acceptance criteria**: Parser emits `InlineMath`/`DisplayMath` events for `$...$` and `$$...$$`

### Task 2: Add `latex2mathml` dependency and convert math events
- Add `latex2mathml` to Cargo.toml
- Create a custom event handler that converts math events to MathML HTML
- Integrate into the existing parser pipeline
- **Acceptance criteria**: Math expressions render as MathML in HTML output

### Task 3: Add CSS for math styling (if needed)
- MathML is mostly self-styled, but may need minor CSS tweaks
- Consider display math centering and margins
- **Acceptance criteria**: Math renders cleanly in light/dark themes

### Task 4: Handle conversion errors gracefully
- If `latex2mathml` fails to parse, show the raw LaTeX in a `<code>` block with error styling
- Log a warning but don't fail the whole document
- **Acceptance criteria**: Invalid LaTeX doesn't break page rendering

### Task 5: Add tests
- Unit tests for math event handling
- Integration test with sample document containing math
- **Acceptance criteria**: Tests cover inline math, display math, and error cases

### Task 6: Update documentation
- Add math syntax to any user-facing docs
- Note browser compatibility requirements (if any)
- **Acceptance criteria**: README or docs mention math support

## Example implementation sketch

```rust
use latex2mathml::{latex_to_mathml, DisplayStyle};
use pulldown_cmark::{Event, Options, Parser};

fn render_markdown(contents: &str, doc_id: &str) -> String {
    let mut body = String::new();
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_MATH);
    
    let parser = Parser::new_ext(contents, options).map(|event| {
        match event {
            Event::InlineMath(latex) => {
                match latex_to_mathml(&latex, DisplayStyle::Inline) {
                    Ok(mathml) => Event::Html(mathml.into()),
                    Err(_) => Event::Code(latex), // fallback
                }
            }
            Event::DisplayMath(latex) => {
                match latex_to_mathml(&latex, DisplayStyle::Block) {
                    Ok(mathml) => Event::Html(mathml.into()),
                    Err(_) => Event::Html(
                        format!("<pre><code>{}</code></pre>", html_escape(&latex)).into()
                    ),
                }
            }
            other => rewrite_relative_md_links(other, doc_id),
        }
    });
    
    pulldown_cmark::html::push_html(&mut body, parser);
    body
}
```

## Risks and limitations

1. **Browser compatibility**: MathML requires relatively modern browsers. Users on older browsers will see raw MathML markup.
   - Mitigation: Document minimum browser versions. Consider polyfill as future enhancement.

2. **LaTeX coverage**: `latex2mathml` doesn't support all LaTeX commands.
   - Mitigation: Document supported subset. Error handling shows raw LaTeX on failure.

3. **Maintenance**: `latex2mathml` is stable but not actively developed.
   - Mitigation: It's a simple crate; we can fork/maintain if needed. Can upgrade to `pulldown-latex` later.

## Non-goals

- Syntax highlighting for math in the editor (separate feature)
- Live preview of math while editing (would require client-side JS)
- Supporting non-LaTeX math syntaxes (AsciiMath, etc.)

## Follow-ups (out of scope)

- [ ] Consider `pulldown-latex` if `latex2mathml` proves limiting
- [ ] Consider optional client-side KaTeX for users who need full LaTeX support
- [ ] Math in search snippets (currently would show raw LaTeX)