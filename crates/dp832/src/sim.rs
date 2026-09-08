//! An in-process DP832 that speaks the same SCPI subset as the real one.
//!
//! This exists so the server can be developed and tested on a machine with no
//! supply attached, and — more importantly — so the failure paths can be
//! exercised at all. Timeouts, disconnects and garbage responses are the states
//! the server's degraded-mode handling exists to cope with, and there is no way
//! to provoke them against real hardware in an automated test.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crate::error::TransportError;
use crate::transport::Transport;
use crate::types::{ChannelId, LockState, Protection, Setpoint};

/// A fault to inject into the simulated link.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Fault {
    /// Behave normally.
    #[default]
    None,
    /// Every command times out, as when a supply has locked up.
    Timeout,
    /// Every command reports the device node as gone, as when the USB cable is
    /// pulled.
    Disconnected,
    /// Answer queries with something unintelligible, exercising the protocol
    /// error path that must *not* take the supply offline.
    Garbage,
}

#[derive(Debug, Clone, Copy)]
struct SimChannel {
    setpoint: Setpoint,
    protection: Protection,
    output_on: bool,
    /// Resistive load across the terminals, or `None` for an open circuit.
    load_ohms: Option<f64>,
}

impl Default for SimChannel {
    fn default() -> Self {
        Self {
            setpoint: Setpoint {
                volt: 0.0,
                curr_limit: 1.0,
            },
            protection: Protection {
                ovp_level: 33.0,
                ovp_enabled: false,
                ocp_level: 3.3,
                ocp_enabled: false,
            },
            output_on: false,
            load_ohms: Some(10.0),
        }
    }
}

#[derive(Debug)]
struct SimState {
    serial: String,
    channels: BTreeMap<ChannelId, SimChannel>,
    lock: LockState,
    fault: Fault,
}

/// A simulated supply, usable anywhere a [`Transport`] is.
///
/// Cloning yields another handle to the *same* instrument, which is how a test
/// keeps a control handle after moving one into a [`crate::Dp832`].
#[derive(Debug, Clone)]
pub struct SimulatedDp832 {
    state: Arc<Mutex<SimState>>,
}

impl SimulatedDp832 {
    pub fn new(serial: impl Into<String>) -> Self {
        let mut channels = BTreeMap::new();
        for ch in ChannelId::ALL {
            channels.insert(ch, SimChannel::default());
        }
        Self {
            state: Arc::new(Mutex::new(SimState {
                serial: serial.into(),
                channels,
                lock: LockState::Unlocked,
                fault: Fault::None,
            })),
        }
    }

    /// Inject (or clear) a link fault.
    pub fn set_fault(&self, fault: Fault) {
        self.state.lock().unwrap().fault = fault;
    }

    /// Whether the front panel is currently locked out.
    ///
    /// Lets a test assert the lock lifecycle — in particular that the lock is
    /// released on the shutdown path.
    pub fn lock_state(&self) -> LockState {
        self.state.lock().unwrap().lock
    }

    /// Change the load on a channel, or set an open circuit with `None`.
    pub fn set_load(&self, ch: ChannelId, ohms: Option<f64>) {
        if let Some(c) = self.state.lock().unwrap().channels.get_mut(&ch) {
            c.load_ohms = ohms;
        }
    }

    /// Change a channel out from under the server, standing in for someone
    /// turning the physical knob. This is what the setpoint poll exists to
    /// notice.
    pub fn front_panel_set(&self, ch: ChannelId, sp: Setpoint, output_on: bool) {
        if let Some(c) = self.state.lock().unwrap().channels.get_mut(&ch) {
            c.setpoint = sp;
            c.output_on = output_on;
        }
    }

    pub fn output_state(&self, ch: ChannelId) -> bool {
        self.state
            .lock()
            .unwrap()
            .channels
            .get(&ch)
            .map(|c| c.output_on)
            .unwrap_or(false)
    }

    fn dispatch(&self, command: &str) -> Result<Option<String>, TransportError> {
        let mut state = self.state.lock().unwrap();

        match state.fault {
            Fault::None => {}
            Fault::Timeout => return Err(TransportError::Timeout(Duration::from_secs(5))),
            Fault::Disconnected => return Err(TransportError::DeviceGone),
            Fault::Garbage => {
                return if command.contains('?') {
                    Ok(Some("<<garbage>>".to_string()))
                } else {
                    Ok(None)
                }
            }
        }

        let (keyword, args) = split_command(command);
        let args: Vec<String> = args
            .split(',')
            .map(|a| a.trim().to_string())
            .filter(|a| !a.is_empty())
            .collect();

        // The channel, when the command takes one, is always the first argument.
        let channel = args.first().and_then(|a| a.parse::<ChannelId>().ok());
        let value = |idx: usize| -> Option<f64> { args.get(idx).and_then(|a| a.parse().ok()) };
        let flag = |idx: usize| -> Option<bool> {
            args.get(idx)
                .and_then(|a| match a.to_ascii_uppercase().as_str() {
                    "ON" | "1" => Some(true),
                    "OFF" | "0" => Some(false),
                    _ => None,
                })
        };

        let unknown = || {
            Err(TransportError::Protocol(format!(
                "simulator does not implement `{command}`"
            )))
        };

        if matches(&keyword, &["*IDN?"]) {
            let serial = state.serial.clone();
            return Ok(Some(format!("RIGOL TECHNOLOGIES,DP832,{serial},00.01.16")));
        }

        if matches(&keyword, &["SYSTEM:LOCK", "SYST:LOCK"]) {
            state.lock = match flag(0) {
                Some(true) => LockState::Locked,
                Some(false) => LockState::Unlocked,
                None => return unknown(),
            };
            return Ok(None);
        }

        let Some(ch) = channel else {
            return unknown();
        };
        let Some(c) = state.channels.get_mut(&ch) else {
            return unknown();
        };

        if matches(&keyword, &["MEASURE:ALL?", "MEAS:ALL?"]) {
            let (v, i) = terminal_readings(c);
            return Ok(Some(format!("{v:.4},{i:.4},{:.4}", v * i)));
        }

        if matches(&keyword, &["APPLY?", "APPL?"]) {
            let rating = match ch {
                ChannelId::Ch3 => "5V/3A",
                _ => "30V/3A",
            };
            return Ok(Some(format!(
                "{ch}:{rating},{:.4},{:.4}",
                c.setpoint.volt, c.setpoint.curr_limit
            )));
        }

        if matches(&keyword, &["APPLY", "APPL"]) {
            match (value(1), value(2)) {
                (Some(volt), Some(curr_limit)) => {
                    c.setpoint = Setpoint { volt, curr_limit };
                    return Ok(None);
                }
                _ => return unknown(),
            }
        }

        if matches(
            &keyword,
            &["OUTPUT:STATE?", "OUTP:STAT?", "OUTPUT?", "OUTP?"],
        ) {
            return Ok(Some(if c.output_on { "ON" } else { "OFF" }.to_string()));
        }

        if matches(&keyword, &["OUTPUT:STATE", "OUTP:STAT", "OUTPUT", "OUTP"]) {
            match flag(1) {
                Some(on) => {
                    c.output_on = on;
                    return Ok(None);
                }
                None => return unknown(),
            }
        }

        if matches(&keyword, &["OUTPUT:OVP:VALUE?", "OUTP:OVP:VAL?"]) {
            return Ok(Some(format!("{:.4}", c.protection.ovp_level)));
        }
        if matches(&keyword, &["OUTPUT:OVP:VALUE", "OUTP:OVP:VAL"]) {
            match value(1) {
                Some(v) => {
                    c.protection.ovp_level = v;
                    return Ok(None);
                }
                None => return unknown(),
            }
        }
        if matches(&keyword, &["OUTPUT:OVP:STATE?", "OUTP:OVP:STAT?"]) {
            return Ok(Some(
                if c.protection.ovp_enabled {
                    "ON"
                } else {
                    "OFF"
                }
                .to_string(),
            ));
        }
        if matches(&keyword, &["OUTPUT:OVP:STATE", "OUTP:OVP:STAT"]) {
            match flag(1) {
                Some(on) => {
                    c.protection.ovp_enabled = on;
                    return Ok(None);
                }
                None => return unknown(),
            }
        }

        if matches(&keyword, &["OUTPUT:OCP:VALUE?", "OUTP:OCP:VAL?"]) {
            return Ok(Some(format!("{:.4}", c.protection.ocp_level)));
        }
        if matches(&keyword, &["OUTPUT:OCP:VALUE", "OUTP:OCP:VAL"]) {
            match value(1) {
                Some(v) => {
                    c.protection.ocp_level = v;
                    return Ok(None);
                }
                None => return unknown(),
            }
        }
        if matches(&keyword, &["OUTPUT:OCP:STATE?", "OUTP:OCP:STAT?"]) {
            return Ok(Some(
                if c.protection.ocp_enabled {
                    "ON"
                } else {
                    "OFF"
                }
                .to_string(),
            ));
        }
        if matches(&keyword, &["OUTPUT:OCP:STATE", "OUTP:OCP:STAT"]) {
            match flag(1) {
                Some(on) => {
                    c.protection.ocp_enabled = on;
                    return Ok(None);
                }
                None => return unknown(),
            }
        }

        unknown()
    }
}

impl Transport for SimulatedDp832 {
    fn write(&mut self, command: &str) -> Result<(), TransportError> {
        self.dispatch(command).map(|_| ())
    }

    fn query(&mut self, command: &str) -> Result<String, TransportError> {
        match self.dispatch(command)? {
            Some(response) => Ok(response),
            None => Err(TransportError::Protocol(format!(
                "`{command}` is not a query"
            ))),
        }
    }

    fn describe(&self) -> String {
        format!("sim:{}", self.state.lock().unwrap().serial)
    }
}

/// What the terminals actually show, given the setpoint and the load.
///
/// Models the constant-voltage / constant-current crossover so that a test can
/// drive a channel into current limit and see the voltage sag, rather than
/// always reading back exactly what was set.
fn terminal_readings(c: &SimChannel) -> (f64, f64) {
    if !c.output_on {
        return (0.0, 0.0);
    }
    match c.load_ohms {
        None => (c.setpoint.volt, 0.0),
        Some(r) if r > 0.0 => {
            let unlimited = c.setpoint.volt / r;
            if unlimited > c.setpoint.curr_limit {
                (c.setpoint.curr_limit * r, c.setpoint.curr_limit)
            } else {
                (c.setpoint.volt, unlimited)
            }
        }
        Some(_) => (0.0, c.setpoint.curr_limit),
    }
}

fn split_command(command: &str) -> (String, String) {
    let trimmed = command.trim().trim_start_matches(':');
    match trimmed.split_once(' ') {
        Some((kw, args)) => (kw.to_ascii_uppercase(), args.to_string()),
        None => (trimmed.to_ascii_uppercase(), String::new()),
    }
}

fn matches(keyword: &str, alternatives: &[&str]) -> bool {
    alternatives.contains(&keyword)
}
