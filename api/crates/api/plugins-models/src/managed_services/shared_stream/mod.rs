//! Process-owned shared producers. Runtime workers never supply identity or cache keys.
mod authority;
mod broker;
mod cache;
mod host;
mod snapshot;
mod subscribers;

pub(super) use authority::authorize_frame;
pub(super) use broker::global;
pub(super) use snapshot::{snapshot, SnapshotLookup};
pub(super) use subscribers::subscribe;

#[cfg(test)]
#[path = "../_tests/shared_stream/mod.rs"]
mod tests;
