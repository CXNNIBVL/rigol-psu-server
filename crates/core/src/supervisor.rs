//! Owns every supply thread and publishes the aggregate state.
//!
//! Snapshots are published on a timer rather than on every change, so a burst of
//! commands cannot flood connected clients, and every client sees the same
//! coherent picture rather than a stream of partial updates.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};

use dp832::{ChannelId, Protection, Setpoint, Transport};
use tokio::sync::watch;

use crate::actor::{self, CommandError, SupplyHandle};
use crate::config::{Config, SupplyConfig};
use crate::model::{ChannelView, DeviceStatus, DeviceView, OfflineCode, SystemState};

pub struct Supervisor {
    handles: BTreeMap<String, SupplyHandle>,
    state: Arc<RwLock<SystemState>>,
    snapshots: watch::Sender<Arc<SystemState>>,
    /// Taken by whichever call to [`Supervisor::shutdown`] gets there first, so
    /// shutting down twice is harmless.
    threads: Mutex<Vec<std::thread::JoinHandle<()>>>,
}

impl Supervisor {
    /// Build the initial state and start one thread per configured supply.
    ///
    /// `open` yields a transport for a supply, or an error if it cannot be
    /// reached. A supply that cannot be opened does **not** stop startup: it is
    /// published as offline and retried, because refusing to boot over one
    /// switched-off supply takes the whole bench down.
    pub fn start<T, F, E>(config: &Config, mut open: F) -> Self
    where
        T: Transport + 'static,
        F: FnMut(&SupplyConfig) -> Result<T, E>,
        E: std::fmt::Display,
    {
        let devices = config.supplies.iter().map(initial_view).collect();
        let state = Arc::new(RwLock::new(SystemState {
            generation: 0,
            devices,
        }));

        let mut handles = BTreeMap::new();
        let mut threads = Vec::new();

        for supply in &config.supplies {
            match open(supply) {
                Ok(transport) => {
                    let (handle, join) = actor::spawn(
                        supply,
                        transport,
                        Arc::clone(&state),
                        config.poll.clone(),
                        config.lock.idle_timeout(),
                    );
                    handles.insert(supply.slot.clone(), handle);
                    threads.push(join);
                }
                Err(e) => {
                    tracing::warn!(
                        slot = %supply.slot,
                        device = %supply.device.display(),
                        error = %e,
                        "could not open supply; starting degraded"
                    );
                    if let Some(d) = state.write().unwrap().device_mut(&supply.slot) {
                        d.status = DeviceStatus::Offline {
                            code: OfflineCode::DeviceGone,
                            message: e.to_string(),
                        };
                    }
                }
            }
        }

        let snapshot = Arc::new(state.read().unwrap().clone());
        let (snapshots, _) = watch::channel(snapshot);

        Self {
            handles,
            state,
            snapshots,
            threads: Mutex::new(threads),
        }
    }

    /// A receiver of published snapshots, for WebSocket clients.
    pub fn subscribe(&self) -> watch::Receiver<Arc<SystemState>> {
        self.snapshots.subscribe()
    }

    /// The state as of right now, for one-shot HTTP reads.
    pub fn snapshot(&self) -> SystemState {
        self.state.read().unwrap().clone()
    }

    /// Stamp and publish the current state. Called on the publish timer.
    pub fn publish(&self) {
        let snapshot = {
            let mut state = self.state.write().unwrap();
            state.generation += 1;
            Arc::new(state.clone())
        };
        // A send error only means nobody is listening, which is normal.
        let _ = self.snapshots.send(snapshot);
    }

    pub fn slots(&self) -> impl Iterator<Item = &str> {
        self.handles.keys().map(String::as_str)
    }

    fn handle(&self, slot: &str) -> Result<&SupplyHandle, CommandError> {
        self.handles.get(slot).ok_or(CommandError::UnknownSlot)
    }

    pub async fn set_setpoint(
        &self,
        slot: &str,
        channel: ChannelId,
        setpoint: Setpoint,
    ) -> Result<(), CommandError> {
        self.handle(slot)?.set_setpoint(channel, setpoint).await
    }

    pub async fn set_protection(
        &self,
        slot: &str,
        channel: ChannelId,
        protection: Protection,
    ) -> Result<(), CommandError> {
        self.handle(slot)?.set_protection(channel, protection).await
    }

    pub async fn set_output(
        &self,
        slot: &str,
        channel: ChannelId,
        on: bool,
    ) -> Result<(), CommandError> {
        self.handle(slot)?.set_output(channel, on).await
    }

    /// Release every front panel and stop every supply thread.
    ///
    /// Outputs are left exactly as they are. Shutting the service down must not
    /// de-energise hardware someone is depending on.
    ///
    /// Takes `&self` rather than consuming, because the supervisor is shared
    /// behind an `Arc` with every request handler; requiring sole ownership to
    /// shut down would mean racing the last in-flight request for it. Calling
    /// this twice is harmless.
    pub async fn shutdown(&self) {
        for handle in self.handles.values() {
            handle.shutdown().await;
        }

        let threads = std::mem::take(&mut *self.threads.lock().unwrap());
        if threads.is_empty() {
            return;
        }
        // Joining blocks, so it must not happen on an async worker.
        let _ = tokio::task::spawn_blocking(move || {
            for thread in threads {
                let _ = thread.join();
            }
        })
        .await;
    }
}

fn initial_view(supply: &SupplyConfig) -> DeviceView {
    let channels = supply
        .channels
        .iter()
        .filter_map(|c| c.channel_id().map(|id| ChannelView::unknown(id, c.clamp())))
        .collect::<Vec<_>>();

    DeviceView {
        slot: supply.slot.clone(),
        label: supply.label.clone(),
        device: supply.device.display().to_string(),
        expected_serial: supply.serial.clone(),
        reported_serial: None,
        // Nothing is assumed until the supply has actually answered.
        status: DeviceStatus::Offline {
            code: OfflineCode::NotYetContacted,
            message: "not yet contacted".into(),
        },
        front_panel_locked: false,
        channels,
    }
}

/// Channels declared in config, in instrument order.
pub fn declared_channels(supply: &SupplyConfig) -> Vec<ChannelId> {
    let mut ids: Vec<ChannelId> = supply
        .channels
        .iter()
        .filter_map(|c| c.channel_id())
        .collect();
    ids.sort();
    ids
}
