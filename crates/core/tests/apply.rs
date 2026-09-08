//! Applying profiles: the safety rules from §9.2 and §10 of the plan, exercised
//! end to end against simulated supplies.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use dp832::{ChannelId, Fault, SimulatedDp832};
use psu_core::{
    apply, power_off, power_on, Config, NewChannelPreset, NewDeviceProfile, NewProfileBinding,
    Store, Supervisor,
};

const SERIAL1: &str = "SIM-0001";
const SERIAL2: &str = "SIM-0002";

fn config() -> Config {
    let text = format!(
        r#"
bind = "127.0.0.1:0"

[poll]
measurement_hz = 50.0
setpoint_hz = 25.0

[[psu]]
slot = "psu1"
device = "/dev/rigol-psu1"
label = "Left"
serial = "{SERIAL1}"
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

[[psu]]
slot = "psu2"
device = "/dev/rigol-psu2"
label = "Right"
serial = "{SERIAL2}"
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
    Config::parse(&text, "test.toml").unwrap()
}

struct Bench {
    supervisor: Supervisor,
    store: Store,
    sims: BTreeMap<String, SimulatedDp832>,
}

fn bench() -> Bench {
    let config = config();
    let mut sims = BTreeMap::new();
    sims.insert("psu1".to_string(), SimulatedDp832::new(SERIAL1));
    sims.insert("psu2".to_string(), SimulatedDp832::new(SERIAL2));

    let for_start = sims.clone();
    let supervisor = Supervisor::start(&config, |supply| {
        for_start
            .get(&supply.slot)
            .cloned()
            .ok_or_else(|| format!("no sim for {}", supply.slot))
    });

    Bench {
        supervisor,
        store: Store::in_memory().unwrap(),
        sims,
    }
}

async fn wait_ready(b: &Bench) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let state = b.supervisor.snapshot();
        let ready = state
            .devices
            .iter()
            .all(|d| d.channels.iter().all(|c| c.output_on.is_some()));
        if ready {
            return;
        }
        assert!(Instant::now() < deadline, "supplies never became ready");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

fn preset(store: &Store, name: &str, volt: f64, on: bool) -> i64 {
    store
        .create_preset(&NewChannelPreset {
            name: name.into(),
            volt,
            curr_limit: 1.0,
            ovp_level: volt + 0.2,
            ovp_enabled: true,
            ocp_level: 1.5,
            ocp_enabled: true,
            output_on: on,
        })
        .unwrap()
        .id
}

fn profile(store: &Store, name: &str, bindings: Vec<NewProfileBinding>) -> i64 {
    store
        .create_profile(&NewDeviceProfile {
            name: name.into(),
            description: String::new(),
            bindings,
        })
        .unwrap()
        .id
}

fn binding(slot: &str, channel: &str, preset_id: i64, order: i64, delay: i64) -> NewProfileBinding {
    NewProfileBinding {
        slot: slot.into(),
        channel: channel.into(),
        preset_id,
        sequence_order: order,
        delay_ms: delay,
    }
}

#[tokio::test]
async fn applying_stages_setpoints_without_energising_anything() {
    // The central rule: an apply prepares, it does not commit.
    let b = bench();
    wait_ready(&b).await;

    let p = preset(&b.store, "3v3", 3.3, true);
    let id = profile(&b.store, "Board", vec![binding("psu1", "CH1", p, 1, 0)]);

    apply(&b.store, &b.supervisor, id, false).await.unwrap();

    let sim = &b.sims["psu1"];
    assert!(
        !sim.output_state(ChannelId::Ch1),
        "apply must never energise, even when the preset says the output is on"
    );

    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let state = b.supervisor.snapshot();
        let sp = state
            .device("psu1")
            .and_then(|d| d.channel(ChannelId::Ch1))
            .and_then(|c| c.setpoint);
        if sp.is_some_and(|s| (s.volt - 3.3).abs() < 1e-6) {
            break;
        }
        assert!(Instant::now() < deadline, "setpoint was never staged");
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    b.supervisor.shutdown().await;
}

#[tokio::test]
async fn powering_on_energises_only_the_rails_the_profile_marks_on() {
    let b = bench();
    wait_ready(&b).await;

    let on = preset(&b.store, "on", 3.3, true);
    let off = preset(&b.store, "staged only", 5.0, false);
    let id = profile(
        &b.store,
        "Board",
        vec![
            binding("psu1", "CH1", on, 1, 0),
            binding("psu1", "CH2", off, 2, 0),
        ],
    );

    apply(&b.store, &b.supervisor, id, false).await.unwrap();
    power_on(&b.store, &b.supervisor, id).await.unwrap();

    let sim = &b.sims["psu1"];
    assert!(sim.output_state(ChannelId::Ch1));
    assert!(!sim.output_state(ChannelId::Ch2), "not marked on");

    b.supervisor.shutdown().await;
}

#[tokio::test]
async fn power_up_honours_the_sequence_delay() {
    // A device whose rails must settle in order gets the gap it asked for.
    let b = bench();
    wait_ready(&b).await;

    let core = preset(&b.store, "core", 1.2, true);
    let io = preset(&b.store, "io", 3.3, true);
    let id = profile(
        &b.store,
        "Sequenced",
        vec![
            binding("psu1", "CH1", core, 1, 120),
            binding("psu1", "CH2", io, 2, 0),
        ],
    );

    apply(&b.store, &b.supervisor, id, false).await.unwrap();
    let started = Instant::now();
    power_on(&b.store, &b.supervisor, id).await.unwrap();
    assert!(
        started.elapsed() >= Duration::from_millis(120),
        "the dwell between rails was skipped"
    );

    b.supervisor.shutdown().await;
}

#[tokio::test]
async fn applying_over_a_live_channel_is_refused_but_forceable() {
    let b = bench();
    wait_ready(&b).await;
    b.supervisor
        .set_output("psu1", ChannelId::Ch1, true)
        .await
        .unwrap();

    let p = preset(&b.store, "3v3", 3.3, true);
    let id = profile(&b.store, "Board", vec![binding("psu1", "CH1", p, 1, 0)]);

    let deadline = Instant::now() + Duration::from_secs(2);
    while b
        .supervisor
        .snapshot()
        .device("psu1")
        .and_then(|d| d.channel(ChannelId::Ch1))
        .and_then(|c| c.output_on)
        != Some(true)
    {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let err = apply(&b.store, &b.supervisor, id, false).await.unwrap_err();
    assert_eq!(err.code(), "CHANNEL_LIVE");
    assert!(err.is_forceable());

    apply(&b.store, &b.supervisor, id, true)
        .await
        .expect("force overrides exactly this rule");

    b.supervisor.shutdown().await;
}

#[tokio::test]
async fn force_does_not_override_a_clamp() {
    // The one thing `force` must never do. A force flag that could raise an
    // operator limit would make the root-owned config file pointless.
    let b = bench();
    wait_ready(&b).await;

    // CH1 is clamped at 5.5 V.
    let p = preset(&b.store, "too much", 12.0, true);
    let id = profile(&b.store, "Board", vec![binding("psu1", "CH1", p, 1, 0)]);

    for force in [false, true] {
        let err = apply(&b.store, &b.supervisor, id, force).await.unwrap_err();
        assert_eq!(err.code(), "CLAMP_EXCEEDED", "force = {force}");
        assert!(!err.is_forceable());
    }

    b.supervisor.shutdown().await;
}

#[tokio::test]
async fn a_profile_spanning_an_offline_supply_is_refused_outright() {
    // Half-powering a board can push current through an unpowered rail's
    // protection diodes, so this is all-or-nothing and not forceable.
    let b = bench();
    wait_ready(&b).await;

    let p = preset(&b.store, "3v3", 3.3, true);
    let id = profile(
        &b.store,
        "Spanning",
        vec![
            binding("psu1", "CH1", p, 1, 0),
            binding("psu2", "CH1", p, 2, 0),
        ],
    );

    b.sims["psu2"].set_fault(Fault::Disconnected);
    let deadline = Instant::now() + Duration::from_secs(2);
    while b
        .supervisor
        .snapshot()
        .device("psu2")
        .is_some_and(|d| d.status.is_online())
    {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    for force in [false, true] {
        let err = apply(&b.store, &b.supervisor, id, force).await.unwrap_err();
        assert_eq!(err.code(), "PARTIAL_APPLY_REFUSED", "force = {force}");
        assert!(!err.is_forceable());
    }

    // And nothing was written to the supply that *was* reachable.
    assert!(!b.sims["psu1"].output_state(ChannelId::Ch1));

    b.supervisor.shutdown().await;
}

#[tokio::test]
async fn nothing_is_written_when_one_binding_fails_validation() {
    // All-or-nothing means the first channel must not be staged when the second
    // is out of bounds.
    let b = bench();
    wait_ready(&b).await;

    let good = preset(&b.store, "good", 3.3, true);
    let bad = preset(&b.store, "bad", 12.0, true); // CH3 is clamped at 5 V
    let id = profile(
        &b.store,
        "Board",
        vec![
            binding("psu1", "CH1", good, 1, 0),
            binding("psu1", "CH3", bad, 2, 0),
        ],
    );

    assert_eq!(
        apply(&b.store, &b.supervisor, id, false)
            .await
            .unwrap_err()
            .code(),
        "CLAMP_EXCEEDED"
    );

    let state = b.supervisor.snapshot();
    let ch1 = state
        .device("psu1")
        .and_then(|d| d.channel(ChannelId::Ch1))
        .and_then(|c| c.setpoint)
        .unwrap();
    assert_eq!(ch1.volt, 0.0, "CH1 must not have been staged");

    b.supervisor.shutdown().await;
}

#[tokio::test]
async fn an_empty_profile_is_refused() {
    let b = bench();
    wait_ready(&b).await;
    let id = profile(&b.store, "Nothing", vec![]);
    assert_eq!(
        apply(&b.store, &b.supervisor, id, false)
            .await
            .unwrap_err()
            .code(),
        "PROFILE_EMPTY"
    );
    b.supervisor.shutdown().await;
}

#[tokio::test]
async fn a_profile_bound_to_an_unconfigured_slot_is_refused() {
    let b = bench();
    wait_ready(&b).await;
    let p = preset(&b.store, "3v3", 3.3, true);
    let id = profile(&b.store, "Ghost", vec![binding("psu9", "CH1", p, 1, 0)]);
    assert_eq!(
        apply(&b.store, &b.supervisor, id, false)
            .await
            .unwrap_err()
            .code(),
        "UNKNOWN_SLOT"
    );
    b.supervisor.shutdown().await;
}

#[tokio::test]
async fn powering_off_switches_every_rail_off() {
    let b = bench();
    wait_ready(&b).await;

    let p = preset(&b.store, "3v3", 3.3, true);
    let id = profile(
        &b.store,
        "Board",
        vec![
            binding("psu1", "CH1", p, 1, 0),
            binding("psu1", "CH2", p, 2, 0),
        ],
    );

    apply(&b.store, &b.supervisor, id, false).await.unwrap();
    power_on(&b.store, &b.supervisor, id).await.unwrap();
    assert!(b.sims["psu1"].output_state(ChannelId::Ch1));

    power_off(&b.store, &b.supervisor, id).await.unwrap();
    assert!(!b.sims["psu1"].output_state(ChannelId::Ch1));
    assert!(!b.sims["psu1"].output_state(ChannelId::Ch2));

    b.supervisor.shutdown().await;
}

#[tokio::test]
async fn powering_off_still_reaches_the_supplies_it_can() {
    // Removing power is always permitted, so one unreachable supply must not
    // stop the rest of a device being switched off.
    let b = bench();
    wait_ready(&b).await;

    let p = preset(&b.store, "3v3", 3.3, true);
    let id = profile(
        &b.store,
        "Spanning",
        vec![
            binding("psu1", "CH1", p, 1, 0),
            binding("psu2", "CH1", p, 2, 0),
        ],
    );

    apply(&b.store, &b.supervisor, id, false).await.unwrap();
    power_on(&b.store, &b.supervisor, id).await.unwrap();

    b.sims["psu2"].set_fault(Fault::Disconnected);
    let deadline = Instant::now() + Duration::from_secs(2);
    while b
        .supervisor
        .snapshot()
        .device("psu2")
        .is_some_and(|d| d.status.is_online())
    {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    // Reports the failure, but psu1 is off regardless.
    assert!(power_off(&b.store, &b.supervisor, id).await.is_err());
    assert!(!b.sims["psu1"].output_state(ChannelId::Ch1));

    b.supervisor.shutdown().await;
}

#[tokio::test]
async fn a_quarantined_supply_blocks_a_profile() {
    let config = config();
    let mut sims = BTreeMap::new();
    sims.insert("psu1".to_string(), SimulatedDp832::new("WRONG-UNIT"));
    sims.insert("psu2".to_string(), SimulatedDp832::new(SERIAL2));
    let for_start = sims.clone();
    let supervisor = Supervisor::start(&config, |supply| {
        for_start.get(&supply.slot).cloned().ok_or("missing")
    });
    let store = Store::in_memory().unwrap();

    let deadline = Instant::now() + Duration::from_secs(2);
    while !supervisor
        .snapshot()
        .device("psu1")
        .is_some_and(|d| d.status.is_quarantined())
    {
        assert!(Instant::now() < deadline);
        tokio::time::sleep(Duration::from_millis(5)).await;
    }

    let p = preset(&store, "3v3", 3.3, true);
    let id = profile(&store, "Board", vec![binding("psu1", "CH1", p, 1, 0)]);

    for force in [false, true] {
        let err = apply(&store, &supervisor, id, force).await.unwrap_err();
        assert_eq!(err.code(), "DEVICE_QUARANTINED", "force = {force}");
        assert!(!err.is_forceable());
    }

    supervisor.shutdown().await;
}
