//! The HTTP error surface.
//!
//! Every failure carries a stable machine-readable `code` alongside its prose.
//! The UI has to distinguish "you asked for more volts than the operator
//! permits" from "that supply is not answering" and say something specific about
//! each; making it pattern-match on English would be both fragile and useless
//! for anything but the happy path.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use psu_core::{ClampViolation, CommandError};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ApiError {
    pub code: &'static str,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub slot: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,
    /// Present on clamp failures, carrying the requested value and the limit so
    /// the UI can render both numbers without parsing the message.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub violation: Option<ClampViolation>,
    #[serde(skip)]
    pub status: StatusCode,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            slot: None,
            channel: None,
            violation: None,
            status,
        }
    }

    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "NOT_FOUND", message)
    }

    pub fn at(mut self, slot: &str, channel: Option<String>) -> Self {
        self.slot = Some(slot.to_string());
        self.channel = channel;
        self
    }

    /// Map a command failure onto HTTP.
    ///
    /// The status codes are chosen so that a client can act on them without
    /// reading the body: 409 for "the system is in a state that forbids this",
    /// 422 for "your request is out of bounds", 503 for "come back later".
    pub fn from_command(e: CommandError) -> Self {
        let message = e.to_string();
        match e {
            CommandError::ClampExceeded(violation) => {
                let mut err = Self::new(StatusCode::UNPROCESSABLE_ENTITY, e.code(), message);
                err.violation = Some(violation);
                err
            }
            CommandError::ChannelFrozen(violation) => {
                let mut err = Self::new(StatusCode::CONFLICT, "CHANNEL_FROZEN", message);
                err.violation = Some(violation);
                err
            }
            CommandError::DeviceQuarantined => {
                Self::new(StatusCode::CONFLICT, "DEVICE_QUARANTINED", message)
            }
            CommandError::DeviceOffline { .. } => {
                Self::new(StatusCode::SERVICE_UNAVAILABLE, "DEVICE_OFFLINE", message)
            }
            CommandError::UnknownSlot => Self::new(StatusCode::NOT_FOUND, "UNKNOWN_SLOT", message),
            CommandError::Shutdown => {
                Self::new(StatusCode::SERVICE_UNAVAILABLE, "SHUTTING_DOWN", message)
            }
            CommandError::Transport(_) => {
                Self::new(StatusCode::BAD_GATEWAY, "INSTRUMENT_ERROR", message)
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.status, Json(self)).into_response()
    }
}
