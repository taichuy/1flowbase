//! Frozen data-model-definition HTTP request DTOs and concrete Axum decode boundary.
mod body;
mod query;
mod requests;

pub use requests::*;

#[cfg(test)]
#[path = "_tests/extractor_parity.rs"]
mod extractor_parity;
