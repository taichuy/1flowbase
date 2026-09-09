mod authentication_activation;
mod authentication_invocation;
mod managed_interface;
pub(crate) mod managed_projection;
pub(crate) use authentication_invocation::AuthenticatedInvocation;
mod boot_snapshot;
#[cfg(test)]
pub(crate) use boot_snapshot::DurableHostInfrastructureProvidersViewQuery;
mod input_assembly;
mod managed_activation;
pub(crate) use managed_activation::{
    ManagedExtensionComposition, ManagedWorkspacePublicationSource, ManagedWorkspaceSnapshot,
};
mod interface_contributions;

#[cfg(test)]
pub(crate) use authentication_activation::HostExtensionAuthenticationFactoryCatalog;
pub(crate) use authentication_activation::{
    ApplicationApiKeyAuthenticationCredential, AuthenticationAdapterFactoryBinding,
    AuthenticationAdapterFactoryRegistry, CONSOLE_SESSION_CREDENTIAL_CONTRACT_ID,
    CONSOLE_SESSION_CREDENTIAL_CONTRACT_VERSION, ConsoleAuthenticationCredential,
    ConsoleProtocolAdmission, McpUserApiKeyAuthenticationCredential,
    PublicAuthenticationCredential, RuntimeModelAuthenticationCredential,
    activated_host_authentication, production_host_extension_authentication_factories,
};
pub use boot_snapshot::{
    EFFECTIVE_EXTENSION_PLAN_SCHEMA_V1, EffectiveExtensionPlan, ExtensionBootSnapshot,
    compile_extension_boot_snapshot,
};
pub(crate) use input_assembly::infrastructure_provider_contribution_id;
pub use input_assembly::{
    BOOT_CORE_MODULE_ID, CACHE_STORE_CONTRACT_ID, CACHE_STORE_CONTRACT_VERSION,
    CACHE_STORE_EXTENSION_POINT_ID, DEFAULT_PLUGIN_SET_PATH, ExtensionGraphInputAssembly,
    INTERFACE_AUTHENTICATION_ADAPTER_CONTRACT_ID,
    INTERFACE_AUTHENTICATION_ADAPTER_CONTRACT_VERSION, INTERFACE_AUTHENTICATION_ADAPTER_POINT_ID,
    INTERFACE_COMPLETION_HOOK_CONTRIBUTION_ID, INTERFACE_COMPLETION_HOOK_POINT_ID,
    INTERFACE_LIFECYCLE_HOOK_CONTRACT_ID, INTERFACE_LIFECYCLE_HOOK_CONTRACT_VERSION,
    MODEL_PROVIDER_CONTRACT_ID, MODEL_PROVIDER_CONTRACT_VERSION, MODEL_PROVIDER_EXTENSION_POINT_ID,
    ModuleActivationFact, PROVIDER_INPUT_PIPELINE_CONTRACT_ID,
    PROVIDER_INPUT_PIPELINE_CONTRACT_VERSION, PROVIDER_INPUT_PIPELINE_POINT_ID,
    RUNTIME_EVENT_AFTER_COMMIT_CONTRACT_ID, RUNTIME_EVENT_AFTER_COMMIT_POINT_ID,
    RUNTIME_EVENT_DIAGNOSTIC_CONTRACT_ID, RUNTIME_EVENT_DIAGNOSTIC_POINT_ID,
    RUNTIME_EVENT_LANE_CONTRACT_VERSION, RUNTIME_EVENT_REQUIRED_CONTRACT_ID,
    RUNTIME_EVENT_REQUIRED_POINT_ID, assemble_extension_graph_input,
};
pub(crate) use interface_contributions::{
    InterfaceContributionCollector, production_interface_contributions,
};
