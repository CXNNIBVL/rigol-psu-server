//! HTTP and WebSocket routes.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post, put};
use axum::{Json, Router};
use dp832::{ChannelId, Protection, Setpoint};
use psu_core::{Supervisor, SystemState};
use serde::{Deserialize, Serialize};

use crate::error::ApiError;

#[derive(Clone)]
pub struct AppState {
    pub supervisor: Arc<Supervisor>,
    #[cfg(feature = "simulation")]
    pub simulation: Option<Arc<crate::simulation::SimRegistry>>,
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/state", get(get_state))
        .route("/devices", get(get_devices))
        .route(
            "/devices/{slot}/channels/{channel}/setpoint",
            put(put_setpoint),
        )
        .route(
            "/devices/{slot}/channels/{channel}/protection",
            put(put_protection),
        )
        .route(
            "/devices/{slot}/channels/{channel}/output",
            post(post_output),
        )
        .route("/stream", get(stream));

    #[cfg(feature = "simulation")]
    let api = api.merge(crate::simulation::routes());

    Router::new()
        .nest("/api", api)
        // The SPA is served from the same origin and port as the API, so there
        // is no CORS surface and nothing to configure per deployment.
        .merge(crate::assets::routes())
        .with_state(state)
}

async fn get_state(State(state): State<AppState>) -> Json<SystemState> {
    Json(state.supervisor.snapshot())
}

async fn get_devices(State(state): State<AppState>) -> Json<Vec<psu_core::DeviceView>> {
    Json(state.supervisor.snapshot().devices)
}

#[derive(Debug, Deserialize)]
pub struct SetpointBody {
    pub volt: f64,
    pub curr_limit: f64,
}

#[derive(Debug, Deserialize)]
pub struct ProtectionBody {
    pub ovp_level: f64,
    pub ovp_enabled: bool,
    pub ocp_level: f64,
    pub ocp_enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct OutputBody {
    pub on: bool,
}

#[derive(Debug, Serialize)]
pub struct Accepted {
    pub ok: bool,
}

async fn put_setpoint(
    State(state): State<AppState>,
    Path((slot, channel)): Path<(String, String)>,
    Json(body): Json<SetpointBody>,
) -> Result<Json<Accepted>, ApiError> {
    let channel = parse_channel(&channel)?;
    validate_finite(body.volt, "volt")?;
    validate_finite(body.curr_limit, "curr_limit")?;

    state
        .supervisor
        .set_setpoint(
            &slot,
            channel,
            Setpoint {
                volt: body.volt,
                curr_limit: body.curr_limit,
            },
        )
        .await
        .map_err(|e| ApiError::from_command(e).at(&slot, Some(channel.to_string())))?;

    Ok(Json(Accepted { ok: true }))
}

async fn put_protection(
    State(state): State<AppState>,
    Path((slot, channel)): Path<(String, String)>,
    Json(body): Json<ProtectionBody>,
) -> Result<Json<Accepted>, ApiError> {
    let channel = parse_channel(&channel)?;
    validate_finite(body.ovp_level, "ovp_level")?;
    validate_finite(body.ocp_level, "ocp_level")?;

    state
        .supervisor
        .set_protection(
            &slot,
            channel,
            Protection {
                ovp_level: body.ovp_level,
                ovp_enabled: body.ovp_enabled,
                ocp_level: body.ocp_level,
                ocp_enabled: body.ocp_enabled,
            },
        )
        .await
        .map_err(|e| ApiError::from_command(e).at(&slot, Some(channel.to_string())))?;

    Ok(Json(Accepted { ok: true }))
}

async fn post_output(
    State(state): State<AppState>,
    Path((slot, channel)): Path<(String, String)>,
    Json(body): Json<OutputBody>,
) -> Result<Json<Accepted>, ApiError> {
    let channel = parse_channel(&channel)?;

    state
        .supervisor
        .set_output(&slot, channel, body.on)
        .await
        .map_err(|e| ApiError::from_command(e).at(&slot, Some(channel.to_string())))?;

    Ok(Json(Accepted { ok: true }))
}

/// Full state snapshots, pushed on every publish tick.
///
/// Snapshots rather than deltas: nine channels is a couple of kilobytes, which
/// makes reconnection stateless and removes any possibility of a client drifting
/// out of sync with the server.
async fn stream(State(state): State<AppState>, ws: WebSocketUpgrade) -> Response {
    ws.on_upgrade(move |socket| push_snapshots(socket, state))
}

async fn push_snapshots(mut socket: WebSocket, state: AppState) {
    let mut updates = state.supervisor.subscribe();

    // Send immediately on connect so a new client is never staring at an empty
    // dashboard until the next tick.
    let initial = updates.borrow_and_update().clone();
    if send_snapshot(&mut socket, &initial).await.is_err() {
        return;
    }

    loop {
        tokio::select! {
            changed = updates.changed() => {
                if changed.is_err() {
                    return;
                }
                let snapshot = updates.borrow_and_update().clone();
                if send_snapshot(&mut socket, &snapshot).await.is_err() {
                    return;
                }
            }
            // Drain client frames so a close is noticed promptly rather than at
            // the next publish tick.
            incoming = socket.recv() => {
                match incoming {
                    None | Some(Err(_)) | Some(Ok(Message::Close(_))) => return,
                    _ => {}
                }
            }
        }
    }
}

async fn send_snapshot(socket: &mut WebSocket, state: &SystemState) -> Result<(), ()> {
    let payload = serde_json::to_string(state).map_err(|e| {
        tracing::error!(error = %e, "could not serialise state");
    })?;
    socket
        .send(Message::Text(payload.into()))
        .await
        .map_err(|_| ())
}

fn parse_channel(raw: &str) -> Result<ChannelId, ApiError> {
    raw.parse::<ChannelId>().map_err(|_| {
        ApiError::not_found(format!(
            "`{raw}` is not a channel; expected CH1, CH2 or CH3"
        ))
    })
}

/// Rejects NaN and infinity before they reach a clamp comparison, where every
/// comparison against NaN is false and the limit would silently not bind.
fn validate_finite(value: f64, field: &str) -> Result<(), ApiError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(ApiError::bad_request(
            "VALIDATION_FAILED",
            format!("`{field}` must be a finite number"),
        ))
    }
}

pub async fn fallback() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "code": "NOT_FOUND",
            "message": "no such endpoint"
        })),
    )
}
