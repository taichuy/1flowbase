//! Minimal author-side helpers for versioned RuntimeExtension Host Services.
//!
//! This crate contains no Host, storage, process or transport implementation. A worker owns the
//! reader/writer and explicitly passes them to [`PluginDataClient`].

mod managed_hook;
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
pub use managed_hook::{
    serve_managed_hook, serve_managed_interface_hook, serve_managed_interface_reference_hook,
};
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
