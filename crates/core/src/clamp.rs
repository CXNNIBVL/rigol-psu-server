//! Operator-configured per-channel limits.
//!
//! These are deliberately *not* the instrument's own OVP/OCP. The supply's
//! protection is a last-resort hardware trip; these clamps are a policy gate
//! that a request must pass before it ever reaches the wire. They live in a
//! root-owned config file rather than the database precisely so that nothing
//! reachable from the API can raise them.

use dp832::{Protection, Setpoint};
use serde::Serialize;

/// The maximum this channel may ever be asked to produce.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Clamp {
    pub max_volt: f64,
    pub max_curr: f64,
}

/// Which quantity broke the limit, and by how much.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case", tag = "quantity")]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum ClampViolation {
    Voltage { requested: f64, limit: f64 },
    Current { requested: f64, limit: f64 },
}

impl std::fmt::Display for ClampViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message())
    }
}

impl ClampViolation {
    pub fn message(&self) -> String {
        match self {
            ClampViolation::Voltage { requested, limit } => {
                format!("requested {requested:.3} V exceeds the configured limit of {limit:.3} V")
            }
            ClampViolation::Current { requested, limit } => {
                format!("requested {requested:.3} A exceeds the configured limit of {limit:.3} A")
            }
        }
    }
}

impl Clamp {
    /// Check a setpoint before it is written.
    ///
    /// Voltage is checked first so that a request breaching both limits reports
    /// the voltage — it is the one that destroys hardware.
    pub fn check_setpoint(&self, sp: Setpoint) -> Result<(), ClampViolation> {
        if sp.volt > self.max_volt {
            return Err(ClampViolation::Voltage {
                requested: sp.volt,
                limit: self.max_volt,
            });
        }
        if sp.curr_limit > self.max_curr {
            return Err(ClampViolation::Current {
                requested: sp.curr_limit,
                limit: self.max_curr,
            });
        }
        Ok(())
    }

    /// Protection thresholds are clamped too.
    ///
    /// An OVP trip point above the clamp is not dangerous by itself, but it
    /// means the operator's intent and the instrument's configuration disagree,
    /// and silently allowing it makes the clamp value misleading to anyone
    /// reading the UI.
    pub fn check_protection(&self, p: Protection) -> Result<(), ClampViolation> {
        if p.ovp_enabled && p.ovp_level > self.max_volt {
            return Err(ClampViolation::Voltage {
                requested: p.ovp_level,
                limit: self.max_volt,
            });
        }
        if p.ocp_enabled && p.ocp_level > self.max_curr {
            return Err(ClampViolation::Current {
                requested: p.ocp_level,
                limit: self.max_curr,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CLAMP: Clamp = Clamp {
        max_volt: 5.5,
        max_curr: 2.0,
    };

    fn sp(volt: f64, curr_limit: f64) -> Setpoint {
        Setpoint { volt, curr_limit }
    }

    #[test]
    fn accepts_a_setpoint_within_limits() {
        assert!(CLAMP.check_setpoint(sp(3.3, 1.0)).is_ok());
    }

    #[test]
    fn accepts_a_setpoint_exactly_at_the_limit() {
        // The limit is the maximum permitted value, not the first forbidden one.
        assert!(CLAMP.check_setpoint(sp(5.5, 2.0)).is_ok());
    }

    #[test]
    fn rejects_overvoltage() {
        let err = CLAMP.check_setpoint(sp(12.0, 1.0)).unwrap_err();
        assert_eq!(
            err,
            ClampViolation::Voltage {
                requested: 12.0,
                limit: 5.5
            }
        );
    }

    #[test]
    fn rejects_overcurrent() {
        let err = CLAMP.check_setpoint(sp(3.3, 3.0)).unwrap_err();
        assert!(matches!(err, ClampViolation::Current { .. }));
    }

    #[test]
    fn reports_voltage_first_when_both_are_breached() {
        // Voltage is the one that releases smoke.
        let err = CLAMP.check_setpoint(sp(12.0, 3.0)).unwrap_err();
        assert!(matches!(err, ClampViolation::Voltage { .. }));
    }

    #[test]
    fn ignores_protection_thresholds_that_are_disabled() {
        let p = Protection {
            ovp_level: 30.0,
            ovp_enabled: false,
            ocp_level: 3.0,
            ocp_enabled: false,
        };
        assert!(CLAMP.check_protection(p).is_ok());
    }

    #[test]
    fn rejects_an_enabled_ovp_above_the_clamp() {
        let p = Protection {
            ovp_level: 30.0,
            ovp_enabled: true,
            ocp_level: 1.0,
            ocp_enabled: true,
        };
        assert!(matches!(
            CLAMP.check_protection(p).unwrap_err(),
            ClampViolation::Voltage { .. }
        ));
    }

    #[test]
    fn violation_messages_name_both_numbers() {
        let msg = ClampViolation::Voltage {
            requested: 12.0,
            limit: 5.5,
        }
        .message();
        assert!(msg.contains("12.000"));
        assert!(msg.contains("5.500"));
    }
}
