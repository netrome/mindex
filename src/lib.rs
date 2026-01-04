use axum::{
    Router,
    extract::{Path as AxumPath, State},
    http::StatusCode,
    response::Html,
    routing::get,
};
use pulldown_cmark::{Parser, html};
use std::{
    io::ErrorKind,
    net::SocketAddr,
    path::{Component, Path, PathBuf},
};

#[derive(Clone)]
struct AppState {
    root: PathBuf,
}

pub fn app(root: PathBuf) -> Router {
    let state = AppState { root };
    Router::new()
        .route("/", get(list_documents))
        .route("/doc/{*path}", get(view_document))
        .route("/health", get(health))
        .with_state(state)
}

pub async fn serve(addr: SocketAddr, root: PathBuf) {
    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("failed to bind address");
    axum::serve(listener, app(root))
        .await
        .expect("server error");
}

async fn health() -> &'static str {
    "ok"
}

async fn list_documents(
    State(state): State<AppState>,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    let paths = collect_markdown_paths(&state.root).map_err(|err| {
        eprintln!("failed to list markdown files: {err}");
        (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

    let mut doc_ids: Vec<String> = paths
        .into_iter()
        .filter_map(|path| doc_id_from_path(&state.root, &path))
        .collect();
    doc_ids.sort();

    Ok(Html(render_document_list(&doc_ids)))
}

async fn view_document(
    State(state): State<AppState>,
    AxumPath(doc_id): AxumPath<String>,
) -> Result<Html<String>, (StatusCode, &'static str)> {
    let contents = load_document(&state.root, &doc_id).map_err(|err| match err {
        DocError::NotFound | DocError::BadPath => (StatusCode::NOT_FOUND, "not found"),
        DocError::Io(err) => {
            eprintln!("failed to load document {doc_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    Ok(Html(render_markdown_document(&doc_id, &contents)))
}

fn collect_markdown_paths(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    collect_markdown_paths_recursive(root, &mut paths)?;
    Ok(paths)
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

fn doc_id_from_path(root: &Path, path: &Path) -> Option<String> {
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

fn render_document_list(doc_ids: &[String]) -> String {
    let mut html = String::from(
        "<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>Mindex</title>\n</head>\n<body>\n<h1>Documents</h1>\n",
    );
    if doc_ids.is_empty() {
        html.push_str("<p>No documents found.</p>\n");
    } else {
        html.push_str("<ul>\n");
        for doc_id in doc_ids {
            html.push_str("  <li><a href=\"/doc/");
            html.push_str(doc_id);
            html.push_str("\">");
            html.push_str(doc_id);
            html.push_str("</a></li>\n");
        }
        html.push_str("</ul>\n");
    }
    html.push_str("</body>\n</html>\n");
    html
}

fn load_document(root: &Path, doc_id: &str) -> Result<String, DocError> {
    let path = resolve_doc_path(root, doc_id)?;
    std::fs::read_to_string(&path).map_err(|err| match err.kind() {
        ErrorKind::NotFound | ErrorKind::IsADirectory => DocError::NotFound,
        _ => DocError::Io(err),
    })
}

fn resolve_doc_path(root: &Path, doc_id: &str) -> Result<PathBuf, DocError> {
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

fn doc_id_to_path(doc_id: &str) -> Option<PathBuf> {
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

fn render_markdown_document(doc_id: &str, contents: &str) -> String {
    let mut body = String::new();
    let parser = Parser::new(contents);
    html::push_html(&mut body, parser);

    let title = escape_html(doc_id);
    let mut html =
        String::from("<!doctype html>\n<html>\n<head>\n<meta charset=\"utf-8\">\n<title>");
    html.push_str(&title);
    html.push_str(" - Mindex</title>\n</head>\n<body>\n");
    html.push_str("<p><a href=\"/\">Back</a></p>\n");
    html.push_str("<h1>");
    html.push_str(&title);
    html.push_str("</h1>\n");
    html.push_str(&body);
    html.push_str("\n</body>\n</html>\n");
    html
}

fn escape_html(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => escaped.push_str("&amp;"),
            '<' => escaped.push_str("&lt;"),
            '>' => escaped.push_str("&gt;"),
            '"' => escaped.push_str("&quot;"),
            '\'' => escaped.push_str("&#39;"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

#[derive(Debug)]
enum DocError {
    BadPath,
    NotFound,
    Io(std::io::Error),
}

#[cfg(test)]
#[allow(non_snake_case)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn app__should_return_ok_on_health_endpoint() {
        let response = app(std::env::current_dir().expect("cwd"))
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
        let html = render_document_list(&doc_ids);
        assert!(html.contains(r#"<a href="/doc/notes/a.md">notes/a.md</a>"#));
        assert!(html.contains(r#"<a href="/doc/b.md">b.md</a>"#));
    }

    #[tokio::test]
    async fn view_document__should_return_not_found_for_missing_doc() {
        let root = create_temp_root("missing-doc");
        let response = app(root.clone())
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
