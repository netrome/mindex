---
name: No mod.rs files
description: This repo uses the file+directory convention (e.g. push.rs + push/) instead of mod.rs for multi-file modules
type: feedback
---

Do not use `mod.rs` files when splitting modules into submodules. Use the parent-file convention instead: `src/foo.rs` as the parent with submodules in `src/foo/*.rs`.

**Why:** Repo convention — all existing multi-file modules (`app`, `push`, `ports`, `types`) follow this pattern.

**How to apply:** When creating submodules for any module, keep the parent as `src/module.rs` and add children under `src/module/`.
