mod capability_catalog;
mod catalog;
pub mod contract;
mod dispatcher;
pub mod json_contract;
pub mod schema_projection;

pub use capability_catalog::{
    build_openapi_capability_catalog, get_openapi_capability, get_openapi_capability_by_route,
    operation_risk_level, query_openapi_capability_catalog, ActivatedInterfaceOperationProjection,
    OpenApiCapabilityCatalogEntry, OpenApiCapabilityCatalogPage, OpenApiCapabilityCatalogQuery,
    OpenApiCapabilityCatalogSummary, OpenApiCapabilitySource,
};
pub use capability_catalog::{
    build_openapi_capability_catalog_with, get_openapi_capability_by_route_with,
    get_openapi_capability_index_entry_with, get_openapi_capability_with,
    list_openapi_capability_index_with, query_openapi_capability_catalog_with,
    root_workspace_only_capability_ids, OpenApiCapabilityCatalogDependencies,
    OpenApiCapabilityIndexEntry,
};
pub use catalog::catalog_entry_from_view;
pub use catalog::{
    catalog_entry_from_operation, OpenApiInterfaceCatalogEntry, OpenApiParameterDescriptor,
    OpenApiParameterLocation,
};
pub use dispatcher::{
    console_router_callable_dispatch_port, dispatch, dispatch_with_console_router,
    CallableDispatchError, CallableDispatchForwarding, CallableDispatchHeader,
    CallableDispatchHttpResponse, CallableDispatchPort, CallableDispatchResult, DispatchArguments,
    DispatchError, DispatchSuccess,
};

pub use catalog::close_api_event_schema;

pub mod otlp_document;
