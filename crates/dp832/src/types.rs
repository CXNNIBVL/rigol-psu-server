use std::fmt;
use std::str::FromStr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Serialize};
#[cfg(feature = "openapi")]
use utoipa::ToSchema;

/// One of the DP832's three output channels.
///
/// CH1 and CH2 are 30 V / 3 A; CH3 is 5 V / 3 A. The driver does not enforce
/// those ratings — per-channel limits are an operator concern and live in the
/// server's configuration, deliberately outside this crate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
#[cfg_attr(feature = "serde", serde(rename_all = "UPPERCASE"))]
pub enum ChannelId {
    Ch1,
    Ch2,
    Ch3,
}

impl ChannelId {
    pub const ALL: [ChannelId; 3] = [ChannelId::Ch1, ChannelId::Ch2, ChannelId::Ch3];

    /// The token the instrument expects, e.g. `CH1`.
    pub fn as_scpi(self) -> &'static str {
        match self {
            ChannelId::Ch1 => "CH1",
            ChannelId::Ch2 => "CH2",
            ChannelId::Ch3 => "CH3",
        }
    }
}

impl fmt::Display for ChannelId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_scpi())
    }
}

impl FromStr for ChannelId {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_uppercase().as_str() {
            "CH1" | "1" => Ok(ChannelId::Ch1),
            "CH2" | "2" => Ok(ChannelId::Ch2),
            "CH3" | "3" => Ok(ChannelId::Ch3),
            _ => Err(()),
        }
    }
}

/// What the instrument reports itself to be, from `*IDN?`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct Identity {
    pub vendor: String,
    pub model: String,
    /// Used to assert that a `/dev/rigol-psuN` symlink still points at the unit
    /// the operator configured. A mismatch quarantines the supply.
    pub serial: String,
    pub firmware: String,
}

/// An instantaneous reading from a channel's output terminal.
///
/// All three values come from a single `:MEASure:ALL?` round-trip, so they are
/// coherent with one another.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct Measurement {
    pub volt: f64,
    pub curr: f64,
    pub power: f64,
}

/// What a channel has been *told* to do, as distinct from what it is doing.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct Setpoint {
    pub volt: f64,
    pub curr_limit: f64,
}

/// Over-voltage and over-current protection, as configured on the instrument.
///
/// This is the supply's own protection, which is independent of — and no
/// substitute for — the server's operator-configured clamps.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct Protection {
    pub ovp_level: f64,
    pub ovp_enabled: bool,
    pub ocp_level: f64,
    pub ocp_enabled: bool,
}

/// The full readback of a channel: what it was told, and whether it is live.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize, Deserialize))]
#[cfg_attr(feature = "openapi", derive(ToSchema))]
pub struct ChannelState {
    pub setpoint: Setpoint,
    pub protection: Protection,
    pub output_on: bool,
}

/// Whether the instrument's front panel is locked out.
///
/// Acquired lazily on first command and released on graceful shutdown, so a
/// crashed server leaves an idle bench usable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LockState {
    Locked,
    Unlocked,
}

impl LockState {
    pub fn as_scpi(self) -> &'static str {
        match self {
            LockState::Locked => "ON",
            LockState::Unlocked => "OFF",
        }
    }
}
