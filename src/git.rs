use std::path::{Path, PathBuf};

#[allow(dead_code)]
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
