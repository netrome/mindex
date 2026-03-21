use super::paths::{collect_markdown_paths, doc_id_from_path};
use std::path::Path;

pub(crate) struct SearchResult {
    pub(crate) doc_id: String,
    pub(crate) snippet: String,
}

pub(crate) fn search_documents(root: &Path, query: &str) -> std::io::Result<Vec<SearchResult>> {
    let mut results = Vec::new();
    let needle = query.to_lowercase();
    for path in collect_markdown_paths(root)? {
        let doc_id = match doc_id_from_path(root, &path) {
            Some(doc_id) => doc_id,
            None => continue,
        };
        let contents = std::fs::read_to_string(&path)?;
        if let Some(snippet) = find_match_snippet(&contents, &needle) {
            results.push(SearchResult { doc_id, snippet });
        }
    }
    results.sort_by(|a, b| a.doc_id.cmp(&b.doc_id));
    Ok(results)
}

fn find_match_snippet(contents: &str, needle: &str) -> Option<String> {
    for line in contents.lines() {
        if line.to_lowercase().contains(needle) {
            return Some(line.trim().to_string());
        }
    }
    None
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::test_support::create_temp_root;

    #[test]
    fn find_match_snippet__should_return_first_matching_line() {
        // Given
        let contents = "First line\nSecond contains hello world\nThird line\n";

        // When
        let snippet = find_match_snippet(contents, "hello");

        // Then
        assert_eq!(snippet, Some("Second contains hello world".to_string()));
    }

    #[test]
    fn find_match_snippet__should_be_case_insensitive() {
        // Given
        let contents = "Title: Hello World\n";

        // When
        let snippet = find_match_snippet(contents, "hello");

        // Then
        assert_eq!(snippet, Some("Title: Hello World".to_string()));
    }

    #[test]
    fn find_match_snippet__should_return_none_when_no_match() {
        // Given
        let contents = "Nothing relevant here\n";

        // When
        let snippet = find_match_snippet(contents, "missing");

        // Then
        assert_eq!(snippet, None);
    }

    #[test]
    fn search_documents__should_find_matching_docs() {
        // Given
        let root = create_temp_root("search");
        std::fs::write(root.join("alpha.md"), "Alpha has the needle").expect("write");
        std::fs::write(root.join("beta.md"), "Beta has nothing").expect("write");
        std::fs::write(root.join("gamma.md"), "Gamma also has the needle").expect("write");

        // When
        let results = search_documents(&root, "needle").expect("search");

        // Then
        assert_eq!(results.len(), 2);
        assert_eq!(results[0].doc_id, "alpha.md");
        assert_eq!(results[0].snippet, "Alpha has the needle");
        assert_eq!(results[1].doc_id, "gamma.md");

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn search_documents__should_return_empty_for_no_matches() {
        // Given
        let root = create_temp_root("search-empty");
        std::fs::write(root.join("doc.md"), "Nothing here").expect("write");

        // When
        let results = search_documents(&root, "missing").expect("search");

        // Then
        assert!(results.is_empty());

        std::fs::remove_dir_all(&root).expect("cleanup");
    }
}
