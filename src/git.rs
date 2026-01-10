use std::path::{Path, PathBuf};

use gix::bstr::{BStr, ByteSlice};
use gix::status::index_worktree::iter::Item as WorktreeItem;
use gix::status::plumbing::index_as_worktree::{Change as WorktreeChange, EntryStatus};
use imara_diff::intern::InternedInput;
use imara_diff::{Algorithm, UnifiedDiffBuilder, diff};

pub(crate) fn git_dir_within_root(root: &Path) -> std::io::Result<Option<PathBuf>> {
    let root = std::fs::canonicalize(root)?;
    let dot_git = root.join(".git");
    let metadata = match std::fs::symlink_metadata(&dot_git) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };

    let resolved_dot_git = std::fs::canonicalize(&dot_git)?;
    if !resolved_dot_git.starts_with(&root) {
        return Ok(None);
    }

    let resolved_meta = if metadata.file_type().is_symlink() {
        std::fs::metadata(&resolved_dot_git)?
    } else {
        metadata
    };

    if resolved_meta.is_dir() {
        return Ok(Some(resolved_dot_git));
    }

    if !resolved_meta.is_file() {
        return Ok(None);
    }

    let gitdir = match parse_gitdir_path(&resolved_dot_git)? {
        Some(path) => path,
        None => return Ok(None),
    };
    let gitdir = if gitdir.is_absolute() {
        gitdir
    } else {
        let base = match resolved_dot_git.parent() {
            Some(base) => base,
            None => return Ok(None),
        };
        base.join(gitdir)
    };

    let resolved_gitdir = match std::fs::canonicalize(&gitdir) {
        Ok(path) => path,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    if !resolved_gitdir.starts_with(&root) {
        return Ok(None);
    }
    let gitdir_meta = match std::fs::metadata(&resolved_gitdir) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    if !gitdir_meta.is_dir() {
        return Ok(None);
    }

    Ok(Some(resolved_gitdir))
}

pub(crate) struct GitSnapshot {
    pub(crate) changed_files: usize,
    pub(crate) diff: String,
}

#[derive(Debug)]
pub(crate) struct GitError(String);

impl GitError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl std::fmt::Display for GitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for GitError {}

pub(crate) fn git_status_and_diff(root: &Path) -> Result<GitSnapshot, GitError> {
    let repo = gix::open(root).map_err(|err| GitError::new(format!("open repo: {err}")))?;
    let iter = repo
        .status(gix::progress::Discard)
        .map_err(|err| GitError::new(format!("status: {err}")))?
        .untracked_files(gix::status::UntrackedFiles::None)
        .into_index_worktree_iter(Vec::new())
        .map_err(|err| GitError::new(format!("status iterator: {err}")))?;

    let mut diff = String::new();
    let mut changed_files = 0usize;
    for item in iter {
        let item = item.map_err(|err| GitError::new(format!("status item: {err}")))?;
        if item.summary().is_some() {
            changed_files += 1;
        }
        let Some(file_diff) = diff_for_item(&repo, root, &item)? else {
            continue;
        };
        diff.push_str(&file_diff);
    }

    Ok(GitSnapshot {
        changed_files,
        diff,
    })
}

fn diff_for_item(
    repo: &gix::Repository,
    root: &Path,
    item: &WorktreeItem,
) -> Result<Option<String>, GitError> {
    let (entry, rela_path, status) = match item {
        WorktreeItem::Modification {
            entry,
            rela_path,
            status,
            ..
        } => (entry, rela_path.as_ref(), status),
        _ => return Ok(None),
    };

    if entry.mode == gix::index::entry::Mode::COMMIT {
        return Ok(None);
    }

    let (old_bytes, new_bytes) = match status {
        EntryStatus::Change(change) => match change {
            WorktreeChange::Removed => {
                let old = blob_bytes(repo, entry.id)?;
                (Some(old), None)
            }
            WorktreeChange::Modification { .. } | WorktreeChange::Type => {
                let old = blob_bytes(repo, entry.id)?;
                let new = read_worktree_bytes(root, rela_path)?;
                (Some(old), new)
            }
            WorktreeChange::SubmoduleModification(_) => return Ok(None),
        },
        _ => return Ok(None),
    };

    let path = rela_path.to_str_lossy();
    Ok(Some(build_file_diff(
        &path,
        old_bytes.as_deref(),
        new_bytes.as_deref(),
    )))
}

fn blob_bytes(repo: &gix::Repository, id: gix::hash::ObjectId) -> Result<Vec<u8>, GitError> {
    let mut blob = repo
        .find_blob(id)
        .map_err(|err| GitError::new(format!("load blob: {err}")))?;
    Ok(blob.take_data())
}

fn read_worktree_bytes(root: &Path, rela_path: &BStr) -> Result<Option<Vec<u8>>, GitError> {
    let rel = rela_path.to_str_lossy();
    let path = root.join(rel.as_ref());
    let resolved = match std::fs::canonicalize(&path) {
        Ok(path) => path,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(GitError::new(format!("read worktree: {err}"))),
    };
    if !resolved.starts_with(root) {
        return Ok(None);
    }
    let data = match std::fs::read(&resolved) {
        Ok(data) => data,
        Err(err) if err.kind() == std::io::ErrorKind::IsADirectory => return Ok(None),
        Err(err) => return Err(GitError::new(format!("read worktree file: {err}"))),
    };
    Ok(Some(data))
}

fn build_file_diff(path: &str, old: Option<&[u8]>, new: Option<&[u8]>) -> String {
    let mut output = String::new();
    output.push_str(&format!("diff --git a/{path} b/{path}\n"));

    match (old, new) {
        (None, Some(_)) => {
            output.push_str("--- /dev/null\n");
            output.push_str(&format!("+++ b/{path}\n"));
        }
        (Some(_), None) => {
            output.push_str(&format!("--- a/{path}\n"));
            output.push_str("+++ /dev/null\n");
        }
        (Some(_), Some(_)) => {
            output.push_str(&format!("--- a/{path}\n"));
            output.push_str(&format!("+++ b/{path}\n"));
        }
        (None, None) => return output,
    }

    if old.map(is_binary).unwrap_or(false) || new.map(is_binary).unwrap_or(false) {
        output.push_str(&format!("Binary files a/{path} and b/{path} differ\n"));
        return output;
    }

    let before = old
        .map(String::from_utf8_lossy)
        .unwrap_or_else(|| "".into());
    let after = new
        .map(String::from_utf8_lossy)
        .unwrap_or_else(|| "".into());

    let input = InternedInput::new(before.as_ref(), after.as_ref());
    let mut file_diff = diff(
        Algorithm::Histogram,
        &input,
        UnifiedDiffBuilder::new(&input),
    );
    if !file_diff.ends_with('\n') && !file_diff.is_empty() {
        file_diff.push('\n');
    }
    output.push_str(&file_diff);
    output
}

fn is_binary(bytes: &[u8]) -> bool {
    bytes.contains(&0)
}

fn parse_gitdir_path(dot_git: &Path) -> std::io::Result<Option<PathBuf>> {
    let contents = std::fs::read_to_string(dot_git)?;
    for line in contents.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("gitdir:") {
            let path = rest.trim();
            if path.is_empty() {
                return Ok(None);
            }
            return Ok(Some(PathBuf::from(path)));
        }
    }
    Ok(None)
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::git_dir_within_root;
    use std::path::PathBuf;

    #[test]
    fn git_dir_within_root__should_accept_dot_git_directory() {
        // Given
        let root = create_temp_root("git-dir");
        let git_dir = root.join(".git");
        std::fs::create_dir_all(&git_dir).expect("create .git dir");

        // When
        let detected = git_dir_within_root(&root).expect("detect git dir");

        // Then
        let expected = std::fs::canonicalize(&git_dir).expect("canonicalize git dir");
        assert_eq!(detected, Some(expected));

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn git_dir_within_root__should_accept_gitdir_file_with_relative_path() {
        // Given
        let root = create_temp_root("gitdir-file");
        let actual_git = root.join("git-data");
        std::fs::create_dir_all(&actual_git).expect("create git dir");
        std::fs::write(root.join(".git"), "gitdir: git-data\n").expect("write .git file");

        // When
        let detected = git_dir_within_root(&root).expect("detect git dir");

        // Then
        let expected = std::fs::canonicalize(&actual_git).expect("canonicalize git dir");
        assert_eq!(detected, Some(expected));

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn git_dir_within_root__should_reject_gitdir_file_outside_root() {
        // Given
        let root = create_temp_root("gitdir-outside");
        let outside = create_temp_root("gitdir-outside-target");
        let outside_git = outside.join("repo");
        std::fs::create_dir_all(&outside_git).expect("create outside git dir");
        let contents = format!("gitdir: {}\n", outside_git.display());
        std::fs::write(root.join(".git"), contents).expect("write .git file");

        // When
        let detected = git_dir_within_root(&root).expect("detect git dir");

        // Then
        assert!(detected.is_none());

        std::fs::remove_dir_all(&root).expect("cleanup");
        std::fs::remove_dir_all(&outside).expect("cleanup outside");
    }

    #[test]
    fn git_dir_within_root__should_return_none_when_missing() {
        // Given
        let root = create_temp_root("git-missing");

        // When
        let detected = git_dir_within_root(&root).expect("detect git dir");

        // Then
        assert!(detected.is_none());

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn git_dir_within_root__should_ignore_invalid_gitdir_file() {
        // Given
        let root = create_temp_root("git-invalid");
        std::fs::write(root.join(".git"), "not-a-gitdir").expect("write .git file");

        // When
        let detected = git_dir_within_root(&root).expect("detect git dir");

        // Then
        assert!(detected.is_none());

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn git_dir_within_root__should_reject_symlink_outside_root() {
        // Given
        use std::os::unix::fs::symlink;

        let root = create_temp_root("git-symlink");
        let outside = create_temp_root("git-symlink-outside");
        let outside_git = outside.join(".git");
        std::fs::create_dir_all(&outside_git).expect("create outside git dir");
        symlink(&outside_git, root.join(".git")).expect("create .git symlink");

        // When
        let detected = git_dir_within_root(&root).expect("detect git dir");

        // Then
        assert!(detected.is_none());

        std::fs::remove_dir_all(&root).expect("cleanup");
        std::fs::remove_dir_all(&outside).expect("cleanup outside");
    }

    fn create_temp_root(test_name: &str) -> PathBuf {
        let mut root = std::env::temp_dir();
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        root.push(format!("mindex-{}-{}", test_name, nanos));
        std::fs::create_dir_all(&root).expect("create temp dir");
        root
    }
}
