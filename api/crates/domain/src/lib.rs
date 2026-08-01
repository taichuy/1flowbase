extern crate self as domain;

pub mod ai_native_operation;
pub mod application;
pub mod audit;
pub mod auth;
pub mod base;
pub mod builtin_data_model;
pub mod console_policy;
pub mod data_source;
pub mod extension_installation;
pub mod file_management;
pub mod flow;
pub mod frontend_block_catalog;
pub mod frontstage;
pub mod host_extension;
pub mod i18n_catalog;
pub mod js_dependency;
pub mod mcp_bundle;
pub mod mcp_management;
pub mod model_provider;
pub mod modeling;
pub mod node_contribution;
pub mod orchestration;
pub mod plugin_worker;
pub mod resource;
pub mod resource_filter;
pub mod runtime_observability;
pub mod scope;
pub mod system_defaults;

pub use ai_native_operation::{AiNativeCompactProfile, AiNativeGenerateProfile, AiNativeOperation};
pub use application::{
    ApplicationApiSection, ApplicationEnvironmentVariable, ApplicationLogsSection,
    ApplicationMonitoringSection, ApplicationOrchestrationSection, ApplicationPublicationStatus,
    ApplicationRecord, ApplicationSections, ApplicationTag, ApplicationTagCatalogEntry,
    ApplicationType, WorkflowTriggerType,
};
pub use audit::AuditLogRecord;
pub use auth::{
    password_local_contact_identity_claims, password_local_identity_claims, ActorContext,
    ApiKeyKind, ApiKeyRecord, AuthenticatorRecord, BoundRole, ExternalIdentityClaim,
    PermissionDefinition, RoleScopeKind, RoleTemplate, SessionRecord, UserAuthIdentity, UserRecord,
    UserStatus, AUTH_SUBJECT_TYPE_ACCOUNT, AUTH_SUBJECT_TYPE_EMAIL, AUTH_SUBJECT_TYPE_PHONE,
    PASSWORD_LOCAL_AUTHENTICATOR_ID,
};
pub use base::BaseFields;
pub use builtin_data_model::{
    builtin_contract_for_model, builtin_data_model_contract, data_model_capabilities,
    data_model_field_capabilities, BuiltinDataModelContract, BuiltinDataModelFieldContract,
    BuiltinDataModelKind, DataModelCapabilities, DataModelFieldCapabilities,
    DataModelFieldOwnership, DataModelRecordCapabilities,
};
pub use console_policy::{
    effective_console_row_scope, effective_console_simple_operation, ConsoleOperationId,
    ConsoleOperationPolicy, ConsoleOperationRowScope, ConsolePolicyGroup, ConsolePolicyGroupId,
    ConsolePolicyGroupKind, ConsolePolicyIdentifierError, ConsolePolicyMode, ConsolePolicyStrategy,
    RoleConsoleGroupPolicy, RoleConsolePolicy,
};
pub use data_source::{
    data_source_secret_ref, DataSourceCatalogCacheRecord, DataSourceCatalogRefreshStatus,
    DataSourceDefaults, DataSourceInstanceRecord, DataSourceInstanceStatus,
    DataSourcePreviewSessionRecord, DataSourceSecretRecord,
};
pub use extension_installation::{
    ExtensionCatalogIdentity, ExtensionCategory, ExtensionCompatibilityWarning,
    ExtensionInstallationIdentity, ExtensionInstallationReceipt, ExtensionInstallationRecord,
    ExtensionInstallationStatus, ExtensionIntegrityWarning, ExtensionRiskChallenge,
    ExtensionSignatureStatus,
};
pub use file_management::{
    FileStorageHealthStatus, FileStorageRecord, FileTableRecord, FileTableScopeKind,
};
pub use flow::{
    default_flow_document, default_flow_document_for_application, FlowChangeKind, FlowDraftRecord,
    FlowEditorState, FlowRecord, FlowVersionRecord, FlowVersionTrigger,
    FLOW_AUTOSAVE_INTERVAL_SECONDS, FLOW_HISTORY_LIMIT, FLOW_SCHEMA_VERSION,
    FLOW_USER_PROTECTION_LIMIT, WORKFLOW_SYNC_TIMEOUT_MS,
};
pub use frontend_block_catalog::{
    FrontendBlockCatalogEntry, FrontendBlockCodeModule, FrontendBlockContextContract,
    FrontendBlockPermissions, FrontendComponentContract, FrontendComponentExample,
    FrontendComponentProp, FrontendComponentUpstream, FrontendModuleAsset, FrontendModuleAssetRole,
    FrontendModuleBinding,
};
pub use frontstage::{FrontstagePageKind, FrontstagePageRecord, FrontstagePageTreeNode};
pub use host_extension::{
    HostExtensionActivationStatus, HostExtensionInventoryRecord, HostExtensionTrustLevel,
    HostInfrastructureConfigStatus, HostInfrastructureProviderConfigRecord,
};
pub use i18n_catalog::{
    ActiveOfficialCatalogMessage, CatalogDigest, CatalogLocale, CatalogMessageIdentity,
    CatalogSeedFile, CatalogTranslation, CatalogVersion, I18nCatalogInvariantError,
    ObsoleteCatalogMessage, OfficialCatalogMessage, VerifiedCatalogRelease,
    WorkspaceCatalogRevision, WorkspaceCatalogState, I18N_CATALOG_SEED_SCHEMA_VERSION,
    I18N_CATALOG_SOURCE_LOCALE,
};
pub use js_dependency::{
    ApplicationJsDependencySelection, JsDependencyPermissions, JsDependencyRegistryEntry,
};
pub use mcp_bundle::{
    McpBundleFile, McpBundleFileKind, McpBundleGroup, McpBundleImportReport, McpBundleInstance,
    McpBundleInstanceDiscoveryPolicy, McpBundleItemReport, McpBundleManifest, McpBundlePackage,
    McpBundlePreview, McpBundleTool, McpBundleToolBinding, McpBundleUpstreamConnection,
    McpBundleVersionStatus, MCP_BUNDLE_SCHEMA_VERSION,
};
pub use mcp_management::{
    McpCallToolResult, McpCatalogSnapshot, McpDescriptionCheckResult, McpExportPackage,
    McpFieldMapping, McpGroupRecord, McpInstanceDiscoveryPolicyRecord, McpInstanceRecord,
    McpInstanceStatus, McpInterfaceCatalogEntry, McpListItemKind, McpListItemSummary, McpRiskLevel,
    McpToolAvailabilityStatus, McpToolBindingRecord, McpToolExecutionTarget, McpToolRecord,
    McpToolStatus, McpUpstreamAuthType, McpUpstreamConnectionRecord, McpUpstreamConnectionStatus,
    McpUpstreamSourceStatus, McpUpstreamToolSourceRecord, McpUpstreamTransport,
};
pub use model_provider::{
    ModelCatalogSyncRunRecord, ModelFailoverQueueItemRecord, ModelFailoverQueueSnapshotRecord,
    ModelFailoverQueueTemplateRecord, ModelProviderCatalogCacheRecord,
    ModelProviderCatalogEntryRecord, ModelProviderCatalogRefreshStatus, ModelProviderCatalogSource,
    ModelProviderCatalogSourceRecord, ModelProviderConfiguredModel, ModelProviderDiscoveryMode,
    ModelProviderDistributionRule, ModelProviderInstanceRecord, ModelProviderInstanceStatus,
    ModelProviderMainInstanceRecord, ModelProviderMainModelRoutingPolicy,
    ModelProviderMainModelRoutingPolicyRecord, ModelProviderPreviewSessionRecord,
    ModelProviderSecretRecord, ModelProviderValidationStatus, PluginArtifactInstanceRecord,
    PluginArtifactInstanceStatus, PluginArtifactStatus, PluginAssignmentRecord,
    PluginAvailabilityStatus, PluginDesiredState, PluginInstallationRecord,
    PluginPackageCatalogProjectionRecord, PluginPackageCatalogProjectionStatus,
    PluginRuntimeStatus, PluginTaskKind, PluginTaskRecord, PluginTaskStatus,
    PluginVerificationStatus,
};
pub use modeling::{
    DataModelAdvisorFinding, DataModelAdvisorSeverity, DataModelOwnerKind, DataModelProtection,
    DataModelScopeKind, DataModelSourceKind, DataModelStatus, MetadataAvailabilityStatus,
    ModelDefinitionRecord, ModelFieldKind, ModelFieldRecord, RoleDataModelPolicyRecord,
    RoleDataPolicyRecord, RoleDataPolicyScope, ScopeDataModelGrantRecord,
    ScopeDataModelPermissionProfile,
};
pub use node_contribution::{NodeContributionDependencyStatus, NodeContributionRegistryEntry};
pub use orchestration::{
    ApplicationConversationRunSummary, ApplicationRunConversationMessageItem, ApplicationRunDetail,
    ApplicationRunLogSummary, ApplicationRunStitchedTrace, ApplicationRunSubagentTrace,
    ApplicationRunSummary, ApplicationRunTraceNodeContentRecord, ApplicationRunTraceNodeRecord,
    ApplicationRunTraceProjectionDiagnostic, ApplicationRunTraceProjectionStatus,
    ApplicationRunTraceProjectionStatusRecord, CallbackTaskRecord, CallbackTaskStatus,
    CheckpointRecord, CompiledPlanRecord, DataModelSideEffectReceiptRecord,
    FlowRunCallbackResumeAttemptRecord, FlowRunCallbackResumeAttemptStatus, FlowRunExecutionStage,
    FlowRunInvocationContext, FlowRunInvocationSource, FlowRunMode, FlowRunPrincipal,
    FlowRunPrincipalKind, FlowRunRecord, FlowRunStatus, NodeDebugPreviewResult, NodeLastRun,
    NodeRunRecord, NodeRunStatus, RunEventRecord, RuntimeDebugArtifactRecord,
};
pub use plugin_worker::{PluginWorkerLeaseRecord, PluginWorkerStatus};
pub use resource::runtime_model_resource_code;
pub use resource_filter::{ResourceFilterExpr, ResourceFilterOperator};
pub use runtime_observability::{
    AuditHashRecord, BillingSessionRecord, BillingSessionStatus, CapabilityInvocationRecord,
    ContextProjectionRecord, CostLedgerRecord, CreditLedgerRecord,
    ModelFailoverAttemptLedgerRecord, RuntimeEventDurability, RuntimeEventLayer,
    RuntimeEventRecord, RuntimeEventSource, RuntimeEventVisibility, RuntimeItemKind,
    RuntimeItemRecord, RuntimeItemStatus, RuntimeSpanKind, RuntimeSpanRecord, RuntimeSpanStatus,
    RuntimeTrustLevel, UsageLedgerRecord, UsageLedgerStatus,
};
pub use scope::{ScopeContext, TenantRecord, WorkspaceRecord, DEFAULT_SCOPE_ID, SYSTEM_SCOPE_ID};
pub use system_defaults::{
    DefaultUpgradePolicy, DEFAULT_AUTO_INCLUDE_NEW_PROVIDER_INSTANCES,
    DEFAULT_CODE_ISOLATION_TIMEOUT_MS,
};

pub fn crate_name() -> &'static str {
    "domain"
}

#[cfg(test)]
mod _tests;

#[cfg(test)]
mod attribution_exports_tests {
    #[test]
    fn attribution_types_are_available_from_the_domain_root() {
        let principal = crate::FlowRunPrincipal {
            kind: crate::FlowRunPrincipalKind::UserApiKey,
            id: Some(uuid::Uuid::now_v7()),
            display_name: None,
        };
        let context = crate::FlowRunInvocationContext {
            execution_stage: crate::FlowRunExecutionStage::Published,
            invocation_source: crate::FlowRunInvocationSource::WorkflowHttp,
            principal,
        };

        assert_eq!(
            context.principal.kind,
            crate::FlowRunPrincipalKind::UserApiKey
        );
    }
}
