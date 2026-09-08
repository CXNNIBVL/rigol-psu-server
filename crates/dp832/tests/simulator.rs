//! Behavioural tests driving [`Dp832`] against the simulator.
//!
//! These cover the paths that cannot be tested against real hardware in CI —
//! disconnects, timeouts, garbage responses and front-panel interference — as
//! well as the ordinary control flow.

use std::time::Duration;

use dp832::{
    ChannelId, Dp832, Fault, LockState, Protection, Setpoint, SimulatedDp832, TransportError,
};

fn supply() -> (SimulatedDp832, Dp832<SimulatedDp832>) {
    let sim = SimulatedDp832::new("DP8A241800001");
    let device = Dp832::new(sim.clone());
    (sim, device)
}

#[test]
fn reports_its_identity() {
    let (_sim, mut psu) = supply();
    let id = psu.identify().unwrap();
    assert_eq!(id.vendor, "RIGOL TECHNOLOGIES");
    assert_eq!(id.model, "DP832");
    assert_eq!(id.serial, "DP8A241800001");
}

#[test]
fn setpoints_round_trip() {
    let (_sim, mut psu) = supply();
    let want = Setpoint {
        volt: 3.3,
        curr_limit: 0.5,
    };
    psu.set_setpoint(ChannelId::Ch1, want).unwrap();
    assert_eq!(psu.setpoint(ChannelId::Ch1).unwrap(), want);
}

#[test]
fn channels_are_independent() {
    let (_sim, mut psu) = supply();
    psu.set_setpoint(
        ChannelId::Ch1,
        Setpoint {
            volt: 3.3,
            curr_limit: 1.0,
        },
    )
    .unwrap();
    psu.set_setpoint(
        ChannelId::Ch2,
        Setpoint {
            volt: 12.0,
            curr_limit: 2.0,
        },
    )
    .unwrap();

    assert_eq!(psu.setpoint(ChannelId::Ch1).unwrap().volt, 3.3);
    assert_eq!(psu.setpoint(ChannelId::Ch2).unwrap().volt, 12.0);
    assert_eq!(psu.setpoint(ChannelId::Ch3).unwrap().volt, 0.0);
}

#[test]
fn an_off_channel_measures_zero() {
    let (_sim, mut psu) = supply();
    psu.set_setpoint(
        ChannelId::Ch1,
        Setpoint {
            volt: 5.0,
            curr_limit: 1.0,
        },
    )
    .unwrap();

    let m = psu.measure(ChannelId::Ch1).unwrap();
    assert_eq!(m.volt, 0.0);
    assert_eq!(m.curr, 0.0);
    assert_eq!(m.power, 0.0);
}

#[test]
fn a_live_channel_measures_its_load() {
    let (sim, mut psu) = supply();
    sim.set_load(ChannelId::Ch1, Some(10.0));
    psu.set_setpoint(
        ChannelId::Ch1,
        Setpoint {
            volt: 5.0,
            curr_limit: 1.0,
        },
    )
    .unwrap();
    psu.set_output(ChannelId::Ch1, true).unwrap();

    let m = psu.measure(ChannelId::Ch1).unwrap();
    assert!((m.volt - 5.0).abs() < 1e-6);
    assert!((m.curr - 0.5).abs() < 1e-6);
    assert!((m.power - 2.5).abs() < 1e-6);
}

#[test]
fn voltage_sags_when_the_current_limit_binds() {
    let (sim, mut psu) = supply();
    sim.set_load(ChannelId::Ch1, Some(1.0));
    psu.set_setpoint(
        ChannelId::Ch1,
        Setpoint {
            volt: 5.0,
            curr_limit: 0.5,
        },
    )
    .unwrap();
    psu.set_output(ChannelId::Ch1, true).unwrap();

    // 5 V into 1 Ω wants 5 A, but the limit is 0.5 A, so the supply drops into
    // constant-current mode at 0.5 V.
    let m = psu.measure(ChannelId::Ch1).unwrap();
    assert!((m.curr - 0.5).abs() < 1e-6);
    assert!((m.volt - 0.5).abs() < 1e-6);
}

#[test]
fn protection_round_trips() {
    let (_sim, mut psu) = supply();
    let want = Protection {
        ovp_level: 5.5,
        ovp_enabled: true,
        ocp_level: 1.2,
        ocp_enabled: true,
    };
    psu.set_protection(ChannelId::Ch2, want).unwrap();
    assert_eq!(psu.protection(ChannelId::Ch2).unwrap(), want);
}

#[test]
fn output_toggles() {
    let (sim, mut psu) = supply();
    assert!(!psu.output(ChannelId::Ch3).unwrap());

    psu.set_output(ChannelId::Ch3, true).unwrap();
    assert!(psu.output(ChannelId::Ch3).unwrap());
    assert!(sim.output_state(ChannelId::Ch3));

    psu.set_output(ChannelId::Ch3, false).unwrap();
    assert!(!psu.output(ChannelId::Ch3).unwrap());
}

#[test]
fn lock_is_released_as_well_as_acquired() {
    let (sim, mut psu) = supply();
    assert_eq!(sim.lock_state(), LockState::Unlocked);

    psu.set_lock(LockState::Locked).unwrap();
    assert_eq!(sim.lock_state(), LockState::Locked);

    // The shutdown path must actually get the panel back; a lock that cannot be
    // released leaves the mains switch as the only recovery.
    psu.set_lock(LockState::Unlocked).unwrap();
    assert_eq!(sim.lock_state(), LockState::Unlocked);
}

#[test]
fn a_full_channel_readback_reflects_every_field() {
    let (_sim, mut psu) = supply();
    let sp = Setpoint {
        volt: 3.3,
        curr_limit: 0.8,
    };
    let prot = Protection {
        ovp_level: 3.6,
        ovp_enabled: true,
        ocp_level: 1.0,
        ocp_enabled: false,
    };
    psu.set_setpoint(ChannelId::Ch1, sp).unwrap();
    psu.set_protection(ChannelId::Ch1, prot).unwrap();
    psu.set_output(ChannelId::Ch1, true).unwrap();

    let state = psu.channel_state(ChannelId::Ch1).unwrap();
    assert_eq!(state.setpoint, sp);
    assert_eq!(state.protection, prot);
    assert!(state.output_on);
}

#[test]
fn front_panel_changes_are_visible_to_the_setpoint_poll() {
    // The server is not the only writer: someone can walk up and turn the knob.
    // Polling setpoints is what stops the UI from showing fiction.
    let (sim, mut psu) = supply();
    psu.set_setpoint(
        ChannelId::Ch1,
        Setpoint {
            volt: 3.3,
            curr_limit: 1.0,
        },
    )
    .unwrap();

    sim.front_panel_set(
        ChannelId::Ch1,
        Setpoint {
            volt: 12.0,
            curr_limit: 2.0,
        },
        true,
    );

    let state = psu.channel_state(ChannelId::Ch1).unwrap();
    assert_eq!(state.setpoint.volt, 12.0);
    assert!(state.output_on);
}

#[test]
fn a_disconnect_is_reported_as_one() {
    let (sim, mut psu) = supply();
    sim.set_fault(Fault::Disconnected);

    let err = psu.measure(ChannelId::Ch1).unwrap_err();
    assert!(matches!(err, TransportError::DeviceGone));
    assert!(err.is_disconnect());
}

#[test]
fn a_timeout_is_reported_as_one() {
    let (sim, mut psu) = supply();
    sim.set_fault(Fault::Timeout);

    let err = psu.identify().unwrap_err();
    assert!(matches!(err, TransportError::Timeout(_)));
    assert!(err.is_disconnect());
}

#[test]
fn a_garbage_response_does_not_take_the_supply_offline() {
    // A protocol error means we misunderstood the answer, not that the link
    // died. Dropping the supply offline here would hide the real bug.
    let (sim, mut psu) = supply();
    sim.set_fault(Fault::Garbage);

    let err = psu.measure(ChannelId::Ch1).unwrap_err();
    assert!(matches!(err, TransportError::Protocol(_)));
    assert!(!err.is_disconnect());
}

#[test]
fn recovers_when_the_fault_clears() {
    let (sim, mut psu) = supply();
    sim.set_fault(Fault::Disconnected);
    assert!(psu.identify().is_err());

    sim.set_fault(Fault::None);
    assert_eq!(psu.identify().unwrap().serial, "DP8A241800001");
}

#[test]
fn timeout_error_carries_the_duration() {
    let (sim, mut psu) = supply();
    sim.set_fault(Fault::Timeout);
    match psu.measure(ChannelId::Ch2).unwrap_err() {
        TransportError::Timeout(d) => assert!(d > Duration::ZERO),
        other => panic!("expected a timeout, got {other:?}"),
    }
}
