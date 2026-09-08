//! The shipped example must parse and validate. An example that would be
//! rejected is worse than none, because it is the thing an operator copies.

use psu_core::Config;

#[test]
fn the_shipped_example_config_is_valid() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../config.example.toml");
    let config = Config::load(path).expect("config.example.toml must be valid");
    assert_eq!(config.supplies.len(), 3, "the example describes the bench");
    for supply in &config.supplies {
        assert_eq!(supply.channels.len(), 3);
    }
}
