use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};
use time::OffsetDateTime;

pub(crate) const UPLOADS_DIR: &str = "mindex-uploads";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageType {
    Png,
    Jpeg,
    Gif,
    Webp,
}

impl ImageType {
    fn from_content_type(content_type: &str) -> Option<Self> {
        match content_type {
            "image/png" => Some(Self::Png),
            "image/jpeg" => Some(Self::Jpeg),
            "image/gif" => Some(Self::Gif),
            "image/webp" => Some(Self::Webp),
            _ => None,
        }
    }

    fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "gif" => Some(Self::Gif),
            "webp" => Some(Self::Webp),
            _ => None,
        }
    }

    fn extension(self) -> &'static str {
        match self {
            Self::Png => "png",
            Self::Jpeg => "jpg",
            Self::Gif => "gif",
            Self::Webp => "webp",
        }
    }

    pub(crate) fn content_type(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
            Self::Gif => "image/gif",
            Self::Webp => "image/webp",
        }
    }
}

#[derive(Debug)]
pub(crate) enum UploadError {
    BadPath,
    NotFound,
    EmptyBody,
    UnsupportedType,
    Io(std::io::Error),
}

pub(crate) struct StoredUpload {
    pub(crate) rel_path: String,
}

pub(crate) fn store_upload(
    root: &Path,
    bytes: &[u8],
    content_type: Option<&str>,
    filename: Option<&str>,
) -> Result<StoredUpload, UploadError> {
    if bytes.is_empty() {
        return Err(UploadError::EmptyBody);
    }

    let image_type = detect_image_type(content_type, filename, bytes)?;
    let now = OffsetDateTime::now_utc();
    let year = now.year();
    let month = u8::from(now.month());
    let day = now.day();
    let hour = now.hour();
    let minute = now.minute();
    let second = now.second();

    let base = sanitize_base_name(filename);
    let dir = format!("{}/{:04}/{:02}", UPLOADS_DIR, year, month);

    for _ in 0..10 {
        let suffix = random_suffix();
        let file_name = format!(
            "{}-{:04}{:02}{:02}-{:02}{:02}{:02}-{}.{}",
            base,
            year,
            month,
            day,
            hour,
            minute,
            second,
            suffix,
            image_type.extension()
        );
        let rel_path = format!("{dir}/{file_name}");
        let rel_path_buf = Path::new(&rel_path);
        ensure_parent_dirs(root, rel_path_buf)?;
        let target = root.join(rel_path_buf);
        if target.exists() {
            continue;
        }
        atomic_write_bytes(&target, bytes).map_err(UploadError::Io)?;
        return Ok(StoredUpload { rel_path });
    }

    Err(UploadError::Io(std::io::Error::new(
        ErrorKind::AlreadyExists,
        "failed to allocate upload name",
    )))
}

pub(crate) fn resolve_file_path(root: &Path, rel_path: &str) -> Result<PathBuf, UploadError> {
    let safe_path = relative_path_to_path(rel_path).ok_or(UploadError::BadPath)?;
    let mut current = root.to_path_buf();

    for component in safe_path.components() {
        let component = match component {
            Component::Normal(component) => component,
            _ => return Err(UploadError::BadPath),
        };
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(UploadError::BadPath);
                }
                if metadata.is_dir() {
                    let resolved = std::fs::canonicalize(&current).map_err(UploadError::Io)?;
                    if !resolved.starts_with(root) {
                        return Err(UploadError::BadPath);
                    }
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => return Err(UploadError::NotFound),
            Err(err) => return Err(UploadError::Io(err)),
        }
    }

    let resolved = std::fs::canonicalize(&current).map_err(|err| match err.kind() {
        ErrorKind::NotFound => UploadError::NotFound,
        _ => UploadError::Io(err),
    })?;
    if !resolved.starts_with(root) {
        return Err(UploadError::BadPath);
    }
    let metadata = std::fs::metadata(&resolved).map_err(|err| match err.kind() {
        ErrorKind::NotFound => UploadError::NotFound,
        _ => UploadError::Io(err),
    })?;
    if !metadata.is_file() {
        return Err(UploadError::NotFound);
    }
    Ok(resolved)
}

pub(crate) fn content_type_for_path(rel_path: &str) -> Option<&'static str> {
    let ext = Path::new(rel_path).extension()?.to_str()?;
    if ext.eq_ignore_ascii_case("pdf") {
        return Some("application/pdf");
    }
    ImageType::from_extension(ext).map(ImageType::content_type)
}

fn detect_image_type(
    content_type: Option<&str>,
    filename: Option<&str>,
    bytes: &[u8],
) -> Result<ImageType, UploadError> {
    let content_type = content_type.filter(|value| *value != "application/octet-stream");
    let sniffed = sniff_image_type(bytes);
    if let Some(content_type) = content_type {
        let from_header =
            ImageType::from_content_type(content_type).ok_or(UploadError::UnsupportedType)?;
        if Some(from_header) != sniffed {
            return Err(UploadError::UnsupportedType);
        }
        return Ok(from_header);
    }

    if let Some(sniffed) = sniffed {
        return Ok(sniffed);
    }

    if let Some(filename) = filename
        && let Some(ext) = Path::new(filename).extension().and_then(|ext| ext.to_str())
        && let Some(kind) = ImageType::from_extension(ext)
    {
        return Ok(kind);
    }

    Err(UploadError::UnsupportedType)
}

fn sniff_image_type(bytes: &[u8]) -> Option<ImageType> {
    if bytes.len() >= 8 && bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        return Some(ImageType::Png);
    }
    if bytes.len() >= 3 && bytes[0] == 0xFF && bytes[1] == 0xD8 && bytes[2] == 0xFF {
        return Some(ImageType::Jpeg);
    }
    if bytes.len() >= 6 && (bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a")) {
        return Some(ImageType::Gif);
    }
    if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" {
        return Some(ImageType::Webp);
    }
    None
}

fn sanitize_base_name(filename: Option<&str>) -> String {
    let base = filename
        .and_then(|name| Path::new(name).file_stem().and_then(|stem| stem.to_str()))
        .unwrap_or("image");
    let mut out = String::with_capacity(base.len());
    let mut last_dash = false;

    for ch in base.chars() {
        let mapped = if ch.is_ascii_alphanumeric() {
            last_dash = false;
            Some(ch.to_ascii_lowercase())
        } else {
            if last_dash || out.is_empty() {
                continue;
            }
            last_dash = true;
            Some('-')
        };

        if let Some(mapped) = mapped {
            out.push(mapped);
        }
    }

    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "image".to_string()
    } else if trimmed.len() > 40 {
        trimmed[..40].to_string()
    } else {
        trimmed.to_string()
    }
}

fn random_suffix() -> String {
    let value: u16 = rand::random();
    format!("{:04x}", value)
}

fn relative_path_to_path(rel_path: &str) -> Option<PathBuf> {
    if rel_path.is_empty() {
        return None;
    }
    let path = Path::new(rel_path);
    if path.is_absolute() {
        return None;
    }
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => components.push(part),
            _ => return None,
        }
    }
    if components.is_empty() {
        return None;
    }
    Some(components.iter().collect())
}

fn ensure_parent_dirs(root: &Path, rel_path: &Path) -> Result<(), UploadError> {
    let Some(parent) = rel_path.parent() else {
        return Ok(());
    };
    let mut current = root.to_path_buf();
    for component in parent.components() {
        let component = match component {
            Component::Normal(component) => component,
            _ => return Err(UploadError::BadPath),
        };
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    return Err(UploadError::BadPath);
                }
                if !metadata.is_dir() {
                    return Err(UploadError::BadPath);
                }
                let resolved = std::fs::canonicalize(&current).map_err(UploadError::Io)?;
                if !resolved.starts_with(root) {
                    return Err(UploadError::BadPath);
                }
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {
                std::fs::create_dir(&current).map_err(UploadError::Io)?;
            }
            Err(err) => return Err(UploadError::Io(err)),
        }
    }
    Ok(())
}

fn atomic_write_bytes(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("missing parent directory"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("upload.bin");
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();

    for attempt in 0..10u32 {
        let temp_name = format!(".{}.tmp-{}-{}-{}", file_name, pid, nanos, attempt);
        let temp_path = parent.join(temp_name);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)
        {
            Ok(mut file) => {
                use std::io::Write as _;
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

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;

    #[test]
    fn store_upload__should_write_under_root() {
        // Given
        let root = create_temp_root("upload-store");
        let bytes = [0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A];

        // When
        let stored =
            store_upload(&root, &bytes, Some("image/png"), Some("test.png")).expect("store upload");
        let target = root.join(&stored.rel_path);

        // Then
        assert!(stored.rel_path.starts_with(UPLOADS_DIR));
        assert!(target.exists());

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn resolve_file_path__should_reject_traversal() {
        // Given
        let root = create_temp_root("upload-traversal");

        // When
        let result = resolve_file_path(&root, "../outside.png");

        // Then
        assert!(matches!(result, Err(UploadError::BadPath)));

        std::fs::remove_dir_all(&root).expect("cleanup");
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
