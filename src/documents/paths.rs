use super::DocError;
use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

pub(crate) fn collect_markdown_paths(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_markdown_paths_recursive(root, &mut paths)?;
    Ok(paths)
}

/// A browsable file under root: its normalized relative path and its kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BrowsableFile {
    pub(crate) path: String,
    pub(crate) kind: FileKind,
}

/// Recursively lists every file the directory browser would show, mirroring its
/// rules: skip symlinks and hidden entries, include only recognized file kinds.
/// Paths are normalized relative paths (the document-ID convention), sorted.
pub(crate) fn collect_browsable_files(root: &Path) -> std::io::Result<Vec<BrowsableFile>> {
    let mut files = Vec::new();
    collect_browsable_files_recursive(root, root, &mut files)?;
    files.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(files)
}

pub(crate) fn doc_id_from_path(root: &Path, path: &Path) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let mut parts = Vec::new();
    for component in rel.components() {
        match component {
            Component::Normal(os_str) => {
                parts.push(os_str.to_string_lossy().into_owned());
            }
            _ => return None,
        }
    }
    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

/// Returns the Highlight.js language identifier for a file extension.
pub(crate) fn highlight_lang_for_extension(ext: &str) -> &'static str {
    match ext.to_ascii_lowercase().as_str() {
        "json" => "json",
        "yaml" | "yml" => "yaml",
        "toml" => "toml",
        _ => "plaintext",
    }
}

#[derive(Debug)]
pub(crate) struct DirectoryListing {
    pub(crate) directories: Vec<String>,
    pub(crate) files: Vec<DirectoryFile>,
}

#[derive(Debug)]
pub(crate) struct DirectoryFile {
    pub(crate) name: String,
    pub(crate) kind: FileKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FileKind {
    Document,
    Pdf,
    Image,
    Text,
}

impl FileKind {
    /// Stable lowercase tag used in API responses (e.g. for result icons).
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Document => "document",
            Self::Pdf => "pdf",
            Self::Image => "image",
            Self::Text => "text",
        }
    }

    pub(crate) fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "md" => Some(Self::Document),
            "pdf" => Some(Self::Pdf),
            "png" | "jpg" | "jpeg" | "gif" | "webp" => Some(Self::Image),
            "json" | "yaml" | "yml" | "toml" => Some(Self::Text),
            _ => None,
        }
    }
}

pub(crate) fn list_directory(
    root: &Path,
    relative_dir: &str,
) -> Result<DirectoryListing, DocError> {
    let dir_path = if relative_dir.is_empty() {
        root.to_path_buf()
    } else {
        let rel = dir_to_path(relative_dir).ok_or(DocError::BadPath)?;
        let candidate = root.join(rel);
        let canonical = candidate.canonicalize().map_err(|err| match err.kind() {
            ErrorKind::NotFound => DocError::NotFound,
            _ => DocError::Io(err),
        })?;
        if !canonical.starts_with(root) {
            return Err(DocError::BadPath);
        }
        canonical
    };

    if !dir_path.is_dir() {
        return Err(DocError::NotFound);
    }

    let mut directories = Vec::new();
    let mut files = Vec::new();

    for entry in std::fs::read_dir(&dir_path).map_err(DocError::Io)? {
        let entry = entry.map_err(DocError::Io)?;
        let file_type = entry.file_type().map_err(DocError::Io)?;

        if file_type.is_symlink() {
            continue;
        }

        let name = entry.file_name();
        let name_str = match name.to_str() {
            Some(s) => s,
            None => continue,
        };

        if name_str.starts_with('.') {
            continue;
        }

        if file_type.is_dir() {
            directories.push(name_str.to_string());
        } else if file_type.is_file() {
            let kind = Path::new(name_str)
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(FileKind::from_extension);
            if let Some(kind) = kind {
                files.push(DirectoryFile {
                    name: name_str.to_string(),
                    kind,
                });
            }
        }
    }

    directories.sort();
    files.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(DirectoryListing { directories, files })
}

pub(crate) fn resolve_doc_path(root: &Path, doc_id: &str) -> Result<PathBuf, DocError> {
    let doc_path = doc_id_to_path(doc_id).ok_or(DocError::BadPath)?;
    let joined = root.join(doc_path);
    let resolved = match std::fs::canonicalize(&joined) {
        Ok(path) => path,
        Err(err) if err.kind() == ErrorKind::NotFound => return Err(DocError::NotFound),
        Err(err) => return Err(DocError::Io(err)),
    };
    if !resolved.starts_with(root) {
        return Err(DocError::NotFound);
    }
    Ok(resolved)
}

pub(crate) fn is_text_extension(ext: &str) -> bool {
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "json" | "yaml" | "yml" | "toml"
    )
}

pub(crate) fn resolve_text_file_path(root: &Path, file_id: &str) -> Result<PathBuf, DocError> {
    let path = text_file_id_to_path(file_id).ok_or(DocError::BadPath)?;
    let joined = root.join(path);
    let resolved = match std::fs::canonicalize(&joined) {
        Ok(path) => path,
        Err(err) if err.kind() == ErrorKind::NotFound => return Err(DocError::NotFound),
        Err(err) => return Err(DocError::Io(err)),
    };
    if !resolved.starts_with(root) {
        return Err(DocError::NotFound);
    }
    Ok(resolved)
}

pub(super) fn doc_id_to_path(doc_id: &str) -> Option<PathBuf> {
    if doc_id.is_empty() {
        return None;
    }
    let path = Path::new(doc_id);
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => return None,
        }
    }
    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return None;
    }
    Some(path.to_path_buf())
}

fn collect_browsable_files_recursive(
    root: &Path,
    dir: &Path,
    files: &mut Vec<BrowsableFile>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }

        let is_hidden = entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.starts_with('.'));
        if is_hidden {
            continue;
        }

        let path = entry.path();
        if file_type.is_dir() {
            collect_browsable_files_recursive(root, &path, files)?;
            continue;
        }

        if file_type.is_file() {
            let kind = path
                .extension()
                .and_then(|ext| ext.to_str())
                .and_then(FileKind::from_extension);
            if let (Some(kind), Some(rel)) = (kind, doc_id_from_path(root, &path)) {
                files.push(BrowsableFile { path: rel, kind });
            }
        }
    }
    Ok(())
}

fn collect_markdown_paths_recursive(dir: &Path, paths: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }

        let path = entry.path();
        if file_type.is_dir() {
            let is_hidden = entry
                .file_name()
                .to_str()
                .is_some_and(|name| name.starts_with('.'));
            if !is_hidden {
                collect_markdown_paths_recursive(&path, paths)?;
            }
            continue;
        }

        if file_type.is_file()
            && matches!(path.extension().and_then(|ext| ext.to_str()), Some("md"))
        {
            paths.push(path);
        }
    }
    Ok(())
}

pub(super) fn dir_to_path(dir: &str) -> Option<PathBuf> {
    if dir.is_empty() {
        return Some(PathBuf::new());
    }
    let path = Path::new(dir);
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => return None,
        }
    }
    Some(path.to_path_buf())
}

pub(super) fn supported_file_id_to_path(file_id: &str) -> Option<PathBuf> {
    if file_id.is_empty() {
        return None;
    }
    let path = Path::new(file_id);
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => return None,
        }
    }
    let ext = path.extension()?.to_str()?;
    FileKind::from_extension(ext)?;
    Some(path.to_path_buf())
}

fn text_file_id_to_path(file_id: &str) -> Option<PathBuf> {
    if file_id.is_empty() {
        return None;
    }
    let path = Path::new(file_id);
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            _ => return None,
        }
    }
    let ext = path.extension()?.to_str()?;
    if !is_text_extension(ext) {
        return None;
    }
    Some(path.to_path_buf())
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use crate::test_support::create_temp_root;

    #[test]
    fn collect_markdown_paths__should_ignore_non_md_files_and_symlinks() {
        // Given
        let root = create_temp_root("collect");
        std::fs::write(root.join("a.md"), "# A").expect("write a.md");
        std::fs::write(root.join("b.txt"), "B").expect("write b.txt");
        std::fs::create_dir_all(root.join("notes")).expect("create notes dir");
        std::fs::write(root.join("notes").join("c.md"), "# C").expect("write c.md");

        #[cfg(unix)]
        {
            use std::os::unix::fs::symlink;
            symlink(root.join("a.md"), root.join("link.md")).expect("create symlink");
        }

        // When
        let mut doc_ids: Vec<String> = collect_markdown_paths(&root)
            .expect("collect paths")
            .into_iter()
            .filter_map(|path| doc_id_from_path(&root, &path))
            .collect();
        doc_ids.sort();

        // Then
        assert_eq!(doc_ids, vec!["a.md".to_string(), "notes/c.md".to_string()]);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn collect_markdown_paths__should_skip_hidden_directories() {
        // Given
        let root = create_temp_root("collect-hidden");
        std::fs::write(root.join("a.md"), "# A").expect("write a.md");
        std::fs::create_dir_all(root.join(".git")).expect("create .git dir");
        std::fs::write(root.join(".git").join("HEAD.md"), "ref").expect("write .git/HEAD.md");
        std::fs::create_dir_all(root.join(".obsidian")).expect("create .obsidian dir");
        std::fs::write(root.join(".obsidian").join("config.md"), "{}").expect("write config.md");
        std::fs::create_dir_all(root.join("visible")).expect("create visible dir");
        std::fs::write(root.join("visible").join("b.md"), "# B").expect("write b.md");

        // When
        let mut doc_ids: Vec<String> = collect_markdown_paths(&root)
            .expect("collect paths")
            .into_iter()
            .filter_map(|path| doc_id_from_path(&root, &path))
            .collect();
        doc_ids.sort();

        // Then
        assert_eq!(
            doc_ids,
            vec!["a.md".to_string(), "visible/b.md".to_string()]
        );

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    // -- collect_browsable_files --

    #[test]
    fn collect_browsable_files__should_include_all_recognized_kinds_with_relative_paths() {
        // Given
        let root = create_temp_root("browsable-kinds");
        std::fs::write(root.join("doc.md"), "# Doc").expect("write");
        std::fs::write(root.join("scan.pdf"), "pdf").expect("write");
        std::fs::write(root.join("photo.png"), "png").expect("write");
        std::fs::write(root.join("config.json"), "{}").expect("write");
        std::fs::create_dir_all(root.join("notes")).expect("mkdir");
        std::fs::write(root.join("notes/todo.md"), "# Todo").expect("write");

        // When
        let files = collect_browsable_files(&root).expect("collect");

        // Then
        assert_eq!(
            files,
            vec![
                BrowsableFile {
                    path: "config.json".to_string(),
                    kind: FileKind::Text,
                },
                BrowsableFile {
                    path: "doc.md".to_string(),
                    kind: FileKind::Document,
                },
                BrowsableFile {
                    path: "notes/todo.md".to_string(),
                    kind: FileKind::Document,
                },
                BrowsableFile {
                    path: "photo.png".to_string(),
                    kind: FileKind::Image,
                },
                BrowsableFile {
                    path: "scan.pdf".to_string(),
                    kind: FileKind::Pdf,
                },
            ]
        );

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn collect_browsable_files__should_exclude_unrecognized_and_hidden_entries() {
        // Given
        let root = create_temp_root("browsable-excludes");
        std::fs::write(root.join("a.md"), "# A").expect("write");
        std::fs::write(root.join("b.rs"), "fn main() {}").expect("write");
        std::fs::write(root.join(".secret.md"), "# Secret").expect("write");
        std::fs::create_dir_all(root.join(".git")).expect("mkdir .git");
        std::fs::write(root.join(".git/HEAD.md"), "ref").expect("write");

        // When
        let files = collect_browsable_files(&root).expect("collect");

        // Then
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["a.md"]);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn collect_browsable_files__should_exclude_symlinks() {
        use std::os::unix::fs::symlink;

        // Given
        let root = create_temp_root("browsable-symlink");
        std::fs::write(root.join("a.md"), "# A").expect("write");
        symlink(root.join("a.md"), root.join("link.md")).expect("symlink");

        // When
        let files = collect_browsable_files(&root).expect("collect");

        // Then
        let paths: Vec<&str> = files.iter().map(|f| f.path.as_str()).collect();
        assert_eq!(paths, vec!["a.md"]);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    // -- list_directory --

    #[test]
    fn list_directory__should_list_root_contents() {
        // Given
        let root = create_temp_root("listdir-root");
        std::fs::write(root.join("a.md"), "# A").expect("write");
        std::fs::write(root.join("b.md"), "# B").expect("write");
        std::fs::create_dir_all(root.join("notes")).expect("mkdir");
        std::fs::write(root.join("notes/c.md"), "# C").expect("write");

        // When
        let listing = list_directory(&root, "").expect("list root");

        // Then
        assert_eq!(listing.directories, vec!["notes"]);
        let names: Vec<&str> = listing.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a.md", "b.md"]);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn list_directory__should_list_subdirectory_contents() {
        // Given
        let root = create_temp_root("listdir-sub");
        std::fs::create_dir_all(root.join("notes/work")).expect("mkdir");
        std::fs::write(root.join("notes/todo.md"), "# Todo").expect("write");
        std::fs::write(root.join("notes/ideas.md"), "# Ideas").expect("write");

        // When
        let listing = list_directory(&root, "notes").expect("list notes");

        // Then
        assert_eq!(listing.directories, vec!["work"]);
        let names: Vec<&str> = listing.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["ideas.md", "todo.md"]);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn list_directory__should_exclude_hidden_entries() {
        // Given
        let root = create_temp_root("listdir-hidden");
        std::fs::write(root.join("a.md"), "# A").expect("write");
        std::fs::create_dir_all(root.join(".git")).expect("mkdir .git");
        std::fs::write(root.join(".secret.md"), "# Secret").expect("write");

        // When
        let listing = list_directory(&root, "").expect("list root");

        // Then
        assert!(listing.directories.is_empty());
        let names: Vec<&str> = listing.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a.md"]);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn list_directory__should_exclude_unrecognized_extensions() {
        // Given
        let root = create_temp_root("listdir-nonmd");
        std::fs::write(root.join("a.md"), "# A").expect("write");
        std::fs::write(root.join("b.txt"), "B").expect("write");
        std::fs::write(root.join("c.rs"), "fn main() {}").expect("write");

        // When
        let listing = list_directory(&root, "").expect("list root");

        // Then
        let names: Vec<&str> = listing.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a.md"]);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn list_directory__should_include_all_recognized_file_types() {
        // Given
        let root = create_temp_root("listdir-filetypes");
        std::fs::write(root.join("doc.md"), "# Doc").expect("write");
        std::fs::write(root.join("scan.pdf"), "pdf").expect("write");
        std::fs::write(root.join("photo.png"), "png").expect("write");
        std::fs::write(root.join("config.json"), "{}").expect("write");
        std::fs::write(root.join("data.yaml"), "key: val").expect("write");
        std::fs::write(root.join("settings.toml"), "[s]").expect("write");
        std::fs::write(root.join("unknown.xyz"), "?").expect("write");

        // When
        let listing = list_directory(&root, "").expect("list root");

        // Then
        let names: Vec<&str> = listing.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(
            names,
            vec![
                "config.json",
                "data.yaml",
                "doc.md",
                "photo.png",
                "scan.pdf",
                "settings.toml"
            ]
        );
        let kinds: Vec<FileKind> = listing.files.iter().map(|f| f.kind).collect();
        assert_eq!(
            kinds,
            vec![
                FileKind::Text,
                FileKind::Text,
                FileKind::Document,
                FileKind::Image,
                FileKind::Pdf,
                FileKind::Text,
            ]
        );

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn list_directory__should_exclude_symlinks() {
        use std::os::unix::fs::symlink;

        // Given
        let root = create_temp_root("listdir-symlink");
        std::fs::write(root.join("a.md"), "# A").expect("write");
        symlink(root.join("a.md"), root.join("link.md")).expect("symlink");

        // When
        let listing = list_directory(&root, "").expect("list root");

        // Then
        let names: Vec<&str> = listing.files.iter().map(|f| f.name.as_str()).collect();
        assert_eq!(names, vec!["a.md"]);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn list_directory__should_reject_path_traversal() {
        // Given
        let root = create_temp_root("listdir-traversal");

        // When
        let err = list_directory(&root, "../").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::BadPath));

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn list_directory__should_return_not_found_for_missing_directory() {
        // Given
        let root = create_temp_root("listdir-missing");

        // When
        let err = list_directory(&root, "nonexistent").expect_err("should fail");

        // Then
        assert!(matches!(err, DocError::NotFound));

        std::fs::remove_dir_all(&root).expect("cleanup");
    }
}
