# Engineering Excellence Sweep

## Status
In progress

## Goal
Bring the codebase to a high bar of quality, readability, and maintainability
before adding new features. Address bugs, security issues, code duplication,
naming problems, and stale documentation found during a full code review.

## Context
Mindex has grown organically over several months across multiple feature
additions (push notifications, auth, git integration, PDF support, ABC
notation, reorder mode). Each feature was well-implemented in isolation, but the
cumulative result has a few rough edges: duplicated helpers, a naming mismatch
where the auth system depends on a struct called `push_registries`, a debug
endpoint that leaks password hashes, and minor inconsistencies between modules.

None of these are showstoppers for the current usage, but they increase the risk
of bugs and make the code harder to reason about as more features are added.

## Findings

### Security

**S1: Password hashes exposed via debug endpoint**
`/api/debug/push/registry` returns the full `DirectiveRegistries` as JSON,
which includes `password_hash` for every user. The `User` struct derives
`Serialize` unconditionally. Even with auth enabled, any authenticated user can
read everyone's hashes.

### Correctness

**C1: Fence detection inconsistency between documents and directives**
`documents.rs` treats both `` ``` `` and `~~~` as fence delimiters.
`directives.rs` only recognizes backtick fences. If a directive block is wrapped
in `~~~` fences, the directive parser will treat the content as a real directive
while the document renderer hides it inside a code block. This could cause
unintended user registrations or notification scheduling.

**C2: `render_task_list_form` doesn't HTML-escape `doc_id`**
`documents.rs:681` interpolates `doc_id` directly into an HTML attribute value
without escaping. The `doc_id` is validated (normal path components, `.md`
extension), so exploitation is unlikely, but this is a defense-in-depth gap.

**C3: `collect_markdown_paths` recurses into hidden directories**
The recursive directory walker enters `.git`, `.obsidian`, etc. Only `.md` files
are collected so it's functionally correct, but it causes unnecessary I/O and
could pick up `.md` files that live inside `.git` or other tool directories.

### Naming and organization

**N1: `push_registries` naming is misleading**
`AppState.push_registries` holds users, subscriptions, and notifications. It is
the primary user store used for authentication (login credential lookup). The
`push_` prefix is confusing when reading auth code. Should be renamed to
`registries` or `directive_registries`.

**N2: `search_documents` lives in the handler module**
The full-text search logic (iterate files, match text, return snippets) is in
`app/documents.rs`. This is domain logic that belongs in `documents.rs`
alongside `load_document` and `collect_markdown_paths`, making it independently
testable without HTTP.

**N3: `document_view` handler does too much**
The markdown-to-HTML pipeline (parsing, mermaid/abc extraction, math rendering,
HTML generation) is inlined in the `document_view` handler (~100 lines). Extracting
it into a function in `documents.rs` would improve testability and keep the
handler focused on HTTP concerns.

### Code duplication

**D1: Two `atomic_write` implementations**
`documents.rs` has `atomic_write(path, &str)` and `uploads.rs` has
`atomic_write_bytes(path, &[u8])`. Identical temp-file-then-rename logic. The
`str` version should delegate to the `bytes` version.

**D2: Two `ensure_parent_dirs` implementations**
`documents.rs` and `uploads.rs` each have their own, differing only in error
type. Should be consolidated.

**D3: `create_temp_root` test helper duplicated 5 times**
Identical helper in `documents::tests`, `directives::tests`, `git::tests`,
`uploads::tests`, and `cli::tests`. Should be a shared test utility.

### Housekeeping

**H1: Stale `#[allow(unused)]` with outdated TODO**
`types/push.rs:4` has `#[allow(unused)] // TODO: Will be used when we implement
subscription UI` but the subscription UI exists and the field is actively used.

**H2: Dead wrapper function `resolve_relative_doc_id`**
`documents.rs:990-993` is a trivial pass-through to `resolve_relative_path`.
Its single call site can call `resolve_relative_path` directly.

### Documentation

**DOC1: ARCHITECTURE.md is stale**
Does not reflect the current module structure. Missing: `app/` submodules
(`auth.rs`, `documents.rs`, `git.rs`, `push.rs`, `uploads.rs`), `cli.rs`,
`git.rs`, `adapters.rs`.

**DOC2: Typo in INVARIANTS.md**
Line 30: "secyrity" should be "security".

## Task breakdown

Each task is a single focused PR.

### Security

- [x] **Task S1: Stop leaking password hashes in debug endpoint**
  - Either exclude `password_hash` from serialization (e.g. `#[serde(skip)]`)
    or filter the response in the handler.
  - Acceptance criteria: `/api/debug/push/registry` response contains no
    `password_hash` fields. Existing tests updated.

### Correctness

- [ ] **Task C1: Align fence detection between documents and directives**
  - Add `~~~` fence support to `directives.rs`, matching `documents.rs`.
  - Acceptance criteria: A `/user` block inside `~~~` fences is treated as
    fenced (not parsed as a directive). Test added.

- [ ] **Task C2: HTML-escape `doc_id` in task list form**
  - Use the existing `html_escape` function from `math.rs` (or equivalent) when
    interpolating `doc_id` into the form HTML.
  - Acceptance criteria: A `doc_id` containing `"` does not break the HTML.
    Test added.

- [ ] **Task C3: Skip hidden directories in `collect_markdown_paths`**
  - Skip directory entries whose name starts with `.` during recursive traversal.
  - Acceptance criteria: `.git/` and `.obsidian/` are not entered. Test added.

### Naming and organization

- [ ] **Task N1: Rename `push_registries` to `registries`**
  - Rename the field in `AppState` and all references.
  - Acceptance criteria: `cargo clippy` and all tests pass. No functional
    change.

- [ ] **Task N2: Move `search_documents` to `documents.rs`**
  - Move `search_documents` and `find_match_snippet` from `app/documents.rs`
    to `documents.rs`. Add unit tests.
  - Acceptance criteria: Search works as before. New unit tests for
    `search_documents` without HTTP.

- [ ] **Task N3: Extract markdown rendering from `document_view`**
  - Create a `render_document_html` function in `documents.rs` that takes
    markdown content and doc_id, returns rendered HTML + flags (has_mermaid,
    has_abc).
  - Acceptance criteria: `document_view` delegates to the new function. New
    unit tests for rendering. Existing integration tests still pass.

### Code duplication

- [ ] **Task D1: Consolidate `atomic_write`**
  - Keep `atomic_write_bytes` in a shared location. Have `atomic_write` call it
    with `.as_bytes()`.
  - Acceptance criteria: Single implementation. All tests pass.

- [ ] **Task D2: Consolidate `ensure_parent_dirs`**
  - Extract a shared implementation that returns `std::io::Error`, with callers
    mapping to their domain error type.
  - Acceptance criteria: Single implementation. All tests pass.

- [ ] **Task D3: Extract shared test utility for temp dirs**
  - Create a `#[cfg(test)]` helper module with `create_temp_root`.
  - Acceptance criteria: All 5 test modules use the shared helper. All tests
    pass.

### Housekeeping

- [ ] **Task H1: Remove stale `#[allow(unused)]` and TODO**
  - Remove the annotation and comment from `types/push.rs:4`.
  - Acceptance criteria: `cargo clippy` passes.

- [ ] **Task H2: Inline `resolve_relative_doc_id`**
  - Replace the single call site with a direct call to `resolve_relative_path`.
    Remove the function.
  - Acceptance criteria: All tests pass. No functional change.

### Documentation

- [ ] **Task DOC1: Update ARCHITECTURE.md**
  - Add missing modules and submodules. Remove outdated descriptions.
  - Acceptance criteria: Module list matches current `src/` layout.

- [ ] **Task DOC2: Fix typo in INVARIANTS.md**
  - "secyrity" -> "security"
  - Acceptance criteria: Typo fixed.

## Non-goals
- No new features.
- No refactors beyond what is listed above.
- No dependency changes.

## Risks
- N1 (rename `push_registries`) touches many files. Keeping it as a pure rename
  with no logic changes minimizes risk.
- N3 (extract rendering) changes the handler structure. Comprehensive existing
  integration tests provide a safety net.
