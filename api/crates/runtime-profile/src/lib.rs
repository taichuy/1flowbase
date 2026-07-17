extern crate self as runtime_profile;

pub mod fingerprint;
pub mod locale;
pub mod metrics;
pub mod profile;

pub use fingerprint::*;
pub use locale::*;
pub use metrics::*;
pub use profile::*;

pub fn crate_name() -> &'static str {
    "runtime-profile"
}

#[cfg(test)]
mod _tests;
