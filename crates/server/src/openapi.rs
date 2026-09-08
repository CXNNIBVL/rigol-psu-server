//! The OpenAPI description of the HTTP API.
//!
//! Generated from the same Rust types the handlers return, so the document
//! cannot drift from the implementation: a field renamed in `psu-core` changes
//! the schema, and a response type that stops matching stops compiling.
//!
//! The schema derives live in `psu-core` and `dp832` behind an `openapi`
//! feature, rather than being mirrored here. Mirroring would mean two
//! definitions of every response and a standing invitation for them to
//! disagree; the feature flag keeps the domain crates free of any web
//! dependency for every consumer that does not want one.

use psu_core::{
    ChannelPreset, ChannelView, ClampViolation, DeviceProfile, DeviceStatus, DeviceView, Limits,
    NewChannelPreset, NewDeviceProfile, NewProfileBinding, OfflineCode, ProfileBinding,
    SystemState,
};
use utoipa::openapi::security::{ApiKey, ApiKeyValue, SecurityScheme};
use utoipa::{Modify, OpenApi};

use crate::error::ApiError;
use crate::routes::{Accepted, OutputBody, ProtectionBody, SetpointBody};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Rigol PSU bench",
        description = "Control and monitoring for USB-attached Rigol DP832 supplies.\n\n\
                       Per-channel limits are set by the operator in a root-owned config file \
                       and cannot be raised through this API. Applying a profile stages \
                       setpoints and never energises anything; `power-on` is the only call \
                       that puts energy into a device.",
        version = env!("CARGO_PKG_VERSION"),
    ),
    paths(
        crate::routes::get_state,
        crate::routes::get_devices,
        crate::routes::put_setpoint,
        crate::routes::put_protection,
        crate::routes::post_output,
        crate::profiles::list_presets,
        crate::profiles::get_preset,
        crate::profiles::create_preset,
        crate::profiles::update_preset,
        crate::profiles::delete_preset,
        crate::profiles::list_profiles,
        crate::profiles::get_profile,
        crate::profiles::create_profile,
        crate::profiles::update_profile,
        crate::profiles::delete_profile,
        crate::profiles::apply_profile,
        crate::profiles::power_on_profile,
        crate::profiles::power_off_profile,
    ),
    components(schemas(
        SystemState, DeviceView, DeviceStatus, ChannelView, Limits, OfflineCode,
        ClampViolation, ChannelPreset, NewChannelPreset, DeviceProfile, NewDeviceProfile,
        ProfileBinding, NewProfileBinding,
        SetpointBody, ProtectionBody, OutputBody, Accepted, ApiError,
    )),
    modifiers(&NoteOnAccess),
    tags(
        (name = "bench", description = "Live state and direct channel control"),
        (name = "presets", description = "Per-channel setting bundles"),
        (name = "profiles", description = "Named devices, and applying them"),
    )
)]
pub struct ApiDoc;

/// Records how access is actually controlled, so a reader of the document is
/// not left assuming there is authentication here.
struct NoteOnAccess;

impl Modify for NoteOnAccess {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "network",
                SecurityScheme::ApiKey(ApiKey::Header(ApiKeyValue::with_description(
                    "none",
                    "This API has no authentication. It is expected to be reachable only \
                     over the VPN, and the server requires an explicit --bind so that \
                     exposing it is always a deliberate act.",
                ))),
            );
        }
    }
}
