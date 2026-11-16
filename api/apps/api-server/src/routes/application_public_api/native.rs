pub use api_public_gateway::routes::application_public_api::native::*;
#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
use axum::http::StatusCode;
#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "native/_tests/authentication_failures.rs"]
mod authentication_failure_tests;

#[cfg_attr(test, api_test_selection::group(public_api))]
#[cfg(test)]
#[path = "native/_tests/catalog_admission.rs"]
mod catalog_admission_tests;

#[path = "native/runtime_invoker.rs"]
mod runtime_invoker;
#[cfg_attr(test, api_test_selection::group(contracts, public_api))]
#[cfg(test)]
pub(crate) use api_public_gateway::routes::application_public_api::native::test_support::*;
pub(crate) use runtime_invoker::{
    native_runtime_invoker_factory, NativeRuntimeInvokerFactoryDependencies,
};
