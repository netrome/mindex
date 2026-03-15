use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::io::Write as _;
use std::path::{Component, Path};

pub(crate) fn ensure_parent_dirs(root: &Path, rel_path: &Path) -> std::io::Result<()> {
    let Some(parent) = rel_path.parent() else {
        return Ok(());
    };
    let mut current = root.to_path_buf();
    for component in parent.components() {
        let component = match component {
            Component::Normal(component) => component,
            _ => {
                return Err(std::io::Error::new(
                    ErrorKind::InvalidInput,
                    "path contains non-normal component",
                ));
            }
        };
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(std::io::Error::new(
                        ErrorKind::InvalidInput,
                        "symlink in path",
                    ));
                }
                if !metadata.is_dir() {
                    return Err(std::io::Error::new(
                        ErrorKind::InvalidInput,
                        "non-directory in path",
                    ));
                }
                let resolved = std::fs::canonicalize(&current)?;
                if !resolved.starts_with(root) {
                    return Err(std::io::Error::new(
                        ErrorKind::InvalidInput,
                        "path escapes root",
                    ));
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                std::fs::create_dir(&current)?;
            }
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

pub(crate) fn atomic_write(path: &Path, contents: &str) -> std::io::Result<()> {
    atomic_write_bytes(path, contents.as_bytes())
}

pub(crate) fn atomic_write_bytes(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("missing parent directory"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("tmp");
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    for attempt in 0..10u32 {
        let temp_name = format!(".{}.tmp-{}-{}-{}", file_name, pid, nanos, attempt);
        let temp_path = parent.join(temp_name);
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(mut file) => {
                file.write_all(contents)?;
                file.flush()?;
                std::fs::rename(&temp_path, path)?;
                return Ok(());
            }
            Err(err) if err.kind() == ErrorKind::AlreadyExists => continue,
            Err(err) => return Err(err),
        }
    }

    Err(std::io::Error::new(
        ErrorKind::AlreadyExists,
        "failed to create temp file",
    ))
}
