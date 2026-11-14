//! Pure OTLP/HTTP Trace projections of immutable imported facts.
//! This module owns no provider, runtime, authentication, network or persistence.
mod attributes;
mod codec;
mod export;
mod facts;
mod observations;
mod traces;

pub use codec::{decode, encode_gzip, encode_json, encode_protobuf};
pub use export::batches_to_traces;
pub use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
pub use traces::traces_to_batches;

#[cfg(test)]
mod _tests;
