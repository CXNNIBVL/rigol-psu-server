use crate::error::TransportError;
use crate::scpi;
use crate::transport::Transport;
use crate::types::{
    ChannelId, ChannelState, Identity, LockState, Measurement, Protection, Setpoint,
};

/// A DP832 addressed through some [`Transport`].
///
/// Every method is one or more synchronous round-trips. Nothing here retries,
/// caches, or decides policy: the server's per-supply task owns scheduling,
/// backoff and clamp enforcement, and this type stays a thin typed shell over
/// the wire protocol.
pub struct Dp832<T: Transport> {
    transport: T,
}

impl<T: Transport> Dp832<T> {
    pub fn new(transport: T) -> Self {
        Self { transport }
    }

    pub fn describe(&self) -> String {
        self.transport.describe()
    }

    pub fn into_transport(self) -> T {
        self.transport
    }

    pub fn identify(&mut self) -> Result<Identity, TransportError> {
        let response = self.transport.query(&scpi::identify())?;
        Ok(scpi::parse_identity(&response)?)
    }

    /// Voltage, current and power in a single round-trip, so the three values
    /// are coherent with one another.
    pub fn measure(&mut self, ch: ChannelId) -> Result<Measurement, TransportError> {
        let response = self.transport.query(&scpi::measure_all(ch))?;
        Ok(scpi::parse_measurement(ch, &response)?)
    }

    pub fn setpoint(&mut self, ch: ChannelId) -> Result<Setpoint, TransportError> {
        let response = self.transport.query(&scpi::query_setpoint(ch))?;
        Ok(scpi::parse_setpoint(ch, &response)?)
    }

    pub fn set_setpoint(&mut self, ch: ChannelId, sp: Setpoint) -> Result<(), TransportError> {
        self.transport.write(&scpi::set_setpoint(ch, sp))
    }

    pub fn output(&mut self, ch: ChannelId) -> Result<bool, TransportError> {
        let cmd = scpi::query_output(ch);
        let response = self.transport.query(&cmd)?;
        Ok(scpi::parse_bool(&cmd, &response)?)
    }

    pub fn set_output(&mut self, ch: ChannelId, on: bool) -> Result<(), TransportError> {
        self.transport.write(&scpi::set_output(ch, on))
    }

    /// Four round-trips: the instrument has no combined protection query.
    pub fn protection(&mut self, ch: ChannelId) -> Result<Protection, TransportError> {
        let ovp_level = self.query_scalar(scpi::query_ovp_level(ch))?;
        let ovp_enabled = self.query_bool(scpi::query_ovp_state(ch))?;
        let ocp_level = self.query_scalar(scpi::query_ocp_level(ch))?;
        let ocp_enabled = self.query_bool(scpi::query_ocp_state(ch))?;
        Ok(scpi::protection_from_parts(
            ovp_level,
            ovp_enabled,
            ocp_level,
            ocp_enabled,
        ))
    }

    /// Levels are written before the enables, so protection is never armed at a
    /// threshold the caller did not ask for — even momentarily.
    pub fn set_protection(&mut self, ch: ChannelId, p: Protection) -> Result<(), TransportError> {
        self.transport
            .write(&scpi::set_ovp_level(ch, p.ovp_level))?;
        self.transport
            .write(&scpi::set_ocp_level(ch, p.ocp_level))?;
        self.transport
            .write(&scpi::set_ovp_state(ch, p.ovp_enabled))?;
        self.transport
            .write(&scpi::set_ocp_state(ch, p.ocp_enabled))?;
        Ok(())
    }

    /// The full readback a setpoint poll needs.
    pub fn channel_state(&mut self, ch: ChannelId) -> Result<ChannelState, TransportError> {
        Ok(ChannelState {
            setpoint: self.setpoint(ch)?,
            protection: self.protection(ch)?,
            output_on: self.output(ch)?,
        })
    }

    /// Front panel lockout. Held only while the server is actively driving this
    /// supply, so a crash leaves an idle bench usable from the front panel.
    pub fn set_lock(&mut self, state: LockState) -> Result<(), TransportError> {
        self.transport.write(&scpi::set_lock(state))
    }

    fn query_scalar(&mut self, cmd: String) -> Result<f64, TransportError> {
        let response = self.transport.query(&cmd)?;
        Ok(scpi::parse_scalar(&cmd, &response)?)
    }

    fn query_bool(&mut self, cmd: String) -> Result<bool, TransportError> {
        let response = self.transport.query(&cmd)?;
        Ok(scpi::parse_bool(&cmd, &response)?)
    }
}
