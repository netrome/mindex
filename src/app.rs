use crate::adapters::WebPushSender;
use crate::assets;
use crate::config;
use crate::documents::{
    DocError, atomic_write, collect_markdown_paths, doc_id_from_path, load_document,
    normalize_newlines, resolve_doc_path, rewrite_relative_md_links,
};
use crate::ports::PushSender;
use crate::push;
use crate::push_types;
use crate::state;
use crate::templates;

use axum::Json;
use axum::Router;
use axum::extract::Form;
use axum::extract::Path as AxumPath;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::get;
use axum::routing::post;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;

use std::io::ErrorKind;
use std::path::Path;

pub fn app(config: config::AppConfig) -> Router {
    let push_registries = match push_types::DirectiveRegistries::load(&config.root) {
        Ok(registries) => std::sync::Arc::new(registries),
        Err(err) => {
            eprintln!("failed to load push directive registries: {err}");
            std::sync::Arc::new(push_types::DirectiveRegistries::default())
        }
    };
    let push_handles = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let state = state::AppState {
        config,
        push_registries,
        push_handles: std::sync::Arc::clone(&push_handles),
    };
    push::maybe_start_scheduler(
        &state.config,
        std::sync::Arc::clone(&state.push_registries),
        std::sync::Arc::clone(&push_handles),
    );
    Router::new()
        .route("/", get(document_list))
        .route("/search", get(document_search))
        .route("/edit/{*path}", get(document_edit).post(document_save))
        .route("/doc/{*path}", get(document_view))
        .route("/push/subscribe", get(push_subscribe))
        .route("/api/push/public-key", get(push_public_key))
        .route("/api/push/test", post(push_test))
        .route("/api/debug/push/registry", get(push_registry_debug))
        .route("/api/debug/push/schedule", get(push_schedule_debug))
        .route("/static/style.css", get(assets::stylesheet))
        .route("/static/theme.js", get(assets::theme_script))
        .route("/static/manifest.json", get(assets::manifest))
        .route("/sw.js", get(assets::service_worker))
        .route("/static/icons/icon-192.png", get(assets::icon_192))
        .route("/static/icons/icon-512.png", get(assets::icon_512))
        .route("/health", get(health))
        .with_state(state)
}

pub(crate) async fn health() -> &'static str {
    "ok"
}

pub(crate) async fn push_registry_debug(
    State(state): State<state::AppState>,
) -> Json<push_types::DirectiveRegistries> {
    Json((*state.push_registries).clone())
}

#[derive(Serialize, Deserialize)]
pub(crate) struct PushScheduleDebugResponse {
    server_time: OffsetDateTime,
    scheduled: Vec<PushScheduleEntry>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct PushScheduleEntry {
    doc_id: String,
    at: OffsetDateTime,
    message: String,
    to: Vec<String>,
    scheduled_at: OffsetDateTime,
    finished: bool,
}

pub(crate) async fn push_schedule_debug(
    State(state): State<state::AppState>,
) -> Json<PushScheduleDebugResponse> {
    let server_time = OffsetDateTime::now_utc();
    let scheduled = {
        let handles = state.push_handles.lock().expect("push handles lock");
        handles
            .iter()
            .map(|handle| PushScheduleEntry {
                doc_id: handle.notification.doc_id.clone(),
                at: handle.notification.at,
                message: handle.notification.message.clone(),
                to: handle.notification.to.clone(),
                scheduled_at: handle.scheduled_at,
                finished: handle.is_finished(),
            })
            .collect()
    };
    Json(PushScheduleDebugResponse {
        server_time,
        scheduled,
    })
}

#[derive(Serialize)]
pub(crate) struct PublicKeyResponse {
    #[serde(rename = "publicKey")]
    public_key: String,
}

#[derive(Serialize)]
pub(crate) struct ErrorResponse {
    error: &'static str,
}

pub(crate) async fn push_public_key(
    State(state): State<state::AppState>,
) -> Result<Json<PublicKeyResponse>, (StatusCode, Json<ErrorResponse>)> {
    let vapid = match push::load_vapid_config(&state.config) {
        push::VapidConfigStatus::Ready(vapid) => vapid,
        push::VapidConfigStatus::Incomplete | push::VapidConfigStatus::Missing => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Push notifications are not configured.",
                }),
            ));
        }
    };

    Ok(Json(PublicKeyResponse {
        public_key: vapid.public_key,
    }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct TestPushRequest {
    pub(crate) endpoint: String,
    pub(crate) p256dh: String,
    pub(crate) auth: String,
    pub(crate) message: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct TestPushResponse {
    status: &'static str,
}

pub(crate) async fn push_test(
    State(state): State<state::AppState>,
    Json(request): Json<TestPushRequest>,
) -> Result<Json<TestPushResponse>, (StatusCode, Json<ErrorResponse>)> {
    let vapid = match push::load_vapid_config(&state.config) {
        push::VapidConfigStatus::Ready(vapid) => vapid,
        push::VapidConfigStatus::Incomplete | push::VapidConfigStatus::Missing => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Push notifications are not configured.",
                }),
            ));
        }
    };

    if request.endpoint.trim().is_empty()
        || request.p256dh.trim().is_empty()
        || request.auth.trim().is_empty()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "endpoint, p256dh, and auth are required.",
            }),
        ));
    }

    let message = request
        .message
        .as_deref()
        .unwrap_or("Test notification from Mindex")
        .trim();
    if message.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "message must not be empty.",
            }),
        ));
    }

    let sender = WebPushSender::new(vapid).map_err(|err| {
        eprintln!("push test error: failed to init web-push ({err})");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to initialize push sender.",
            }),
        )
    })?;

    let subscription = push_types::Subscription {
        endpoint: request.endpoint,
        p256dh: request.p256dh,
        auth: request.auth,
    };

    if let Err(err) = sender.send(&subscription, message).await {
        eprintln!("push test error: {err}");
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "Failed to send test notification.",
            }),
        ));
    }

    Ok(Json(TestPushResponse { status: "sent" }))
}

pub(crate) async fn push_subscribe(
    State(state): State<state::AppState>,
) -> templates::PushSubscribeTemplate {
    templates::PushSubscribeTemplate {
        app_name: state.config.app_name,
    }
}

pub(crate) async fn document_list(
    State(state): State<state::AppState>,
) -> Result<templates::DocumentListTemplate, (StatusCode, &'static str)> {
    let state::AppState {
        config: config::AppConfig { root, app_name, .. },
        ..
    } = state;
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
    let query = query.q.unwrap_or_default();
    let trimmed = query.trim();
    let results = if trimmed.is_empty() {
        Vec::new()
    } else {
        search_documents(&state.config.root, trimmed).map_err(|err| {
            eprintln!("failed to search documents: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        })?
    };

    Ok(templates::SearchTemplate {
        app_name: state.config.app_name,
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
    let contents = load_document(&state.config.root, &doc_id).map_err(|err| match err {
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
        app_name: state.config.app_name,
        doc_id,
        content: body,
    })
}

pub(crate) async fn document_edit(
    State(state): State<state::AppState>,
    AxumPath(doc_id): AxumPath<String>,
) -> Result<templates::EditTemplate, (StatusCode, &'static str)> {
    let contents = load_document(&state.config.root, &doc_id).map_err(|err| match err {
        DocError::NotFound | DocError::BadPath => (StatusCode::NOT_FOUND, "not found"),
        DocError::Io(err) => {
            eprintln!("failed to load document {doc_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    Ok(templates::EditTemplate {
        app_name: state.config.app_name,
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
    let path = resolve_doc_path(&state.config.root, &doc_id).map_err(|err| match err {
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
        app_name: state.config.app_name,
        doc_id,
        contents: normalized,
        notice: "Saved.".to_string(),
    })
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

#[derive(Debug, Deserialize)]
pub(crate) struct EditForm {
    pub(crate) contents: String,
}

#[cfg(test)]
#[allow(non_snake_case)]
pub(crate) mod tests {
    use super::*;
    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::http::Request;
    use axum::http::StatusCode;
    use serde_json::from_slice as json_from_slice;
    use tower::ServiceExt;

    use askama::Template as _;
    use std::path::PathBuf;

    #[tokio::test]
    async fn app__should_return_ok_on_health_endpoint() {
        let response = app(config::AppConfig::default())
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

    #[tokio::test]
    async fn push_registry_debug__should_return_loaded_directives() {
        let root = create_temp_root("push-registry");
        let contents = r#"/user
```toml
name = "marten"
display_name = "Marten"
```

/subscription
```toml
user = "marten"
endpoint = "https://push.example/123"
p256dh = "p256"
auth = "auth"
```

/notify
```toml
to = "marten"
at = "2025-01-12T09:30:00Z"
message = "Check the daily log."
```
"#;
        std::fs::write(root.join("notify.md"), contents).expect("write notify.md");
        let app_config = config::AppConfig {
            root: root.clone(),
            ..Default::default()
        };

        let response = app(app_config)
            .oneshot(
                Request::builder()
                    .uri("/api/debug/push/registry")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request failed");

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let registries: push_types::DirectiveRegistries =
            json_from_slice(&body).expect("parse json");

        let user = registries.users.get("marten").expect("user entry");
        assert_eq!(user.display_name.as_deref(), Some("Marten"));

        let subscriptions = registries
            .subscriptions
            .get("marten")
            .expect("subscriptions");
        assert_eq!(subscriptions.len(), 1);
        assert_eq!(subscriptions[0].endpoint, "https://push.example/123");
        assert_eq!(subscriptions[0].p256dh, "p256");
        assert_eq!(subscriptions[0].auth, "auth");

        assert_eq!(registries.notifications.len(), 1);
        let notification = &registries.notifications[0];
        assert_eq!(notification.to, vec!["marten".to_string()]);
        assert_eq!(
            notification.at.to_string(),
            "2025-01-12 9:30:00.0 +00:00:00"
        );
        assert_eq!(notification.message, "Check the daily log.");
        assert_eq!(notification.doc_id, "notify.md");

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[tokio::test]
    async fn push_schedule_debug__should_return_server_time_and_entries() {
        let root = create_temp_root("push-schedule");
        let app_config = config::AppConfig {
            root: root.clone(),
            ..Default::default()
        };

        let response = app(app_config)
            .oneshot(
                Request::builder()
                    .uri("/api/debug/push/schedule")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request failed");

        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let debug: PushScheduleDebugResponse = json_from_slice(&body).expect("parse json");

        assert!(debug.server_time.unix_timestamp() > 0);
        assert!(debug.scheduled.is_empty());

        std::fs::remove_dir_all(&root).expect("cleanup");
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

    #[tokio::test]
    async fn view_document__should_return_not_found_for_missing_doc() {
        let root = create_temp_root("missing-doc");
        let app_config = config::AppConfig {
            root: root.clone(),
            ..Default::default()
        };

        let response = app(app_config)
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
