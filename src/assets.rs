use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

use crate::{state, templates};

use axum::extract::State;

use askama::Template as _;

pub(crate) const ICON_192_FALLBACK: &[u8] = include_bytes!("../assets/icons/icon-192.png");
pub(crate) const ICON_512_FALLBACK: &[u8] = include_bytes!("../assets/icons/icon-512.png");

pub(crate) async fn manifest(State(state): State<state::AppState>) -> axum::response::Response {
    let manifest_body = templates::ManifestTemplate {
        app_name: &state.config.app_name,
    }
    .render()
    .unwrap_or_else(|err| {
        eprintln!("failed to render manifest: {err}");
        "{}".to_string()
    });

    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/manifest+json")
        .header("cache-control", "public, max-age=3600")
        .body(manifest_body.into())
        .unwrap()
}

pub(crate) async fn stylesheet() -> axum::response::Response {
    const CSS_CONTENT: &str = include_str!("../assets/style.css");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "text/css")
        .header("cache-control", "public, max-age=3600")
        .body(CSS_CONTENT.into())
        .unwrap()
}

pub(crate) async fn theme_script() -> axum::response::Response {
    const THEME_JS_CONTENT: &str = include_str!("../assets/theme.js");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/javascript")
        .header("cache-control", "public, max-age=3600")
        .body(THEME_JS_CONTENT.into())
        .unwrap()
}

pub(crate) async fn app_script() -> axum::response::Response {
    const APP_JS_CONTENT: &str = include_str!("../assets/app.js");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/javascript")
        .header("cache-control", "public, max-age=3600")
        .body(APP_JS_CONTENT.into())
        .unwrap()
}

pub(crate) async fn mermaid_script() -> axum::response::Response {
    const MERMAID_JS_CONTENT: &str = include_str!("../assets/mermaid.min.js");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/javascript")
        .header("cache-control", "public, max-age=3600")
        .body(MERMAID_JS_CONTENT.into())
        .unwrap()
}

pub(crate) async fn abcjs_script() -> axum::response::Response {
    const ABCJS_CONTENT: &str = include_str!("../assets/vendor/abcjs.min.js");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/javascript")
        .header("cache-control", "public, max-age=3600")
        .body(ABCJS_CONTENT.into())
        .unwrap()
}

pub(crate) async fn abc_render_script() -> axum::response::Response {
    const ABC_RENDER_JS_CONTENT: &str = include_str!("../assets/features/abc_render.js");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/javascript")
        .header("cache-control", "public, max-age=3600")
        .body(ABC_RENDER_JS_CONTENT.into())
        .unwrap()
}

pub(crate) async fn todo_toggle_script() -> axum::response::Response {
    const TODO_TOGGLE_JS_CONTENT: &str = include_str!("../assets/features/todo_toggle.js");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/javascript")
        .header("cache-control", "public, max-age=3600")
        .body(TODO_TOGGLE_JS_CONTENT.into())
        .unwrap()
}

pub(crate) async fn reorder_script() -> axum::response::Response {
    const REORDER_JS_CONTENT: &str = include_str!("../assets/features/reorder.js");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/javascript")
        .header("cache-control", "public, max-age=3600")
        .body(REORDER_JS_CONTENT.into())
        .unwrap()
}

pub(crate) async fn push_subscribe_script() -> axum::response::Response {
    const PUSH_SUBSCRIBE_JS_CONTENT: &str = include_str!("../assets/features/push_subscribe.js");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/javascript")
        .header("cache-control", "public, max-age=3600")
        .body(PUSH_SUBSCRIBE_JS_CONTENT.into())
        .unwrap()
}

pub(crate) async fn uploads_script() -> axum::response::Response {
    const UPLOADS_JS_CONTENT: &str = include_str!("../assets/features/uploads.js");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/javascript")
        .header("cache-control", "public, max-age=3600")
        .body(UPLOADS_JS_CONTENT.into())
        .unwrap()
}

pub(crate) async fn editor_paste_upload_script() -> axum::response::Response {
    const EDITOR_PASTE_JS_CONTENT: &str = include_str!("../assets/features/editor_paste_upload.js");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/javascript")
        .header("cache-control", "public, max-age=3600")
        .body(EDITOR_PASTE_JS_CONTENT.into())
        .unwrap()
}

pub(crate) async fn sw_register_script() -> axum::response::Response {
    const SW_REGISTER_JS_CONTENT: &str = include_str!("../assets/features/sw_register.js");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/javascript")
        .header("cache-control", "public, max-age=3600")
        .body(SW_REGISTER_JS_CONTENT.into())
        .unwrap()
}

pub(crate) async fn service_worker(
    State(state): State<state::AppState>,
) -> axum::response::Response {
    let auth_enabled = state.config.auth.is_some();
    let rendered = templates::ServiceWorkerTemplate { auth_enabled }
        .render()
        .unwrap_or_else(|err| {
            eprintln!("failed to render service worker: {err}");
            String::new()
        });
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/javascript")
        .header("cache-control", "no-cache")
        .body(rendered.into())
        .unwrap()
}

pub(crate) async fn icon_192(State(state): State<state::AppState>) -> axum::response::Response {
    let icon_192_bytes = load_icon_bytes(
        &state.config.root,
        state.config.icon_192.as_deref(),
        ICON_192_FALLBACK,
        "icon-192",
    );

    axum::response::Response::builder()
        .status(200)
        .header("content-type", "image/png")
        .header("cache-control", "public, max-age=86400")
        .body(icon_192_bytes.into())
        .unwrap()
}

pub(crate) async fn icon_512(State(state): State<state::AppState>) -> axum::response::Response {
    let icon_512_bytes = load_icon_bytes(
        &state.config.root,
        state.config.icon_512.as_deref(),
        ICON_512_FALLBACK,
        "icon-512",
    );

    axum::response::Response::builder()
        .status(200)
        .header("content-type", "image/png")
        .header("cache-control", "public, max-age=86400")
        .body(icon_512_bytes.into())
        .unwrap()
}

fn load_icon_bytes(root: &Path, path: Option<&Path>, fallback: &[u8], label: &str) -> Vec<u8> {
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

fn resolve_asset_path(root: &Path, path: &Path) -> std::io::Result<PathBuf> {
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
