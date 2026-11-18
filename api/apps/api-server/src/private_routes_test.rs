#![recursion_limit = "256"]

extern crate self as api_server;

include!("lib.rs");

#[cfg(test)]
#[path = "_tests/private_root_support.rs"]
mod _tests;
