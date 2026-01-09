use crate::documents::{collect_markdown_paths, doc_id_from_path};
use crate::push::directives;
use crate::types::push::DirectiveRegistries;

use std::path::Path;

impl DirectiveRegistries {
    pub fn load(root: &Path) -> std::io::Result<Self> {
        let mut registries = DirectiveRegistries::default();
        let paths = collect_markdown_paths(root)?;
        for path in paths {
            let doc_id = match doc_id_from_path(root, &path) {
                Some(doc_id) => doc_id,
                None => continue,
            };
            let contents = std::fs::read_to_string(&path)?;
            let warnings = directives::parse_document(&doc_id, &contents, &mut registries);
            for warning in warnings {
                eprintln!(
                    "push directive warning: {}:{}: {}",
                    warning.doc_id, warning.line, warning.message
                );
            }
        }
        Ok(registries)
    }
}
