pub use api_host_kernel::openapi_interface::json_contract::*;
#[cfg_attr(test, api_test_selection::group(contracts))]
#[cfg(test)]
#[path = "_tests/json_contract.rs"]
mod tests;
