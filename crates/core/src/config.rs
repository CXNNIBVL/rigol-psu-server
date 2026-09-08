//! Operator configuration, loaded from a root-owned TOML file.
//!
//! Validation is **fail-closed**: a config that is absent, unparseable, or
//! incomplete stops the server rather than starting it with weaker limits than
//! the operator intended. In particular every channel must be declared with
//! explicit limits — an undeclared channel is a channel with no clamp, and the
//! safe response to that is to refuse to run, not to invent a default.

use std::collections::BTreeSet;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::time::Duration;

use dp832::ChannelId;
use serde::Deserialize;

use crate::clamp::Clamp;

/// The DP832's own ratings. A clamp above these is a configuration mistake:
/// the operator has asked for a limit the hardware cannot reach, which means
/// the limit is not doing what they think it is.
const fn channel_rating(ch: ChannelId) -> (f64, f64) {
    match ch {
        ChannelId::Ch1 | ChannelId::Ch2 => (30.0, 3.0),
        ChannelId::Ch3 => (5.0, 3.0),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("could not read {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("could not parse {path}: {source}")]
    Parse {
        path: PathBuf,
        #[source]
        source: toml::de::Error,
    },

    #[error("invalid configuration: {0}")]
    Invalid(String),
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Where to listen. Deliberately has no default: exposing this service
    /// beyond localhost must be a deliberate act, not an accident of a default.
    pub bind: SocketAddr,

    #[serde(default)]
    pub poll: PollConfig,

    #[serde(default)]
    pub lock: LockConfig,

    #[serde(rename = "psu")]
    pub supplies: Vec<SupplyConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PollConfig {
    /// How often to read V/I/P. Drives how live the dashboard feels.
    pub measurement_hz: f64,
    /// How often to re-read setpoints, which is how out-of-band changes made at
    /// the front panel are detected.
    pub setpoint_hz: f64,
}

impl Default for PollConfig {
    fn default() -> Self {
        Self {
            measurement_hz: 2.0,
            setpoint_hz: 0.5,
        }
    }
}

impl PollConfig {
    pub fn measurement_interval(&self) -> Duration {
        Duration::from_secs_f64(1.0 / self.measurement_hz)
    }

    pub fn setpoint_interval(&self) -> Duration {
        Duration::from_secs_f64(1.0 / self.setpoint_hz)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LockConfig {
    /// How long a supply must go without a command before the front panel is
    /// released. Bounds how long a crash can leave a bench locked.
    pub idle_timeout_secs: u64,
}

impl Default for LockConfig {
    fn default() -> Self {
        Self {
            idle_timeout_secs: 60,
        }
    }
}

impl LockConfig {
    pub fn idle_timeout(&self) -> Duration {
        Duration::from_secs(self.idle_timeout_secs)
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SupplyConfig {
    /// Stable logical identity, e.g. `psu1`. This is what the database keys on,
    /// so that replacing a dead supply is a udev-rule edit rather than a data
    /// migration.
    pub slot: String,
    /// The udev-created symlink. Never `/dev/usbtmcN` directly: enumeration
    /// order shuffles, and three identical supplies would then be
    /// indistinguishable.
    pub device: PathBuf,
    pub label: String,
    /// Asserted against `*IDN?` at startup. A mismatch quarantines the supply.
    pub serial: String,
    #[serde(rename = "channel")]
    pub channels: Vec<ChannelConfig>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChannelConfig {
    pub id: String,
    pub max_volt: f64,
    pub max_curr: f64,
}

impl ChannelConfig {
    pub fn channel_id(&self) -> Option<ChannelId> {
        self.id.parse().ok()
    }

    pub fn clamp(&self) -> Clamp {
        Clamp {
            max_volt: self.max_volt,
            max_curr: self.max_curr,
        }
    }
}

impl Config {
    /// Load and validate. Any failure is fatal by design.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        Self::parse(&text, path)
    }

    pub fn parse(text: &str, path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let config: Config = toml::from_str(text).map_err(|source| ConfigError::Parse {
            path: path.as_ref().to_path_buf(),
            source,
        })?;
        config.validate()?;
        Ok(config)
    }

    pub fn supply(&self, slot: &str) -> Option<&SupplyConfig> {
        self.supplies.iter().find(|s| s.slot == slot)
    }

    fn validate(&self) -> Result<(), ConfigError> {
        let invalid = |msg: String| ConfigError::Invalid(msg);

        if self.supplies.is_empty() {
            return Err(invalid("no [[psu]] entries; nothing to control".into()));
        }
        if !(self.poll.measurement_hz.is_finite() && self.poll.measurement_hz > 0.0) {
            return Err(invalid(format!(
                "poll.measurement_hz must be a positive number, got {}",
                self.poll.measurement_hz
            )));
        }
        if !(self.poll.setpoint_hz.is_finite() && self.poll.setpoint_hz > 0.0) {
            return Err(invalid(format!(
                "poll.setpoint_hz must be a positive number, got {}",
                self.poll.setpoint_hz
            )));
        }

        let mut seen_slots = BTreeSet::new();
        let mut seen_devices = BTreeSet::new();

        for supply in &self.supplies {
            if supply.slot.is_empty() {
                return Err(invalid("a [[psu]] has an empty slot name".into()));
            }
            if !supply
                .slot
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
            {
                return Err(invalid(format!(
                    "slot `{}` must be alphanumeric, `-` or `_`; it appears in URLs and database keys",
                    supply.slot
                )));
            }
            if !seen_slots.insert(supply.slot.as_str()) {
                return Err(invalid(format!("duplicate slot `{}`", supply.slot)));
            }
            if !seen_devices.insert(supply.device.clone()) {
                return Err(invalid(format!(
                    "device {} is claimed by more than one [[psu]]",
                    supply.device.display()
                )));
            }
            if supply.serial.trim().is_empty() {
                return Err(invalid(format!(
                    "slot `{}` has no serial; the identity assertion would be vacuous",
                    supply.slot
                )));
            }

            let mut seen_channels = BTreeSet::new();
            for channel in &supply.channels {
                let Some(id) = channel.channel_id() else {
                    return Err(invalid(format!(
                        "slot `{}` declares unknown channel `{}`",
                        supply.slot, channel.id
                    )));
                };
                if !seen_channels.insert(id) {
                    return Err(invalid(format!(
                        "slot `{}` declares channel {id} more than once",
                        supply.slot
                    )));
                }

                let (rated_volt, rated_curr) = channel_rating(id);
                if !(channel.max_volt.is_finite() && channel.max_volt > 0.0) {
                    return Err(invalid(format!(
                        "{}/{id}: max_volt must be a positive number",
                        supply.slot
                    )));
                }
                if !(channel.max_curr.is_finite() && channel.max_curr > 0.0) {
                    return Err(invalid(format!(
                        "{}/{id}: max_curr must be a positive number",
                        supply.slot
                    )));
                }
                if channel.max_volt > rated_volt {
                    return Err(invalid(format!(
                        "{}/{id}: max_volt {} exceeds the channel's {rated_volt} V rating, so the limit would never bind",
                        supply.slot, channel.max_volt
                    )));
                }
                if channel.max_curr > rated_curr {
                    return Err(invalid(format!(
                        "{}/{id}: max_curr {} exceeds the channel's {rated_curr} A rating, so the limit would never bind",
                        supply.slot, channel.max_curr
                    )));
                }
            }

            // Every channel must be declared. An undeclared channel has no
            // clamp, and a channel with no clamp must not be controllable —
            // refusing to start is the only safe response.
            for id in ChannelId::ALL {
                if !seen_channels.contains(&id) {
                    return Err(invalid(format!(
                        "slot `{}` does not declare {id}; every channel needs explicit limits",
                        supply.slot
                    )));
                }
            }
        }

        Ok(())
    }
}
