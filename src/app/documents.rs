use crate::documents::{
    BlockKind, DocError, FileKind, MagentRegion, ReorderError, accept_magent_edit,
    add_task_item_in_list, collect_mentions, create_document, delete_file, find_magent_regions,
    insert_directive, line_count, lines_for_display, list_directory, load_document, move_file,
    normalize_newlines, remove_magent_interaction, render_document_html, render_magent_blocks,
    render_markdown_snippet, reorder_range, resolve_doc_path, scan_block_ranges, search_documents,
    toggle_task_item,
};
use crate::fs::atomic_write;
use crate::push as push_service;
use crate::state;
use crate::templates;

use axum::extract::Form;
use axum::extract::Path as AxumPath;
use axum::extract::Query;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;

use std::io::ErrorKind;

use super::push::refresh_push_state;

pub(crate) async fn directory_browse_root(
    State(state): State<state::AppState>,
) -> Result<templates::DirectoryBrowseTemplate, (StatusCode, &'static str)> {
    directory_browse(state, String::new())
}

pub(crate) async fn resolve_path(
    State(state): State<state::AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<Response, (StatusCode, &'static str)> {
    let ext = std::path::Path::new(&path)
        .extension()
        .and_then(|e| e.to_str());
    match ext.and_then(FileKind::from_extension) {
        Some(FileKind::Document) => document_view(state, path).map(IntoResponse::into_response),
        Some(FileKind::Pdf) => Ok(Redirect::to(&format!("/pdf/{path}")).into_response()),
        Some(FileKind::Image) => Ok(Redirect::to(&format!("/file/{path}")).into_response()),
        Some(FileKind::Text) => Ok(Redirect::to(&format!("/view/{path}")).into_response()),
        None => directory_browse(state, path).map(IntoResponse::into_response),
    }
}

fn directory_browse(
    state: state::AppState,
    current_dir: String,
) -> Result<templates::DirectoryBrowseTemplate, (StatusCode, &'static str)> {
    let git_enabled = state.git_dir.is_some();
    let listing = list_directory(&state.config.root, &current_dir).map_err(|err| match err {
        DocError::BadPath | DocError::Conflict => (StatusCode::BAD_REQUEST, "invalid path"),
        DocError::NotFound => (StatusCode::NOT_FOUND, "not found"),
        DocError::Io(err) => {
            eprintln!("failed to list directory: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let parent_url = if current_dir.is_empty() {
        None
    } else {
        match current_dir.rfind('/') {
            Some(pos) => Some(format!("/d/{}", &current_dir[..pos])),
            None => Some("/".to_string()),
        }
    };

    let path_prefix = if current_dir.is_empty() {
        String::new()
    } else {
        format!("{current_dir}/")
    };

    let breadcrumbs = build_breadcrumbs(&current_dir);

    let current_dir_name = current_dir
        .rsplit('/')
        .next()
        .unwrap_or(&current_dir)
        .to_string();

    let files = listing
        .files
        .into_iter()
        .map(|f| {
            let full_path = format!("{path_prefix}{}", f.name);
            let url = match f.kind {
                FileKind::Document => format!("/d/{full_path}"),
                FileKind::Pdf => format!("/pdf/{full_path}"),
                FileKind::Image => format!("/file/{full_path}"),
                FileKind::Text => format!("/view/{full_path}"),
            };
            templates::DirectoryFileEntry {
                name: f.name,
                kind: f.kind,
                url,
            }
        })
        .collect();

    Ok(templates::DirectoryBrowseTemplate {
        app_name: state.config.app_name,
        current_dir,
        current_dir_name,
        path_prefix,
        parent_url,
        breadcrumbs,
        directories: listing.directories,
        files,
        git_enabled,
    })
}

fn build_breadcrumbs(current_dir: &str) -> Vec<templates::BreadcrumbSegment> {
    let mut breadcrumbs = vec![templates::BreadcrumbSegment {
        name: "Documents".to_string(),
        url: "/".to_string(),
    }];

    if !current_dir.is_empty() {
        let segments: Vec<&str> = current_dir.split('/').collect();
        // All segments except the last become links
        for (i, segment) in segments.iter().enumerate() {
            if i < segments.len() - 1 {
                let path = segments[..=i].join("/");
                breadcrumbs.push(templates::BreadcrumbSegment {
                    name: segment.to_string(),
                    url: format!("/d/{path}"),
                });
            }
        }
    }

    breadcrumbs
}

/// Build breadcrumbs for a document page. All directory segments become links
/// (the document filename itself is the "current" item, rendered in the template).
fn build_breadcrumbs_for_doc(parent_dir: &str) -> Vec<templates::BreadcrumbSegment> {
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

#[derive(Debug, Deserialize)]
pub(crate) struct NewDocumentQuery {
    pub(crate) dir: Option<String>,
}

pub(crate) async fn document_new(
    State(state): State<state::AppState>,
    Query(query): Query<NewDocumentQuery>,
) -> templates::NewDocumentTemplate {
    let doc_id = match query.dir {
        Some(ref dir) if !dir.is_empty() => format!("{dir}/"),
        _ => String::new(),
    };
    templates::NewDocumentTemplate {
        app_name: state.config.app_name,
        doc_id,
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
        Err(DocError::BadPath) | Err(DocError::Conflict) => Err((
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

fn document_view(
    state: state::AppState,
    doc_id: String,
) -> Result<templates::DocumentTemplate, (StatusCode, &'static str)> {
    let git_enabled = state.git_dir.is_some();
    let contents = load_document(&state.config.root, &doc_id).map_err(|err| match err {
        DocError::NotFound => (StatusCode::NOT_FOUND, "not found"),
        _ => {
            eprintln!("failed to load document {doc_id}: {err:?}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let rendered = render_document_html(&contents, &doc_id);

    let doc_name = doc_id.rsplit('/').next().unwrap_or(&doc_id).to_string();

    let parent_dir = match doc_id.rfind('/') {
        Some(pos) => &doc_id[..pos],
        None => "",
    };
    let breadcrumbs = build_breadcrumbs_for_doc(parent_dir);

    Ok(templates::DocumentTemplate {
        app_name: state.config.app_name,
        doc_id,
        doc_name,
        breadcrumbs,
        content: rendered.html,
        has_mermaid: rendered.has_mermaid,
        has_abc: rendered.has_abc,
        has_code: rendered.has_code,
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

// ---------------------------------------------------------------------------
// Agent view
// ---------------------------------------------------------------------------

pub(crate) async fn document_agent_view(
    State(state): State<state::AppState>,
    AxumPath(doc_id): AxumPath<String>,
) -> Result<templates::AgentViewTemplate, (StatusCode, &'static str)> {
    let git_enabled = state.git_dir.is_some();
    let contents = load_document(&state.config.root, &doc_id).map_err(|err| match err {
        DocError::NotFound => (StatusCode::NOT_FOUND, "document not found"),
        _ => {
            eprintln!("failed to load document {doc_id}: {err:?}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let display_lines = lines_for_display(&contents);
    let total_lines = display_lines.len();
    let scan_blocks = scan_block_ranges(&contents);
    let magent_regions = find_magent_regions(&contents);
    let blocks = build_agent_blocks(&display_lines, &scan_blocks, &magent_regions, &doc_id);

    Ok(templates::AgentViewTemplate {
        app_name: state.config.app_name,
        doc_id,
        blocks,
        line_count: total_lines,
        git_enabled,
    })
}

fn build_agent_blocks(
    lines: &[String],
    scan_blocks: &[crate::documents::BlockRange],
    magent_regions: &[MagentRegion],
    doc_id: &str,
) -> Vec<templates::AgentBlock> {
    let mut result = Vec::new();
    let mut block_idx = 0;

    while block_idx < scan_blocks.len() {
        let block = &scan_blocks[block_idx];

        // Check if this block falls within a magent response region.
        if let Some(region) = magent_regions
            .iter()
            .find(|r| block.start >= r.start_line && block.start <= r.end_line)
        {
            // Render the full magent response block.
            let raw_text = lines[region.start_line..=region.end_line].join("\n") + "\n";
            let (html, _) = render_magent_blocks(&raw_text);
            result.push(templates::AgentBlock {
                html,
                start_line: region.start_line,
                end_line: region.end_line,
                is_magent: true,
                is_directive: false,
            });

            // Skip all scan blocks within this region.
            while block_idx < scan_blocks.len() && scan_blocks[block_idx].start <= region.end_line {
                block_idx += 1;
            }
        } else if block.kind == BlockKind::Blank {
            // Skip blank blocks — the agent view uses insert points for spacing.
            block_idx += 1;
        } else {
            // Regular content block.
            let text = lines[block.start..=block.end].join("\n");
            let is_directive =
                block.start == block.end && text.trim_start().starts_with("@magent ");
            let html = render_markdown_snippet(&text, doc_id);
            result.push(templates::AgentBlock {
                html,
                start_line: block.start,
                end_line: block.end,
                is_magent: false,
                is_directive,
            });
            block_idx += 1;
        }
    }

    result
}

pub(crate) async fn document_edit(
    State(state): State<state::AppState>,
    AxumPath(doc_id): AxumPath<String>,
) -> Result<templates::EditTemplate, (StatusCode, &'static str)> {
    let git_enabled = state.git_dir.is_some();
    let contents = load_document(&state.config.root, &doc_id).map_err(|err| match err {
        DocError::NotFound | DocError::BadPath | DocError::Conflict => {
            (StatusCode::NOT_FOUND, "not found")
        }
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
        DocError::NotFound | DocError::BadPath | DocError::Conflict => {
            (StatusCode::NOT_FOUND, "not found")
        }
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
        let registries_snapshot = state.registries.lock().expect("registries lock").clone();
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
        DocError::NotFound | DocError::BadPath | DocError::Conflict => {
            (StatusCode::NOT_FOUND, "not found")
        }
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
pub(crate) struct AcceptMagentEditForm {
    pub(crate) doc_id: String,
    pub(crate) edit_index: usize,
}

pub(crate) async fn document_accept_magent_edit(
    State(state): State<state::AppState>,
    Form(form): Form<AcceptMagentEditForm>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let doc_id = form.doc_id.trim();
    if doc_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "doc_id is required"));
    }

    let path = resolve_doc_path(&state.config.root, doc_id).map_err(|err| match err {
        DocError::NotFound | DocError::BadPath | DocError::Conflict => {
            (StatusCode::NOT_FOUND, "not found")
        }
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

    let updated = accept_magent_edit(&contents, form.edit_index)
        .ok_or((StatusCode::NOT_FOUND, "edit not found"))?;

    atomic_write(&path, &updated).map_err(|err| {
        eprintln!("failed to save document {doc_id}: {err}");
        (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

    if let Err(err) = refresh_push_state(&state) {
        eprintln!("failed to reload push registries after accept: {err}");
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub(crate) struct InsertMagentDirectiveForm {
    pub(crate) doc_id: String,
    pub(crate) after_line: usize,
    pub(crate) directive: String,
}

pub(crate) async fn document_insert_magent_directive(
    State(state): State<state::AppState>,
    Form(form): Form<InsertMagentDirectiveForm>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let doc_id = form.doc_id.trim();
    if doc_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "doc_id is required"));
    }

    let path = resolve_doc_path(&state.config.root, doc_id).map_err(|err| match err {
        DocError::NotFound | DocError::BadPath | DocError::Conflict => {
            (StatusCode::NOT_FOUND, "not found")
        }
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

    let updated = insert_directive(&contents, form.after_line, &form.directive)
        .ok_or((StatusCode::BAD_REQUEST, "invalid line or empty directive"))?;

    atomic_write(&path, &updated).map_err(|err| {
        eprintln!("failed to save document {doc_id}: {err}");
        (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

    if let Err(err) = refresh_push_state(&state) {
        eprintln!("failed to reload push registries after insert: {err}");
    }

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub(crate) struct RemoveMagentInteractionForm {
    pub(crate) doc_id: String,
    pub(crate) directive_line: usize,
}

pub(crate) async fn document_remove_magent_interaction(
    State(state): State<state::AppState>,
    Form(form): Form<RemoveMagentInteractionForm>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    let doc_id = form.doc_id.trim();
    if doc_id.is_empty() {
        return Err((StatusCode::BAD_REQUEST, "doc_id is required"));
    }

    let path = resolve_doc_path(&state.config.root, doc_id).map_err(|err| match err {
        DocError::NotFound | DocError::BadPath | DocError::Conflict => {
            (StatusCode::NOT_FOUND, "not found")
        }
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

    let updated = remove_magent_interaction(&contents, form.directive_line)
        .ok_or((StatusCode::NOT_FOUND, "interaction not found"))?;

    atomic_write(&path, &updated).map_err(|err| {
        eprintln!("failed to save document {doc_id}: {err}");
        (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

    if let Err(err) = refresh_push_state(&state) {
        eprintln!("failed to reload push registries after remove: {err}");
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
        DocError::NotFound | DocError::BadPath | DocError::Conflict => {
            (StatusCode::NOT_FOUND, "not found")
        }
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

    Ok(Redirect::to(&format!("/d/{doc_id}")))
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
        DocError::NotFound | DocError::BadPath | DocError::Conflict => {
            (StatusCode::NOT_FOUND, "not found")
        }
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

#[derive(Debug, Deserialize)]
pub(crate) struct MoveFileForm {
    pub(crate) source_path: String,
    pub(crate) target_dir: String,
}

pub(crate) async fn document_move_file(
    State(state): State<state::AppState>,
    Form(form): Form<MoveFileForm>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    move_file(&state.config.root, &form.source_path, &form.target_dir).map_err(
        |err| match err {
            DocError::BadPath => (StatusCode::BAD_REQUEST, "invalid path"),
            DocError::NotFound => (StatusCode::NOT_FOUND, "not found"),
            DocError::Conflict => (StatusCode::CONFLICT, "destination already exists"),
            DocError::Io(err) => {
                eprintln!(
                    "failed to move file {} to {}: {err}",
                    form.source_path, form.target_dir
                );
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
            }
        },
    )?;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize)]
pub(crate) struct DeleteFileForm {
    pub(crate) file_path: String,
}

pub(crate) async fn document_delete_file(
    State(state): State<state::AppState>,
    Form(form): Form<DeleteFileForm>,
) -> Result<StatusCode, (StatusCode, &'static str)> {
    delete_file(&state.config.root, &form.file_path).map_err(|err| match err {
        DocError::BadPath => (StatusCode::BAD_REQUEST, "invalid path"),
        DocError::NotFound => (StatusCode::NOT_FOUND, "not found"),
        DocError::Conflict => (StatusCode::BAD_REQUEST, "invalid path"),
        DocError::Io(err) => {
            eprintln!("failed to delete file {}: {err}", form.file_path);
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    Ok(StatusCode::NO_CONTENT)
}

pub(crate) async fn file_move_view_root(
    State(state): State<state::AppState>,
) -> Result<templates::FileMoveTemplate, (StatusCode, &'static str)> {
    file_move_view(state, String::new())
}

pub(crate) async fn file_move_view_path(
    State(state): State<state::AppState>,
    AxumPath(path): AxumPath<String>,
) -> Result<templates::FileMoveTemplate, (StatusCode, &'static str)> {
    file_move_view(state, path)
}

fn file_move_view(
    state: state::AppState,
    current_dir: String,
) -> Result<templates::FileMoveTemplate, (StatusCode, &'static str)> {
    let listing = list_directory(&state.config.root, &current_dir).map_err(|err| match err {
        DocError::BadPath | DocError::Conflict => (StatusCode::BAD_REQUEST, "invalid path"),
        DocError::NotFound => (StatusCode::NOT_FOUND, "not found"),
        DocError::Io(err) => {
            eprintln!("failed to list directory: {err}");
            (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
        }
    })?;

    let path_prefix = if current_dir.is_empty() {
        String::new()
    } else {
        format!("{current_dir}/")
    };

    let parent_dir = if current_dir.is_empty() {
        String::new()
    } else {
        match current_dir.rfind('/') {
            Some(pos) => current_dir[..pos].to_string(),
            None => String::new(),
        }
    };

    let breadcrumbs = build_breadcrumbs(&current_dir);

    let current_dir_name = current_dir
        .rsplit('/')
        .next()
        .unwrap_or(&current_dir)
        .to_string();

    let files = listing
        .files
        .into_iter()
        .map(|f| {
            let full_path = format!("{path_prefix}{}", f.name);
            let url = match f.kind {
                FileKind::Document => format!("/d/{full_path}"),
                FileKind::Pdf => format!("/pdf/{full_path}"),
                FileKind::Image => format!("/file/{full_path}"),
                FileKind::Text => format!("/view/{full_path}"),
            };
            templates::DirectoryFileEntry {
                name: f.name,
                kind: f.kind,
                url,
            }
        })
        .collect();

    Ok(templates::FileMoveTemplate {
        app_name: state.config.app_name,
        current_dir,
        current_dir_name,
        path_prefix,
        parent_dir,
        breadcrumbs,
        directories: listing.directories,
        files,
    })
}

#[derive(Debug, Deserialize)]
pub(crate) struct EditForm {
    pub(crate) contents: String,
}
