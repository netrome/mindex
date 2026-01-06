use crate::state::AppState;

use axum::extract::State;

pub(crate) const ICON_192_FALLBACK: &[u8] = include_bytes!("../static/icons/icon-192.png");
pub(crate) const ICON_512_FALLBACK: &[u8] = include_bytes!("../static/icons/icon-512.png");

pub(crate) async fn manifest(State(state): State<AppState>) -> axum::response::Response {
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/manifest+json")
        .header("cache-control", "public, max-age=3600")
        .body(state.manifest.into())
        .unwrap()
}

pub(crate) async fn stylesheet() -> axum::response::Response {
    const CSS_CONTENT: &str = include_str!("../static/style.css");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "text/css")
        .header("cache-control", "public, max-age=3600")
        .body(CSS_CONTENT.into())
        .unwrap()
}

pub(crate) async fn theme_script() -> axum::response::Response {
    const THEME_JS_CONTENT: &str = include_str!("../static/theme.js");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/javascript")
        .header("cache-control", "public, max-age=3600")
        .body(THEME_JS_CONTENT.into())
        .unwrap()
}

pub(crate) async fn service_worker() -> axum::response::Response {
    const SW_CONTENT: &str = include_str!("../static/sw.js");
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "application/javascript")
        .header("cache-control", "no-cache")
        .body(SW_CONTENT.into())
        .unwrap()
}

pub(crate) async fn icon_192(State(state): State<AppState>) -> axum::response::Response {
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "image/png")
        .header("cache-control", "public, max-age=86400")
        .body(state.icon_192.into())
        .unwrap()
}

pub(crate) async fn icon_512(State(state): State<AppState>) -> axum::response::Response {
    axum::response::Response::builder()
        .status(200)
        .header("content-type", "image/png")
        .header("cache-control", "public, max-age=86400")
        .body(state.icon_512.into())
        .unwrap()
}
