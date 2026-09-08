//! Domain model, configuration and supply supervision.
//!
//! This crate holds everything the server knows that is not HTTP: what a supply
//! is, what its operator-configured limits are, how its state is polled and
//! published, and which commands are permitted. It is generic over
//! [`dp832::Transport`], so the whole thing runs against simulated supplies with
//! no hardware present.

pub mod actor;
pub mod clamp;
pub mod config;
pub mod model;
pub mod profile;
pub mod store;
pub mod supervisor;

pub use actor::{CommandError, SupplyHandle};
pub use clamp::{Clamp, ClampViolation};
pub use config::{ChannelConfig, Config, ConfigError, PollConfig, SupplyConfig};
pub use model::{ChannelView, DeviceStatus, DeviceView, Limits, OfflineCode, SystemState};
pub use profile::{apply, power_off, power_on, ApplyError};
pub use store::{
    ChannelPreset, DeviceProfile, NewChannelPreset, NewDeviceProfile, NewProfileBinding,
    ProfileBinding, Store, StoreError,
};
pub use supervisor::Supervisor;
