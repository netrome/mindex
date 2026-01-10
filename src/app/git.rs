use crate::git;
use crate::state;
use crate::templates;

use axum::extract::State;
use axum::http::StatusCode;

pub(crate) async fn git_view(
    State(state): State<state::AppState>,
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
        app_name: state.config.app_name,
        status,
        diff,
        git_enabled: state.git_dir.is_some(),
    })
}
