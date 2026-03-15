# Code Style Guide

Conventions for the Mindex codebase. These are not aspirational — they
describe what the code already does. Follow them for new code and refactors.

## Module organization

### Public before private

All public (`pub`/`pub(crate)`) functions are defined before any private
(`fn`) functions. The external interface of a module is more significant than
its internal helpers, and should be what the reader encounters first.

```
// ✓ public API first
pub fn search_documents(…) { … }
pub fn render_document_html(…) { … }
pub fn render_task_list_markdown(…) { … }
fn find_match_snippet(…) { … }     // private helper
fn is_mermaid_info(…) { … }        // private helper
```

### Callers before callees

Within the public (or private) section, if function A calls function B,
define A before B. The reader encounters the high-level API before the
implementation details.

Public types that appear in a function's signature are defined immediately
above that function:

```
pub struct RenderedDocument { … }
pub fn render_document_html(…) -> RenderedDocument { … }
```

### Separation of concerns

Domain logic lives in top-level modules (`documents.rs`, `git.rs`,
`auth.rs`, …). These modules contain pure functions or functions that take a
`&Path` root — they never depend on HTTP types (`axum`, `StatusCode`, etc.).

HTTP handlers live in `app/` and are thin wrappers: extract request data,
call a domain function, map the result into a response. Keep handlers short;
push logic down into the domain layer where it can be unit-tested without
HTTP.

```
// app/documents.rs — thin handler
let contents = load_document(&state.config.root, &doc_id).map_err(…)?;
let rendered = render_document_html(&contents, &doc_id);
Ok(templates::DocumentTemplate { … })
```

### I/O at the edges

Business logic should be free of direct I/O. Ideally, functions are generic
over a trait defining the interface they need, so they can be tested with
fakes. The push notification system demonstrates this: domain code depends on
the `PushSender` trait (`ports/push.rs`), and the real `WebPushSender`
(`adapters.rs`) is injected only at the call site.

**Pragmatic exception**: for filesystem access, functions currently take a
`&Path` root directly rather than abstracting behind a trait. At this scale
the trait doesn't yet pay for itself, but new I/O boundaries (external
services, network calls) should use the trait-based approach.

## Naming

### Functions and types

- Use descriptive names that convey intent (`render_document_html`, not
  `render` or `process`).
- Avoid redundant prefixes that repeat the module name.

### `AppState` fields

Name fields by what they hold, not where they came from. `registries` (not
`push_registries`) because the struct holds users, subscriptions, and
notifications — not just push-related data.

## Testing

### Test naming

Use double underscores to separate the subject from the expectation:

```
fn function_name__should_describe_expected_behavior()
```

Every test module carries `#[allow(non_snake_case)]` to permit this
convention.

### Test structure

Use `// Given`, `// When`, `// Then` comment sections:

```rust
#[test]
fn search_documents__should_find_matching_docs() {
    // Given
    let root = create_temp_root("search");
    std::fs::write(root.join("alpha.md"), "Alpha has the needle").expect("write");

    // When
    let results = search_documents(&root, "needle").expect("search");

    // Then
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].doc_id, "alpha.md");
}
```

For trivial one-liner assertions the sections can be omitted, but prefer them
for anything with setup.

### Test isolation

Tests that touch the filesystem use `create_temp_root` (shared test helper) to
get a unique temporary directory. Clean up with `std::fs::remove_dir_all` at
the end.
