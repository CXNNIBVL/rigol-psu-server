//! Configuration is a safety surface, so these tests are mostly about what it
//! *refuses*. A config that starts the server with weaker limits than the
//! operator intended is the failure mode worth engineering against.

use psu_core::Config;

const VALID: &str = r#"
bind = "127.0.0.1:8080"

[[psu]]
slot = "psu1"
device = "/dev/rigol-psu1"
label = "Bench Left"
serial = "DP8A0001"

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
"#;

fn parse(text: &str) -> Result<Config, String> {
    Config::parse(text, "test.toml").map_err(|e| e.to_string())
}

#[test]
fn accepts_a_complete_config() {
    let config = parse(VALID).unwrap();
    assert_eq!(config.supplies.len(), 1);
    assert_eq!(config.supplies[0].slot, "psu1");
    assert_eq!(config.supplies[0].channels.len(), 3);
}

#[test]
fn applies_documented_poll_defaults() {
    let config = parse(VALID).unwrap();
    assert_eq!(config.poll.measurement_hz, 2.0);
    assert_eq!(config.poll.setpoint_hz, 0.5);
}

#[test]
fn requires_an_explicit_bind_address() {
    // Exposing this service must be a deliberate act, never the consequence of
    // a default.
    let text = VALID.replace("bind = \"127.0.0.1:8080\"", "");
    assert!(parse(&text).is_err());
}

#[test]
fn rejects_a_supply_with_an_undeclared_channel() {
    // An undeclared channel has no clamp, and a channel with no clamp must not
    // be controllable. Refusing to start is the only safe response.
    let text = VALID
        .split("  [[psu.channel]]\n  id = \"CH3\"")
        .next()
        .unwrap()
        .to_string();
    let err = parse(&text).unwrap_err();
    assert!(err.contains("CH3"), "unexpected error: {err}");
}

#[test]
fn rejects_a_clamp_above_the_channel_rating() {
    // CH3 is a 5 V channel; a 30 V limit would never bind, which means it is not
    // the protection the operator thinks it is.
    let text = VALID.replace(
        "  id = \"CH3\"\n  max_volt = 5.0",
        "  id = \"CH3\"\n  max_volt = 30.0",
    );
    let err = parse(&text).unwrap_err();
    assert!(err.contains("rating"), "unexpected error: {err}");
}

#[test]
fn rejects_a_negative_clamp() {
    let text = VALID.replace("max_volt = 5.5", "max_volt = -1.0");
    assert!(parse(&text).is_err());
}

/// Two supplies, differing only in the fields named. Written out rather than
/// derived by string surgery, so the fixture cannot drift from what the test
/// claims to be checking.
fn two_supplies(slot_b: &str, device_b: &str) -> String {
    format!(
        r#"
bind = "127.0.0.1:8080"

[[psu]]
slot = "psu1"
device = "/dev/rigol-psu1"
label = "Bench Left"
serial = "DP8A0001"
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
slot = "{slot_b}"
device = "{device_b}"
label = "Bench Right"
serial = "DP8A0002"
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
    )
}

#[test]
fn accepts_two_distinct_supplies() {
    // Guards the fixture itself: the rejection tests below are only meaningful
    // if this baseline is accepted.
    assert!(parse(&two_supplies("psu2", "/dev/rigol-psu2")).is_ok());
}

#[test]
fn rejects_duplicate_slots() {
    let err = parse(&two_supplies("psu1", "/dev/rigol-psu2")).unwrap_err();
    assert!(err.contains("duplicate"), "unexpected error: {err}");
}

#[test]
fn rejects_an_empty_serial() {
    // Without a serial the identity assertion is vacuous, and quarantine could
    // never fire.
    let text = VALID.replace("serial = \"DP8A0001\"", "serial = \"\"");
    let err = parse(&text).unwrap_err();
    assert!(err.contains("serial"), "unexpected error: {err}");
}

#[test]
fn rejects_a_slot_name_that_would_not_survive_a_url() {
    let text = VALID.replace("slot = \"psu1\"", "slot = \"psu 1/../etc\"");
    assert!(parse(&text).is_err());
}

#[test]
fn rejects_two_supplies_claiming_one_device() {
    // Two slots pointing at one symlink means two threads fighting over a
    // handle that only permits one command at a time.
    let err = parse(&two_supplies("psu2", "/dev/rigol-psu1")).unwrap_err();
    assert!(err.contains("more than one"), "unexpected error: {err}");
}

#[test]
fn rejects_unknown_keys() {
    // Catches typos that would otherwise silently leave a limit at its default.
    let text = VALID.replace("max_volt = 5.5", "max_volts = 5.5");
    assert!(parse(&text).is_err());
}

#[test]
fn rejects_a_zero_poll_rate() {
    let text = format!("{VALID}\n[poll]\nmeasurement_hz = 0.0\nsetpoint_hz = 0.5\n");
    assert!(parse(&text).is_err());
}

#[test]
fn rejects_a_config_with_no_supplies() {
    assert!(parse("bind = \"127.0.0.1:8080\"\n").is_err());
}
