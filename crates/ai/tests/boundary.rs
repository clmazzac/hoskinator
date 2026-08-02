//! Holds the ADR-0005 dependency boundary open.
//!
//! `hoskinator-ai` may use `hoskinator-core`. Cargo rejects the reverse, so only this direction
//! needs asserting.

#[test]
fn the_ai_crate_can_use_core_types() {
    let profile = hoskinator_core::profile::Profile::default();

    assert!(profile.name.is_none());
}
