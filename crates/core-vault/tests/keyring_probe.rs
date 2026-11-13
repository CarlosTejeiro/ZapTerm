//! Real OS keyring round trip. Ignored: needs a keyring service (Secret
//! Service on Linux). Run with `cargo test -p core-vault --test keyring_probe -- --ignored`.
#[test]
#[ignore = "needs a real OS keyring service"]
fn keyring_round_trip_persists() {
    let key = format!("zapterm-test:{}", std::process::id());
    core_vault::Vault::store(&key, "s3cret").expect("store");
    assert_eq!(core_vault::Vault::retrieve(&key).unwrap(), "s3cret");
    core_vault::Vault::delete(&key).expect("delete");
    assert!(core_vault::Vault::retrieve(&key).is_err());
}
