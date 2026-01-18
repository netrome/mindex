use crate::assets;
use crate::auth as auth_service;
use crate::config;
use crate::git as git_service;
use crate::push as push_service;
use crate::state;
use crate::types::directives;

use axum::Router;
use axum::middleware;
use axum::routing::get;
use axum::routing::post;

mod auth;
mod documents;
mod git;
mod push;

pub fn app(config: config::AppConfig) -> Router {
    let auth = auth_service::AuthState::from_config(&config)
        .unwrap_or_else(|err| panic!("invalid auth configuration: {err}"));
    let git_dir = match git_service::git_dir_within_root(&config.root) {
        Ok(git_dir) => git_dir,
        Err(err) => {
            eprintln!("failed to resolve git directory: {err}");
            None
        }
    };
    let push_registries = match directives::DirectiveRegistries::load(&config.root) {
        Ok(registries) => registries,
        Err(err) => {
            eprintln!("failed to load push directive registries: {err}");
            directives::DirectiveRegistries::default()
        }
    };
    let push_registries = std::sync::Arc::new(std::sync::Mutex::new(push_registries));
    let push_handles = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let state = state::AppState {
        config,
        auth,
        push_registries: std::sync::Arc::clone(&push_registries),
        push_handles: std::sync::Arc::clone(&push_handles),
        git_dir,
    };
    let registries_snapshot = {
        let registries = push_registries.lock().expect("push registries lock");
        std::sync::Arc::new(registries.clone())
    };
    push_service::maybe_start_scheduler(&state.config, registries_snapshot, push_handles);
    Router::new()
        .route("/", get(documents::document_list))
        .route("/login", get(auth::login_form).post(auth::login_submit))
        .route("/logout", post(auth::logout))
        .route("/search", get(documents::document_search))
        .route(
            "/new",
            get(documents::document_new).post(documents::document_create),
        )
        .route(
            "/edit/{*path}",
            get(documents::document_edit).post(documents::document_save),
        )
        .route("/reorder/{*path}", get(documents::document_reorder))
        .route("/doc/{*path}", get(documents::document_view))
        .route("/git", get(git::git_view))
        .route("/git/commit", post(git::git_commit))
        .route("/git/push", post(git::git_push))
        .route("/git/pull", post(git::git_pull))
        .route(
            "/api/doc/toggle-task",
            post(documents::document_toggle_task),
        )
        .route("/api/doc/add-task", post(documents::document_add_task))
        .route(
            "/api/doc/reorder-range",
            post(documents::document_reorder_range),
        )
        .route("/push/subscribe", get(push::push_subscribe))
        .route("/api/push/public-key", get(push::push_public_key))
        .route("/api/push/test", post(push::push_test))
        .route("/api/debug/push/registry", get(push::push_registry_debug))
        .route("/api/debug/push/schedule", get(push::push_schedule_debug))
        .route("/static/style.css", get(assets::stylesheet))
        .route("/static/theme.js", get(assets::theme_script))
        .route("/static/app.js", get(assets::app_script))
        .route(
            "/static/features/todo_toggle.js",
            get(assets::todo_toggle_script),
        )
        .route(
            "/static/features/push_subscribe.js",
            get(assets::push_subscribe_script),
        )
        .route(
            "/static/features/sw_register.js",
            get(assets::sw_register_script),
        )
        .route("/static/manifest.json", get(assets::manifest))
        .route("/sw.js", get(assets::service_worker))
        .route("/static/icons/icon-192.png", get(assets::icon_192))
        .route("/static/icons/icon-512.png", get(assets::icon_512))
        .route("/health", get(health))
        .with_state(state.clone())
        .layer(middleware::from_fn_with_state(state, auth::auth_middleware))
}

pub(crate) async fn health() -> &'static str {
    "ok"
}

#[cfg(test)]
#[allow(non_snake_case)]
pub(crate) mod tests {
    use super::*;
    use crate::documents::rewrite_relative_md_links;
    use crate::templates;
    use crate::types::directives;
    use argon2::password_hash::SaltString;
    use argon2::{Argon2, PasswordHasher};
    use axum::body::Body;
    use axum::body::to_bytes;
    use axum::extract::Form;
    use axum::extract::Path as AxumPath;
    use axum::extract::State;
    use axum::http::Request;
    use axum::http::StatusCode;
    use axum::http::header::{COOKIE, LOCATION, SET_COOKIE};
    use base64::{URL_SAFE_NO_PAD, encode_config};
    use jwt_simple::algorithms::MACLike;
    use jwt_simple::prelude::{Claims, Duration as JwtDuration, HS256Key};
    use pulldown_cmark::{Event, Options, Parser};
    use serde_json::Value as JsonValue;
    use serde_json::from_slice as json_from_slice;
    use time::Duration;
    use tower::ServiceExt;

    use askama::Template as _;
    use std::path::{Path, PathBuf};

    #[tokio::test]
    async fn app__should_return_ok_on_health_endpoint() {
        // Given
        let app = app(config::AppConfig::default());

        // When
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request failed");

        // Then
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        assert_eq!(body.as_ref(), b"ok");
    }

    #[tokio::test]
    async fn auth_middleware__should_redirect_html_when_missing_cookie() {
        // Given
        let root = create_temp_root("auth-redirect");
        let key_bytes = b"auth-redirect-secret";
        let app_config = auth_app_config(root.clone(), key_bytes);

        // When
        let response = app(app_config)
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .expect("request failed");

        // Then
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let location = response.headers().get(LOCATION).expect("location header");
        assert_eq!(location, "/login");

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[tokio::test]
    async fn auth_middleware__should_return_json_unauthorized_for_api() {
        // Given
        let root = create_temp_root("auth-api-unauthorized");
        let key_bytes = b"auth-api-secret";
        let app_config = auth_app_config(root.clone(), key_bytes);

        // When
        let response = app(app_config)
            .oneshot(
                Request::builder()
                    .uri("/api/debug/push/registry")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request failed");

        // Then
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let payload: JsonValue = json_from_slice(&body).expect("parse json");
        assert_eq!(payload["error"], "unauthorized");

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[tokio::test]
    async fn auth_middleware__should_allow_valid_cookie() {
        // Given
        let root = create_temp_root("auth-valid");
        let key_bytes = b"auth-valid-secret";
        let app_config = auth_app_config(root.clone(), key_bytes);
        let issuer = app_config.app_name.clone();
        let cookie_name = app_config
            .auth
            .as_ref()
            .expect("auth config")
            .cookie_name
            .clone();
        let token = auth_token(key_bytes, &issuer, "marten");
        let cookie = format!("{cookie_name}={token}");

        // When
        let response = app(app_config)
            .oneshot(
                Request::builder()
                    .uri("/api/debug/push/registry")
                    .header(COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request failed");

        // Then
        assert_eq!(response.status(), StatusCode::OK);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[tokio::test]
    async fn login__should_set_cookie_and_redirect() {
        // Given
        let root = create_temp_root("login-success");
        let key_bytes = b"auth-login-secret";
        let app_config = auth_app_config(root.clone(), key_bytes);
        let password_hash = hash_password_for_test("secret");
        write_user_doc(&root, "marten", "marten@example.com", &password_hash);
        let form = "name=marten&password=secret&next=%2Fdoc%2Fnote.md";

        // When
        let response = app(app_config)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/login")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(form))
                    .unwrap(),
            )
            .await
            .expect("request failed");

        // Then
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get(LOCATION).expect("location header"),
            "/doc/note.md"
        );
        let cookie = response.headers().get(SET_COOKIE).expect("set-cookie");
        let cookie = cookie.to_str().expect("cookie header");
        assert!(cookie.contains("mindex_auth="));
        assert!(cookie.contains("HttpOnly"));
        assert!(cookie.contains("SameSite=Lax"));

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[tokio::test]
    async fn login__should_reject_invalid_credentials() {
        // Given
        let root = create_temp_root("login-failure");
        let key_bytes = b"auth-login-fail";
        let app_config = auth_app_config(root.clone(), key_bytes);
        let password_hash = hash_password_for_test("secret");
        write_user_doc(&root, "marten", "marten@example.com", &password_hash);
        let form = "name=marten&password=wrong";

        // When
        let response = app(app_config)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/login")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(form))
                    .unwrap(),
            )
            .await
            .expect("request failed");

        // Then
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let body = std::str::from_utf8(&body).expect("utf8 body");
        assert!(body.contains("Invalid username or password."));

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[tokio::test]
    async fn logout__should_clear_cookie() {
        // Given
        let root = create_temp_root("logout");
        let key_bytes = b"auth-logout-secret";
        let app_config = auth_app_config(root.clone(), key_bytes);

        // When
        let response = app(app_config)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/logout")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request failed");

        // Then
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers().get(LOCATION).expect("location header"),
            "/login"
        );
        let cookie = response.headers().get(SET_COOKIE).expect("set-cookie");
        let cookie = cookie.to_str().expect("cookie header");
        assert!(cookie.contains("Max-Age=0"));

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[tokio::test]
    async fn push_registry_debug__should_return_loaded_directives() {
        // Given
        let root = create_temp_root("push-registry");
        let contents = r#"/user
```toml
name = "marten"
display_name = "Marten"
email = "marten@example.com"
password_hash = "hash"
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

        // When
        let response = app(app_config)
            .oneshot(
                Request::builder()
                    .uri("/api/debug/push/registry")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request failed");

        // Then
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let registries: directives::DirectiveRegistries =
            json_from_slice(&body).expect("parse json");

        let user = registries.users.get("marten").expect("user entry");
        assert_eq!(user.display_name.as_deref(), Some("Marten"));
        assert_eq!(user.email, "marten@example.com");
        assert_eq!(user.password_hash, "hash");

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
        // Given
        let root = create_temp_root("push-schedule");
        let app_config = config::AppConfig {
            root: root.clone(),
            ..Default::default()
        };

        // When
        let response = app(app_config)
            .oneshot(
                Request::builder()
                    .uri("/api/debug/push/schedule")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request failed");

        // Then
        assert_eq!(response.status(), StatusCode::OK);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let debug: push::PushScheduleDebugResponse = json_from_slice(&body).expect("parse json");

        assert!(debug.server_time.unix_timestamp() > 0);
        assert!(debug.scheduled.is_empty());

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[tokio::test]
    async fn document_save__should_refresh_push_registries() {
        // Given
        let root = create_temp_root("push-refresh");
        std::fs::write(root.join("note.md"), "Initial").expect("write note.md");
        let app_state = state::AppState {
            config: config::AppConfig {
                root: root.clone(),
                ..Default::default()
            },
            auth: None,
            push_registries: std::sync::Arc::new(std::sync::Mutex::new(
                directives::DirectiveRegistries::default(),
            )),
            push_handles: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            git_dir: None,
        };

        let form = documents::EditForm {
            contents: r#"/user
```toml
name = "marten"
email = "marten@example.com"
password_hash = "hash"
```
"#
            .to_string(),
        };

        // When
        documents::document_save(
            State(app_state.clone()),
            AxumPath("note.md".to_string()),
            Form(form),
        )
        .await
        .expect("save note.md");

        // Then
        let registries = app_state
            .push_registries
            .lock()
            .expect("push registries lock")
            .clone();
        assert!(registries.users.contains_key("marten"));

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[test]
    fn render_document_list__should_include_links() {
        // Given
        let doc_ids = vec!["notes/a.md".to_string(), "b.md".to_string()];
        let template = templates::DocumentListTemplate {
            app_name: "Mindex".to_string(),
            documents: doc_ids,
            git_enabled: false,
        };

        // When
        let html = template.render().unwrap();

        // Then
        assert!(html.contains(r#"<a href="/doc/notes/a.md">notes/a.md</a>"#));
        assert!(html.contains(r#"<a href="/doc/b.md">b.md</a>"#));
    }

    #[test]
    fn render_markdown_document__should_render_tables() {
        // Given
        let markdown = "\
| A | B |
| --- | --- |
| 1 | 2 |
";
        let mut body = String::new();
        let mut options = Options::empty();
        options.insert(Options::ENABLE_TABLES);

        // When
        let parser = Parser::new_ext(markdown, options)
            .map(|event| rewrite_relative_md_links(event, "table.md"));
        pulldown_cmark::html::push_html(&mut body, parser);
        let template = templates::DocumentTemplate {
            app_name: "Mindex".to_string(),
            doc_id: "table.md".to_string(),
            content: body,
            git_enabled: false,
        };
        let html = template.render().unwrap();

        // Then
        assert!(html.contains("<table>"));
        assert!(html.contains("<thead>"));
        assert!(html.contains("<tbody>"));
        assert!(html.contains("<td>1</td>"));
        assert!(html.contains("<td>2</td>"));
    }

    #[test]
    fn render_markdown_document__should_render_inline_math() {
        // Given
        let markdown = "The equation $x^2$ is simple.";
        let mut body = String::new();
        let mut options = Options::empty();
        options.insert(Options::ENABLE_MATH);

        // When
        let parser = Parser::new_ext(markdown, options).map(|event| match event {
            Event::InlineMath(latex) => {
                let html =
                    crate::math::render_math(&latex, crate::math::MathStyle::Inline).into_html();
                Event::Html(html.into())
            }
            other => other,
        });
        pulldown_cmark::html::push_html(&mut body, parser);

        // Then
        assert!(body.contains("<math"));
        assert!(body.contains("</math>"));
        // Should not contain the raw dollar signs
        assert!(!body.contains("$x^2$"));
    }

    #[test]
    fn render_markdown_document__should_render_display_math() {
        // Given
        let markdown = "$$\\frac{a}{b}$$";
        let mut body = String::new();
        let mut options = Options::empty();
        options.insert(Options::ENABLE_MATH);

        // When
        let parser = Parser::new_ext(markdown, options).map(|event| match event {
            Event::DisplayMath(latex) => {
                let html =
                    crate::math::render_math(&latex, crate::math::MathStyle::Display).into_html();
                Event::Html(html.into())
            }
            other => other,
        });
        pulldown_cmark::html::push_html(&mut body, parser);

        // Then
        assert!(body.contains("<math"));
        assert!(body.contains(r#"display="block""#));
        assert!(body.contains("<mfrac>"));
    }

    #[test]
    fn render_edit_form__should_include_action_and_contents() {
        // Given
        let template = templates::EditTemplate {
            app_name: "Mindex".to_string(),
            doc_id: "notes/food.md".to_string(),
            contents: "Line 1\nLine 2".to_string(),
            notice: String::new(),
            git_enabled: false,
        };

        // When
        let html = template.render().unwrap();

        // Then
        assert!(html.contains(r#"action="/edit/notes/food.md""#));
        assert!(html.contains(r#"name="contents""#));
        assert!(html.contains("Line 1\nLine 2"));
    }

    #[test]
    fn render_edit_form__should_include_notice_when_present() {
        // Given
        let template = templates::EditTemplate {
            app_name: "Mindex".to_string(),
            doc_id: "notes/food.md".to_string(),
            contents: "Body".to_string(),
            notice: "Saved.".to_string(),
            git_enabled: false,
        };

        // When
        let html = template.render().unwrap();

        // Then
        assert!(html.contains("Saved."));
    }

    #[test]
    fn render_reorder_page__should_render_line_entries() {
        // Given
        let template = templates::ReorderTemplate {
            app_name: "Mindex".to_string(),
            doc_id: "notes/food.md".to_string(),
            lines: vec![
                templates::ReorderLine {
                    index: 0,
                    text: "First line".to_string(),
                },
                templates::ReorderLine {
                    index: 1,
                    text: String::new(),
                },
            ],
            git_enabled: false,
        };

        // When
        let html = template.render().unwrap();

        // Then
        assert!(html.contains("Reorder notes/food.md"));
        assert!(html.contains(r#"data-start-line="0""#));
        assert!(html.contains(r#"data-end-line="0""#));
        assert!(html.contains("First line"));
    }

    #[tokio::test]
    async fn view_document__should_return_not_found_for_missing_doc() {
        // Given
        let root = create_temp_root("missing-doc");
        let app_config = config::AppConfig {
            root: root.clone(),
            ..Default::default()
        };

        // When
        let response = app(app_config)
            .oneshot(
                Request::builder()
                    .uri("/doc/missing.md")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request failed");

        // Then
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[tokio::test]
    async fn view_document_reorder__should_render_reorder_page() {
        // Given
        let root = create_temp_root("reorder-view");
        std::fs::write(root.join("note.md"), "Line 1\nLine 2").expect("write note.md");
        let app_config = config::AppConfig {
            root: root.clone(),
            ..Default::default()
        };

        // When
        let response = app(app_config)
            .oneshot(
                Request::builder()
                    .uri("/reorder/note.md")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .expect("request failed");

        // Then
        assert_eq!(response.status(), StatusCode::OK);
        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read body");
        let body = std::str::from_utf8(&body).expect("utf8");
        assert!(body.contains("Reorder note.md"));
        assert!(body.contains(r#"data-start-line="0""#));

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[tokio::test]
    async fn document_reorder_range__should_update_document() {
        // Given
        let root = create_temp_root("reorder-range");
        std::fs::write(root.join("note.md"), "a\nb\nc\n").expect("write note.md");
        let app_config = config::AppConfig {
            root: root.clone(),
            ..Default::default()
        };

        // When
        let body = "doc_id=note.md&start_line=0&end_line=0&insert_before_line=2&mode=line";
        let response = app(app_config)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/doc/reorder-range")
                    .header("content-type", "application/x-www-form-urlencoded")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .expect("request failed");

        // Then
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        let updated = std::fs::read_to_string(root.join("note.md")).expect("read note.md");
        assert_eq!(updated, "b\na\nc\n");

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    #[tokio::test]
    async fn git_view__should_return_not_found_when_git_unavailable() {
        // Given
        let root = create_temp_root("git-missing");
        let app_config = config::AppConfig {
            root: root.clone(),
            ..Default::default()
        };

        // When
        let response = app(app_config)
            .oneshot(Request::builder().uri("/git").body(Body::empty()).unwrap())
            .await
            .expect("request failed");

        // Then
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        std::fs::remove_dir_all(&root).expect("cleanup");
    }

    fn auth_app_config(root: PathBuf, key_bytes: &[u8]) -> config::AppConfig {
        let key = encode_config(key_bytes, URL_SAFE_NO_PAD);
        config::AppConfig {
            root,
            app_name: "Mindex".to_string(),
            auth: Some(config::AuthConfig {
                key,
                token_ttl: Duration::days(1),
                cookie_name: "mindex_auth".to_string(),
                cookie_secure: false,
            }),
            ..Default::default()
        }
    }

    fn auth_token(key_bytes: &[u8], issuer: &str, subject: &str) -> String {
        let key = HS256Key::from_bytes(key_bytes);
        let claims = Claims::create(JwtDuration::from_hours(1))
            .with_issuer(issuer)
            .with_subject(subject);
        key.authenticate(claims).expect("authenticate token")
    }

    fn hash_password_for_test(password: &str) -> String {
        let salt = SaltString::encode_b64(b"mindex-tests").expect("salt");
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .expect("hash password")
            .to_string()
    }

    fn write_user_doc(root: &Path, name: &str, email: &str, password_hash: &str) {
        let contents = format!(
            r#"/user
```toml
name = "{name}"
email = "{email}"
password_hash = "{password_hash}"
```
"#
        );
        std::fs::write(root.join("users.md"), contents).expect("write users.md");
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
