use crate::fs::{atomic_write, ensure_parent_dirs};
use std::io::ErrorKind;
use std::path::Path;

mod editing;
mod magent;
mod paths;
mod rendering;
mod search;
mod tasks;

#[allow(unused_imports)]
pub(crate) use editing::{
    BlockKind, BlockRange, ReorderError, line_count, lines_for_display, reorder_range,
    scan_block_ranges,
};
pub(crate) use magent::accept_magent_edit;
#[allow(unused_imports)]
pub(crate) use paths::{
    DirectoryFile, DirectoryListing, FileKind, collect_markdown_paths, doc_id_from_path,
    highlight_lang_for_extension, is_text_extension, list_directory, resolve_doc_path,
    resolve_text_file_path,
};
#[allow(unused_imports)]
pub(crate) use rendering::{
    RenderedDocument, render_document_html, render_task_list_markdown,
    rewrite_relative_image_links, rewrite_relative_md_links,
};
pub(crate) use search::{SearchResult, search_documents};
pub(crate) use tasks::{add_task_item_in_list, collect_mentions, toggle_task_item};

use paths::doc_id_to_path;

#[derive(Debug)]
pub(crate) enum DocError {
    BadPath,
    NotFound,
    Io(std::io::Error),
}

pub(crate) fn load_document(root: &Path, doc_id: &str) -> Result<String, DocError> {
    let path = resolve_doc_path(root, doc_id)?;
    std::fs::read_to_string(&path).map_err(|err| match err.kind() {
        ErrorKind::NotFound | ErrorKind::IsADirectory => DocError::NotFound,
        _ => DocError::Io(err),
    })
}

pub(crate) fn load_text_file(root: &Path, file_id: &str) -> Result<String, DocError> {
    let path = resolve_text_file_path(root, file_id)?;
    std::fs::read_to_string(&path).map_err(|err| match err.kind() {
        ErrorKind::NotFound | ErrorKind::IsADirectory => DocError::NotFound,
        _ => DocError::Io(err),
    })
}

pub(crate) fn create_document(root: &Path, doc_id: &str, contents: &str) -> Result<(), DocError> {
    let doc_path = doc_id_to_path(doc_id).ok_or(DocError::BadPath)?;
    ensure_parent_dirs(root, &doc_path).map_err(map_io_to_doc_error)?;
    let target = root.join(&doc_path);

    match std::fs::symlink_metadata(&target) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() {
                return Err(DocError::BadPath);
            }
            return Err(DocError::Io(std::io::Error::new(
                ErrorKind::AlreadyExists,
                "document already exists",
            )));
        }
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(DocError::Io(err)),
    }

    atomic_write(&target, contents).map_err(DocError::Io)
}

pub(crate) fn normalize_newlines(contents: &str) -> String {
    if !contents.contains('\r') {
        return contents.to_string();
    }
    let normalized = contents.replace("\r\n", "\n");
    normalized.replace('\r', "\n")
}

fn map_io_to_doc_error(err: std::io::Error) -> DocError {
    if err.kind() == ErrorKind::InvalidInput {
        DocError::BadPath
    } else {
        DocError::Io(err)
    }
}

fn is_fence_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("```") || trimmed.starts_with("~~~")
}

fn split_line_ending(segment: &str) -> (&str, &str) {
    if let Some(without_nl) = segment.strip_suffix('\n') {
        if let Some(without_cr) = without_nl.strip_suffix('\r') {
            return (without_cr, "\r\n");
        }
        return (without_nl, "\n");
    }
    if let Some(without_cr) = segment.strip_suffix('\r') {
        return (without_cr, "\r");
    }
    (segment, "")
}

fn detect_line_ending(contents: &str) -> &'static str {
    if contents.contains("\r\n") {
        "\r\n"
    } else if contents.contains('\n') {
        "\n"
    } else if contents.contains('\r') {
        "\r"
    } else {
        "\n"
    }
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::test_support::create_temp_root;

    #[test]
    fn normalize_newlines__should_convert_crlf_to_lf() {
        // Given
        let contents = "a\r\nb\rc";

        // When
        let normalized = normalize_newlines(contents);

        // Then
        assert_eq!(normalized, "a\nb\nc");
    }

    #[test]
    fn create_document__should_create_file_and_parent_dirs() {
        // Given
        let root = create_temp_root("create");

        // When
        create_document(&root, "notes/new.md", "# New\n").expect("create document");

        // Then
        let contents = std::fs::read_to_string(root.join("notes/new.md")).expect("read file");
        assert_eq!(contents, "# New\n");
    }

    #[test]
    fn create_document__should_reject_duplicate_paths() {
        // Given
        let root = create_temp_root("create-existing");
        std::fs::write(root.join("a.md"), "A").expect("write a.md");

        // When
        let err = create_document(&root, "a.md", "B").expect_err("should fail");

        // Then
        match err {
            DocError::Io(err) => assert_eq!(err.kind(), ErrorKind::AlreadyExists),
            _ => panic!("expected already exists error"),
        }
    }

    #[test]
    fn create_document__should_reject_parent_traversal() {
        // Given
        let root = create_temp_root("create-bad-path");

        // When
        let err = create_document(&root, "../outside.md", "oops").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::BadPath));
    }

    #[cfg(unix)]
    #[test]
    fn create_document__should_reject_symlinked_parent() {
        use std::os::unix::fs::symlink;

        // Given
        let root = create_temp_root("create-symlink");
        let outside = create_temp_root("create-symlink-outside");
        symlink(&outside, root.join("link")).expect("create symlink");

        // When
        let err = create_document(&root, "link/escape.md", "oops").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::BadPath));
    }
}
