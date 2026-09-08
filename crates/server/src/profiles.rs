//! Preset and profile endpoints.
//!
//! The apply endpoints deliberately do not mirror one another in risk: `apply`
//! stages setpoints and cannot energise anything, `power-on` is the only route
//! that puts energy into a device, and `power-off` is always permitted. Keeping
//! them as three separate calls is what makes the destructive one a deliberate
//! act rather than a side effect of loading a profile.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use psu_core::{
    apply, power_off, power_on, ApplyError, ChannelPreset, DeviceProfile, NewChannelPreset,
    NewDeviceProfile, StoreError,
};
use serde::Deserialize;

use crate::error::ApiError;
use crate::routes::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/presets", get(list_presets).post(create_preset))
        .route(
            "/presets/{id}",
            get(get_preset).put(update_preset).delete(delete_preset),
        )
        .route("/profiles", get(list_profiles).post(create_profile))
        .route(
            "/profiles/{id}",
            get(get_profile).put(update_profile).delete(delete_profile),
        )
        .route("/profiles/{id}/apply", post(apply_profile))
        .route("/profiles/{id}/power-on", post(power_on_profile))
        .route("/profiles/{id}/power-off", post(power_off_profile))
        // Named so the router does not silently accept a bare DELETE on the
        // collection.
        .route("/presets/", delete(method_not_allowed))
}

async fn method_not_allowed() -> ApiError {
    ApiError::new(
        StatusCode::METHOD_NOT_ALLOWED,
        "METHOD_NOT_ALLOWED",
        "presets must be deleted individually",
    )
}

// -- presets ---------------------------------------------------------------

async fn list_presets(State(s): State<AppState>) -> Result<Json<Vec<ChannelPreset>>, ApiError> {
    Ok(Json(s.store.presets().map_err(store_error)?))
}

async fn get_preset(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<ChannelPreset>, ApiError> {
    Ok(Json(s.store.preset(id).map_err(store_error)?))
}

async fn create_preset(
    State(s): State<AppState>,
    Json(body): Json<NewChannelPreset>,
) -> Result<(StatusCode, Json<ChannelPreset>), ApiError> {
    let preset = s.store.create_preset(&body).map_err(store_error)?;
    Ok((StatusCode::CREATED, Json(preset)))
}

async fn update_preset(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<NewChannelPreset>,
) -> Result<Json<ChannelPreset>, ApiError> {
    Ok(Json(s.store.update_preset(id, &body).map_err(store_error)?))
}

async fn delete_preset(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    s.store.delete_preset(id).map_err(store_error)?;
    Ok(StatusCode::NO_CONTENT)
}

// -- profiles --------------------------------------------------------------

async fn list_profiles(State(s): State<AppState>) -> Result<Json<Vec<DeviceProfile>>, ApiError> {
    Ok(Json(s.store.profiles().map_err(store_error)?))
}

async fn get_profile(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DeviceProfile>, ApiError> {
    Ok(Json(s.store.profile(id).map_err(store_error)?))
}

async fn create_profile(
    State(s): State<AppState>,
    Json(body): Json<NewDeviceProfile>,
) -> Result<(StatusCode, Json<DeviceProfile>), ApiError> {
    let profile = s.store.create_profile(&body).map_err(store_error)?;
    Ok((StatusCode::CREATED, Json(profile)))
}

async fn update_profile(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Json(body): Json<NewDeviceProfile>,
) -> Result<Json<DeviceProfile>, ApiError> {
    Ok(Json(
        s.store.update_profile(id, &body).map_err(store_error)?,
    ))
}

async fn delete_profile(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    s.store.delete_profile(id).map_err(store_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Debug, Deserialize, Default)]
pub struct ApplyQuery {
    /// Overrides the "every target channel must be off" rule, and nothing else.
    #[serde(default)]
    pub force: bool,
}

/// Stages setpoints. Never energises.
async fn apply_profile(
    State(s): State<AppState>,
    Path(id): Path<i64>,
    Query(query): Query<ApplyQuery>,
) -> Result<Json<DeviceProfile>, ApiError> {
    let profile = apply(&s.store, &s.supervisor, id, query.force)
        .await
        .map_err(apply_error)?;
    Ok(Json(profile))
}

/// The one endpoint that puts energy into a device.
async fn power_on_profile(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DeviceProfile>, ApiError> {
    let profile = power_on(&s.store, &s.supervisor, id)
        .await
        .map_err(apply_error)?;
    Ok(Json(profile))
}

async fn power_off_profile(
    State(s): State<AppState>,
    Path(id): Path<i64>,
) -> Result<Json<DeviceProfile>, ApiError> {
    let profile = power_off(&s.store, &s.supervisor, id)
        .await
        .map_err(apply_error)?;
    Ok(Json(profile))
}

fn store_status(e: &StoreError) -> StatusCode {
    match e {
        StoreError::NotFound { .. } => StatusCode::NOT_FOUND,
        StoreError::DuplicateName { .. } | StoreError::PresetInUse { .. } => StatusCode::CONFLICT,
        StoreError::BadChannel(_) | StoreError::Invalid(_) => StatusCode::UNPROCESSABLE_ENTITY,
        StoreError::Sql(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn store_error(e: StoreError) -> ApiError {
    ApiError::new(store_status(&e), e.code(), e.to_string())
}

fn apply_error(e: ApplyError) -> ApiError {
    if let ApplyError::Store(inner) = &e {
        return ApiError::new(store_status(inner), inner.code(), inner.to_string());
    }

    let status = match &e {
        // Handled above; `StoreError` is not `Clone`, so it is classified by
        // reference rather than moved out.
        ApplyError::Store(_) => StatusCode::INTERNAL_SERVER_ERROR,
        // 409: the bench is in a state that forbids this, and the client can do
        // something about it.
        ApplyError::ChannelLive { .. }
        | ApplyError::ChannelFrozen { .. }
        | ApplyError::ChannelUnknown { .. }
        | ApplyError::SupplyQuarantined { .. } => StatusCode::CONFLICT,
        ApplyError::SupplyOffline { .. } => StatusCode::SERVICE_UNAVAILABLE,
        ApplyError::ClampExceeded { .. } | ApplyError::Empty(_) => StatusCode::UNPROCESSABLE_ENTITY,
        ApplyError::UnknownSlot { .. } => StatusCode::NOT_FOUND,
        ApplyError::Command { .. } => StatusCode::BAD_GATEWAY,
    };

    let mut error = ApiError::new(status, e.code(), e.to_string());
    // Tells the UI whether to offer a "force" affordance, rather than making it
    // guess from the code.
    error.forceable = e.is_forceable();
    if let ApplyError::ClampExceeded { violation, .. } = &e {
        error.violation = Some(*violation);
    }
    error
}
