fn private_anchor() -> u8 {
    7
}

#[cfg_attr(test, api_test_selection::group(core, contracts, public_api, routes))]
#[path = "external.rs"]
mod external;

#[cfg_attr(test, api_test_selection::group(core, contracts, public_api, routes))]
mod inline {
    pub(super) fn check_private_parent() {
        assert_eq!(super::private_anchor(), 7);
    }
}

#[cfg_attr(test, api_test_selection::group(core, contracts))]
fn shared_core_contracts() -> u8 {
    11
}

#[cfg_attr(test, api_test_selection::fixture(core))]
struct CoreFixture {
    owner_field: u8,
}

#[cfg_attr(test, api_test_selection::fixture(contracts, public_api, routes))]
struct OtherFixture {
    owner_field: u8,
}

pub(super) fn verify_shared_boundaries() {
    external::check_private_parent();
    inline::check_private_parent();
    // Every root must retain both fixture types. On non-owners, these field
    // reads are deliberately absent so deny(dead_code) checks the narrow lint.
    assert!(std::mem::size_of::<CoreFixture>() > 0);
    assert!(std::mem::size_of::<OtherFixture>() > 0);
}

#[cfg_attr(test, api_test_selection::group(core))]
pub(super) fn verify_owner_fixture() {
    assert_eq!(CoreFixture { owner_field: 13 }.owner_field, 13);
}

#[cfg_attr(test, api_test_selection::group(contracts, public_api, routes))]
pub(super) fn verify_owner_fixture() {
    assert_eq!(OtherFixture { owner_field: 17 }.owner_field, 17);
}

#[cfg_attr(test, api_test_selection::group(core))]
#[test]
pub(super) fn case_core() {
    assert!(matches!(env!("CARGO_CRATE_NAME"), "api_server"));
    assert_eq!(shared_core_contracts(), 11);
}

#[cfg_attr(test, api_test_selection::group(contracts))]
#[test]
pub(super) fn case_contracts() {
    assert!(matches!(env!("CARGO_CRATE_NAME"), "api_private_contracts"));
    assert_eq!(shared_core_contracts(), 11);
}

#[cfg_attr(test, api_test_selection::group(public_api))]
#[test]
pub(super) fn case_public_api() {
    assert!(matches!(env!("CARGO_CRATE_NAME"), "api_public_api"));
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[test]
pub(super) fn case_routes() {
    assert!(matches!(env!("CARGO_CRATE_NAME"), "api_private_routes"));
}

#[cfg_attr(test, api_test_selection::group(core, contracts))]
#[test]
pub(super) fn case_core_contracts() {
    assert!(matches!(
        env!("CARGO_CRATE_NAME"),
        "api_server" | "api_private_contracts"
    ));
    assert_eq!(shared_core_contracts(), 11);
}

#[cfg_attr(test, api_test_selection::group(core, public_api))]
#[test]
pub(super) fn case_core_public_api() {
    assert!(matches!(
        env!("CARGO_CRATE_NAME"),
        "api_server" | "api_public_api"
    ));
}

#[cfg_attr(test, api_test_selection::group(core, routes))]
#[test]
pub(super) fn case_core_routes() {
    assert!(matches!(
        env!("CARGO_CRATE_NAME"),
        "api_server" | "api_private_routes"
    ));
}

#[cfg_attr(test, api_test_selection::group(contracts, public_api))]
#[test]
pub(super) fn case_contracts_public_api() {
    assert!(matches!(
        env!("CARGO_CRATE_NAME"),
        "api_private_contracts" | "api_public_api"
    ));
}

#[cfg_attr(test, api_test_selection::group(contracts, routes))]
#[test]
pub(super) fn case_contracts_routes() {
    assert!(matches!(
        env!("CARGO_CRATE_NAME"),
        "api_private_contracts" | "api_private_routes"
    ));
}

#[cfg_attr(test, api_test_selection::group(public_api, routes))]
#[test]
pub(super) fn case_public_api_routes() {
    assert!(matches!(
        env!("CARGO_CRATE_NAME"),
        "api_public_api" | "api_private_routes"
    ));
}

#[cfg_attr(test, api_test_selection::group(core, contracts, public_api))]
#[test]
pub(super) fn case_core_contracts_public_api() {
    assert!(matches!(
        env!("CARGO_CRATE_NAME"),
        "api_server" | "api_private_contracts" | "api_public_api"
    ));
}

#[cfg_attr(test, api_test_selection::group(core, contracts, routes))]
#[test]
pub(super) fn case_core_contracts_routes() {
    assert!(matches!(
        env!("CARGO_CRATE_NAME"),
        "api_server" | "api_private_contracts" | "api_private_routes"
    ));
}

#[cfg_attr(test, api_test_selection::group(core, public_api, routes))]
#[test]
pub(super) fn case_core_public_api_routes() {
    assert!(matches!(
        env!("CARGO_CRATE_NAME"),
        "api_server" | "api_public_api" | "api_private_routes"
    ));
}

#[cfg_attr(test, api_test_selection::group(contracts, public_api, routes))]
#[test]
pub(super) fn case_contracts_public_api_routes() {
    assert!(matches!(
        env!("CARGO_CRATE_NAME"),
        "api_private_contracts" | "api_public_api" | "api_private_routes"
    ));
}

#[cfg_attr(test, api_test_selection::group(core, contracts, public_api, routes))]
#[test]
pub(super) fn case_all() {
    assert!(matches!(
        env!("CARGO_CRATE_NAME"),
        "api_server" | "api_private_contracts" | "api_public_api" | "api_private_routes"
    ));
}
