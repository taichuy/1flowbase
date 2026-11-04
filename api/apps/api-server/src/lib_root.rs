#![recursion_limit = "256"]

extern crate self as api_server;

// Each standard Cargo test root selects its own private test modules.
macro_rules! api_route_tests {
    ($($item:item)*) => {};
}
macro_rules! api_nonroute_tests {
    ($($item:item)*) => { $($item)* };
}

// A shared test-only variant/field keeps its original shape in both roots.
// Only its non-owner test mirror suppresses dead-code diagnostics on this item.
macro_rules! api_test_fixture {
    (nonroutes; $($item:item)*) => { $($item)* };
    (routes; $($item:item)*) => {
        $(#[cfg_attr(test, allow(dead_code))] $item)*
    };
}

include!("lib.rs");
