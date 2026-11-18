// Reuse the ordinary api_server library; keep the original test leaves in src/_tests.
#[path = "../src/_tests/host_extensions/host_extensions_builtin_tests.rs"]
mod host_extensions_builtin_tests;
#[path = "../src/_tests/host_extensions/host_route_registry_tests.rs"]
mod host_route_registry_tests;
#[path = "../src/_tests/host_extensions/host_worker_registry_tests.rs"]
mod host_worker_registry_tests;
