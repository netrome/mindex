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
pub(crate) use magent::{
    MagentRegion, accept_magent_edit, find_magent_regions, insert_directive,
    remove_magent_interaction, render_magent_blocks,
};
#[allow(unused_imports)]
pub(crate) use paths::{
    DirectoryFile, DirectoryListing, FileKind, collect_markdown_paths, doc_id_from_path,
    highlight_lang_for_extension, is_text_extension, list_directory, resolve_doc_path,
    resolve_text_file_path,
};
#[allow(unused_imports)]
pub(crate) use rendering::{
    RenderedDocument, render_document_html, render_markdown_snippet, render_task_list_markdown,
    rewrite_relative_image_links, rewrite_relative_md_links,
};
pub(crate) use search::{SearchResult, search_documents};
pub(crate) use tasks::{add_task_item_in_list, collect_mentions, toggle_task_item};

use paths::{dir_to_path, doc_id_to_path, supported_file_id_to_path};

#[derive(Debug)]
pub(crate) enum DocError {
    BadPath,
    NotFound,
    Conflict,
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

pub(crate) fn move_file(root: &Path, source_path: &str, target_dir: &str) -> Result<(), DocError> {
    let source_rel = supported_file_id_to_path(source_path).ok_or(DocError::BadPath)?;
    let target_dir_rel = dir_to_path(target_dir).ok_or(DocError::BadPath)?;

    // Resolve source: must exist, be a file, and be within root.
    let source_abs = root.join(&source_rel);
    let source_resolved = source_abs.canonicalize().map_err(|err| match err.kind() {
        ErrorKind::NotFound => DocError::NotFound,
        _ => DocError::Io(err),
    })?;
    if !source_resolved.starts_with(root) {
        return Err(DocError::BadPath);
    }
    if !source_resolved.is_file() {
        return Err(DocError::NotFound);
    }

    // Resolve target directory: must exist, be a directory, and be within root.
    let target_resolved = if target_dir.is_empty() {
        root.to_path_buf()
    } else {
        let target_abs = root.join(&target_dir_rel);
        let resolved = target_abs.canonicalize().map_err(|err| match err.kind() {
            ErrorKind::NotFound => DocError::NotFound,
            _ => DocError::Io(err),
        })?;
        if !resolved.starts_with(root) {
            return Err(DocError::BadPath);
        }
        resolved
    };
    if !target_resolved.is_dir() {
        return Err(DocError::NotFound);
    }

    // Build destination and check for conflicts.
    let file_name = source_rel.file_name().ok_or(DocError::BadPath)?;
    let dest = target_resolved.join(file_name);
    if dest.exists() {
        return Err(DocError::Conflict);
    }

    std::fs::rename(&source_resolved, &dest).map_err(DocError::Io)
}

pub(crate) fn delete_file(root: &Path, file_path: &str) -> Result<(), DocError> {
    let file_rel = supported_file_id_to_path(file_path).ok_or(DocError::BadPath)?;

    let file_abs = root.join(&file_rel);
    let resolved = file_abs.canonicalize().map_err(|err| match err.kind() {
        ErrorKind::NotFound => DocError::NotFound,
        _ => DocError::Io(err),
    })?;
    if !resolved.starts_with(root) {
        return Err(DocError::BadPath);
    }
    if !resolved.is_file() {
        return Err(DocError::NotFound);
    }

    std::fs::remove_file(&resolved).map_err(DocError::Io)
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

    // -- move_file --

    #[test]
    fn move_file__should_move_file_to_another_directory() {
        // Given
        let root = create_temp_root("move-basic");
        std::fs::create_dir_all(root.join("a")).expect("mkdir a");
        std::fs::create_dir_all(root.join("x")).expect("mkdir x");
        std::fs::write(root.join("a/b.md"), "# B").expect("write");

        // When
        move_file(&root, "a/b.md", "x").expect("move file");

        // Then
        assert!(!root.join("a/b.md").exists());
        assert_eq!(
            std::fs::read_to_string(root.join("x/b.md")).expect("read"),
            "# B"
        );
    }

    #[test]
    fn move_file__should_move_file_to_root() {
        // Given
        let root = create_temp_root("move-to-root");
        std::fs::create_dir_all(root.join("sub")).expect("mkdir");
        std::fs::write(root.join("sub/doc.md"), "# Doc").expect("write");

        // When
        move_file(&root, "sub/doc.md", "").expect("move file");

        // Then
        assert!(!root.join("sub/doc.md").exists());
        assert_eq!(
            std::fs::read_to_string(root.join("doc.md")).expect("read"),
            "# Doc"
        );
    }

    #[test]
    fn move_file__should_move_non_markdown_supported_files() {
        // Given
        let root = create_temp_root("move-non-md");
        std::fs::create_dir_all(root.join("a")).expect("mkdir a");
        std::fs::create_dir_all(root.join("b")).expect("mkdir b");
        std::fs::write(root.join("a/photo.png"), "png-data").expect("write");

        // When
        move_file(&root, "a/photo.png", "b").expect("move file");

        // Then
        assert!(!root.join("a/photo.png").exists());
        assert_eq!(
            std::fs::read_to_string(root.join("b/photo.png")).expect("read"),
            "png-data"
        );
    }

    #[test]
    fn move_file__should_return_not_found_for_missing_source() {
        // Given
        let root = create_temp_root("move-not-found");
        std::fs::create_dir_all(root.join("target")).expect("mkdir");

        // When
        let err = move_file(&root, "missing.md", "target").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::NotFound));
    }

    #[test]
    fn move_file__should_return_conflict_when_destination_exists() {
        // Given
        let root = create_temp_root("move-conflict");
        std::fs::create_dir_all(root.join("a")).expect("mkdir a");
        std::fs::create_dir_all(root.join("b")).expect("mkdir b");
        std::fs::write(root.join("a/doc.md"), "source").expect("write");
        std::fs::write(root.join("b/doc.md"), "existing").expect("write");

        // When
        let err = move_file(&root, "a/doc.md", "b").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::Conflict));
        // Source should be untouched.
        assert_eq!(
            std::fs::read_to_string(root.join("a/doc.md")).expect("read"),
            "source"
        );
    }

    #[test]
    fn move_file__should_reject_source_path_traversal() {
        // Given
        let root = create_temp_root("move-traversal-src");
        std::fs::create_dir_all(root.join("target")).expect("mkdir");

        // When
        let err = move_file(&root, "../escape.md", "target").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::BadPath));
    }

    #[test]
    fn move_file__should_reject_target_path_traversal() {
        // Given
        let root = create_temp_root("move-traversal-tgt");
        std::fs::write(root.join("doc.md"), "# Doc").expect("write");

        // When
        let err = move_file(&root, "doc.md", "../").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::BadPath));
    }

    #[test]
    fn move_file__should_reject_unsupported_extension() {
        // Given
        let root = create_temp_root("move-bad-ext");
        std::fs::create_dir_all(root.join("target")).expect("mkdir");
        std::fs::write(root.join("script.sh"), "#!/bin/sh").expect("write");

        // When
        let err = move_file(&root, "script.sh", "target").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::BadPath));
    }

    #[test]
    fn move_file__should_return_not_found_for_missing_target_dir() {
        // Given
        let root = create_temp_root("move-no-target");
        std::fs::write(root.join("doc.md"), "# Doc").expect("write");

        // When
        let err = move_file(&root, "doc.md", "nonexistent").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::NotFound));
    }

    #[cfg(unix)]
    #[test]
    fn move_file__should_reject_symlink_escape_in_source() {
        use std::os::unix::fs::symlink;

        // Given
        let root = create_temp_root("move-symlink-src");
        let outside = create_temp_root("move-symlink-src-outside");
        std::fs::write(outside.join("secret.md"), "secret").expect("write");
        symlink(&outside, root.join("link")).expect("symlink");
        std::fs::create_dir_all(root.join("target")).expect("mkdir");

        // When
        let err = move_file(&root, "link/secret.md", "target").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::BadPath));
    }

    #[cfg(unix)]
    #[test]
    fn move_file__should_reject_symlink_escape_in_target() {
        use std::os::unix::fs::symlink;

        // Given
        let root = create_temp_root("move-symlink-tgt");
        let outside = create_temp_root("move-symlink-tgt-outside");
        symlink(&outside, root.join("link")).expect("symlink");
        std::fs::write(root.join("doc.md"), "# Doc").expect("write");

        // When
        let err = move_file(&root, "doc.md", "link").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::BadPath));
    }

    // -- delete_file --

    #[test]
    fn delete_file__should_remove_file() {
        // Given
        let root = create_temp_root("delete-basic");
        std::fs::write(root.join("doc.md"), "# Doc").expect("write");

        // When
        delete_file(&root, "doc.md").expect("delete file");

        // Then
        assert!(!root.join("doc.md").exists());
    }

    #[test]
    fn delete_file__should_remove_file_in_subdirectory() {
        // Given
        let root = create_temp_root("delete-sub");
        std::fs::create_dir_all(root.join("notes")).expect("mkdir");
        std::fs::write(root.join("notes/todo.md"), "# Todo").expect("write");

        // When
        delete_file(&root, "notes/todo.md").expect("delete file");

        // Then
        assert!(!root.join("notes/todo.md").exists());
    }

    #[test]
    fn delete_file__should_remove_non_markdown_supported_files() {
        // Given
        let root = create_temp_root("delete-non-md");
        std::fs::write(root.join("photo.png"), "png-data").expect("write");

        // When
        delete_file(&root, "photo.png").expect("delete file");

        // Then
        assert!(!root.join("photo.png").exists());
    }

    #[test]
    fn delete_file__should_return_not_found_for_missing_file() {
        // Given
        let root = create_temp_root("delete-not-found");

        // When
        let err = delete_file(&root, "missing.md").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::NotFound));
    }

    #[test]
    fn delete_file__should_reject_path_traversal() {
        // Given
        let root = create_temp_root("delete-traversal");

        // When
        let err = delete_file(&root, "../escape.md").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::BadPath));
    }

    #[test]
    fn delete_file__should_reject_unsupported_extension() {
        // Given
        let root = create_temp_root("delete-bad-ext");
        std::fs::write(root.join("script.sh"), "#!/bin/sh").expect("write");

        // When
        let err = delete_file(&root, "script.sh").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::BadPath));
    }

    #[cfg(unix)]
    #[test]
    fn delete_file__should_reject_symlink_escape() {
        use std::os::unix::fs::symlink;

        // Given
        let root = create_temp_root("delete-symlink");
        let outside = create_temp_root("delete-symlink-outside");
        std::fs::write(outside.join("secret.md"), "secret").expect("write");
        symlink(&outside, root.join("link")).expect("symlink");

        // When
        let err = delete_file(&root, "link/secret.md").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::BadPath));
        // File outside root should be untouched.
        assert!(outside.join("secret.md").exists());
    }
}
