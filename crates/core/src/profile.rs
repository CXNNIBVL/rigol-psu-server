//! Applying a device profile to the bench.
//!
//! The governing rule is that **applying a profile never energises anything**.
//! An apply validates and stages setpoints; a separate, explicit power-on
//! commits. The stored `output_on` is intent — it tells the UI what to arm, not
//! the instrument what to do.
//!
//! The second rule is **all or nothing**. A profile describes one device's
//! rails, and half-powering a board can push current through an unpowered
//! rail's protection diodes. So every precondition is checked across every
//! binding before a single value is written, and a profile touching an
//! unreachable supply is refused outright rather than partially applied.

use std::time::Duration;

use crate::clamp::{Clamp, ClampViolation};
use crate::model::SystemState;
use crate::store::{Claim, DeviceProfile, ProfileBinding, Store, StoreError};
use crate::supervisor::Supervisor;
use crate::CommandError;

#[derive(Debug, thiserror::Error)]
pub enum ApplyError {
    #[error(transparent)]
    Store(#[from] StoreError),

    #[error("profile `{0}` has no channels bound; there is nothing to apply")]
    Empty(String),

    #[error("profile `{profile}` is bound to slot `{slot}`, which is not configured")]
    UnknownSlot { profile: String, slot: String },

    #[error(
        "{slot} is not answering, so `{profile}` cannot be applied in full; \
             a partially powered device is worse than an unpowered one"
    )]
    SupplyOffline { profile: String, slot: String },

    #[error(
        "{slot} is quarantined, so `{profile}` cannot be applied; \
             correct the configured serial and restart the service"
    )]
    SupplyQuarantined { profile: String, slot: String },

    #[error("{slot}/{channel} is frozen after a limit violation was found at startup")]
    ChannelFrozen { slot: String, channel: String },

    #[error("{slot}/{channel} is currently on; switch it off first, or re-send with force")]
    ChannelLive { slot: String, channel: String },

    #[error("{slot}/{channel} state is unknown, so it cannot be confirmed safe to apply")]
    ChannelUnknown { slot: String, channel: String },

    #[error("{slot}/{channel}: {violation}")]
    ClampExceeded {
        slot: String,
        channel: String,
        violation: ClampViolation,
    },

    #[error("{slot}/{channel}: {source}")]
    Command {
        slot: String,
        channel: String,
        #[source]
        source: CommandError,
    },
}

impl ApplyError {
    pub fn code(&self) -> &'static str {
        match self {
            ApplyError::Store(e) => e.code(),
            ApplyError::Empty(_) => "PROFILE_EMPTY",
            ApplyError::UnknownSlot { .. } => "UNKNOWN_SLOT",
            // Deliberately its own code rather than DEVICE_OFFLINE: the reason
            // this fails is the all-or-nothing rule, and the UI should say so.
            ApplyError::SupplyOffline { .. } => "PARTIAL_APPLY_REFUSED",
            ApplyError::SupplyQuarantined { .. } => "DEVICE_QUARANTINED",
            ApplyError::ChannelFrozen { .. } => "CHANNEL_FROZEN",
            ApplyError::ChannelLive { .. } => "CHANNEL_LIVE",
            ApplyError::ChannelUnknown { .. } => "CHANNEL_UNKNOWN",
            ApplyError::ClampExceeded { .. } => "CLAMP_EXCEEDED",
            ApplyError::Command { source, .. } => source.code(),
        }
    }

    /// Whether re-sending with `force` could succeed.
    ///
    /// True for exactly one case. `force` overrides the "channel must be off"
    /// rule and nothing else — never a clamp, never quarantine, never the
    /// all-or-nothing rule.
    pub fn is_forceable(&self) -> bool {
        matches!(self, ApplyError::ChannelLive { .. })
    }
}

/// Stage a profile's setpoints. Does not energise anything.
pub async fn apply(
    store: &Store,
    supervisor: &Supervisor,
    profile_id: i64,
    force: bool,
) -> Result<DeviceProfile, ApplyError> {
    let profile = store.profile(profile_id)?;
    preflight(&profile, &supervisor.snapshot(), force)?;

    for binding in &profile.bindings {
        // Protection first, so thresholds are in place before the voltage they
        // guard is written.
        supervisor
            .set_protection(&binding.slot, binding.channel, binding.preset.protection())
            .await
            .map_err(|source| command_error(binding, source))?;
        supervisor
            .set_setpoint(&binding.slot, binding.channel, binding.preset.setpoint())
            .await
            .map_err(|source| command_error(binding, source))?;
    }

    // The profile now owns these channels: it wrote what they hold. The claim is
    // checked against the instruments on every sweep and dropped as soon as that
    // stops being true, which is what makes it safe to report.
    claim_channels(store, &profile)?;

    tracing::info!(profile = %profile.name, force, "staged profile setpoints");
    Ok(profile)
}

fn claim_channels(store: &Store, profile: &DeviceProfile) -> Result<(), StoreError> {
    let channels: Vec<(String, dp832::ChannelId)> = profile
        .bindings
        .iter()
        .map(|b| (b.slot.clone(), b.channel))
        .collect();
    store.claim(profile.id, &channels)
}

/// Claims whose channel no longer holds what the claiming profile specifies.
///
/// A channel that cannot be read is left alone rather than released — a supply
/// briefly dropping off the bus is not evidence that somebody changed it, and
/// releasing on every blip would make ownership useless.
pub fn stale_claims(
    claims: &[Claim],
    profiles: &[DeviceProfile],
    state: &SystemState,
) -> Vec<Claim> {
    claims
        .iter()
        .filter(|claim| {
            let Some(profile) = profiles.iter().find(|p| p.id == claim.profile_id) else {
                return true;
            };
            let Some(binding) = profile
                .bindings
                .iter()
                .find(|b| b.slot == claim.slot && b.channel == claim.channel)
            else {
                // The profile no longer uses this channel.
                return true;
            };
            let Some(channel) = state
                .device(&claim.slot)
                .and_then(|d| d.channel(claim.channel))
            else {
                return true;
            };
            match channel.setpoint {
                None => false, // unreadable: no evidence either way
                Some(setpoint) => {
                    (setpoint.volt - binding.preset.volt).abs() > SETPOINT_EPSILON
                        || (setpoint.curr_limit - binding.preset.curr_limit).abs()
                            > SETPOINT_EPSILON
                }
            }
        })
        .cloned()
        .collect()
}

/// Setpoints are floats round-tripped through an instrument.
const SETPOINT_EPSILON: f64 = 1e-3;

/// Energise the rails a profile marks as on, in sequence.
pub async fn power_on(
    store: &Store,
    supervisor: &Supervisor,
    profile_id: i64,
) -> Result<DeviceProfile, ApplyError> {
    let profile = store.profile(profile_id)?;
    // Re-checked here rather than trusted from the apply: time has passed, and
    // a supply may have dropped in between.
    preflight(&profile, &supervisor.snapshot(), true)?;

    for binding in &profile.bindings {
        // The binding decides, not the preset: the same settings may be
        // auto-started by one profile and left staged by another.
        if !binding.output_on {
            continue;
        }
        supervisor
            .set_output(&binding.slot, binding.channel, true)
            .await
            .map_err(|source| command_error(binding, source))?;

        // The dwell belongs after the rail comes up, so a device whose supplies
        // must settle in order gets the gap where it was asked for.
        if binding.delay_ms > 0 {
            tokio::time::sleep(Duration::from_millis(binding.delay_ms as u64)).await;
        }
    }

    // Powering on without a prior apply is possible through the API, so the
    // claim is taken here too rather than assumed.
    claim_channels(store, &profile)?;

    tracing::info!(profile = %profile.name, "powered on profile");
    Ok(profile)
}

/// Switch a profile's rails off, wait, and bring them back up in sequence.
///
/// The action someone actually wants at a bench when a device under test has
/// wedged. Sequencing matters more here than anywhere else: a device that needs
/// its rails in order on the way up needs them in order every time, not only the
/// first.
pub async fn power_cycle(
    store: &Store,
    supervisor: &Supervisor,
    profile_id: i64,
    settle: Duration,
) -> Result<DeviceProfile, ApplyError> {
    power_off(store, supervisor, profile_id).await?;
    tokio::time::sleep(settle).await;
    power_on(store, supervisor, profile_id).await
}

/// Switch every rail of a profile off, in reverse sequence.
///
/// Best-effort by design: removing power is always permitted, so a supply that
/// refuses one channel must not prevent the rest from being switched off. Any
/// failures are reported after everything reachable has been dealt with.
pub async fn power_off(
    store: &Store,
    supervisor: &Supervisor,
    profile_id: i64,
) -> Result<DeviceProfile, ApplyError> {
    let profile = store.profile(profile_id)?;
    let mut first_error = None;

    for binding in profile.bindings.iter().rev() {
        if let Err(source) = supervisor
            .set_output(&binding.slot, binding.channel, false)
            .await
        {
            tracing::warn!(
                slot = %binding.slot,
                channel = %binding.channel,
                error = %source,
                "could not switch a rail off"
            );
            first_error.get_or_insert_with(|| command_error(binding, source));
        }
    }

    match first_error {
        Some(e) => Err(e),
        None => {
            tracing::info!(profile = %profile.name, "powered off profile");
            Ok(profile)
        }
    }
}

/// Every precondition, checked across every binding before anything is written.
fn preflight(profile: &DeviceProfile, state: &SystemState, force: bool) -> Result<(), ApplyError> {
    if profile.bindings.is_empty() {
        return Err(ApplyError::Empty(profile.name.clone()));
    }

    for binding in &profile.bindings {
        let channel_name = binding.channel.to_string();

        let device = state
            .device(&binding.slot)
            .ok_or_else(|| ApplyError::UnknownSlot {
                profile: profile.name.clone(),
                slot: binding.slot.clone(),
            })?;

        if !device.status.is_reachable() {
            return Err(ApplyError::SupplyOffline {
                profile: profile.name.clone(),
                slot: binding.slot.clone(),
            });
        }
        if device.status.is_quarantined() {
            return Err(ApplyError::SupplyQuarantined {
                profile: profile.name.clone(),
                slot: binding.slot.clone(),
            });
        }

        let channel = device
            .channel(binding.channel)
            .ok_or_else(|| ApplyError::UnknownSlot {
                profile: profile.name.clone(),
                slot: binding.slot.clone(),
            })?;

        if channel.violation.is_some() {
            return Err(ApplyError::ChannelFrozen {
                slot: binding.slot.clone(),
                channel: channel_name,
            });
        }

        match channel.output_on {
            Some(true) if !force => {
                return Err(ApplyError::ChannelLive {
                    slot: binding.slot.clone(),
                    channel: channel_name,
                })
            }
            // Not knowing whether a rail is live is not the same as knowing it
            // is off, and `force` does not cover ignorance.
            None => {
                return Err(ApplyError::ChannelUnknown {
                    slot: binding.slot.clone(),
                    channel: channel_name,
                })
            }
            _ => {}
        }

        let clamp = Clamp {
            max_volt: channel.limits.max_volt,
            max_curr: channel.limits.max_curr,
        };
        // Clamps bind regardless of `force`. A force flag that could raise an
        // operator limit would make the whole config-file arrangement pointless.
        let preset = &binding.preset;
        // The setpoint only: a trip point above the ceiling is the instrument's
        // backstop, not an output.
        clamp
            .check_setpoint(preset.setpoint())
            .map_err(|violation| ApplyError::ClampExceeded {
                slot: binding.slot.clone(),
                channel: channel_name.clone(),
                violation,
            })?;
    }

    Ok(())
}

fn command_error(binding: &ProfileBinding, source: CommandError) -> ApplyError {
    ApplyError::Command {
        slot: binding.slot.clone(),
        channel: binding.channel.to_string(),
        source,
    }
}
