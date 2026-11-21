use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ControlPlaneContractError {
    #[error("not authenticated")]
    NotAuthenticated,
    #[error("permission denied: {0}")]
    PermissionDenied(&'static str),
    #[error("resource not found: {0}")]
    NotFound(&'static str),
    #[error("conflict: {0}")]
    Conflict(&'static str),
    #[error(
        "plugin dependency {provider_publisher_namespace}/{provider_plugin_code}: {code}; resource={resource_code:?}; enabled dependents={dependents:?}"
    )]
    PluginDependency {
        code: String,
        provider_publisher_namespace: String,
        provider_plugin_code: String,
        resource_code: Option<String>,
        dependents: Vec<String>,
    },
    #[error("plugin unavailable")]
    PluginUnavailable,
    #[error("invalid input: {0}")]
    InvalidInput(&'static str),
    #[error("catalog module export is not registered: {module_source}.{export_name}")]
    InvalidComponentModuleExport {
        module_source: String,
        export_name: String,
    },
    #[error("invalid state transition for {resource}: action={action}, from={from}, to={to}")]
    InvalidStateTransition {
        resource: &'static str,
        action: &'static str,
        from: String,
        to: String,
    },
    #[error("upstream unavailable: {0}")]
    UpstreamUnavailable(&'static str),
}
