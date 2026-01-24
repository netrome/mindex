use crate::state;
use crate::templates;
use crate::uploads;

use axum::Json;
use axum::body::Bytes;
use axum::extract::Path as AxumPath;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum::response::Response;
use serde::Serialize;

#[derive(Serialize)]
pub(crate) struct UploadResponse {
    pub(crate) path: String,
    pub(crate) url: String,
    pub(crate) markdown: String,
}

#[derive(Serialize)]
pub(crate) struct UploadErrorResponse {
    pub(crate) error: &'static str,
}

pub(crate) async fn upload_form(
    State(state): State<state::AppState>,
) -> Result<templates::UploadTemplate, (StatusCode, &'static str)> {
    Ok(templates::UploadTemplate {
        app_name: state.config.app_name,
        git_enabled: state.git_dir.is_some(),
    })
}

pub(crate) async fn upload_file(
    State(state): State<state::AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, (StatusCode, &'static str)> {
    let Some(content_type) = uploads::content_type_for_path(&path) else {
        return Err((StatusCode::NOT_FOUND, "not found"));
    };

    let resolved = match uploads::resolve_file_path(&state.config.root, &path) {
        Ok(path) => path,
        Err(uploads::UploadError::NotFound) | Err(uploads::UploadError::BadPath) => {
            return Err((StatusCode::NOT_FOUND, "not found"));
        }
        Err(uploads::UploadError::Io(err)) => {
            eprintln!("failed to resolve file path {path}: {err}");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "internal error"));
        }
        Err(err) => {
            eprintln!("failed to resolve file path {path}: {err:?}");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "internal error"));
        }
    };

    let bytes = match std::fs::read(&resolved) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err((StatusCode::NOT_FOUND, "not found"));
        }
        Err(err) => {
            eprintln!("failed to read file {resolved:?}: {err}");
            return Err((StatusCode::INTERNAL_SERVER_ERROR, "internal error"));
        }
    };

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header("content-type", content_type)
        .header("cache-control", "public, max-age=86400")
        .body(bytes.into())
        .unwrap())
}

pub(crate) async fn upload_image(
    State(state): State<state::AppState>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Json<UploadResponse>, (StatusCode, Json<UploadErrorResponse>)> {
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok());
    let filename = headers
        .get("x-upload-filename")
        .and_then(|value| value.to_str().ok());

    let stored = match uploads::store_upload(&state.config.root, &body, content_type, filename) {
        Ok(stored) => stored,
        Err(uploads::UploadError::EmptyBody) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(UploadErrorResponse {
                    error: "upload body was empty",
                }),
            ));
        }
        Err(uploads::UploadError::UnsupportedType) => {
            return Err((
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(UploadErrorResponse {
                    error: "unsupported image type",
                }),
            ));
        }
        Err(uploads::UploadError::BadPath) => {
            return Err((
                StatusCode::BAD_REQUEST,
                Json(UploadErrorResponse {
                    error: "invalid upload path",
                }),
            ));
        }
        Err(uploads::UploadError::Io(err)) => {
            eprintln!("failed to store upload: {err}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadErrorResponse {
                    error: "failed to store upload",
                }),
            ));
        }
        Err(err) => {
            eprintln!("failed to store upload: {err:?}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(UploadErrorResponse {
                    error: "failed to store upload",
                }),
            ));
        }
    };

    let url = format!("/file/{}", stored.rel_path);
    let markdown = format!("![]({})", stored.rel_path);

    Ok(Json(UploadResponse {
        path: stored.rel_path,
        url,
        markdown,
    }))
}
