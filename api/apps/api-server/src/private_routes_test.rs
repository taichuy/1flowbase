#![recursion_limit = "256"]

extern crate self as api_server;

// Compile the same private production module tree with only route tests mounted.
macro_rules! api_route_tests {
    ($($item:item)*) => { $($item)* };
}
macro_rules! api_nonroute_tests {
    ($($item:item)*) => {};
}

// A shared test-only variant/field keeps its original shape in both roots.
// Only its non-owner test mirror suppresses dead-code diagnostics on this item.
macro_rules! api_test_fixture {
    (routes; $($item:item)*) => { $($item)* };
    (nonroutes; $($item:item)*) => {
        $(#[cfg_attr(test, allow(dead_code))] $item)*
    };
}

include!("lib.rs");

#[cfg(test)]
mod _tests {
    pub(crate) mod support {
        pub(crate) use crate::test_support::support::*;
    }
    pub(crate) use crate::native_settings_fixture::NativeSettingsFixture;
}
