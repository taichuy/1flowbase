use crate::routes::mcp_protocol::virtual_ui::RuntimeInternalToolInvokerFactory;
use api_host_kernel::{
    app_state::ApiState,
    error_response::ApiError,
    runtime_ports::{
        CommunicationBindingValidator, RuntimeDebugMcpPort, RuntimeToolInvokerFactoryHandle,
        TraceProjectionRebuildPort,
    },
};
use std::sync::Arc;

pub(crate) struct RuntimeDebugMcpAdapter;
#[async_trait::async_trait]
impl RuntimeDebugMcpPort for RuntimeDebugMcpAdapter {
    async fn for_actor(
        &self,
        state: &Arc<ApiState>,
        actor: &domain::ActorContext,
    ) -> Result<Arc<dyn RuntimeToolInvokerFactoryHandle>, ApiError> {
        Ok(Arc::new(
            crate::runtime_internal_tool_invoker_factory(state, actor).await?,
        ))
    }
}
#[async_trait::async_trait]
impl RuntimeToolInvokerFactoryHandle for RuntimeInternalToolInvokerFactory {
    async fn create_server_delegated(
        &self,
        headers: axum::http::HeaderMap,
        actor: domain::ActorContext,
        instance_ids: Vec<String>,
    ) -> Result<
        Arc<dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker>,
        ApiError,
    > {
        Ok(Arc::new(
            RuntimeInternalToolInvokerFactory::create_server_delegated(
                self,
                headers,
                actor,
                instance_ids,
            )
            .await?,
        ))
    }
    async fn create_workflow_credential_host(
        &self,
        actor: domain::ActorContext,
    ) -> Result<
        Arc<dyn orchestration_runtime::execution_engine::RuntimeInternalToolInvoker>,
        ApiError,
    > {
        Ok(Arc::new(
            RuntimeInternalToolInvokerFactory::create_workflow_credential_host(self, actor).await?,
        ))
    }
}
pub(crate) struct BindingValidator;
#[async_trait::async_trait]
impl CommunicationBindingValidator for BindingValidator {
    async fn validate_binding(
        &self,
        store: &storage_durable_postgres::MainDurableStore,
        binding: &domain::CommunicationBindingRecord,
    ) -> anyhow::Result<domain::ActorContext> {
        crate::communication_runtime::validate_binding(store, binding).await
    }
}
pub(crate) struct TraceRebuilder;
#[async_trait::async_trait]
impl TraceProjectionRebuildPort for TraceRebuilder {
    async fn rebuild(
        &self,
        store: &storage_durable_postgres::MainDurableStore,
        application_id: uuid::Uuid,
        run_id: uuid::Uuid,
    ) -> anyhow::Result<()> {
        crate::workers::trace_projection::rebuild_trace_projection(store, application_id, run_id)
            .await
    }
}
pub(crate) struct PublicDocs;
impl api_host_kernel::application_public_docs::ApplicationPublicDocsPort for PublicDocs {
    fn catalog(
        &self,
        context: &api_host_kernel::application_public_docs::ApplicationPublicDocsContext,
    ) -> api_host_kernel::openapi_docs::DocsCatalog {
        crate::application_public_docs::build_application_public_docs_catalog(context)
    }
    fn category_operations(
        &self,
        context: &api_host_kernel::application_public_docs::ApplicationPublicDocsContext,
        category_id: &str,
    ) -> Option<api_host_kernel::openapi_docs::DocsCatalogCategoryOperations> {
        crate::application_public_docs::build_application_public_docs_category_operations(
            context,
            category_id,
        )
    }
    fn category_spec(
        &self,
        context: &api_host_kernel::application_public_docs::ApplicationPublicDocsContext,
        category_id: &str,
    ) -> Option<serde_json::Value> {
        crate::application_public_docs::build_application_public_docs_category_spec(
            context,
            category_id,
        )
    }
    fn operation_spec(
        &self,
        context: &api_host_kernel::application_public_docs::ApplicationPublicDocsContext,
        operation_id: &str,
    ) -> Option<serde_json::Value> {
        crate::application_public_docs::build_application_public_docs_operation_spec(
            context,
            operation_id,
        )
    }
}
