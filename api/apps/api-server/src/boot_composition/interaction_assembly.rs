//! Stateless adapters into the existing business owners.
use std::sync::Arc;
use crate::error_response::ApiError;
pub(crate) struct OfficialPluginInstallAdapter;
#[async_trait::async_trait]
impl api_host_kernel::plugin_transport::OfficialPluginInstallResolver for OfficialPluginInstallAdapter {
 async fn resolve(&self, source: &dyn api_host_kernel::official_extension_catalog::OfficialExtensionCatalogSourcePort, actor_user_id: uuid::Uuid, workspace_id: uuid::Uuid, body: api_host_kernel::plugin_transport::InstallOfficialPluginBody) -> Result<control_plane::plugin_management::InstallResolvedOfficialPluginCommand,ApiError> {
  api_plugins_models::routes::plugins::resolve_official_install_command(source,actor_user_id,workspace_id,body).await
 }
}
pub(crate) struct RuntimeEventSseAdapter;
impl api_host_kernel::debug_stream_contract::RuntimeEventSsePort for RuntimeEventSseAdapter {
 fn websocket_send(self: Arc<Self>, stream: Arc<dyn control_plane::ports::RuntimeEventStream>, backfill: Arc<dyn api_host_kernel::debug_stream_contract::RuntimeEventBackfillSource>, run_id: uuid::Uuid, from_sequence: Option<i64>, sender: tokio::sync::mpsc::Sender<serde_json::Value>) -> std::pin::Pin<Box<dyn std::future::Future<Output=()>+Send+'static>> {
  Box::pin(api_applications::routes::debug_run_stream::send_runtime_event_websocket_stream(stream,backfill,run_id,from_sequence,sender))
 }
 fn send(self: Arc<Self>, stream: Arc<dyn control_plane::ports::RuntimeEventStream>, backfill: Arc<dyn api_host_kernel::debug_stream_contract::RuntimeEventBackfillSource>, run_id: uuid::Uuid, from_sequence: Option<i64>, sender: tokio::sync::mpsc::Sender<Result<axum::response::sse::Event,std::convert::Infallible>>) -> std::pin::Pin<Box<dyn std::future::Future<Output=()>+Send+'static>> {
  Box::pin(api_applications::routes::debug_run_stream::send_runtime_event_stream(stream,backfill,run_id,from_sequence,sender))
 }
}
pub(crate) struct ApiEventDescriptors;
#[async_trait::async_trait]
impl api_host_kernel::event_descriptor_ports::ApiEventDescriptorPort for ApiEventDescriptors {
 async fn workflow(&self, dependencies: &api_host_kernel::openapi_interface::OpenApiCapabilityCatalogDependencies, actor: &domain::ActorContext, entry: &api_host_kernel::openapi_interface::OpenApiCapabilityCatalogEntry) -> Result<Option<interface_runtime::DocumentOperationDescriptor>,ApiError> {
  api_public_gateway::routes::application_public_api::workflow_extension_interface::document_descriptor_for_entry(dependencies,actor,entry).await
 }
 async fn runtime_model(&self, dependencies: &api_host_kernel::openapi_interface::OpenApiCapabilityCatalogDependencies, actor: &domain::ActorContext, entry: &api_host_kernel::openapi_interface::OpenApiCapabilityCatalogEntry) -> Result<Option<interface_runtime::DocumentOperationDescriptor>,ApiError> {
  api_plugins_models::routes::runtime_models::event_projection::document_descriptor_for_entry(dependencies,actor,entry).await
 }
}
