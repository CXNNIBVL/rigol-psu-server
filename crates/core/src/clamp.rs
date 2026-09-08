//! Operator-configured per-channel limits.
//!
//! These are deliberately *not* the instrument's own OVP/OCP. The supply's
//! protection is a last-resort hardware trip; these clamps are a policy gate
//! that a request must pass before it ever reaches the wire. They live in a
//! root-owned config file rather than the database precisely so that nothing
//! reachable from the API can raise them.

use dp832::Setpoint;
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

    // Protection thresholds are deliberately *not* clamped.
    //
    // This limit governs what a channel may be asked to *output*. OVP and OCP
    // are trip points: the instrument's own backstop, which by definition sits
    // above the value being run. Clamping them to the same ceiling as the
    // output made protection unavailable exactly where the limits are tightest
    // — a channel limited to 5 V could not carry an OVP at 5.5 V, so the only
    // way to configure it at all was to leave it off. That is strictly less
    // safe than allowing a trip point above the ceiling, which cannot itself
    // produce a volt of output.
    //
    // A trip point *below* the value being run is a real mistake, and it is
    // caught where it can be judged against its own setpoint: see
    // `validate_preset` in the store.
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
    fn a_protection_trip_point_may_sit_above_the_limit() {
        // The limit governs output. A trip point above it is the instrument's
        // backstop and cannot itself produce a volt; forbidding it left the
        // tightest channels with no way to configure protection at all.
        let sp = sp(5.0, 1.0);
        assert!(Clamp {
            max_volt: 5.0,
            max_curr: 2.0
        }
        .check_setpoint(sp)
        .is_ok());
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
