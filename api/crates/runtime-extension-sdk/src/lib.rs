//! Minimal author-side helpers for versioned RuntimeExtension Host Services.
//!
//! This crate contains no Host, storage, process or transport implementation. A worker owns the
//! reader/writer and explicitly passes them to [`PluginDataClient`].

mod plugin_data;
mod simulator;

pub use plugin_data::{PluginDataClient, RuntimeExtensionSdkError};
pub use simulator::PluginDataHostSimulator;

pub use extension_contracts::{
    PluginDataError, PluginDataFilter, PluginDataFilterOperator, PluginDataOperation,
    PluginDataOperationResult, PluginDataOrder, PluginDataOrderDirection, PluginDataPage,
    PluginDataRequest, PluginDataResponse, PluginDataRow, PluginDataTarget, PluginDataValue,
};

#[cfg(test)]
mod _tests;
