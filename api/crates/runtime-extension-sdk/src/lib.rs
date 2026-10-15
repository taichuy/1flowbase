//! Minimal author-side helpers for versioned RuntimeExtension Host Services.
//!
//! This crate contains typed clients and a bounded, versioned stdio dispatcher.
//! It owns no Host registry, process supervisor, storage or business routing.

mod application_logs;
pub use application_logs::{ApplicationLogsClient, ApplicationLogsClientError};
pub use extension_contracts::{ApplicationLogsRequest, ApplicationLogsResponse};
mod http_egress;
pub use extension_contracts::{
    ApplicationLogsApplication, ApplicationLogsRecord, HostServiceError,
};
pub use extension_contracts::{HttpEgressRequest, HttpEgressResponse};
pub use http_egress::{HttpEgressClient, HttpEgressClientError};
mod managed_hook;
mod multiplex;
mod notification;
mod plugin_data;
mod simulator;

pub use extension_contracts::{
    CapabilityStdioError, CapabilityStdioMethod, CapabilityStdioRequest, CapabilityStdioResponse,
    NotificationAuthorizationActionLocale, NotificationAuthorizationLocale,
    NotificationFieldLocale, NotificationFormLocale, NotificationTypeDescriptor,
    NotificationTypeLocale,
};
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
pub use extension_contracts::{
    NotificationAuthorizationAction, NotificationAuthorizationActionKind,
    NotificationAuthorizationDescriptor, NotificationAuthorizationOperation,
    NotificationAuthorizationStatus, NotificationAuthorizationUserAction,
    NotificationAuthorizationView, NotificationDeliveryWindowStatus,
    NotificationDeliveryWindowView, NotificationFailureCode, NotificationOperation,
    NotificationRuntimeInput, NotificationRuntimeResult, NotificationSendOutcome,
};
pub use managed_hook::{
    serve_managed_hook, serve_managed_interface_hook, serve_managed_interface_reference_hook,
};
pub use multiplex::{
    serve, serve_duplex, serve_duplex_io, serve_io, MultiplexEmitter, MultiplexError,
    MultiplexInput, MultiplexResourceLease,
};
pub use notification::{
    complete_authorizable_notification, execute_authorizable_notification, execute_notification,
    prepare_authorizable_notification,
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

mod plugin_credential;
pub use extension_contracts::{
    PluginCredentialError, PluginCredentialRequest, PluginCredentialResponse,
};
pub use plugin_credential::{PluginCredentialClient, PluginCredentialClientError};

mod file_import;
pub use extension_contracts::{FileImportError, FileImportRequest, FileImportResponse};
pub use file_import::{FileImportClient, FileImportClientError};

mod file_read;
pub use extension_contracts::{FileReadError, FileReadMetadata};
pub use file_read::{FileReadClient, FileReadClientError, FileReader, TransferReport};
