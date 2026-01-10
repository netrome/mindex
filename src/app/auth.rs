use crate::state;
use crate::templates;
use argon2::{Argon2, PasswordHash, PasswordVerifier};
use axum::Json;
use axum::body::Body;
use axum::extract::Form;
use axum::extract::Query;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use axum::http::Request;
use axum::http::StatusCode;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::middleware::Next;
use axum::response::{IntoResponse, Redirect, Response};
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize)]
struct AuthErrorResponse {
    error: &'static str,
}

pub(crate) async fn auth_middleware(
    State(state): State<state::AppState>,
    req: Request<Body>,
    next: Next,
) -> Response {
    let auth = match &state.auth {
        Some(auth) => auth,
        None => return next.run(req).await,
    };

    let path = req.uri().path();
    if is_auth_bypass_path(path) {
        return next.run(req).await;
    }

    if let Some(token) = auth_cookie(req.headers(), auth.cookie_name())
        && auth.verify_token(token).is_ok()
    {
        return next.run(req).await;
    }

    if path.starts_with("/api/") {
        return (
            StatusCode::UNAUTHORIZED,
            Json(AuthErrorResponse {
                error: "unauthorized",
            }),
        )
            .into_response();
    }

    Redirect::to("/login").into_response()
}

fn is_auth_bypass_path(path: &str) -> bool {
    path == "/login"
        || path == "/logout"
        || path == "/sw.js"
        || path == "/health"
        || path.starts_with("/static/")
}

fn auth_cookie<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    for header in headers.get_all(COOKIE).iter() {
        if let Ok(raw) = header.to_str()
            && let Some(value) = cookie_from_header(raw, name)
        {
            return Some(value);
        }
    }
    None
}

fn cookie_from_header<'a>(header: &'a str, name: &str) -> Option<&'a str> {
    for part in header.split(';') {
        let trimmed = part.trim();
        if let Some((cookie_name, cookie_value)) = trimmed.split_once('=')
            && cookie_name == name
        {
            return Some(cookie_value);
        }
    }
    None
}

#[derive(Debug, Deserialize)]
pub(crate) struct LoginQuery {
    next: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct LoginForm {
    name: String,
    password: String,
    next: Option<String>,
}

pub(crate) async fn login_form(
    State(state): State<state::AppState>,
    Query(query): Query<LoginQuery>,
) -> Result<templates::LoginTemplate, (StatusCode, &'static str)> {
    if state.auth.is_none() {
        return Err((StatusCode::NOT_FOUND, "not found"));
    }
    let next = sanitize_next(query.next.as_deref()).unwrap_or_else(|| "/".to_string());

    Ok(templates::LoginTemplate {
        app_name: state.config.app_name,
        error: String::new(),
        next,
    })
}

pub(crate) async fn login_submit(
    State(state): State<state::AppState>,
    Form(form): Form<LoginForm>,
) -> Result<Response, (StatusCode, templates::LoginTemplate)> {
    let auth = state.auth.as_ref().ok_or((
        StatusCode::NOT_FOUND,
        templates::LoginTemplate {
            app_name: state.config.app_name.clone(),
            error: "Auth is not enabled.".to_string(),
            next: String::new(),
        },
    ))?;
    let name = form.name.trim();
    let password = form.password;
    let next = sanitize_next(form.next.as_deref()).unwrap_or_else(|| "/".to_string());

    if name.is_empty() || password.trim().is_empty() {
        return Err(login_error(&state.config.app_name, &next));
    }

    let password_hash = {
        let registries = state.push_registries.lock().expect("push registries lock");
        registries
            .users
            .get(name)
            .map(|user| user.password_hash.clone())
    };

    let Some(password_hash) = password_hash else {
        return Err(login_error(&state.config.app_name, &next));
    };

    if !verify_password(&password, &password_hash) {
        return Err(login_error(&state.config.app_name, &next));
    }

    let token = match auth.issue_token(name) {
        Ok(token) => token,
        Err(err) => {
            eprintln!("failed to issue auth token: {err}");
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                templates::LoginTemplate {
                    app_name: state.config.app_name,
                    error: "Failed to sign in.".to_string(),
                    next,
                },
            ));
        }
    };

    let mut response = Redirect::to(&next).into_response();
    let cookie = auth.auth_cookie(&token);
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("auth cookie header"),
    );
    Ok(response)
}

pub(crate) async fn logout(
    State(state): State<state::AppState>,
) -> Result<Response, (StatusCode, &'static str)> {
    let auth = state
        .auth
        .as_ref()
        .ok_or((StatusCode::NOT_FOUND, "not found"))?;
    let mut response = Redirect::to("/login").into_response();
    let cookie = auth.clear_cookie();
    response.headers_mut().append(
        SET_COOKIE,
        HeaderValue::from_str(&cookie).expect("logout cookie header"),
    );
    Ok(response)
}

fn verify_password(password: &str, password_hash: &str) -> bool {
    let hash = match PasswordHash::new(password_hash) {
        Ok(hash) => hash,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &hash)
        .is_ok()
}

fn sanitize_next(next: Option<&str>) -> Option<String> {
    let next = next?.trim();
    if next.is_empty() {
        return None;
    }
    if !next.starts_with('/') || next.starts_with("//") || next.contains("://") {
        return None;
    }
    Some(next.to_string())
}

fn login_error(app_name: &str, next: &str) -> (StatusCode, templates::LoginTemplate) {
    (
        StatusCode::UNAUTHORIZED,
        templates::LoginTemplate {
            app_name: app_name.to_string(),
            error: "Invalid username or password.".to_string(),
            next: next.to_string(),
        },
    )
}
