use crate::app::auth;
use crate::git;
use crate::state;
use crate::templates;

use axum::extract::Form;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::StatusCode;
use serde::Deserialize;

pub(crate) async fn git_view(
    State(state): State<state::AppState>,
) -> Result<templates::GitTemplate, (StatusCode, &'static str)> {
    git_template(&state, String::new(), String::new(), String::new())
}

#[derive(Debug, Deserialize)]
pub(crate) struct CommitForm {
    message: String,
}

pub(crate) async fn git_commit(
    State(state): State<state::AppState>,
    headers: HeaderMap,
    Form(form): Form<CommitForm>,
) -> Result<templates::GitTemplate, (StatusCode, &'static str)> {
    if state.git_dir.is_none() {
        return Err((StatusCode::NOT_FOUND, "not found"));
    }

    let trimmed = form.message.trim();
    if trimmed.is_empty() {
        return git_template(
            &state,
            form.message,
            "Commit message is required.".to_string(),
            String::new(),
        );
    }

    let author = match git_author_from_request(&state, &headers) {
        Ok(author) => author,
        Err(message) => {
            return git_template(&state, form.message, message, String::new());
        }
    };

    let commit = match git::git_commit_all(&state.config.root, trimmed, author) {
        Ok(commit) => commit,
        Err(err) => {
            return git_template(&state, form.message, err.to_string(), String::new());
        }
    };

    git_template(
        &state,
        String::new(),
        String::new(),
        format!("Committed {}.", commit.id),
    )
}

pub(crate) async fn git_push(
    State(state): State<state::AppState>,
) -> Result<templates::GitTemplate, (StatusCode, &'static str)> {
    let git_dir = match state.git_dir.as_ref() {
        Some(git_dir) => git_dir,
        None => return Err((StatusCode::NOT_FOUND, "not found")),
    };

    let notice = match git::git_push(
        &state.config.root,
        git_dir,
        &state.config.git_allowed_remote_roots,
    ) {
        Ok(message) => message,
        Err(err) => {
            return git_template(&state, String::new(), err.to_string(), String::new());
        }
    };

    git_template(&state, String::new(), String::new(), notice)
}

pub(crate) async fn git_pull(
    State(state): State<state::AppState>,
) -> Result<templates::GitTemplate, (StatusCode, &'static str)> {
    let git_dir = match state.git_dir.as_ref() {
        Some(git_dir) => git_dir,
        None => return Err((StatusCode::NOT_FOUND, "not found")),
    };

    let notice = match git::git_pull(
        &state.config.root,
        git_dir,
        &state.config.git_allowed_remote_roots,
    ) {
        Ok(message) => message,
        Err(err) => {
            return git_template(&state, String::new(), err.to_string(), String::new());
        }
    };

    git_template(&state, String::new(), String::new(), notice)
}

fn git_template(
    state: &state::AppState,
    message: String,
    error: String,
    notice: String,
) -> Result<templates::GitTemplate, (StatusCode, &'static str)> {
    if state.git_dir.is_none() {
        return Err((StatusCode::NOT_FOUND, "not found"));
    }

    let snapshot = git::git_status_and_diff(&state.config.root).map_err(|err| {
        eprintln!("failed to load git status: {err}");
        (StatusCode::INTERNAL_SERVER_ERROR, "internal error")
    })?;

    let status = if snapshot.changed_files == 0 {
        "Clean".to_string()
    } else {
        format!(
            "{} file{} changed",
            snapshot.changed_files,
            if snapshot.changed_files == 1 { "" } else { "s" }
        )
    };
    let diff = if snapshot.diff.trim().is_empty() {
        "No changes.\n".to_string()
    } else {
        snapshot.diff
    };

    Ok(templates::GitTemplate {
        app_name: state.config.app_name.clone(),
        status,
        diff,
        message,
        error,
        notice,
        git_enabled: state.git_dir.is_some(),
    })
}

fn git_author_from_request(
    state: &state::AppState,
    headers: &HeaderMap,
) -> Result<Option<git::GitAuthor>, String> {
    let auth_state = match state.auth.as_ref() {
        Some(auth_state) => auth_state,
        None => return Ok(None),
    };
    let token = auth::auth_cookie(headers, auth_state.cookie_name())
        .ok_or_else(|| "Authentication required.".to_string())?;
    let subject = auth_state
        .subject_from_token(token)
        .map_err(|_| "Authentication required.".to_string())?;
    let registries = state.push_registries.lock().expect("push registries lock");
    let user = registries
        .users
        .get(&subject)
        .ok_or_else(|| "Logged in user not found.".to_string())?;
    if user.email.trim().is_empty() {
        return Err("Logged in user is missing an email.".to_string());
    }
    let name = user
        .display_name
        .clone()
        .unwrap_or_else(|| user.name.clone());
    Ok(Some(git::GitAuthor {
        name,
        email: user.email.clone(),
    }))
}
