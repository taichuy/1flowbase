#![deny(dead_code)]

#[cfg(test)]
#[path = "support/cases.rs"]
mod cases;

#[cfg(not(test))]
fn main() {}

#[test]
fn expected_cases_and_private_bindings_are_present() {
    cases::case_contracts();
    cases::case_core_contracts();
    cases::case_contracts_public_api();
    cases::case_contracts_routes();
    cases::case_core_contracts_public_api();
    cases::case_core_contracts_routes();
    cases::case_contracts_public_api_routes();
    cases::case_all();
    cases::verify_shared_boundaries();
    cases::verify_owner_fixture();
}
