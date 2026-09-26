//! Minimal author-side helpers for versioned RuntimeExtension Host Services.
//!
//! This crate contains typed clients and a bounded, versioned stdio dispatcher.
//! It owns no Host registry, process supervisor, storage or business routing.

mod managed_hook;
mod multiplex;
mod plugin_data;
mod simulator;

pub use extension_contracts::{
    ManagedCreateHookInput, ManagedCreateView, ManagedHookHostFrame, ManagedHookOutcome,
    ManagedHookTerminal,
};
pub use extension_contracts::{
    ManagedInterfaceHostFrame, ManagedInterfaceInput, ManagedInterfaceView,
    ManagedProjectionContract,
};
pub use extension_contracts::{
    ManagedInterfaceReferenceHostFrame, ManagedInterfaceReferenceInput,
    ManagedInterfaceReferenceView, ManagedInterfaceReferenceWorkerFrame,
    ManagedProjectionReference,
};
pub use extension_contracts::{
    MultiplexEnvelope, MultiplexHostMessage, MultiplexHostService, MultiplexWorkerMessage,
    MULTIPLEX_CALL_EVENT_BUDGET_BYTES, MULTIPLEX_MAX_FRAME_BYTES, MULTIPLEX_OUTPUT_BUDGET_BYTES,
    STDIO_JSON_MULTIPLEX_V1,
};
pub use managed_hook::{
    serve_managed_hook, serve_managed_interface_hook, serve_managed_interface_reference_hook,
};
pub use multiplex::{serve, serve_io, MultiplexEmitter, MultiplexError};
pub use plugin_data::{PluginDataClient, RuntimeExtensionSdkError};
pub use simulator::PluginDataHostSimulator;

pub use extension_contracts::{
    PluginDataError, PluginDataFilter, PluginDataFilterOperator, PluginDataOperation,
    PluginDataOperationResult, PluginDataOrder, PluginDataOrderDirection, PluginDataPage,
    PluginDataRequest, PluginDataResponse, PluginDataRow, PluginDataTarget, PluginDataValue,
};

#[cfg(test)]
mod _tests;

mod managed_event;
pub use extension_contracts::{
    ManagedEventHostFrame, ManagedEventOutcome, ManagedEventPayload, ManagedEventPublication,
    ManagedEventSchema, ManagedEventStatus,
};
pub use managed_event::serve_managed_event;
