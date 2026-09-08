//! Driver for the Rigol DP832 programmable DC power supply.
//!
//! The crate is split so that everything except the actual byte-pushing is
//! testable without hardware:
//!
//! - [`scpi`] is pure: it builds command strings and parses responses, and knows
//!   nothing about I/O.
//! - [`Transport`] is the single I/O seam. [`UsbTmcTransport`] talks to a real
//!   supply through the kernel `usbtmc` driver; [`SimulatedDp832`] is an
//!   in-process fake speaking the same SCPI subset.
//! - [`Dp832`] wraps a transport in typed operations and is generic over it, so
//!   the same code drives real and simulated hardware.
//!
//! The simulator is deliberately a first-class citizen rather than a test
//! fixture: development happens on machines with no supply attached, and the
//! offline/timeout paths cannot be exercised any other way — you cannot unplug a
//! USB cable in CI.

mod device;
mod error;
pub mod scpi;
mod sim;
mod transport;
mod types;

#[cfg(unix)]
mod usbtmc;

pub use device::Dp832;
pub use error::{ScpiError, TransportError};
pub use sim::{Fault, SimulatedDp832};
pub use transport::Transport;
pub use types::{ChannelId, ChannelState, Identity, LockState, Measurement, Protection, Setpoint};

#[cfg(unix)]
pub use usbtmc::UsbTmcTransport;
