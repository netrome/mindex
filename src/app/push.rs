use crate::adapters::WebPushSender;
use crate::ports::push::PushSender;
use crate::push as push_service;
use crate::state;
use crate::templates;
use crate::types::directives;

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use serde::Serialize;
use time::OffsetDateTime;

pub(crate) async fn push_registry_debug(
    State(state): State<state::AppState>,
) -> Json<directives::DirectiveRegistries> {
    let registries = state
        .push_registries
        .lock()
        .expect("push registries lock")
        .clone();
    Json(registries)
}

#[derive(Serialize, Deserialize)]
pub(crate) struct PushScheduleDebugResponse {
    pub(crate) server_time: OffsetDateTime,
    pub(crate) scheduled: Vec<PushScheduleEntry>,
}

#[derive(Serialize, Deserialize)]
pub(crate) struct PushScheduleEntry {
    pub(crate) doc_id: String,
    pub(crate) at: OffsetDateTime,
    pub(crate) message: String,
    pub(crate) to: Vec<String>,
    pub(crate) scheduled_at: OffsetDateTime,
    pub(crate) finished: bool,
}

pub(crate) async fn push_schedule_debug(
    State(state): State<state::AppState>,
) -> Json<PushScheduleDebugResponse> {
    let server_time = OffsetDateTime::now_utc();
    let scheduled = {
        let handles = state.push_handles.lock().expect("push handles lock");
        handles
            .iter()
            .map(|handle| PushScheduleEntry {
                doc_id: handle.notification.doc_id.clone(),
                at: handle.notification.at,
                message: handle.notification.message.clone(),
                to: handle.notification.to.clone(),
                scheduled_at: handle.scheduled_at,
                finished: handle.is_finished(),
            })
            .collect()
    };
    Json(PushScheduleDebugResponse {
        server_time,
        scheduled,
    })
}

#[derive(Serialize)]
pub(crate) struct PublicKeyResponse {
    #[serde(rename = "publicKey")]
    pub(crate) public_key: String,
}

#[derive(Serialize)]
pub(crate) struct ErrorResponse {
    pub(crate) error: &'static str,
}

pub(crate) async fn push_public_key(
    State(state): State<state::AppState>,
) -> Result<Json<PublicKeyResponse>, (StatusCode, Json<ErrorResponse>)> {
    let vapid = match push_service::load_vapid_config(&state.config) {
        push_service::VapidConfigStatus::Ready(vapid) => vapid,
        push_service::VapidConfigStatus::Incomplete | push_service::VapidConfigStatus::Missing => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Push notifications are not configured.",
                }),
            ));
        }
    };

    Ok(Json(PublicKeyResponse {
        public_key: vapid.public_key,
    }))
}

#[derive(Debug, Deserialize)]
pub(crate) struct TestPushRequest {
    pub(crate) endpoint: String,
    pub(crate) p256dh: String,
    pub(crate) auth: String,
    pub(crate) message: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct TestPushResponse {
    pub(crate) status: &'static str,
}

pub(crate) async fn push_test(
    State(state): State<state::AppState>,
    Json(request): Json<TestPushRequest>,
) -> Result<Json<TestPushResponse>, (StatusCode, Json<ErrorResponse>)> {
    let vapid = match push_service::load_vapid_config(&state.config) {
        push_service::VapidConfigStatus::Ready(vapid) => vapid,
        push_service::VapidConfigStatus::Incomplete | push_service::VapidConfigStatus::Missing => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                Json(ErrorResponse {
                    error: "Push notifications are not configured.",
                }),
            ));
        }
    };

    if request.endpoint.trim().is_empty()
        || request.p256dh.trim().is_empty()
        || request.auth.trim().is_empty()
    {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "endpoint, p256dh, and auth are required.",
            }),
        ));
    }

    let message = request
        .message
        .as_deref()
        .unwrap_or("Test notification from Mindex")
        .trim();
    if message.is_empty() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "message must not be empty.",
            }),
        ));
    }

    let sender = WebPushSender::new(vapid).map_err(|err| {
        eprintln!("push test error: failed to init web-push ({err})");
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Failed to initialize push sender.",
            }),
        )
    })?;

    let subscription = directives::Subscription {
        endpoint: request.endpoint,
        p256dh: request.p256dh,
        auth: request.auth,
    };

    if let Err(err) = sender.send(&subscription, message).await {
        eprintln!("push test error: {err}");
        return Err((
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: "Failed to send test notification.",
            }),
        ));
    }

    Ok(Json(TestPushResponse { status: "sent" }))
}

pub(crate) async fn push_subscribe(
    State(state): State<state::AppState>,
) -> templates::PushSubscribeTemplate {
    templates::PushSubscribeTemplate {
        app_name: state.config.app_name,
        git_enabled: state.git_dir.is_some(),
    }
}

pub(crate) fn refresh_push_state(state: &state::AppState) -> std::io::Result<()> {
    let registries = directives::DirectiveRegistries::load(&state.config.root)?;
    {
        let mut guard = state.push_registries.lock().expect("push registries lock");
        *guard = registries.clone();
    }
    push_service::restart_scheduler(
        &state.config,
        std::sync::Arc::new(registries),
        std::sync::Arc::clone(&state.push_handles),
    );
    Ok(())
}
