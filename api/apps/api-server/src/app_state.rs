use std::sync::Arc;

use control_plane::ports::{OfficialPluginSourcePort, RuntimeEventStream, SessionStore};
use runtime_core::runtime_engine::RuntimeEngine;
use storage_durable::MainDurableStore;
use time::OffsetDateTime;

use crate::console_surface_registry::ConsoleSurfaceRegistry;
use crate::host_infrastructure::HostInfrastructureRegistry;
use crate::openapi_docs::ApiDocsRegistry;
use crate::{
    official_agent_flow_templates::OfficialAgentFlowTemplateSourcePort,
    official_mcp_bundles::OfficialMcpBundleSourcePort,
    provider_runtime::ApiRuntimeServices,
    runtime_activity::ApplicationRuntimeActivityTracker,
    runtime_profile_client::{ApiRuntimeProfilePort, PluginRunnerSystemPort},
};

#[derive(Clone)]
pub struct ApiState {
    pub store: MainDurableStore,
    pub infrastructure: Arc<HostInfrastructureRegistry>,
    pub console_surface_registry: Arc<ConsoleSurfaceRegistry>,
    pub file_storage_registry: Arc<storage_object::FileStorageDriverRegistry>,
    pub runtime_engine: Arc<RuntimeEngine>,
    pub provider_runtime: Arc<ApiRuntimeServices>,
    pub process_started_at: OffsetDateTime,
    pub runtime_activity: Arc<ApplicationRuntimeActivityTracker>,
    pub api_runtime_profile: Arc<dyn ApiRuntimeProfilePort>,
    pub plugin_runner_system: Arc<dyn PluginRunnerSystemPort>,
    pub official_plugin_source: Arc<dyn OfficialPluginSourcePort>,
    pub official_agent_flow_template_source: Arc<dyn OfficialAgentFlowTemplateSourcePort>,
    pub official_mcp_bundle_source: Arc<dyn OfficialMcpBundleSourcePort>,
    pub api_node_id: String,
    pub provider_install_root: String,
    pub provider_secret_master_key: String,
    pub host_extension_dropin_root: String,
    pub allow_unverified_filesystem_dropins: bool,
    pub allow_uploaded_host_extensions: bool,
    pub session_store: Arc<dyn SessionStore>,
    pub runtime_event_stream: Arc<dyn RuntimeEventStream>,
    pub api_docs: Arc<ApiDocsRegistry>,
    pub cookie_name: String,
    pub cookie_secure: bool,
    pub session_ttl_days: i64,
    pub bootstrap_workspace_name: String,
}
