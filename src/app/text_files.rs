use crate::documents::{
    DocError, highlight_lang_for_extension, load_text_file, resolve_text_file_path,
};
use crate::fs::atomic_write;
use crate::state;
use crate::templates;

use axum::extract::Form;
use axum::extract::Path as AxumPath;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;

use std::path::Path;

pub(crate) async fn text_view(
    State(state): State<state::AppState>,
    AxumPath(file_id): AxumPath<String>,
) -> Result<templates::TextViewTemplate, (StatusCode, &'static str)> {
    let git_enabled = state.git_dir.is_some();
    let contents = load_text_file(&state.config.root, &file_id).map_err(|err| match err {
        DocError::NotFound => (StatusCode::NOT_FOUND, "not found"),
        DocError::BadPath => (StatusCode::BAD_REQUEST, "invalid path"),
        DocError::Io(err) => {
            eprintln!("failed to load text file {file_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let file_name = file_id.rsplit('/').next().unwrap_or(&file_id).to_string();

    let ext = Path::new(&file_id)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    let highlight_lang = highlight_lang_for_extension(ext).to_string();

    let parent_dir = match file_id.rfind('/') {
        Some(pos) => &file_id[..pos],
        None => "",
    };
    let breadcrumbs = build_breadcrumbs_for_file(parent_dir);

    Ok(templates::TextViewTemplate {
        app_name: state.config.app_name,
        file_id,
        file_name,
        breadcrumbs,
        contents,
        highlight_lang,
        git_enabled,
    })
}

pub(crate) async fn text_edit(
    State(state): State<state::AppState>,
    AxumPath(file_id): AxumPath<String>,
) -> Result<templates::TextEditTemplate, (StatusCode, &'static str)> {
    let git_enabled = state.git_dir.is_some();
    let contents = load_text_file(&state.config.root, &file_id).map_err(|err| match err {
        DocError::NotFound | DocError::BadPath => (StatusCode::NOT_FOUND, "not found"),
        DocError::Io(err) => {
            eprintln!("failed to load text file {file_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    Ok(templates::TextEditTemplate {
        app_name: state.config.app_name,
        file_id,
        contents,
        notice: String::new(),
        git_enabled,
    })
}

#[derive(Debug, Deserialize)]
pub(crate) struct TextEditForm {
    pub(crate) contents: String,
}

pub(crate) async fn text_save(
    State(state): State<state::AppState>,
    AxumPath(file_id): AxumPath<String>,
    Form(form): Form<TextEditForm>,
) -> Result<templates::TextEditTemplate, (StatusCode, &'static str)> {
    let git_enabled = state.git_dir.is_some();
    let path = resolve_text_file_path(&state.config.root, &file_id).map_err(|err| match err {
        DocError::NotFound | DocError::BadPath => (StatusCode::NOT_FOUND, "not found"),
        DocError::Io(err) => {
            eprintln!("failed to resolve text file {file_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let metadata = std::fs::metadata(&path).map_err(|err| match err.kind() {
        std::io::ErrorKind::NotFound | std::io::ErrorKind::IsADirectory => {
            (StatusCode::NOT_FOUND, "not found")
        }
        _ => {
            eprintln!("failed to stat text file {file_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;
    if !metadata.is_file() {
        return Err((StatusCode::NOT_FOUND, "not found"));
    }

    atomic_write(&path, &form.contents).map_err(|err| {
        eprintln!("failed to save text file {file_id}: {err}");
        (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

    Ok(templates::TextEditTemplate {
        app_name: state.config.app_name,
        file_id,
        contents: form.contents,
        notice: "Saved.".to_string(),
        git_enabled,
    })
}

fn build_breadcrumbs_for_file(parent_dir: &str) -> Vec<templates::BreadcrumbSegment> {
    let mut breadcrumbs = vec![templates::BreadcrumbSegment {
        name: "Documents".to_string(),
        url: "/".to_string(),
    }];

    if !parent_dir.is_empty() {
        for (i, segment) in parent_dir.split('/').enumerate() {
            let path = parent_dir
                .split('/')
                .take(i + 1)
                .collect::<Vec<_>>()
                .join("/");
            breadcrumbs.push(templates::BreadcrumbSegment {
                name: segment.to_string(),
                url: format!("/d/{path}"),
            });
        }
    }

    breadcrumbs
}
