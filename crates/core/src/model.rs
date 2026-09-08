//! The state the server publishes, and the vocabulary the UI renders.
//!
//! The governing rule here is that **absent knowledge is represented as absent,
//! never as zero**. A channel whose supply has stopped answering serialises its
//! readings as `null`, so a rail that may still be live cannot be mistaken for
//! one sitting safely at 0 V. Every `Option` in this module means "we do not
//! know", and the UI is obliged to render that distinctly.

use dp832::{ChannelId, Measurement, Protection, Setpoint};
use serde::Serialize;

use crate::clamp::{Clamp, ClampViolation};
use crate::store::Claim;

/// Why a supply is not answering. Machine-readable so the UI can say something
/// specific rather than printing a string it cannot interpret.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum OfflineCode {
    /// The supply did not answer in time.
    Timeout,
    /// The device node vanished — typically the USB cable was pulled.
    DeviceGone,
    /// Any other I/O failure against the device node.
    IoError,
    /// Not yet contacted since startup.
    NotYetContacted,
}

/// A supply's operational state.
///
/// `Quarantined` is deliberately distinct from `Offline`: the link is healthy
/// and readings are trustworthy, but the unit is not the one the operator
/// configured, so it must not be allowed to energise anything.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum DeviceStatus {
    Online,
    Offline {
        code: OfflineCode,
        message: String,
    },
    Quarantined {
        expected_serial: String,
        reported_serial: String,
    },
}

impl DeviceStatus {
    pub fn is_online(&self) -> bool {
        matches!(self, DeviceStatus::Online)
    }

    pub fn is_quarantined(&self) -> bool {
        matches!(self, DeviceStatus::Quarantined { .. })
    }

    /// Whether commands can reach the instrument at all. A quarantined supply
    /// is reachable — it is only forbidden from energising.
    pub fn is_reachable(&self) -> bool {
        !matches!(self, DeviceStatus::Offline { .. })
    }
}

/// The operator-configured limits, echoed so the UI can show what it is bounded
/// by without a second request.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Limits {
    pub max_volt: f64,
    pub max_curr: f64,
}

impl From<Clamp> for Limits {
    fn from(c: Clamp) -> Self {
        Self {
            max_volt: c.max_volt,
            max_curr: c.max_curr,
        }
    }
}

/// One channel, as published.
///
/// `measurement`, `setpoint`, `protection` and `output_on` are `None` when the
/// supply is not answering. That is the whole point: the UI must render
/// "Unknown", not a plausible-looking zero.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ChannelView {
    pub id: ChannelId,
    pub limits: Limits,
    pub measurement: Option<Measurement>,
    pub setpoint: Option<Setpoint>,
    pub protection: Option<Protection>,
    pub output_on: Option<bool>,
    /// Which profile wrote this channel's setpoints, if the channel still holds
    /// them. Corroborated on every setpoint poll rather than remembered, so it
    /// disappears the moment reality diverges.
    pub claimed_by: Option<Claim>,
    /// Set when a channel was found live at startup with setpoints outside the
    /// current clamps. The output is deliberately left energised — a restart
    /// must not end someone's experiment — but writes are refused until a human
    /// resolves it.
    pub violation: Option<ClampViolation>,
}

impl ChannelView {
    pub fn unknown(id: ChannelId, clamp: Clamp) -> Self {
        Self {
            id,
            limits: clamp.into(),
            measurement: None,
            setpoint: None,
            protection: None,
            output_on: None,
            claimed_by: None,
            violation: None,
        }
    }

    /// Whether writes to this channel are currently refused.
    pub fn is_frozen(&self) -> bool {
        self.violation.is_some()
    }

    /// Blank the readings while preserving identity, limits and any recorded
    /// violation. Used when a supply drops offline: what we knew a moment ago is
    /// no longer evidence of anything.
    pub fn invalidate(&mut self) {
        self.measurement = None;
        self.setpoint = None;
        self.protection = None;
        self.output_on = None;
    }
}

/// One supply, as published.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DeviceView {
    pub slot: String,
    pub label: String,
    pub device: String,
    pub expected_serial: String,
    pub reported_serial: Option<String>,
    #[serde(flatten)]
    pub status: DeviceStatus,
    pub front_panel_locked: bool,
    pub channels: Vec<ChannelView>,
}

impl DeviceView {
    pub fn channel(&self, id: ChannelId) -> Option<&ChannelView> {
        self.channels.iter().find(|c| c.id == id)
    }

    pub fn channel_mut(&mut self, id: ChannelId) -> Option<&mut ChannelView> {
        self.channels.iter_mut().find(|c| c.id == id)
    }
}

/// The complete published state.
///
/// Broadcast in full on every tick rather than as deltas: nine channels of JSON
/// is a couple of kilobytes, which buys stateless reconnection and removes an
/// entire class of client-desync bug.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SystemState {
    /// Increments on every published snapshot, so a client can tell a stale
    /// frame from a fresh one.
    pub generation: u64,
    pub devices: Vec<DeviceView>,
}

impl SystemState {
    pub fn device(&self, slot: &str) -> Option<&DeviceView> {
        self.devices.iter().find(|d| d.slot == slot)
    }

    pub fn device_mut(&mut self, slot: &str) -> Option<&mut DeviceView> {
        self.devices.iter_mut().find(|d| d.slot == slot)
    }
}
