//! One owner per supply.
//!
//! A USBTMC handle is a serialised resource: one command in flight, no
//! interleaving. That is enforced here by ownership — a single thread owns the
//! [`Dp832`] and everything else talks to it through a channel.
//!
//! The thread is a real OS thread rather than a tokio task because the transport
//! is blocking. Parking an async worker on a `read(2)` against a USB device
//! would stall unrelated work on the same runtime, and would do so only under
//! load, which is the worst way to discover it.
//!
//! Commands take priority over polling: a channel arriving on the queue is
//! serviced before the next scheduled read, so the UI stays responsive while
//! telemetry continues underneath.

use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use dp832::{ChannelId, Dp832, LockState, Protection, Setpoint, Transport, TransportError};
use tokio::sync::oneshot;

use crate::clamp::{Clamp, ClampViolation};
use crate::config::{PollConfig, SupplyConfig};
use crate::model::{ChannelView, DeviceStatus, DeviceView, OfflineCode, SystemState};

/// Why a command could not be carried out.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum CommandError {
    #[error("{0}")]
    ClampExceeded(ClampViolation),

    #[error("supply is offline: {message}")]
    DeviceOffline { code: OfflineCode, message: String },

    #[error("supply is quarantined: reported serial does not match the configured one")]
    DeviceQuarantined,

    #[error("channel is frozen after a clamp violation was found at startup: {0}")]
    ChannelFrozen(ClampViolation),

    #[error("no such supply")]
    UnknownSlot,

    #[error("supply is shutting down")]
    Shutdown,

    #[error("instrument error: {0}")]
    Transport(String),
}

impl CommandError {
    /// Stable identifier for the UI, so it can render something specific rather
    /// than parsing prose.
    pub fn code(&self) -> &'static str {
        match self {
            CommandError::ClampExceeded(_) => "CLAMP_EXCEEDED",
            CommandError::DeviceOffline { .. } => "DEVICE_OFFLINE",
            CommandError::DeviceQuarantined => "DEVICE_QUARANTINED",
            CommandError::ChannelFrozen(_) => "CHANNEL_FROZEN",
            CommandError::UnknownSlot => "UNKNOWN_SLOT",
            CommandError::Shutdown => "SHUTTING_DOWN",
            CommandError::Transport(_) => "INSTRUMENT_ERROR",
        }
    }
}

type Reply = oneshot::Sender<Result<(), CommandError>>;

enum Command {
    SetSetpoint {
        channel: ChannelId,
        setpoint: Setpoint,
        reply: Reply,
    },
    SetProtection {
        channel: ChannelId,
        protection: Protection,
        reply: Reply,
    },
    SetOutput {
        channel: ChannelId,
        on: bool,
        reply: Reply,
    },
    Shutdown {
        reply: oneshot::Sender<()>,
    },
}

/// A handle to a running supply thread.
#[derive(Clone)]
pub struct SupplyHandle {
    slot: String,
    tx: mpsc::Sender<Command>,
}

impl SupplyHandle {
    pub fn slot(&self) -> &str {
        &self.slot
    }

    pub async fn set_setpoint(
        &self,
        channel: ChannelId,
        setpoint: Setpoint,
    ) -> Result<(), CommandError> {
        self.dispatch(|reply| Command::SetSetpoint {
            channel,
            setpoint,
            reply,
        })
        .await
    }

    pub async fn set_protection(
        &self,
        channel: ChannelId,
        protection: Protection,
    ) -> Result<(), CommandError> {
        self.dispatch(|reply| Command::SetProtection {
            channel,
            protection,
            reply,
        })
        .await
    }

    pub async fn set_output(&self, channel: ChannelId, on: bool) -> Result<(), CommandError> {
        self.dispatch(|reply| Command::SetOutput { channel, on, reply })
            .await
    }

    /// Release the front panel and stop the thread. Outputs are deliberately
    /// left as they are — shutting the service down must not de-energise
    /// somebody's experiment.
    pub async fn shutdown(&self) {
        let (reply, rx) = oneshot::channel();
        if self.tx.send(Command::Shutdown { reply }).is_ok() {
            let _ = rx.await;
        }
    }

    async fn dispatch(&self, make: impl FnOnce(Reply) -> Command) -> Result<(), CommandError> {
        let (reply, rx) = oneshot::channel();
        self.tx
            .send(make(reply))
            .map_err(|_| CommandError::Shutdown)?;
        rx.await.map_err(|_| CommandError::Shutdown)?
    }
}

struct ChannelPolicy {
    id: ChannelId,
    clamp: Clamp,
}

/// What a command is trying to do, as far as the safety gates care.
///
/// The distinction that matters is whether the command would put energy into
/// something. Configuring and de-energising are always permitted on a reachable
/// supply; only [`Intent::Energise`] is gated on quarantine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Intent {
    /// Write setpoints or protection thresholds. Changes nothing physically on
    /// its own, because the output state is untouched.
    Configure,
    /// Switch an output on.
    Energise,
    /// Switch an output off. Never blocked.
    Deenergise,
}

/// The thread body. Owns the instrument; writes what it learns into the shared
/// [`SystemState`].
struct Actor<T: Transport> {
    slot: String,
    device: Dp832<T>,
    channels: Vec<ChannelPolicy>,
    state: Arc<RwLock<SystemState>>,
    poll: PollConfig,
    lock_idle_timeout: Duration,
    /// Lazily acquired: held only while we are actively driving this supply, so
    /// a crash leaves an idle bench usable from the front panel.
    lock_held: bool,
    last_command: Option<Instant>,
}

/// Start a supply thread. Returns a handle and the thread's join handle.
pub fn spawn<T: Transport + 'static>(
    config: &SupplyConfig,
    transport: T,
    state: Arc<RwLock<SystemState>>,
    poll: PollConfig,
    lock_idle_timeout: Duration,
) -> (SupplyHandle, std::thread::JoinHandle<()>) {
    let (tx, rx) = mpsc::channel();
    let slot = config.slot.clone();

    let channels = config
        .channels
        .iter()
        .filter_map(|c| {
            c.channel_id().map(|id| ChannelPolicy {
                id,
                clamp: c.clamp(),
            })
        })
        .collect();

    let mut actor = Actor {
        slot: slot.clone(),
        device: Dp832::new(transport),
        channels,
        state,
        poll,
        lock_idle_timeout,
        lock_held: false,
        last_command: None,
    };
    let expected_serial = config.serial.clone();

    let join = std::thread::Builder::new()
        .name(format!("psu-{slot}"))
        .spawn(move || actor.run(rx, expected_serial))
        .expect("failed to spawn supply thread");

    (SupplyHandle { slot, tx }, join)
}

impl<T: Transport> Actor<T> {
    fn run(&mut self, rx: mpsc::Receiver<Command>, expected_serial: String) {
        self.identify_and_adopt(&expected_serial);

        let mut next_measure = Instant::now();
        let mut next_setpoint = Instant::now();

        loop {
            let now = Instant::now();
            let deadline = next_measure.min(next_setpoint);
            let wait = deadline.saturating_duration_since(now);

            match rx.recv_timeout(wait) {
                // Commands are serviced ahead of polling.
                Ok(Command::Shutdown { reply }) => {
                    self.release_lock();
                    let _ = reply.send(());
                    return;
                }
                Ok(command) => self.handle(command),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    // Every handle is gone; nothing can command us again.
                    self.release_lock();
                    return;
                }
            }

            let now = Instant::now();
            if now >= next_measure {
                self.poll_measurements();
                next_measure = now + self.poll.measurement_interval();
            }
            if now >= next_setpoint {
                // Catches changes made at the front panel, which the server does
                // not otherwise hear about.
                self.poll_setpoints();
                next_setpoint = now + self.poll.setpoint_interval();
            }
            self.release_lock_if_idle();
        }
    }

    /// Startup: confirm identity, then adopt whatever the supply is already
    /// doing. Nothing is turned off here — a service restart must not end a
    /// running experiment.
    fn identify_and_adopt(&mut self, expected_serial: &str) {
        let identity = match self.device.identify() {
            Ok(id) => id,
            Err(e) => {
                self.mark_offline(&e);
                return;
            }
        };

        let quarantined = identity.serial != expected_serial;
        if quarantined {
            tracing::error!(
                slot = %self.slot,
                expected = %expected_serial,
                reported = %identity.serial,
                "serial mismatch: quarantining supply; outputs cannot be switched on \
                 until the configuration is corrected"
            );
        }

        self.with_device(|d| {
            d.reported_serial = Some(identity.serial.clone());
            d.status = if quarantined {
                DeviceStatus::Quarantined {
                    expected_serial: expected_serial.to_string(),
                    reported_serial: identity.serial.clone(),
                }
            } else {
                DeviceStatus::Online
            };
        });

        self.adopt_channels();
    }

    fn adopt_channels(&mut self) {
        for policy in self.channels_snapshot() {
            match self.device.channel_state(policy.id) {
                Ok(state) => {
                    // A channel found live outside its clamps is flagged and
                    // frozen, but left energised: the server must not decide on
                    // its own to cut power to something that may be mid-test.
                    let violation = if state.output_on {
                        policy.clamp.check_setpoint(state.setpoint).err()
                    } else {
                        None
                    };
                    if let Some(v) = violation {
                        tracing::warn!(
                            slot = %self.slot,
                            channel = %policy.id,
                            "adopted a live channel outside its configured limits ({}); \
                             leaving it energised and refusing writes",
                            v.message()
                        );
                    }
                    self.with_channel(policy.id, |c| {
                        c.setpoint = Some(state.setpoint);
                        c.protection = Some(state.protection);
                        c.output_on = Some(state.output_on);
                        c.violation = violation;
                    });
                }
                Err(e) => {
                    self.mark_offline(&e);
                    return;
                }
            }
        }
    }

    fn handle(&mut self, command: Command) {
        match command {
            Command::SetSetpoint {
                channel,
                setpoint,
                reply,
            } => {
                let result = self.guarded(channel, Intent::Configure, |actor, policy| {
                    policy
                        .clamp
                        .check_setpoint(setpoint)
                        .map_err(CommandError::ClampExceeded)?;
                    actor
                        .device
                        .set_setpoint(channel, setpoint)
                        .map_err(|e| actor.transport_failure(e))?;
                    actor.with_channel(channel, |c| c.setpoint = Some(setpoint));
                    Ok(())
                });
                let _ = reply.send(result);
            }
            Command::SetProtection {
                channel,
                protection,
                reply,
            } => {
                let result = self.guarded(channel, Intent::Configure, |actor, policy| {
                    policy
                        .clamp
                        .check_protection(protection)
                        .map_err(CommandError::ClampExceeded)?;
                    actor
                        .device
                        .set_protection(channel, protection)
                        .map_err(|e| actor.transport_failure(e))?;
                    actor.with_channel(channel, |c| c.protection = Some(protection));
                    Ok(())
                });
                let _ = reply.send(result);
            }
            Command::SetOutput { channel, on, reply } => {
                let intent = if on {
                    Intent::Energise
                } else {
                    // Removing power is never blocked, whatever state the supply
                    // is in.
                    Intent::Deenergise
                };
                let result = self.guarded(channel, intent, |actor, _policy| {
                    actor
                        .device
                        .set_output(channel, on)
                        .map_err(|e| actor.transport_failure(e))?;
                    actor.with_channel(channel, |c| c.output_on = Some(on));
                    Ok(())
                });
                let _ = reply.send(result);
            }
            Command::Shutdown { reply } => {
                self.release_lock();
                let _ = reply.send(());
            }
        }
    }

    /// Common preconditions for every write: the supply must be reachable, the
    /// channel must exist and must not be frozen, and — for anything that would
    /// energise a rail — the supply must not be quarantined. Acquires the front
    /// panel lock on the way through.
    fn guarded(
        &mut self,
        channel: ChannelId,
        intent: Intent,
        op: impl FnOnce(&mut Self, &ChannelPolicy) -> Result<(), CommandError>,
    ) -> Result<(), CommandError> {
        let status = self.status();
        match &status {
            DeviceStatus::Offline { code, message } => {
                return Err(CommandError::DeviceOffline {
                    code: *code,
                    message: message.clone(),
                })
            }
            // Quarantine is a narrow block: a distrusted supply may still be
            // read, configured and switched off. Only energising is forbidden,
            // because that is the operation whose consequences are physical.
            DeviceStatus::Quarantined { .. } if intent == Intent::Energise => {
                return Err(CommandError::DeviceQuarantined)
            }
            _ => {}
        }

        let policy = self
            .channels
            .iter()
            .find(|c| c.id == channel)
            .map(|c| ChannelPolicy {
                id: c.id,
                clamp: c.clamp,
            })
            .ok_or(CommandError::UnknownSlot)?;

        if let Some(violation) = self.channel_violation(channel) {
            return Err(CommandError::ChannelFrozen(violation));
        }

        self.acquire_lock();
        self.last_command = Some(Instant::now());
        op(self, &policy)
    }

    fn poll_measurements(&mut self) {
        if !self.status().is_reachable() {
            // Keep trying: a supply that comes back should be picked up without
            // operator intervention.
            if let Err(e) = self.device.identify() {
                self.mark_offline(&e);
                return;
            }
            self.mark_recovered();
        }

        for policy in self.channels_snapshot() {
            match self.device.measure(policy.id) {
                Ok(m) => self.with_channel(policy.id, |c| c.measurement = Some(m)),
                Err(e) => {
                    self.mark_offline(&e);
                    return;
                }
            }
        }
    }

    fn poll_setpoints(&mut self) {
        if !self.status().is_reachable() {
            return;
        }
        for policy in self.channels_snapshot() {
            match self.device.channel_state(policy.id) {
                Ok(state) => self.with_channel(policy.id, |c| {
                    c.setpoint = Some(state.setpoint);
                    c.protection = Some(state.protection);
                    c.output_on = Some(state.output_on);
                }),
                Err(e) => {
                    self.mark_offline(&e);
                    return;
                }
            }
        }
    }

    fn acquire_lock(&mut self) {
        if self.lock_held || !self.status().is_reachable() {
            return;
        }
        match self.device.set_lock(LockState::Locked) {
            Ok(()) => {
                self.lock_held = true;
                self.with_device(|d| d.front_panel_locked = true);
            }
            Err(e) => {
                // Not fatal: an unlockable panel is a usability problem, not a
                // safety one, and refusing the command would be worse.
                tracing::warn!(slot = %self.slot, error = %e, "could not lock the front panel");
            }
        }
    }

    fn release_lock(&mut self) {
        if !self.lock_held {
            return;
        }
        match self.device.set_lock(LockState::Unlocked) {
            Ok(()) => tracing::debug!(slot = %self.slot, "released the front panel"),
            Err(e) => tracing::warn!(
                slot = %self.slot,
                error = %e,
                "could not release the front panel; it may need to be cleared at the instrument"
            ),
        }
        self.lock_held = false;
        self.with_device(|d| d.front_panel_locked = false);
    }

    fn release_lock_if_idle(&mut self) {
        let idle = self
            .last_command
            .map(|t| t.elapsed() >= self.lock_idle_timeout)
            .unwrap_or(false);
        if self.lock_held && idle {
            self.release_lock();
        }
    }

    fn transport_failure(&mut self, e: TransportError) -> CommandError {
        if e.is_disconnect() {
            self.mark_offline(&e);
            let (code, message) = classify(&e);
            CommandError::DeviceOffline { code, message }
        } else {
            // A protocol error means we misunderstood a reply, not that the link
            // died. Taking the supply offline here would mask the real bug.
            CommandError::Transport(e.to_string())
        }
    }

    fn mark_offline(&mut self, e: &TransportError) {
        if !e.is_disconnect() {
            tracing::warn!(slot = %self.slot, error = %e, "instrument protocol error");
            return;
        }
        let (code, message) = classify(e);
        let already = matches!(self.status(), DeviceStatus::Offline { .. });
        if !already {
            tracing::warn!(slot = %self.slot, error = %e, "supply went offline");
        }
        // The lock is gone with the link; do not try to release it over a dead
        // connection.
        self.lock_held = false;
        self.with_device(|d| {
            d.status = DeviceStatus::Offline { code, message };
            d.front_panel_locked = false;
            // What we knew a moment ago is no longer evidence. A rail may still
            // be live; showing a stale zero would be a lie.
            for channel in &mut d.channels {
                channel.invalidate();
            }
        });
    }

    fn mark_recovered(&mut self) {
        tracing::info!(slot = %self.slot, "supply is answering again");
        self.with_device(|d| d.status = DeviceStatus::Online);
        self.adopt_channels();
    }

    fn channels_snapshot(&self) -> Vec<ChannelPolicy> {
        self.channels
            .iter()
            .map(|c| ChannelPolicy {
                id: c.id,
                clamp: c.clamp,
            })
            .collect()
    }

    fn status(&self) -> DeviceStatus {
        self.state
            .read()
            .unwrap()
            .device(&self.slot)
            .map(|d| d.status.clone())
            .unwrap_or(DeviceStatus::Offline {
                code: OfflineCode::NotYetContacted,
                message: "not yet contacted".into(),
            })
    }

    fn channel_violation(&self, channel: ChannelId) -> Option<ClampViolation> {
        self.state
            .read()
            .unwrap()
            .device(&self.slot)
            .and_then(|d| d.channel(channel))
            .and_then(|c| c.violation)
    }

    fn with_device(&self, f: impl FnOnce(&mut DeviceView)) {
        if let Some(device) = self.state.write().unwrap().device_mut(&self.slot) {
            f(device);
        }
    }

    fn with_channel(&self, channel: ChannelId, f: impl FnOnce(&mut ChannelView)) {
        if let Some(device) = self.state.write().unwrap().device_mut(&self.slot) {
            if let Some(c) = device.channel_mut(channel) {
                f(c);
            }
        }
    }
}

fn classify(e: &TransportError) -> (OfflineCode, String) {
    let code = match e {
        TransportError::Timeout(_) => OfflineCode::Timeout,
        TransportError::DeviceGone => OfflineCode::DeviceGone,
        _ => OfflineCode::IoError,
    };
    (code, e.to_string())
}
