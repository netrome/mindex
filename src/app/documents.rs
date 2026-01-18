use crate::config;
use crate::documents::{
    BlockKind, DocError, ReorderError, add_task_item_in_list, atomic_write, collect_markdown_paths,
    collect_mentions, create_document, doc_id_from_path, line_count, lines_for_display,
    load_document, normalize_newlines, render_task_list_markdown, reorder_range, resolve_doc_path,
    rewrite_relative_md_links, scan_block_ranges, toggle_task_item,
};
use crate::math::{MathStyle, render_math};
use crate::push as push_service;
use crate::state;
use crate::templates;

use axum::extract::Form;
use axum::extract::Path as AxumPath;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Redirect;
use pulldown_cmark::Event;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use serde::Deserialize;

use std::io::ErrorKind;
use std::path::Path;

use super::push::refresh_push_state;

pub(crate) async fn document_list(
    State(state): State<state::AppState>,
) -> Result<templates::DocumentListTemplate, (StatusCode, &'static str)> {
    let state::AppState {
        config: config::AppConfig { root, app_name, .. },
        git_dir,
        ..
    } = state;
    let git_enabled = git_dir.is_some();
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
        git_enabled,
    })
}

pub(crate) async fn document_new(
    State(state): State<state::AppState>,
) -> templates::NewDocumentTemplate {
    templates::NewDocumentTemplate {
        app_name: state.config.app_name,
        doc_id: String::new(),
        error: String::new(),
        git_enabled: state.git_dir.is_some(),
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct NewDocumentForm {
    pub(crate) doc_id: String,
}

pub(crate) async fn document_create(
    State(state): State<state::AppState>,
    Form(form): Form<NewDocumentForm>,
) -> Result<Redirect, (StatusCode, templates::NewDocumentTemplate)> {
    let app_name = state.config.app_name.clone();
    let git_enabled = state.git_dir.is_some();
    let doc_id = form.doc_id.trim().to_string();
    if doc_id.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            templates::NewDocumentTemplate {
                app_name: app_name.clone(),
                doc_id,
                error: "Document path is required.".to_string(),
                git_enabled,
            },
        ));
    }

    let empty = String::new();
    match create_document(&state.config.root, &doc_id, &empty) {
        Ok(()) => Ok(Redirect::to(&format!("/edit/{doc_id}"))),
        Err(DocError::BadPath) => Err((
            StatusCode::BAD_REQUEST,
            templates::NewDocumentTemplate {
                app_name: app_name.clone(),
                doc_id,
                error: "Invalid path. Use a relative .md path.".to_string(),
                git_enabled,
            },
        )),
        Err(DocError::Io(err)) if err.kind() == ErrorKind::AlreadyExists => Err((
            StatusCode::CONFLICT,
            templates::NewDocumentTemplate {
                app_name: app_name.clone(),
                doc_id,
                error: "A document already exists at that path.".to_string(),
                git_enabled,
            },
        )),
        Err(DocError::Io(err)) => {
            eprintln!("failed to create document {doc_id}: {err}");
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                templates::NewDocumentTemplate {
                    app_name: app_name.clone(),
                    doc_id,
                    error: "Internal error.".to_string(),
                    git_enabled,
                },
            ))
        }
        Err(DocError::NotFound) => Err((
            StatusCode::BAD_REQUEST,
            templates::NewDocumentTemplate {
                app_name: app_name.clone(),
                doc_id,
                error: "Invalid path. Use a relative .md path.".to_string(),
                git_enabled,
            },
        )),
    }
}

pub(crate) async fn document_search(
    State(state): State<state::AppState>,
    Query(query): Query<SearchQuery>,
) -> Result<templates::SearchTemplate, (StatusCode, &'static str)> {
    let git_enabled = state.git_dir.is_some();
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
        git_enabled,
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
    let git_enabled = state.git_dir.is_some();
    let contents = load_document(&state.config.root, &doc_id).map_err(|err| match err {
        DocError::NotFound => (StatusCode::NOT_FOUND, "document not found"),
        _ => {
            eprintln!("failed to load document {doc_id}: {err:?}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let rendered = render_task_list_markdown(&contents, &doc_id);
    let mut body = String::new();
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_MATH);
    let parser = Parser::new_ext(&rendered, options).map(|event| {
        let event = rewrite_relative_md_links(event, &doc_id);
        match event {
            Event::InlineMath(latex) => {
                let html = render_math(&latex, MathStyle::Inline).into_html();
                Event::Html(html.into())
            }
            Event::DisplayMath(latex) => {
                let html = render_math(&latex, MathStyle::Display).into_html();
                Event::Html(html.into())
            }
            other => other,
        }
    });
    pulldown_cmark::html::push_html(&mut body, parser);

    Ok(templates::DocumentTemplate {
        app_name: state.config.app_name,
        doc_id,
        content: body,
        git_enabled,
    })
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReorderQuery {
    pub(crate) mode: Option<String>,
}

pub(crate) async fn document_reorder(
    State(state): State<state::AppState>,
    AxumPath(doc_id): AxumPath<String>,
    Query(query): Query<ReorderQuery>,
) -> Result<templates::ReorderTemplate, (StatusCode, &'static str)> {
    let git_enabled = state.git_dir.is_some();
    let contents = load_document(&state.config.root, &doc_id).map_err(|err| match err {
        DocError::NotFound => (StatusCode::NOT_FOUND, "document not found"),
        _ => {
            eprintln!("failed to load document {doc_id}: {err:?}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let lines = lines_for_display(&contents);
    let line_count = lines.len();
    let blocks = scan_block_ranges(&contents)
        .iter()
        .map(|block| {
            let text = if block.start <= block.end && block.end < lines.len() {
                lines[block.start..=block.end].join("\n")
            } else {
                String::new()
            };
            templates::ReorderBlock {
                start: block.start,
                end: block.end,
                kind: block_kind_label(block.kind),
                text,
                is_blank: block.kind == BlockKind::Blank,
            }
        })
        .collect();

    let mode = match query.mode.as_deref() {
        Some("line") => "line".to_string(),
        _ => "block".to_string(),
    };

    let lines = lines
        .into_iter()
        .enumerate()
        .map(|(index, text)| templates::ReorderLine { index, text })
        .collect();

    Ok(templates::ReorderTemplate {
        app_name: state.config.app_name,
        doc_id,
        lines,
        blocks,
        line_count,
        mode,
        git_enabled,
    })
}

fn block_kind_label(kind: BlockKind) -> String {
    match kind {
        BlockKind::Fence => "Fence",
        BlockKind::Table => "Table",
        BlockKind::ListItem => "List",
        BlockKind::Heading => "Heading",
        BlockKind::Paragraph => "Paragraph",
        BlockKind::Blank => "Blank",
    }
    .to_string()
}

pub(crate) async fn document_edit(
    State(state): State<state::AppState>,
    AxumPath(doc_id): AxumPath<String>,
) -> Result<templates::EditTemplate, (StatusCode, &'static str)> {
    let git_enabled = state.git_dir.is_some();
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
        git_enabled,
    })
}

pub(crate) async fn document_save(
    State(state): State<state::AppState>,
    AxumPath(doc_id): AxumPath<String>,
    Form(form): Form<EditForm>,
) -> Result<templates::EditTemplate, (StatusCode, &'static str)> {
    let git_enabled = state.git_dir.is_some();
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
    let mentions = collect_mentions(&normalized);
    atomic_write(&path, &normalized).map_err(|err| {
        eprintln!("failed to save document {doc_id}: {err}");
        (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

    if let Err(err) = refresh_push_state(&state) {
        eprintln!("failed to reload push registries after save: {err}");
    }
    if !mentions.is_empty() {
        let registries_snapshot = state
            .push_registries
            .lock()
            .expect("push registries lock")
            .clone();
        push_service::send_mentions(&state.config, &registries_snapshot, &doc_id, &mentions).await;
    }

    Ok(templates::EditTemplate {
        app_name: state.config.app_name,
        doc_id,
        contents: normalized,
        notice: "Saved.".to_string(),
        git_enabled,
    })
}

#[derive(Debug, Deserialize)]
pub(crate) struct ToggleTaskForm {
    pub(crate) doc_id: String,
    pub(crate) task_index: usize,
    pub(crate) checked: bool,
}

pub(crate) async fn document_toggle_task(
    State(state): State<state::AppState>,
    Form(form): Form<ToggleTaskForm>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let doc_id = form.doc_id.trim();
    if doc_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "doc_id is required"));
    }

    let path = resolve_doc_path(&state.config.root, doc_id).map_err(|err| match err {
        DocError::NotFound | DocError::BadPath => (StatusCode::NOT_FOUND, "not found"),
        DocError::Io(err) => {
            eprintln!("failed to resolve document {doc_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let contents = std::fs::read_to_string(&path).map_err(|err| match err.kind() {
        ErrorKind::NotFound | ErrorKind::IsADirectory => (StatusCode::NOT_FOUND, "not found"),
        _ => {
            eprintln!("failed to load document {doc_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let updated = toggle_task_item(&contents, form.task_index, form.checked)
        .ok_or((StatusCode::NOT_FOUND, "task not found"))?;

    atomic_write(&path, &updated).map_err(|err| {
        eprintln!("failed to save document {doc_id}: {err}");
        (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

    if let Err(err) = refresh_push_state(&state) {
        eprintln!("failed to reload push registries after toggle: {err}");
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddTaskForm {
    pub(crate) doc_id: String,
    pub(crate) list_index: usize,
    pub(crate) text: String,
}

pub(crate) async fn document_add_task(
    State(state): State<state::AppState>,
    Form(form): Form<AddTaskForm>,
) -> Result<Redirect, (StatusCode, &'static str)> {
    let doc_id = form.doc_id.trim();
    if doc_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "doc_id is required"));
    }
    let text = form.text.trim();
    if text.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "text is required"));
    }

    let path = resolve_doc_path(&state.config.root, doc_id).map_err(|err| match err {
        DocError::NotFound | DocError::BadPath => (StatusCode::NOT_FOUND, "not found"),
        DocError::Io(err) => {
            eprintln!("failed to resolve document {doc_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let contents = std::fs::read_to_string(&path).map_err(|err| match err.kind() {
        ErrorKind::NotFound | ErrorKind::IsADirectory => (StatusCode::NOT_FOUND, "not found"),
        _ => {
            eprintln!("failed to load document {doc_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let updated = add_task_item_in_list(&contents, form.list_index, text);
    atomic_write(&path, &updated).map_err(|err| {
        eprintln!("failed to save document {doc_id}: {err}");
        (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

    if let Err(err) = refresh_push_state(&state) {
        eprintln!("failed to reload push registries after add task: {err}");
    }

    Ok(Redirect::to(&format!("/doc/{doc_id}")))
}

#[derive(Debug, Deserialize)]
pub(crate) struct ReorderRangeForm {
    pub(crate) doc_id: String,
    pub(crate) start_line: usize,
    pub(crate) end_line: usize,
    pub(crate) insert_before_line: usize,
    pub(crate) mode: Option<String>,
}

pub(crate) async fn document_reorder_range(
    State(state): State<state::AppState>,
    Form(form): Form<ReorderRangeForm>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let doc_id = form.doc_id.trim();
    if doc_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "doc_id is required"));
    }

    let path = resolve_doc_path(&state.config.root, doc_id).map_err(|err| match err {
        DocError::NotFound | DocError::BadPath => (StatusCode::NOT_FOUND, "not found"),
        DocError::Io(err) => {
            eprintln!("failed to resolve document {doc_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let contents = std::fs::read_to_string(&path).map_err(|err| match err.kind() {
        ErrorKind::NotFound | ErrorKind::IsADirectory => (StatusCode::NOT_FOUND, "not found"),
        _ => {
            eprintln!("failed to load document {doc_id}: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let total_lines = line_count(&contents);
    if total_lines == 0 {
        return Err((StatusCode::BAD_REQUEST, "document is empty"));
    }
    if form.start_line > form.end_line
        || form.end_line >= total_lines
        || form.insert_before_line > total_lines
    {
        return Err((StatusCode::BAD_REQUEST, "invalid line range"));
    }

    if form.mode.as_deref() == Some("block") {
        let blocks = scan_block_ranges(&contents);
        let matches_block = blocks
            .iter()
            .any(|block| block.start == form.start_line && block.end == form.end_line);
        if !matches_block {
            return Err((StatusCode::CONFLICT, "range does not match a block"));
        }
        let matches_boundary = form.insert_before_line == total_lines
            || blocks
                .iter()
                .any(|block| block.start == form.insert_before_line);
        if !matches_boundary {
            return Err((StatusCode::CONFLICT, "insert point is not a block boundary"));
        }
    }

    let updated = reorder_range(
        &contents,
        form.start_line,
        form.end_line,
        form.insert_before_line,
    )
    .map_err(|err| match err {
        ReorderError::InvalidRange => (StatusCode::BAD_REQUEST, "invalid line range"),
    })?;

    let Some(updated) = updated else {
        return Ok(StatusCode::NO_CONTENT);
    };

    atomic_write(&path, &updated).map_err(|err| {
        eprintln!("failed to save document {doc_id}: {err}");
        (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

    if let Err(err) = refresh_push_state(&state) {
        eprintln!("failed to reload push registries after reorder: {err}");
    }

    Ok(StatusCode::NO_CONTENT)
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
