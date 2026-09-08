//! Store behaviour: CRUD, and the constraints that stop a profile being
//! silently gutted.

use psu_core::{NewChannelPreset, NewDeviceProfile, NewProfileBinding, Store};

fn preset(name: &str, volt: f64) -> NewChannelPreset {
    NewChannelPreset {
        name: name.into(),
        volt,
        curr_limit: 1.0,
        ovp_level: volt + 0.5,
        ovp_enabled: true,
        ocp_level: 1.5,
        ocp_enabled: true,
    }
}

fn store() -> Store {
    Store::in_memory().unwrap()
}

#[test]
fn migrations_run_on_a_fresh_database() {
    let s = store();
    assert!(s.presets().unwrap().is_empty());
    assert!(s.profiles().unwrap().is_empty());
}

#[test]
fn migrations_are_idempotent_across_reopens() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("data.db");

    let first = Store::open(&path).unwrap();
    first.create_preset(&preset("3v3", 3.3)).unwrap();
    drop(first);

    // Reopening must not re-run the migration and wipe the row.
    let second = Store::open(&path).unwrap();
    assert_eq!(second.presets().unwrap().len(), 1);
}

#[test]
fn presets_round_trip() {
    let s = store();
    let created = s.create_preset(&preset("3v3 rail", 3.3)).unwrap();
    assert_eq!(created.name, "3v3 rail");
    assert_eq!(created.volt, 3.3);
    assert_eq!(s.preset(created.id).unwrap(), created);
}

#[test]
fn preset_names_are_unique() {
    let s = store();
    s.create_preset(&preset("3v3", 3.3)).unwrap();
    let err = s.create_preset(&preset("3v3", 5.0)).unwrap_err();
    assert_eq!(err.code(), "DUPLICATE_NAME");
}

#[test]
fn rejects_a_preset_with_a_non_numeric_value() {
    // NaN compares false against every clamp, so it would sail past the limit
    // check later. It has to be stopped at the door.
    let s = store();
    let mut p = preset("bad", 3.3);
    p.volt = f64::NAN;
    assert_eq!(s.create_preset(&p).unwrap_err().code(), "VALIDATION_FAILED");
}

#[test]
fn rejects_a_negative_preset_value() {
    let s = store();
    let mut p = preset("bad", 3.3);
    p.curr_limit = -1.0;
    assert!(s.create_preset(&p).is_err());
}

#[test]
fn updating_a_preset_keeps_its_id() {
    let s = store();
    let created = s.create_preset(&preset("3v3", 3.3)).unwrap();
    let updated = s.update_preset(created.id, &preset("3v3", 3.0)).unwrap();
    assert_eq!(updated.id, created.id);
    assert_eq!(updated.volt, 3.0);
}

#[test]
fn profiles_carry_their_bindings_in_power_up_order() {
    let s = store();
    let core = s.create_preset(&preset("core 1v2", 1.2)).unwrap();
    let io = s.create_preset(&preset("io 3v3", 3.3)).unwrap();

    let profile = s
        .create_profile(&NewDeviceProfile {
            name: "Mainboard".into(),
            description: "Core before I/O".into(),
            bindings: vec![
                NewProfileBinding {
                    slot: "psu1".into(),
                    channel: "CH2".into(),
                    preset_id: io.id,
                    sequence_order: 2,
                    delay_ms: 0,
                    output_on: true,
                },
                NewProfileBinding {
                    slot: "psu1".into(),
                    channel: "CH1".into(),
                    preset_id: core.id,
                    sequence_order: 1,
                    delay_ms: 50,
                    output_on: true,
                },
            ],
        })
        .unwrap();

    // Declared I/O first, but core has the lower order and must come back first.
    let order: Vec<&str> = profile
        .bindings
        .iter()
        .map(|b| b.preset.name.as_str())
        .collect();
    assert_eq!(order, vec!["core 1v2", "io 3v3"]);
    assert_eq!(profile.bindings[0].delay_ms, 50);
}

#[test]
fn a_channel_cannot_be_bound_twice_in_one_profile() {
    let s = store();
    let p = s.create_preset(&preset("3v3", 3.3)).unwrap();
    let err = s
        .create_profile(&NewDeviceProfile {
            name: "Confused".into(),
            description: String::new(),
            bindings: vec![
                NewProfileBinding {
                    slot: "psu1".into(),
                    channel: "CH1".into(),
                    preset_id: p.id,
                    sequence_order: 1,
                    delay_ms: 0,
                    output_on: true,
                },
                NewProfileBinding {
                    slot: "psu1".into(),
                    channel: "CH1".into(),
                    preset_id: p.id,
                    sequence_order: 2,
                    delay_ms: 0,
                    output_on: true,
                },
            ],
        })
        .unwrap_err();
    assert_eq!(err.code(), "VALIDATION_FAILED");
}

#[test]
fn a_binding_to_a_missing_preset_is_refused() {
    let s = store();
    let err = s
        .create_profile(&NewDeviceProfile {
            name: "Dangling".into(),
            description: String::new(),
            bindings: vec![NewProfileBinding {
                slot: "psu1".into(),
                channel: "CH1".into(),
                preset_id: 9999,
                sequence_order: 1,
                delay_ms: 0,
                output_on: true,
            }],
        })
        .unwrap_err();
    assert_eq!(err.code(), "VALIDATION_FAILED");
}

#[test]
fn a_failed_profile_create_leaves_nothing_behind() {
    // The insert of the profile row and its bindings is one transaction, so a
    // bad binding must not leave an empty profile lying around.
    let s = store();
    let _ = s.create_profile(&NewDeviceProfile {
        name: "Doomed".into(),
        description: String::new(),
        bindings: vec![NewProfileBinding {
            slot: "psu1".into(),
            channel: "CH1".into(),
            preset_id: 9999,
            sequence_order: 1,
            delay_ms: 0,
            output_on: true,
        }],
    });
    assert!(s.profiles().unwrap().is_empty());
}

#[test]
fn deleting_a_preset_in_use_is_refused() {
    // Cascading here would silently gut the profile that depends on it.
    let s = store();
    let p = s.create_preset(&preset("3v3", 3.3)).unwrap();
    s.create_profile(&NewDeviceProfile {
        name: "Board".into(),
        description: String::new(),
        bindings: vec![NewProfileBinding {
            slot: "psu1".into(),
            channel: "CH1".into(),
            preset_id: p.id,
            sequence_order: 1,
            delay_ms: 0,
            output_on: true,
        }],
    })
    .unwrap();

    let err = s.delete_preset(p.id).unwrap_err();
    assert_eq!(err.code(), "PRESET_IN_USE");
    assert!(err.to_string().contains("3v3"));
    assert_eq!(s.presets().unwrap().len(), 1, "must still be there");
}

#[test]
fn deleting_a_profile_releases_its_presets() {
    let s = store();
    let p = s.create_preset(&preset("3v3", 3.3)).unwrap();
    let profile = s
        .create_profile(&NewDeviceProfile {
            name: "Board".into(),
            description: String::new(),
            bindings: vec![NewProfileBinding {
                slot: "psu1".into(),
                channel: "CH1".into(),
                preset_id: p.id,
                sequence_order: 1,
                delay_ms: 0,
                output_on: true,
            }],
        })
        .unwrap();

    s.delete_profile(profile.id).unwrap();
    s.delete_preset(p.id).expect("no longer in use");
}

#[test]
fn updating_a_profile_replaces_its_bindings_wholesale() {
    let s = store();
    let a = s.create_preset(&preset("a", 1.0)).unwrap();
    let b = s.create_preset(&preset("b", 2.0)).unwrap();
    let profile = s
        .create_profile(&NewDeviceProfile {
            name: "Board".into(),
            description: String::new(),
            bindings: vec![NewProfileBinding {
                slot: "psu1".into(),
                channel: "CH1".into(),
                preset_id: a.id,
                sequence_order: 1,
                delay_ms: 0,
                output_on: true,
            }],
        })
        .unwrap();

    let updated = s
        .update_profile(
            profile.id,
            &NewDeviceProfile {
                name: "Board".into(),
                description: "now on CH2".into(),
                bindings: vec![NewProfileBinding {
                    slot: "psu1".into(),
                    channel: "CH2".into(),
                    preset_id: b.id,
                    sequence_order: 1,
                    delay_ms: 0,
                    output_on: true,
                }],
            },
        )
        .unwrap();

    assert_eq!(updated.bindings.len(), 1);
    assert_eq!(updated.bindings[0].channel.to_string(), "CH2");
    assert_eq!(updated.bindings[0].preset.name, "b");
}

#[test]
fn missing_rows_report_not_found() {
    let s = store();
    assert_eq!(s.preset(42).unwrap_err().code(), "NOT_FOUND");
    assert_eq!(s.profile(42).unwrap_err().code(), "NOT_FOUND");
    assert_eq!(s.delete_profile(42).unwrap_err().code(), "NOT_FOUND");
}

#[test]
fn accepts_a_trip_point_above_the_value_it_protects() {
    // The case that used to be refused on apply. OVP at 5.5 V on a 5 V rail is
    // the instrument's backstop; a channel limited to 5 V could otherwise carry
    // no protection at all.
    let s = store();
    let mut p = preset("generic 5v", 5.0);
    p.curr_limit = 0.5;
    p.ovp_level = 5.5;
    p.ovp_enabled = true;
    p.ocp_level = 0.55;
    p.ocp_enabled = true;
    assert!(s.create_preset(&p).is_ok());
}

#[test]
fn rejects_over_voltage_protection_that_would_trip_on_itself() {
    // Below its own setpoint, the trip fires the moment the rail comes up —
    // worse than leaving protection off.
    let s = store();
    let mut p = preset("self-tripping", 5.0);
    p.ovp_level = 4.5;
    p.ovp_enabled = true;
    let err = s.create_preset(&p).unwrap_err();
    assert_eq!(err.code(), "VALIDATION_FAILED");
    assert!(err.to_string().contains("as soon as"), "got: {err}");
}

#[test]
fn rejects_over_current_protection_that_would_trip_on_itself() {
    let s = store();
    let mut p = preset("self-tripping", 5.0);
    p.curr_limit = 2.0;
    p.ocp_level = 1.0;
    p.ocp_enabled = true;
    assert!(s.create_preset(&p).is_err());
}

#[test]
fn ignores_a_low_trip_point_that_is_switched_off() {
    let s = store();
    let mut p = preset("off", 5.0);
    p.ovp_level = 1.0;
    p.ovp_enabled = false;
    assert!(s.create_preset(&p).is_ok());
}
