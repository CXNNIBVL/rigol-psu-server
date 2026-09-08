//! Behavioural tests for supply supervision, driven through the simulator.
//!
//! These cover the safety rules that are otherwise only reachable on a bench
//! with a screwdriver and some patience: adoption of already-live channels
//! across a restart, quarantine on a serial mismatch, clamp refusal, and what a
//! channel reports once its supply stops answering.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use dp832::{ChannelId, Fault, LockState, Setpoint, SimulatedDp832};
use psu_core::{Config, DeviceStatus, Supervisor};

const SERIAL: &str = "DP8A0001";

fn config(measurement_hz: f64, setpoint_hz: f64, idle_secs: u64) -> Config {
    let text = format!(
        r#"
bind = "127.0.0.1:0"

[poll]
measurement_hz = {measurement_hz}
setpoint_hz = {setpoint_hz}

[lock]
idle_timeout_secs = {idle_secs}

[[psu]]
slot = "psu1"
device = "/dev/rigol-psu1"
label = "Bench Left"
serial = "{SERIAL}"

  [[psu.channel]]
  id = "CH1"
  max_volt = 5.5
  max_curr = 2.0

  [[psu.channel]]
  id = "CH2"
  max_volt = 15.0
  max_curr = 3.0

  [[psu.channel]]
  id = "CH3"
  max_volt = 5.0
  max_curr = 3.0
"#
    );
    Config::parse(&text, "test.toml").expect("fixture should be valid")
}

/// Start a supervisor over one simulated supply, returning a handle to the
/// simulator so a test can interfere with it from outside.
fn start(config: &Config, sim: SimulatedDp832) -> Supervisor {
    let mut sims = BTreeMap::new();
    sims.insert("psu1".to_string(), sim);
    Supervisor::start(config, |supply| {
        sims.get(&supply.slot)
            .cloned()
            .ok_or_else(|| format!("no simulator for {}", supply.slot))
    })
}

/// Poll the published state until `predicate` holds, or give up.
///
/// The actor runs on its own thread, so a test that asserts immediately after a
/// command races the next poll. This waits on the observable outcome instead of
/// sleeping for a guessed duration.
async fn wait_for(
    supervisor: &Supervisor,
    what: &str,
    predicate: impl Fn(&psu_core::SystemState) -> bool,
) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if predicate(&supervisor.snapshot()) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("timed out waiting for {what}: {:#?}", supervisor.snapshot());
}

#[tokio::test]
async fn publishes_measurements_once_polling_starts() {
    let sim = SimulatedDp832::new(SERIAL);
    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim);

    wait_for(&supervisor, "a measurement to arrive", |s| {
        s.device("psu1")
            .and_then(|d| d.channel(ChannelId::Ch1))
            .is_some_and(|c| c.measurement.is_some())
    })
    .await;

    supervisor.shutdown().await;
}

#[tokio::test]
async fn adopts_a_channel_that_was_already_live() {
    // The supply was powering something before the server started. That
    // experiment must survive the restart.
    let sim = SimulatedDp832::new(SERIAL);
    sim.front_panel_set(
        ChannelId::Ch1,
        Setpoint {
            volt: 3.3,
            curr_limit: 1.0,
        },
        true,
    );

    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim.clone());

    wait_for(&supervisor, "the live channel to be adopted", |s| {
        s.device("psu1")
            .and_then(|d| d.channel(ChannelId::Ch1))
            .is_some_and(|c| c.output_on == Some(true))
    })
    .await;

    // Crucially, still energised.
    assert!(sim.output_state(ChannelId::Ch1));

    supervisor.shutdown().await;

    // And still energised after shutdown, too.
    assert!(sim.output_state(ChannelId::Ch1));
}

#[tokio::test]
async fn freezes_but_does_not_cut_a_live_channel_outside_its_clamps() {
    // CH1 is clamped to 5.5 V but was left running at 12 V. The server must
    // flag it and refuse writes, without deciding on its own to cut the power.
    let sim = SimulatedDp832::new(SERIAL);
    sim.front_panel_set(
        ChannelId::Ch1,
        Setpoint {
            volt: 12.0,
            curr_limit: 1.0,
        },
        true,
    );

    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim.clone());

    wait_for(&supervisor, "the violation to be flagged", |s| {
        s.device("psu1")
            .and_then(|d| d.channel(ChannelId::Ch1))
            .is_some_and(|c| c.violation.is_some())
    })
    .await;

    assert!(sim.output_state(ChannelId::Ch1), "must stay energised");

    let err = supervisor
        .set_setpoint(
            "psu1",
            ChannelId::Ch1,
            Setpoint {
                volt: 3.3,
                curr_limit: 1.0,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "CHANNEL_FROZEN");

    supervisor.shutdown().await;
}

#[tokio::test]
async fn an_off_channel_outside_its_clamps_is_not_a_violation() {
    // Nothing is energised, so there is nothing to flag; the next write has to
    // pass the clamp anyway.
    let sim = SimulatedDp832::new(SERIAL);
    sim.front_panel_set(
        ChannelId::Ch1,
        Setpoint {
            volt: 12.0,
            curr_limit: 1.0,
        },
        false,
    );

    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim);

    wait_for(&supervisor, "the channel to be adopted", |s| {
        s.device("psu1")
            .and_then(|d| d.channel(ChannelId::Ch1))
            .is_some_and(|c| c.setpoint.is_some())
    })
    .await;

    let channel = supervisor.snapshot();
    let channel = channel
        .device("psu1")
        .unwrap()
        .channel(ChannelId::Ch1)
        .unwrap();
    assert!(channel.violation.is_none());

    supervisor.shutdown().await;
}

#[tokio::test]
async fn refuses_a_setpoint_above_the_clamp() {
    let sim = SimulatedDp832::new(SERIAL);
    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim.clone());

    let err = supervisor
        .set_setpoint(
            "psu1",
            ChannelId::Ch1,
            Setpoint {
                volt: 12.0,
                curr_limit: 1.0,
            },
        )
        .await
        .unwrap_err();
    assert_eq!(err.code(), "CLAMP_EXCEEDED");
    assert!(err.to_string().contains("5.500"));

    supervisor.shutdown().await;
}

#[tokio::test]
async fn accepts_a_setpoint_within_the_clamp() {
    let sim = SimulatedDp832::new(SERIAL);
    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim);

    supervisor
        .set_setpoint(
            "psu1",
            ChannelId::Ch1,
            Setpoint {
                volt: 3.3,
                curr_limit: 1.0,
            },
        )
        .await
        .unwrap();

    wait_for(&supervisor, "the setpoint to be published", |s| {
        s.device("psu1")
            .and_then(|d| d.channel(ChannelId::Ch1))
            .and_then(|c| c.setpoint)
            .is_some_and(|sp| (sp.volt - 3.3).abs() < 1e-6)
    })
    .await;

    supervisor.shutdown().await;
}

#[tokio::test]
async fn quarantines_a_supply_whose_serial_does_not_match() {
    let sim = SimulatedDp832::new("SOME-OTHER-UNIT");
    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim);

    wait_for(&supervisor, "quarantine", |s| {
        s.device("psu1").is_some_and(|d| d.status.is_quarantined())
    })
    .await;

    let state = supervisor.snapshot();
    match &state.device("psu1").unwrap().status {
        DeviceStatus::Quarantined {
            expected_serial,
            reported_serial,
        } => {
            assert_eq!(expected_serial, SERIAL);
            assert_eq!(reported_serial, "SOME-OTHER-UNIT");
        }
        other => panic!("expected quarantine, got {other:?}"),
    }

    supervisor.shutdown().await;
}

#[tokio::test]
async fn a_quarantined_supply_refuses_to_energise_but_allows_everything_else() {
    let sim = SimulatedDp832::new("SOME-OTHER-UNIT");
    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim.clone());

    wait_for(&supervisor, "quarantine", |s| {
        s.device("psu1").is_some_and(|d| d.status.is_quarantined())
    })
    .await;

    // Energising is the one thing that is forbidden.
    let err = supervisor
        .set_output("psu1", ChannelId::Ch1, true)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "DEVICE_QUARANTINED");
    assert!(!sim.output_state(ChannelId::Ch1));

    // Staging setpoints is still allowed.
    supervisor
        .set_setpoint(
            "psu1",
            ChannelId::Ch1,
            Setpoint {
                volt: 3.3,
                curr_limit: 1.0,
            },
        )
        .await
        .unwrap();

    supervisor.shutdown().await;
}

#[tokio::test]
async fn a_quarantined_supply_can_always_be_switched_off() {
    // Removing power is never blocked, whatever state the supply is in.
    let sim = SimulatedDp832::new("SOME-OTHER-UNIT");
    sim.front_panel_set(
        ChannelId::Ch1,
        Setpoint {
            volt: 3.3,
            curr_limit: 1.0,
        },
        true,
    );

    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim.clone());

    wait_for(&supervisor, "quarantine", |s| {
        s.device("psu1").is_some_and(|d| d.status.is_quarantined())
    })
    .await;

    supervisor
        .set_output("psu1", ChannelId::Ch1, false)
        .await
        .unwrap();
    assert!(!sim.output_state(ChannelId::Ch1));

    supervisor.shutdown().await;
}

#[tokio::test]
async fn readings_become_unknown_rather_than_zero_when_a_supply_drops() {
    // The rail may still be live. A stale zero would be a lie, and the most
    // expensive kind of lie this system can tell.
    let sim = SimulatedDp832::new(SERIAL);
    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim.clone());

    wait_for(&supervisor, "a measurement to arrive", |s| {
        s.device("psu1")
            .and_then(|d| d.channel(ChannelId::Ch1))
            .is_some_and(|c| c.measurement.is_some())
    })
    .await;

    sim.set_fault(Fault::Disconnected);

    wait_for(&supervisor, "the supply to go offline", |s| {
        s.device("psu1").is_some_and(|d| !d.status.is_online())
    })
    .await;

    let state = supervisor.snapshot();
    let device = state.device("psu1").unwrap();
    let channel = device.channel(ChannelId::Ch1).unwrap();

    assert_eq!(channel.measurement, None, "must not report a stale reading");
    assert_eq!(channel.output_on, None, "output state is no longer known");
    assert_eq!(channel.setpoint, None);
    // The limits are configuration, not observation, so they survive.
    assert_eq!(channel.limits.max_volt, 5.5);

    supervisor.shutdown().await;
}

#[tokio::test]
async fn reports_why_a_supply_went_offline() {
    let sim = SimulatedDp832::new(SERIAL);
    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim.clone());

    sim.set_fault(Fault::Timeout);

    // Note the initial status is already `Offline { NotYetContacted }`, so this
    // has to wait for the specific code rather than merely for "offline".
    wait_for(&supervisor, "a timeout to be reported", |s| {
        matches!(
            s.device("psu1").map(|d| &d.status),
            Some(DeviceStatus::Offline {
                code: psu_core::OfflineCode::Timeout,
                ..
            })
        )
    })
    .await;

    let state = supervisor.snapshot();
    match &state.device("psu1").unwrap().status {
        DeviceStatus::Offline { code, message } => {
            assert_eq!(*code, psu_core::OfflineCode::Timeout);
            assert!(!message.is_empty(), "the UI needs something to show");
        }
        other => panic!("expected offline, got {other:?}"),
    }

    supervisor.shutdown().await;
}

#[tokio::test]
async fn refuses_commands_while_offline() {
    let sim = SimulatedDp832::new(SERIAL);
    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim.clone());

    sim.set_fault(Fault::Disconnected);
    wait_for(&supervisor, "the supply to go offline", |s| {
        s.device("psu1").is_some_and(|d| !d.status.is_reachable())
    })
    .await;

    let err = supervisor
        .set_output("psu1", ChannelId::Ch1, true)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "DEVICE_OFFLINE");

    supervisor.shutdown().await;
}

#[tokio::test]
async fn recovers_without_intervention_when_a_supply_returns() {
    let sim = SimulatedDp832::new(SERIAL);
    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim.clone());

    sim.set_fault(Fault::Disconnected);
    wait_for(&supervisor, "the supply to go offline", |s| {
        s.device("psu1").is_some_and(|d| !d.status.is_online())
    })
    .await;

    sim.set_fault(Fault::None);
    wait_for(&supervisor, "the supply to come back", |s| {
        s.device("psu1").is_some_and(|d| d.status.is_online())
    })
    .await;

    wait_for(&supervisor, "readings to resume", |s| {
        s.device("psu1")
            .and_then(|d| d.channel(ChannelId::Ch1))
            .is_some_and(|c| c.measurement.is_some())
    })
    .await;

    supervisor.shutdown().await;
}

#[tokio::test]
async fn starts_degraded_when_a_supply_cannot_be_opened() {
    // One supply switched off must not take the bench down.
    let config = config(50.0, 25.0, 60);
    let supervisor = Supervisor::start(&config, |_| {
        Err::<SimulatedDp832, _>("no such device".to_string())
    });

    let state = supervisor.snapshot();
    let device = state
        .device("psu1")
        .expect("the supply must still be listed");
    assert!(!device.status.is_online());
    assert_eq!(
        device.channels.len(),
        3,
        "channels stay visible, just unknown"
    );

    supervisor.shutdown().await;
}

#[tokio::test]
async fn locks_the_front_panel_on_first_command_and_releases_it_on_shutdown() {
    let sim = SimulatedDp832::new(SERIAL);
    let config = config(50.0, 25.0, 3600);
    let supervisor = start(&config, sim.clone());

    // Idle: the panel is left alone, so a crash here leaves a usable bench.
    assert_eq!(sim.lock_state(), LockState::Unlocked);

    supervisor
        .set_setpoint(
            "psu1",
            ChannelId::Ch1,
            Setpoint {
                volt: 3.3,
                curr_limit: 1.0,
            },
        )
        .await
        .unwrap();
    assert_eq!(sim.lock_state(), LockState::Locked);

    supervisor.shutdown().await;
    assert_eq!(
        sim.lock_state(),
        LockState::Unlocked,
        "the shutdown path must hand the panel back"
    );
}

#[tokio::test]
async fn releases_the_front_panel_once_idle() {
    let sim = SimulatedDp832::new(SERIAL);
    let config = config(50.0, 25.0, 0);
    let supervisor = start(&config, sim.clone());

    supervisor
        .set_output("psu1", ChannelId::Ch1, false)
        .await
        .unwrap();

    // With a zero idle timeout the next poll tick should hand the panel back.
    let deadline = Instant::now() + Duration::from_secs(2);
    while sim.lock_state() == LockState::Locked && Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    assert_eq!(sim.lock_state(), LockState::Unlocked);

    supervisor.shutdown().await;
}

#[tokio::test]
async fn front_panel_changes_show_up_in_published_state() {
    // Somebody walked up and turned the knob. The setpoint poll is what stops
    // the UI showing fiction.
    let sim = SimulatedDp832::new(SERIAL);
    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim.clone());

    wait_for(&supervisor, "initial adoption", |s| {
        s.device("psu1")
            .and_then(|d| d.channel(ChannelId::Ch2))
            .is_some_and(|c| c.setpoint.is_some())
    })
    .await;

    sim.front_panel_set(
        ChannelId::Ch2,
        Setpoint {
            volt: 9.0,
            curr_limit: 1.5,
        },
        true,
    );

    wait_for(&supervisor, "the knob change to be noticed", |s| {
        s.device("psu1")
            .and_then(|d| d.channel(ChannelId::Ch2))
            .and_then(|c| c.setpoint)
            .is_some_and(|sp| (sp.volt - 9.0).abs() < 1e-6)
    })
    .await;

    supervisor.shutdown().await;
}

#[tokio::test]
async fn publishing_advances_the_generation_counter() {
    let sim = SimulatedDp832::new(SERIAL);
    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim);

    let mut rx = supervisor.subscribe();
    let before = rx.borrow_and_update().generation;
    supervisor.publish();
    rx.changed().await.unwrap();
    assert!(rx.borrow().generation > before);

    supervisor.shutdown().await;
}

#[tokio::test]
async fn rejects_a_command_for_an_unknown_slot() {
    let sim = SimulatedDp832::new(SERIAL);
    let config = config(50.0, 25.0, 60);
    let supervisor = start(&config, sim);

    let err = supervisor
        .set_output("psu-nonexistent", ChannelId::Ch1, true)
        .await
        .unwrap_err();
    assert_eq!(err.code(), "UNKNOWN_SLOT");

    supervisor.shutdown().await;
}
