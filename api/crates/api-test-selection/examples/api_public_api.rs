#![deny(dead_code)]

#[cfg(test)]
#[path = "support/cases.rs"]
mod cases;

#[cfg(not(test))]
fn main() {}

#[test]
fn expected_cases_and_private_bindings_are_present() {
    cases::case_public_api();
    cases::case_core_public_api();
    cases::case_contracts_public_api();
    cases::case_public_api_routes();
    cases::case_core_contracts_public_api();
    cases::case_core_public_api_routes();
    cases::case_contracts_public_api_routes();
    cases::case_all();
    cases::verify_shared_boundaries();
    cases::verify_owner_fixture();
}
