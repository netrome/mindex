use crate::assets;
use crate::config;
use crate::state;
use crate::templates;

use axum::Router;
use axum::extract::Form;
use axum::extract::Path as AxumPath;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use pulldown_cmark::Event;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use serde::Deserialize;

use std::fs::OpenOptions;
use std::io::ErrorKind;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use askama::Template as _;
use std::io::Write as _;

pub fn app(root: PathBuf, config: config::AppConfig) -> Router {
    let manifest_body = templates::ManifestTemplate {
        app_name: &config.app_name,
    }
    .render()
    .unwrap_or_else(|err| {
        eprintln!("failed to render manifest: {err}");
        "{}".to_string()
    });
    let icon_192_bytes = load_icon_bytes(
        &root,
        config.icon_192.as_deref(),
        assets::ICON_192_FALLBACK,
        "icon-192",
    );
    let icon_512_bytes = load_icon_bytes(
        &root,
        config.icon_512.as_deref(),
        assets::ICON_512_FALLBACK,
        "icon-512",
    );
    let state = state::AppState {
        root,
        app_name: config.app_name,
        manifest: manifest_body,
        icon_192: icon_192_bytes,
        icon_512: icon_512_bytes,
    };
    Router::new()
        .route("/", get(document_list))
        .route("/search", get(document_search))
        .route("/edit/{*path}", get(document_edit).post(document_save))
        .route("/doc/{*path}", get(document_view))
        .route("/static/style.css", get(assets::stylesheet))
        .route("/static/theme.js", get(assets::theme_script))
        .route("/static/manifest.json", get(assets::manifest))
        .route("/static/sw.js", get(assets::service_worker))
        .route("/static/icons/icon-192.png", get(assets::icon_192))
        .route("/static/icons/icon-512.png", get(assets::icon_512))
        .route("/health", get(health))
        .with_state(state)
}

pub(crate) async fn health() -> &'static str {
    "ok"
}

pub(crate) async fn document_list(
    State(state): State<state::AppState>,
) -> Result<templates::DocumentListTemplate, (StatusCode, &'static str)> {
    let state::AppState { root, app_name, .. } = state;
    let paths = collect_markdown_paths(&root).map_err(|err| {
        eprintln!("failed to list markdown files: {err}");
        (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

    let mut doc_ids: Vec<String> = paths
        .into_iter()
        .filter_map(|path| doc_id_from_path(&root, &path))
        .collect();
    doc_ids.sort();

    Ok(templates::DocumentListTemplate {
        app_name,
        documents: doc_ids,
    })
}

pub(crate) async fn document_search(
    State(state): State<state::AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<templates::SearchTemplate, (StatusCode, &'static str)> {
    let state::AppState { root, app_name, .. } = state;
    let query = query.q.unwrap_or_default();
    let trimmed = query.trim();
    let results = if trimmed.is_empty() {
        Vec::new()
    } else {
        search_documents(&root, trimmed).map_err(|err| {
            eprintln!("failed to search documents: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        })?
    };

    Ok(templates::SearchTemplate {
        app_name,
        query: trimmed.to_string(),
        results,
    })
}

#[derive(Debug, Deserialize)]
pub(crate) struct SearchQuery {
    pub(crate) q: Option<String>,
}

pub(crate) async fn document_view(
    State(state): State<state::AppState>,
    AxumPath(doc_id): AxumPath<String>,
) -> Result<templates::DocumentTemplate, (StatusCode, &'static str)> {
    let state::AppState { root, app_name, .. } = state;
    let contents = load_document(&root, &doc_id).map_err(|err| match err {
        DocError::NotFound => (StatusCode::NOT_FOUND, "document not found"),
        _ => {
            eprintln!("failed to load document {doc_id}: {err:?}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let mut body = String::new();
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    let parser =
        Parser::new_ext(&contents, options).map(|event| rewrite_relative_md_links(event, &doc_id));
    pulldown_cmark::html::push_html(&mut body, parser);

    Ok(templates::DocumentTemplate {
        app_name,
        doc_id,
        content: body,
    })
}

pub(crate) async fn document_edit(
    State(state): State<state::AppState>,
    AxumPath(doc_id): AxumPath<String>,
) -> Result<templates::EditTemplate, (StatusCode, &'static str)> {
    let state::AppState { root, app_name, .. } = state;
    let contents = load_document(&root, &doc_id).map_err(|err| match err {
        DocError::NotFound | DocError::BadPath => (StatusCode::NOT_FOUND, "not found"),
        DocError::Io(err) => {
            eprintln!("failed to load document {doc_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    Ok(templates::EditTemplate {
        app_name,
        doc_id,
        contents,
        notice: String::new(),
    })
}

pub(crate) async fn document_save(
    State(state): State<state::AppState>,
    AxumPath(doc_id): AxumPath<String>,
    Form(form): Form<EditForm>,
) -> Result<templates::EditTemplate, (StatusCode, &'static str)> {
    let state::AppState { root, app_name, .. } = state;
    let path = resolve_doc_path(&root, &doc_id).map_err(|err| match err {
        DocError::NotFound | DocError::BadPath => (StatusCode::NOT_FOUND, "not found"),
        DocError::Io(err) => {
            eprintln!("failed to resolve document {doc_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let metadata = std::fs::metadata(&path).map_err(|err| match err.kind() {
        ErrorKind::NotFound | ErrorKind::IsADirectory => (StatusCode::NOT_FOUND, "not found"),
        _ => {
            eprintln!("failed to stat document {doc_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;
    if !metadata.is_file() {
        return Err((StatusCode::NOT_FOUND, "not found"));
    }

    let normalized = normalize_newlines(&form.contents);
    atomic_write(&path, &normalized).map_err(|err| {
        eprintln!("failed to save document {doc_id}: {err}");
        (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

    Ok(templates::EditTemplate {
        app_name,
        doc_id,
        contents: normalized,
        notice: "Saved.".to_string(),
    })
}

pub(crate) fn collect_markdown_paths(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_markdown_paths_recursive(root, &mut paths)?;
    Ok(paths)
}

pub(crate) fn load_icon_bytes(
    root: &Path,
    path: Option<&Path>,
    fallback: &[u8],
    label: &str,
) -> Vec<u8> {
    let Some(path) = path else {
        return fallback.to_vec();
    };
    let resolved = match resolve_asset_path(root, path) {
        Ok(resolved) => resolved,
        Err(err) => {
            eprintln!("failed to resolve {label} icon path: {err}");
            return fallback.to_vec();
        }
    };
    match std::fs::read(&resolved) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!("failed to read {label} icon: {err}");
            fallback.to_vec()
        }
    }
}

pub(crate) fn resolve_asset_path(root: &Path, path: &Path) -> std::io::Result<PathBuf> {
    if path.is_absolute() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "absolute paths are not allowed",
        ));
    }
    let mut has_component = false;
    for component in path.components() {
        match component {
            Component::Normal(_) => {
                has_component = true;
            }
            _ => {
                return Err(std::io::Error::new(ErrorKind::InvalidInput, "invalid path"));
            }
        }
    }
    if !has_component {
        return Err(std::io::Error::new(ErrorKind::InvalidInput, "empty path"));
    }
    let joined = root.join(path);
    let resolved = std::fs::canonicalize(&joined)?;
    if !resolved.starts_with(root) {
        return Err(std::io::Error::new(
            ErrorKind::PermissionDenied,
            "path escapes root",
        ));
    }
    let metadata = std::fs::metadata(&resolved)?;
    if !metadata.is_file() {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "icon path is not a file",
        ));
    }
    Ok(resolved)
}

pub(crate) fn search_documents(
    root: &Path,
    query: &str,
) -> std::io::Result<Vec<templates::SearchResult>> {
    let mut results = Vec::new();
    let needle = query.to_lowercase();
    for path in collect_markdown_paths(root)? {
        let doc_id = match doc_id_from_path(root, &path) {
            Some(doc_id) => doc_id,
            None => continue,
        };
        let contents = std::fs::read_to_string(&path)?;
        if let Some(snippet) = find_match_snippet(&contents, &needle) {
            results.push(templates::SearchResult { doc_id, snippet });
        }
    }
    results.sort_by(|a, b| a.doc_id.cmp(&b.doc_id));
    Ok(results)
}

pub(crate) fn find_match_snippet(contents: &str, needle: &str) -> Option<String> {
    for line in contents.lines() {
        if line.to_lowercase().contains(needle) {
            return Some(line.trim().to_string());
        }
    }
    None
}

pub(crate) fn collect_markdown_paths_recursive(
    dir: &Path,
    paths: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let file_type = entry.file_type()?;
        if file_type.is_symlink() {
            continue;
        }

        let path = entry.path();
        if file_type.is_dir() {
            collect_markdown_paths_recursive(&path, paths)?;
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

pub(crate) fn load_document(root: &Path, doc_id: &str) -> Result<String, DocError> {
    let path = resolve_doc_path(root, doc_id)?;
    std::fs::read_to_string(&path).map_err(|err| match err.kind() {
        ErrorKind::NotFound | ErrorKind::IsADirectory => DocError::NotFound,
        _ => DocError::Io(err),
    })
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

pub(crate) fn doc_id_to_path(doc_id: &str) -> Option<PathBuf> {
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

pub(crate) fn rewrite_relative_md_links<'a>(event: Event<'a>, doc_id: &str) -> Event<'a> {
    match event {
        Event::Start(Tag::Link {
            link_type,
            dest_url,
            title,
            id,
        }) => {
            if let Some(new_dest) = rewrite_relative_md_link(doc_id, dest_url.as_ref()) {
                Event::Start(Tag::Link {
                    link_type,
                    dest_url: new_dest.into(),
                    title,
                    id,
                })
            } else {
                Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                })
            }
        }
        _ => event,
    }
}

pub(crate) fn rewrite_relative_md_link(doc_id: &str, dest_url: &str) -> Option<String> {
    let (path_part, fragment) = split_link_fragment(dest_url);
    if path_part.is_empty() || is_absolute_or_scheme(path_part) || !path_part.ends_with(".md") {
        return None;
    }

    let resolved = resolve_relative_doc_id(doc_id, path_part)?;
    doc_id_to_path(&resolved)?;

    let mut new_dest = String::from("/doc/");
    new_dest.push_str(&resolved);
    if let Some(fragment) = fragment {
        new_dest.push('#');
        new_dest.push_str(fragment);
    }
    Some(new_dest)
}

pub(crate) fn split_link_fragment(dest_url: &str) -> (&str, Option<&str>) {
    match dest_url.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (dest_url, None),
    }
}

pub(crate) fn is_absolute_or_scheme(path: &str) -> bool {
    if path.starts_with('/') || path.contains("://") {
        return true;
    }
    if let Some(colon) = path.find(':') {
        let slash = path.find('/');
        if slash.is_none_or(|slash| colon < slash) {
            return true;
        }
    }
    false
}

pub(crate) fn resolve_relative_doc_id(doc_id: &str, dest_path: &str) -> Option<String> {
    let mut parts: Vec<&str> = doc_id.split('/').collect();
    if parts.is_empty() {
        return None;
    }
    parts.pop();

    for part in dest_path.split('/') {
        match part {
            "" => return None,
            "." => {}
            ".." => {
                parts.pop()?;
            }
            _ => parts.push(part),
        }
    }

    if parts.is_empty() {
        return None;
    }
    Some(parts.join("/"))
}

#[derive(Debug, Deserialize)]
pub(crate) struct EditForm {
    pub(crate) contents: String,
}

pub(crate) fn atomic_write(path: &Path, contents: &str) -> std::io::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| std::io::Error::other("missing parent directory"))?;
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("document.md");
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
                file.write_all(contents.as_bytes())?;
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

pub(crate) fn normalize_newlines(contents: &str) -> String {
    if !contents.contains('\r') {
        return contents.to_string();
    }
    let normalized = contents.replace("\r\n", "\n");
    normalized.replace('\r', "\n")
}

#[derive(Debug)]
pub(crate) enum DocError {
    BadPath,
    NotFound,
    Io(std::io::Error),
}

#[cfg(test)]
#[allow(non_snake_case)]
pub(crate) mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn app__should_return_ok_on_health_endpoint() {
        let response = app(
            std::env::current_dir().expect("cwd"),
            config::AppConfig::default(),
        )
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .expect("request failed");

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        assert_eq!(body.as_ref(), b"ok");
    }

    #[test]
    fn render_document_list__should_include_links() {
        let doc_ids = vec!["notes/a.md".to_string(), "b.md".to_string()];
        let template = templates::DocumentListTemplate {
            app_name: "Mindex".to_string(),
            documents: doc_ids,
        };
        let html = template.render().unwrap();
        assert!(html.contains(r#"<a href="/doc/notes/a.md">notes/a.md</a>"#));
        assert!(html.contains(r#"<a href="/doc/b.md">b.md</a>"#));
    }

    #[test]
    fn render_markdown_document__should_rewrite_relative_md_links() {
        let markdown = "\
[B](b.md)
[Up](../c.md)
[Dot](./d.md)
[Frag](b.md#section)
[Abs](https://example.com/a.md)
[Root](/notes/e.md)
[Other](f.txt)
";
        let mut body = String::new();
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        let parser = Parser::new_ext(markdown, options)
            .map(|event| rewrite_relative_md_links(event, "notes/a.md"));
        pulldown_cmark::html::push_html(&mut body, parser);

        let template = templates::DocumentTemplate {
            app_name: "Mindex".to_string(),
            doc_id: "notes/a.md".to_string(),
            content: body,
        };
        let html = template.render().unwrap();
        assert!(html.contains(r#"href="/doc/notes/b.md""#));
        assert!(html.contains(r#"href="/doc/c.md""#));
        assert!(html.contains(r#"href="/doc/notes/d.md""#));
        assert!(html.contains(r#"href="/doc/notes/b.md#section""#));
        assert!(html.contains(r#"href="https://example.com/a.md""#));
        assert!(html.contains(r#"href="/notes/e.md""#));
        assert!(html.contains(r#"href="f.txt""#));
    }

    #[test]
    fn render_markdown_document__should_render_tables() {
        let markdown = "\
| A | B |
| --- | --- |
| 1 | 2 |
";
        let mut body = String::new();
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);
        let parser = Parser::new_ext(markdown, options)
            .map(|event| rewrite_relative_md_links(event, "table.md"));
        pulldown_cmark::html::push_html(&mut body, parser);

        let template = templates::DocumentTemplate {
            app_name: "Mindex".to_string(),
            doc_id: "table.md".to_string(),
            content: body,
        };
        let html = template.render().unwrap();
        assert!(html.contains("<table>"));
        assert!(html.contains("<thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<td>1</td>"));
        assert!(html.contains("<td>2</td>"));
    }

    #[test]
    fn render_edit_form__should_include_action_and_contents() {
        let template = templates::EditTemplate {
            app_name: "Mindex".to_string(),
            doc_id: "notes/food.md".to_string(),
            contents: "Line 1\nLine 2".to_string(),
            notice: String::new(),
        };
        let html = template.render().unwrap();
        assert!(html.contains(r#"action="/edit/notes/food.md""#));
        assert!(html.contains(r#"name="contents""#));
        assert!(html.contains("Line 1\nLine 2"));
    }

    #[test]
    fn render_edit_form__should_include_notice_when_present() {
        let template = templates::EditTemplate {
            app_name: "Mindex".to_string(),
            doc_id: "notes/food.md".to_string(),
            contents: "Body".to_string(),
            notice: "Saved.".to_string(),
        };
        let html = template.render().unwrap();
        assert!(html.contains("Saved."));
    }

    #[test]
    fn normalize_newlines__should_convert_crlf_to_lf() {
        let normalized = normalize_newlines("a\r\nb\rc");
        assert_eq!(normalized, "a\nb\nc");
    }

    #[tokio::test]
    async fn view_document__should_return_not_found_for_missing_doc() {
        let root = create_temp_root("missing-doc");
        let response = app(root.clone(), config::AppConfig::default())
            .oneshot(
                Request::builder()
                    .uri("/doc/missing.md")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request failed");

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn collect_markdown_paths__should_ignore_non_md_files_and_symlinks() {
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

        let mut doc_ids: Vec<String> = collect_markdown_paths(&root)
            .expect("collect paths")
            .into_iter()
            .filter_map(|path| doc_id_from_path(&root, &path))
            .collect();
        doc_ids.sort();

        assert_eq!(doc_ids, vec!["a.md".to_string(), "notes/c.md".to_string()]);

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
