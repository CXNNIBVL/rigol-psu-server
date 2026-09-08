//! Pure SCPI: command construction and response parsing.
//!
//! Nothing here performs I/O. Every function is a total mapping from Rust values
//! to a command string, or from a response string to a Rust value, which means
//! the entire wire format is unit-testable without a supply — or a simulator.
//!
//! Commands use the long keyword forms from the DP800 programming guide. The
//! instrument accepts both, and the long forms read better at a review.

use crate::error::ScpiError;
use crate::types::{ChannelId, Identity, LockState, Measurement, Protection, Setpoint};

// ---------------------------------------------------------------------------
// Command construction
// ---------------------------------------------------------------------------

pub fn identify() -> String {
    "*IDN?".to_string()
}

pub fn measure_all(ch: ChannelId) -> String {
    format!(":MEASure:ALL? {ch}")
}

pub fn query_setpoint(ch: ChannelId) -> String {
    format!(":APPLy? {ch}")
}

/// Voltage and current limit are set together; the instrument has no command to
/// set one without restating the other.
pub fn set_setpoint(ch: ChannelId, sp: Setpoint) -> String {
    format!(":APPLy {ch},{:.3},{:.3}", sp.volt, sp.curr_limit)
}

pub fn query_output(ch: ChannelId) -> String {
    format!(":OUTPut:STATe? {ch}")
}

pub fn set_output(ch: ChannelId, on: bool) -> String {
    format!(":OUTPut:STATe {ch},{}", if on { "ON" } else { "OFF" })
}

pub fn query_ovp_level(ch: ChannelId) -> String {
    format!(":OUTPut:OVP:VALue? {ch}")
}

pub fn set_ovp_level(ch: ChannelId, volts: f64) -> String {
    format!(":OUTPut:OVP:VALue {ch},{volts:.3}")
}

pub fn query_ovp_state(ch: ChannelId) -> String {
    format!(":OUTPut:OVP:STATe? {ch}")
}

pub fn set_ovp_state(ch: ChannelId, on: bool) -> String {
    format!(":OUTPut:OVP:STATe {ch},{}", if on { "ON" } else { "OFF" })
}

pub fn query_ocp_level(ch: ChannelId) -> String {
    format!(":OUTPut:OCP:VALue? {ch}")
}

pub fn set_ocp_level(ch: ChannelId, amps: f64) -> String {
    format!(":OUTPut:OCP:VALue {ch},{amps:.3}")
}

pub fn query_ocp_state(ch: ChannelId) -> String {
    format!(":OUTPut:OCP:STATe? {ch}")
}

pub fn set_ocp_state(ch: ChannelId, on: bool) -> String {
    format!(":OUTPut:OCP:STATe {ch},{}", if on { "ON" } else { "OFF" })
}

/// Front panel lockout. `:SYSTem:LOCK` is the correct command for the DP800
/// series; `:SYSTem:KLOCk` and `:SYSTem:RWLock` are related but not equivalent.
pub fn set_lock(state: LockState) -> String {
    format!(":SYSTem:LOCK {}", state.as_scpi())
}

// ---------------------------------------------------------------------------
// Response parsing
// ---------------------------------------------------------------------------

/// `RIGOL TECHNOLOGIES,DP832,DP8A241800000,00.01.16`
pub fn parse_identity(response: &str) -> Result<Identity, ScpiError> {
    let fields: Vec<&str> = response.trim().split(',').map(str::trim).collect();
    if fields.len() < 4 {
        return Err(ScpiError::new(
            identify(),
            response,
            format!("expected 4 comma-separated fields, got {}", fields.len()),
        ));
    }
    Ok(Identity {
        vendor: fields[0].to_string(),
        model: fields[1].to_string(),
        serial: fields[2].to_string(),
        firmware: fields[3].to_string(),
    })
}

/// `3.000,0.500,1.500` — volts, amps, watts.
pub fn parse_measurement(ch: ChannelId, response: &str) -> Result<Measurement, ScpiError> {
    let cmd = measure_all(ch);
    let values = parse_floats(&cmd, response)?;
    if values.len() != 3 {
        return Err(ScpiError::new(
            cmd,
            response,
            format!("expected 3 values (V, I, P), got {}", values.len()),
        ));
    }
    Ok(Measurement {
        volt: values[0],
        curr: values[1],
        power: values[2],
    })
}

/// `CH1:30V/3A,5.000,1.000` — the leading field restates the channel's rating
/// and is ignored; the setpoint is the trailing pair.
pub fn parse_setpoint(ch: ChannelId, response: &str) -> Result<Setpoint, ScpiError> {
    let cmd = query_setpoint(ch);
    let fields: Vec<&str> = response.trim().split(',').map(str::trim).collect();
    if fields.len() < 2 {
        return Err(ScpiError::new(
            cmd,
            response,
            format!("expected at least 2 fields, got {}", fields.len()),
        ));
    }
    let tail = &fields[fields.len() - 2..];
    let volt = parse_float(&cmd, response, tail[0])?;
    let curr_limit = parse_float(&cmd, response, tail[1])?;
    Ok(Setpoint { volt, curr_limit })
}

/// Accepts the forms the DP800 uses interchangeably across firmware revisions.
pub fn parse_bool(command: &str, response: &str) -> Result<bool, ScpiError> {
    match response.trim().to_ascii_uppercase().as_str() {
        "ON" | "1" | "TRUE" => Ok(true),
        "OFF" | "0" | "FALSE" => Ok(false),
        _ => Err(ScpiError::new(command, response, "expected ON/OFF or 1/0")),
    }
}

pub fn parse_scalar(command: &str, response: &str) -> Result<f64, ScpiError> {
    parse_float(command, response, response.trim())
}

/// Assembles a [`Protection`] from the four separate queries the instrument
/// requires; there is no single command that returns all of it.
pub fn protection_from_parts(
    ovp_level: f64,
    ovp_enabled: bool,
    ocp_level: f64,
    ocp_enabled: bool,
) -> Protection {
    Protection {
        ovp_level,
        ovp_enabled,
        ocp_level,
        ocp_enabled,
    }
}

fn parse_floats(command: &str, response: &str) -> Result<Vec<f64>, ScpiError> {
    response
        .trim()
        .split(',')
        .map(|f| parse_float(command, response, f.trim()))
        .collect()
}

fn parse_float(command: &str, response: &str, field: &str) -> Result<f64, ScpiError> {
    field
        .parse::<f64>()
        .map_err(|_| ScpiError::new(command, response, format!("`{field}` is not a number")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_setpoint_command_with_fixed_precision() {
        let cmd = set_setpoint(
            ChannelId::Ch2,
            Setpoint {
                volt: 3.3,
                curr_limit: 0.5,
            },
        );
        assert_eq!(cmd, ":APPLy CH2,3.300,0.500");
    }

    #[test]
    fn parses_identity() {
        let id = parse_identity("RIGOL TECHNOLOGIES,DP832,DP8A241800000,00.01.16").unwrap();
        assert_eq!(id.model, "DP832");
        assert_eq!(id.serial, "DP8A241800000");
    }

    #[test]
    fn rejects_truncated_identity() {
        assert!(parse_identity("RIGOL TECHNOLOGIES,DP832").is_err());
    }

    #[test]
    fn parses_measurement() {
        let m = parse_measurement(ChannelId::Ch1, "3.000,0.500,1.500\n").unwrap();
        assert_eq!(m.volt, 3.0);
        assert_eq!(m.curr, 0.5);
        assert_eq!(m.power, 1.5);
    }

    #[test]
    fn rejects_measurement_with_wrong_field_count() {
        assert!(parse_measurement(ChannelId::Ch1, "3.000,0.500").is_err());
    }

    #[test]
    fn parses_setpoint_ignoring_the_rating_prefix() {
        let sp = parse_setpoint(ChannelId::Ch3, "CH3:5V/3A,3.300,1.000").unwrap();
        assert_eq!(sp.volt, 3.3);
        assert_eq!(sp.curr_limit, 1.0);
    }

    #[test]
    fn parses_setpoint_without_a_rating_prefix() {
        // Some firmware revisions omit the leading field.
        let sp = parse_setpoint(ChannelId::Ch1, "5.000,2.000").unwrap();
        assert_eq!(sp.volt, 5.0);
        assert_eq!(sp.curr_limit, 2.0);
    }

    #[test]
    fn parses_both_boolean_spellings() {
        assert!(parse_bool("x", "ON").unwrap());
        assert!(parse_bool("x", "1").unwrap());
        assert!(!parse_bool("x", "OFF").unwrap());
        assert!(!parse_bool("x", "0\n").unwrap());
        assert!(parse_bool("x", "MAYBE").is_err());
    }

    #[test]
    fn error_carries_the_command_and_the_offending_response() {
        let err = parse_measurement(ChannelId::Ch1, "banana").unwrap_err();
        assert!(err.to_string().contains("MEASure:ALL? CH1"));
        assert!(err.to_string().contains("banana"));
    }
}
