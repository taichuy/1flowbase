use crate::test_support::support::build_official_provider_package;
use sha2::{Digest, Sha256};

#[test]
fn official_provider_package_is_deterministic_across_file_timestamps() {
    let first = build_official_provider_package("0.2.0");
    std::thread::sleep(std::time::Duration::from_millis(1_100));
    let second = build_official_provider_package("0.2.0");

    assert_eq!(Sha256::digest(first), Sha256::digest(second));
}
